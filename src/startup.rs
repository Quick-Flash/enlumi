#[cfg(windows)]
pub fn reveal_if_first_frame_stalls() {
    std::thread::spawn(|| {
        use std::{ptr, time::Duration};
        use windows_sys::Win32::{
            Foundation::RECT,
            System::Threading::GetCurrentProcessId,
            UI::WindowsAndMessaging::{
                GW_HWNDNEXT, GetClassNameW, GetClientRect, GetTopWindow, GetWindow,
                GetWindowThreadProcessId, IsWindowVisible, SW_SHOW, ShowWindow,
            },
        };
        for _ in 0..100 {
            let mut window = unsafe { GetTopWindow(ptr::null_mut()) };
            while !window.is_null() {
                let mut pid = 0;
                unsafe { GetWindowThreadProcessId(window, &mut pid) };
                if pid == unsafe { GetCurrentProcessId() } {
                    let mut class = [0_u16; 128];
                    let length =
                        unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) };
                    let mut bounds = RECT::default();
                    if String::from_utf16_lossy(&class[..length as usize]) == "Window Class"
                        && unsafe { GetClientRect(window, &mut bounds) } != 0
                        && bounds.right > 100
                        && bounds.bottom > 100
                    {
                        std::thread::sleep(Duration::from_millis(1500));
                        if unsafe { IsWindowVisible(window) } == 0 {
                            unsafe { ShowWindow(window, SW_SHOW) };
                        }
                        return;
                    }
                }
                window = unsafe { GetWindow(window, GW_HWNDNEXT) };
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

#[cfg(not(windows))]
pub fn reveal_if_first_frame_stalls() {}

pub fn desktop_driver() -> zgui::app::Driver {
    zgui::app::desktop()
}

pub fn native_wayland() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Match zgui::app::desktop(), including its explicit backend override.
        match std::env::var("ZGUI_PLATFORM") {
            Ok(name) => name.trim().eq_ignore_ascii_case("wayland"),
            Err(_) => zgui_platform_wayland::conn::is_available(),
        }
    }
    #[cfg(not(target_os = "linux"))]
    false
}
