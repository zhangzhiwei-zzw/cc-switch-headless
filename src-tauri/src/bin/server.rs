//! CC Switch 服务端（web 模式）入口。
//!
//! 不使用 Tauri/webkit，仅在 127.0.0.1 上提供 HTTP 服务：托管前端构建产物，
//! 并把浏览器的 `POST /api/invoke` 转发到与桌面版相同的业务层。
//!
//! 构建方式（Ubuntu 20.04 等系统库过旧的机器）：
//! ```text
//! cargo build --release --no-default-features --features server --bin cc-switch-server
//! ```

fn main() {
    cc_switch_lib::web::run();
}
