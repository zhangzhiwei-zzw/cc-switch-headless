//! 服务端（web 模式）实现。
//!
//! 在 `127.0.0.1` 上提供前端页面与命令桥：
//! - `POST /api/invoke`：与桌面版同名的命令分发（见 [`routes`]）
//! - `GET  /api/events`：SSE，推送与桌面版同名的事件
//! - `GET  /*`：托管前端构建产物（SPA 回退到 `index.html`）
//!
//! 安全约束：只监听回环地址；默认要求访问令牌（cookie / Bearer）；校验
//! `Host` 与 `Origin`（防 DNS rebinding）。远程访问请走 SSH 端口转发。
//!
//! 启动与桌面版一致的部分见 [`state::bootstrap`]。

pub mod mcp;
pub mod prompts;
pub mod routes;
pub mod skills;
pub mod state;
pub mod usage;

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Query, State as AxumState};
use axum::http::{header, HeaderMap, HeaderValue, Request, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use futures::Stream;
use tokio::sync::broadcast;

use crate::store::AppState;

/// 编译进二进制的前端产物（`pnpm build:web` 的输出）。
///
/// 构建时 `dist-web/` 不存在的话，`build.rs` 会先放一个占位页，
/// 保证服务端始终是"单文件可部署"的。
#[derive(rust_embed::RustEmbed)]
#[folder = "../dist-web"]
struct EmbeddedWeb;

/// 默认监听端口（本地回环）。
const DEFAULT_PORT: u16 = 15800;
/// 访问令牌的 cookie 名。
const TOKEN_COOKIE: &str = "ccswitch_web_token";

/// 一条要推给浏览器的事件。
#[derive(Clone)]
pub struct ServerEvent {
    pub name: String,
    pub payload: serde_json::Value,
}

/// 请求处理共享上下文。
pub struct Context {
    /// 数据库不可用（版本过新/初始化失败）时为 `None`，此时只有 `get_init_error` 可用，
    /// 前端会渲染「升级应用」恢复界面。
    pub state: Option<Arc<AppState>>,
    /// `None` 表示用 `--no-token` 显式关闭了令牌校验。
    pub token: Option<String>,
    /// 事件广播：`event_sink` 往这里发，`/api/events` 从这里读。
    pub events: broadcast::Sender<ServerEvent>,
    /// `--dist` 指向的前端产物目录；`None` 表示用编译进二进制的那份。
    pub dist_dir: Option<PathBuf>,
    /// 除回环之外额外允许的 `Host`（`--allow-host`，可多次）。
    /// 用域名/局域网 IP 访问时才需要；默认只认回环。
    pub allowed_hosts: Vec<String>,
}

impl Context {
    /// 取应用状态；数据库不可用时返回给前端的错误文案。
    pub fn require_state(&self) -> Result<Arc<AppState>, String> {
        self.state
            .clone()
            .ok_or_else(|| "数据库未就绪，请查看服务端日志".to_string())
    }
}

/// 服务端入口（由 `src/bin/server.rs` 调用）。
pub fn run() {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("创建运行时失败: {error}");
            std::process::exit(1);
        }
    };

    runtime.block_on(async {
        // 既有代码里有「在普通线程里 block_on」的路径（live.rs 等），
        // 服务端模式靠这个句柄提供运行时。
        crate::host::install_runtime();

        let options = match parse_options() {
            Ok(options) => options,
            Err(error) => {
                eprintln!("cc-switch-server: {error}");
                eprintln!(
                    "用法: cc-switch-server [--port <端口>] [--dist <前端产物目录>] [--no-token]"
                );
                std::process::exit(2);
            }
        };

        if let Err(error) = serve(options).await {
            eprintln!("cc-switch-server 启动失败: {error}");
            std::process::exit(1);
        }
    });
}

