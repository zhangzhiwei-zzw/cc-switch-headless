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
    #[cfg(target_os = "windows")]
    set_console_icon();
    cc_switch_lib::web::run();
}

/// Classic console windows need their icon set explicitly. Windows Terminal owns
/// its own tab/taskbar icon; the executable resource still identifies the server.
#[cfg(target_os = "windows")]
fn set_console_icon() {
    use windows_sys::Win32::{
        System::{Console::GetConsoleWindow, LibraryLoader::GetModuleHandleW},
        UI::WindowsAndMessaging::{
            GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON,
            LR_SHARED, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
        },
    };

    // Shared resource icons live for the process lifetime and must not be destroyed.
    unsafe {
        let window = GetConsoleWindow();
        if window.is_null() {
            return;
        }
        let module = GetModuleHandleW(std::ptr::null());
        if module.is_null() {
            return;
        }
        for (kind, width, height) in [
            (ICON_BIG, SM_CXICON, SM_CYICON),
            (ICON_SMALL, SM_CXSMICON, SM_CYSMICON),
        ] {
            let icon = LoadImageW(
                module,
                32512_usize as *const u16,
                IMAGE_ICON,
                GetSystemMetrics(width),
                GetSystemMetrics(height),
                LR_SHARED,
            );
            if !icon.is_null() {
                SendMessageW(window, WM_SETICON, kind as usize, icon as isize);
            }
        }
    }
}
