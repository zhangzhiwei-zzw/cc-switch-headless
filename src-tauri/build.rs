fn main() {
    // tauri-build 只在桌面版（default feature）编译；server 模式没有 tauri 依赖，
    // 也不需要生成上下文/资源清单。
    #[cfg(feature = "desktop")]
    tauri_build::build();

    // 服务端把 `dist-web/` 编进二进制（rust-embed 要求目录必须存在）。
    // 前端还没构建过时先放一个占位页，让 `cargo build --features server` 能过；
    // 真正构建过前端后这里不会覆盖产物。
    #[cfg(feature = "server")]
    ensure_web_dist_placeholder();

    // Windows: Embed Common Controls v6 manifest for test binaries
    //
    // When running `cargo test`, the generated test executables don't include
    // the standard Tauri application manifest. Without Common Controls v6,
    // `tauri::test` calls fail with STATUS_ENTRYPOINT_NOT_FOUND.
    //
    // This workaround:
    // 1. Embeds the manifest into test binaries via /MANIFEST:EMBED
    // 2. Uses /MANIFEST:NO for the main binary to avoid duplicate resources
    //    (Tauri already handles manifest embedding for the app binary)
    #[cfg(target_os = "windows")]
    {
        let manifest_path = std::path::PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"),
        )
        .join("common-controls.manifest");
        let manifest_arg = format!("/MANIFESTINPUT:{}", manifest_path.display());

        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg={}", manifest_arg);
        // Avoid duplicate manifest resources in binary builds.
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
        println!("cargo:rerun-if-changed={}", manifest_path.display());
    }
}

/// 见 `main` 里的说明：`../dist-web` 必须存在，rust-embed 才能把前端编进二进制。
#[cfg(feature = "server")]
fn ensure_web_dist_placeholder() {
    let dist = std::path::Path::new("../dist-web");
    // 前端产物一变就让 cargo 重新编译（rust-embed 在编译期读文件内容）
    println!("cargo:rerun-if-changed=../dist-web");

    if dist.join("index.html").exists() {
        return;
    }

    if let Err(error) = std::fs::create_dir_all(dist) {
        println!("cargo:warning=创建 {dist:?} 失败: {error}");
        return;
    }

    let placeholder = r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <title>CC Switch Server</title>
  </head>
  <body style="font-family: system-ui; padding: 2rem; line-height: 1.7">
    <h1>前端产物未构建</h1>
    <p>这个二进制里没有前端页面。请在仓库根目录执行：</p>
    <pre>pnpm install &amp;&amp; pnpm build:web</pre>
    <p>然后重新构建服务端：</p>
    <pre>cargo build --release --no-default-features --features server --bin cc-switch-server</pre>
    <p>也可以用 <code>--dist &lt;目录&gt;</code> 指向别处的前端产物。</p>
  </body>
</html>
"#;

    if let Err(error) = std::fs::write(dist.join("index.html"), placeholder) {
        println!("cargo:warning=写入占位页失败: {error}");
    }
}