struct Options {
    port: u16,
    /// 监听地址，默认只绑回环；容器里需要 `0.0.0.0` 才能从端口映射进来
    bind: std::net::IpAddr,
    dist: PathBuf,
    /// 显式传入的令牌；`None` 表示自动生成或读取
    token: Option<String>,
    /// 显式关闭令牌校验（仅当确实只在本机、且能接受同机其他用户访问时使用）
    no_token: bool,
    allow_hosts: Vec<String>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        port: std::env::var("CC_SWITCH_WEB_PORT")
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(DEFAULT_PORT),
        bind: std::env::var("CC_SWITCH_WEB_BIND")
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)),
        dist: std::env::var("CC_SWITCH_WEB_DIST")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("dist-web")),
        token: None,
        no_token: false,
        allow_hosts: Vec::new(),
    };

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                let value = args.next().ok_or("--port 需要一个端口号")?;
                options.port = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("端口号不合法: {value}"))?;
            }
            "--bind" => {
                let value = args.next().ok_or("--bind 需要一个地址")?;
                options.bind = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("监听地址不合法: {value}"))?;
            }
            "--dist" => {
                options.dist = PathBuf::from(args.next().ok_or("--dist 需要一个目录")?);
            }
            "--token" => {
                options.token = Some(args.next().ok_or("--token 需要一个值")?);
            }
            "--no-token" => options.no_token = true,
            "--allow-host" => {
                let value = args.next().ok_or("--allow-host 需要一个主机名")?;
                options.allow_hosts.push(value.trim().to_lowercase());
            }
            "--help" | "-h" => {
                println!(
                    "cc-switch-server [选项]\n\n\
                     选项：\n\
                       --port <端口>            监听端口（默认 {DEFAULT_PORT}）\n\
                       --bind <地址>            监听地址（默认 127.0.0.1；容器内端口映射需 0.0.0.0）\n\
                       --dist <目录>            前端产物目录（缺省时用编译进二进制的那份）\n\
                       --token <令牌>           访问令牌（默认自动生成并写入 <配置目录>/web-token）\n\
                       --no-token               关闭令牌校验（只允许与回环地址同用）\n\
                       --allow-host <主机名>    除回环外额外允许的 Host，可多次（用域名/局域网访问时）\n\n\
                     环境变量：CC_SWITCH_WEB_PORT / CC_SWITCH_WEB_BIND / CC_SWITCH_WEB_DIST /\n\
                     CC_SWITCH_WEB_TOKEN / CC_SWITCH_ALLOW_HOSTS（逗号分隔）/ CC_SWITCH_CONFIG_DIR"
                );
                std::process::exit(0);
            }
            other => return Err(format!("未知参数: {other}")),
        }
    }

    if let Ok(extra) = std::env::var("CC_SWITCH_ALLOW_HOSTS") {
        options.allow_hosts.extend(
            extra
                .split(',')
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty()),
        );
    }

    // 非回环 + 无令牌 = 任何能连上的人都能读写你的 API Key
    if options.no_token && !options.bind.is_loopback() {
        return Err(
            "拒绝启动：--no-token 只能配合回环地址使用（当前 --bind 不是回环地址）".to_string(),
        );
    }

    Ok(options)
}

async fn serve(options: Options) -> Result<(), String> {
    // 1. 启动业务层（数据库、种子、后台任务），与桌面版同序
    let app_state = state::bootstrap();

    // 2. 解析访问令牌（此时配置目录已就绪）
    let token = resolve_token(options.token.clone(), options.no_token)?;

    // 3. 事件广播 + 安装全局事件出口
    let (events, _rx) = broadcast::channel::<ServerEvent>(512);
    {
        let sender = events.clone();
        crate::event_sink::set_sink(Box::new(move |name, payload| {
            let _ = sender.send(ServerEvent {
                name: name.to_string(),
                payload,
            });
        }));
    }

    // 4. 前端产物：--dist 目录优先（便于改前端不重编二进制），否则用编译进二进制的那份
    let dist_dir = if options.dist.join("index.html").exists() {
        log::info!("前端产物来自目录 {:?}", options.dist);
        Some(options.dist.clone())
    } else {
        log::info!("前端产物来自二进制内置（未找到 {:?}）", options.dist);
        None
    };

    let context = Arc::new(Context {
        state: app_state,
        token: token.clone(),
        events,
        dist_dir,
        allowed_hosts: options.allow_hosts.clone(),
    });

    // 5. 路由
    let router = build_router(context);

    // 6. 监听
    let addr = SocketAddr::new(options.bind, options.port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|error| format!("监听 {addr} 失败: {error}"))?;

    if !options.bind.is_loopback() {
        log::warn!(
            "已绑定非回环地址 {addr}：确认这是 Docker 端口映射或受控网络；\
             远程访问建议改用 SSH 端口转发"
        );
    }

    let base_url = match options.bind {
        std::net::IpAddr::V4(v4) if v4.is_unspecified() => {
            format!("http://127.0.0.1:{}", options.port)
        }
        std::net::IpAddr::V6(v6) if v6.is_unspecified() => {
            format!("http://127.0.0.1:{}", options.port)
        }
        std::net::IpAddr::V4(v4) => format!("http://{v4}:{}", options.port),
        std::net::IpAddr::V6(v6) => format!("http://[{v6}]:{}", options.port),
    };
    log::info!("CC Switch 服务端已启动: {base_url}");
    println!("CC Switch 服务端已启动: {base_url}");
    match &token {
        Some(token) => println!("首次访问请打开: {base_url}/auth?token={token}"),
        None => println!("已通过 --no-token 关闭访问令牌校验（仅建议本机使用）"),
    }

    axum::serve(listener, router)
        .await
        .map_err(|error| format!("HTTP 服务异常退出: {error}"))
}

/// 令牌优先级：命令行 → 环境变量 → 已有文件 → 新生成并写入 `<配置目录>/web-token`。
fn resolve_token(explicit: Option<String>, no_token: bool) -> Result<Option<String>, String> {
    if no_token {
        return Ok(None);
    }
    if let Some(token) = explicit {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(Some(token));
        }
    }
    if let Ok(token) = std::env::var("CC_SWITCH_WEB_TOKEN") {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(Some(token));
        }
    }

    let token_path = crate::config::get_app_config_dir().join("web-token");
    if let Ok(existing) = std::fs::read_to_string(&token_path) {
        let existing = existing.trim().to_string();
        if !existing.is_empty() {
            return Ok(Some(existing));
        }
    }

    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    if let Some(parent) = token_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = std::fs::write(&token_path, &token) {
        log::warn!("写入 {token_path:?} 失败（令牌只在本次运行有效）: {error}");
    }
    Ok(Some(token))
}

fn build_router(context: Arc<Context>) -> Router {
    let api = Router::new()
        .route("/invoke", post(routes::invoke))
        .route("/commands", get(routes::list_commands))
        .route("/capabilities", get(routes::capabilities))
        .route("/env", get(env_info))
        .route("/events", get(events))
        // 导入 / 导出：浏览器没有服务端文件系统，用上传 / 下载代替文件对话框
        .route("/upload", post(upload))
        .route("/download", get(download))
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(context.clone(), guard_token));

    Router::new()
        .route("/auth", get(auth))
        .nest("/api", api)
        .fallback(static_files)
        .layer(middleware::from_fn_with_state(context.clone(), guard_host))
        .with_state(context)
}

/// 托管前端产物：`--dist` 目录优先，其次是编译进二进制的那份，最后回退 `index.html`（SPA）。
async fn static_files(AxumState(context): AxumState<Arc<Context>>, uri: Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    let requested = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };

    if let Some(dir) = &context.dist_dir {
        let candidate = dir.join(requested);
        if candidate.is_file() {
            match tokio::fs::read(&candidate).await {
                Ok(bytes) => return asset_response(requested, bytes),
                Err(error) => log::warn!("读取 {candidate:?} 失败: {error}"),
            }
        }
    }

    if let Some(asset) = EmbeddedWeb::get(requested) {
        return asset_response(requested, asset.data.into_owned());
    }

    // SPA：未知路径交给前端路由
    if let Some(index) = EmbeddedWeb::get("index.html") {
        return asset_response("index.html", index.data.into_owned());
    }
    if let Some(dir) = &context.dist_dir {
        if let Ok(bytes) = tokio::fs::read(dir.join("index.html")).await {
            return asset_response("index.html", bytes);
        }
    }

    (
        StatusCode::NOT_FOUND,
        "前端产物缺失：请在仓库根目录执行 `pnpm build:web` 后重新构建服务端",
    )
        .into_response()
}

fn asset_response(path: &str, bytes: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(mime.as_ref()) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    // 产物带内容哈希，可以长缓存；index.html 不缓存，保证升级后立刻生效
    let cache_control = if path.ends_with("index.html") {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
    response
}

/// 校验 `Host` 与 `Origin`：默认只接受本机来源，挡住 DNS rebinding。
///
/// `--allow-host` 可以把额外的主机名加入白名单（用域名/局域网 IP 访问时才需要）。
async fn guard_host(
    AxumState(context): AxumState<Arc<Context>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let headers = request.headers();
    if !host_allowed(headers, &context.allowed_hosts) {
        return (
            StatusCode::BAD_REQUEST,
            "Host 不被允许（用域名访问请在启动时加 --allow-host <主机名>）",
        )
            .into_response();
    }
    if !origin_allowed(headers, &context.allowed_hosts) {
        return (StatusCode::FORBIDDEN, "Origin 不被允许").into_response();
    }
    next.run(request).await
}

/// 令牌校验（cookie 或 `Authorization: Bearer`）。
async fn guard_token(
    AxumState(context): AxumState<Arc<Context>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    if let Some(expected) = &context.token {
        match token_from_headers(request.headers()) {
            Some(token) if token == *expected => {}
            _ => {
                return (StatusCode::UNAUTHORIZED, "缺少或无效的访问令牌").into_response();
            }
        }
    }
    next.run(request).await
}

/// `GET /auth?token=...`：校验通过后下发 cookie 并跳回首页，之后浏览器带着 cookie 调 API。
async fn auth(
    AxumState(context): AxumState<Arc<Context>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let provided = params.get("token").map(String::as_str).unwrap_or_default();

    match &context.token {
        Some(expected) if provided == expected => {
            let cookie = format!("{TOKEN_COOKIE}={expected}; Path=/; HttpOnly; SameSite=Strict");
            let mut response = Redirect::to("/").into_response();
            match HeaderValue::from_str(&cookie) {
                Ok(value) => {
                    response.headers_mut().insert(header::SET_COOKIE, value);
                    response
                }
                Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "令牌格式异常").into_response(),
            }
        }
        Some(_) => (StatusCode::UNAUTHORIZED, "访问令牌不正确").into_response(),
        None => Redirect::to("/").into_response(),
    }
}

/// `GET /api/events`：SSE 事件流。
async fn events(
    AxumState(context): AxumState<Arc<Context>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut receiver = context.events.subscribe();

    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    yield Ok(Event::default()
                        .event(event.name)
                        .data(event.payload.to_string()));
                }
                // 慢客户端：丢掉落后的事件，保持连接
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// `POST /api/upload?name=<文件名>`：把浏览器选中的文件放到服务端的上传目录。
///
/// 返回服务端路径，前端随后把它交给 `import_config_from_file` 等命令使用——
/// 这样"选择文件 → 导入"的既有流程在 web 模式下原样成立。
#[derive(serde::Deserialize)]
struct UploadQuery {
    name: String,
}

async fn upload(Query(params): Query<UploadQuery>, body: axum::body::Bytes) -> Response {
    if body.is_empty() {
        return (StatusCode::BAD_REQUEST, "上传内容为空").into_response();
    }

    // 只取文件名部分，杜绝 `../` 穿越
    let file_name = Path::new(&params.name)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "upload.sql".to_string());

    let dir = crate::config::get_app_config_dir().join("uploads");
    if let Err(error) = std::fs::create_dir_all(&dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("创建上传目录失败: {error}"),
        )
            .into_response();
    }

    let path = dir.join(file_name);
    if let Err(error) = std::fs::write(&path, &body) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("写入上传文件失败: {error}"),
        )
            .into_response();
    }

    log::info!("[web] 已接收上传: {path:?}（{} 字节）", body.len());
    axum::Json(serde_json::json!({ "path": path.to_string_lossy() })).into_response()
}

/// `GET /api/download?path=<服务端路径>`：下载配置目录下的文件（导出、备份、上传）。
///
/// 只允许 `<配置目录>` 之内的路径：即使令牌泄露，也不能拿来读任意文件。
#[derive(serde::Deserialize)]
struct DownloadQuery {
    path: String,
}

async fn download(Query(params): Query<DownloadQuery>) -> Response {
    let root = match crate::config::get_app_config_dir().canonicalize() {
        Ok(root) => root,
        Err(error) => {
            return (StatusCode::NOT_FOUND, format!("配置目录不可用: {error}")).into_response();
        }
    };
    let path = match std::path::PathBuf::from(&params.path).canonicalize() {
        Ok(path) => path,
        Err(error) => {
            return (StatusCode::NOT_FOUND, format!("文件不存在: {error}")).into_response();
        }
    };
    if !path.starts_with(&root) {
        return (StatusCode::FORBIDDEN, "只能下载 CC Switch 配置目录里的文件").into_response();
    }

    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (StatusCode::NOT_FOUND, format!("读取文件失败: {error}")).into_response();
        }
    };

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download.bin".to_string());

    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

/// `GET /api/env`：前端 shim 需要的宿主信息（版本、home 目录、配置目录）。
async fn env_info() -> Response {
    axum::Json(serde_json::json!({
        "name": "cc-switch-server",
        "version": env!("CARGO_PKG_VERSION"),
        "homeDir": dirs::home_dir().map(|path| path.to_string_lossy().to_string()),
        "configDir": crate::config::get_app_config_dir().to_string_lossy().to_string(),
    }))
    .into_response()
}

fn host_allowed(headers: &HeaderMap, extra: &[String]) -> bool {
    match headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    {
        Some(host) => host_matches(host, extra),
        // 没有 Host 的请求（HTTP/1.0）直接拒绝
        None => false,
    }
}

fn origin_allowed(headers: &HeaderMap, extra: &[String]) -> bool {
    match headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        // 非浏览器客户端（curl）不带 Origin；它们仍要过令牌校验
        None => true,
        Some(origin) => {
            let rest = origin
                .strip_prefix("http://")
                .or_else(|| origin.strip_prefix("https://"))
                .unwrap_or(origin);
            host_matches(rest, extra)
        }
    }
}

/// `127.0.0.1`、`localhost`、`[::1]`（可带端口），或 `--allow-host` 列出的主机。
fn host_matches(host: &str, extra: &[String]) -> bool {
    let host = host.trim().to_lowercase();
    ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .copied()
        .chain(extra.iter().map(String::as_str))
        .any(|allowed| {
            host == allowed
                || host
                    .strip_prefix(allowed)
                    .is_some_and(|rest| rest.starts_with(':'))
        })
}

fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = value.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
    }

    let cookies = headers.get(header::COOKIE).and_then(|v| v.to_str().ok())?;
    for part in cookies.split(';') {
        if let Some(token) = part.trim().strip_prefix(&format!("{TOKEN_COOKIE}=")) {
            return Some(token.to_string());
        }
    }
    None
}
