// Temporary workaround for https://github.com/zortax/zgui/pull/11.
// Windows sends no paint to a hidden window, but Zgui shows it only after painting.
#[cfg(windows)]
pub fn force_first_paint(window_title: &str) {
    let window_title = window_title.to_owned();
    std::thread::spawn(move || {
        use windows_sys::Win32::System::Threading::GetCurrentProcessId;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GW_HWNDNEXT, GetTopWindow, GetWindow, GetWindowTextW, GetWindowThreadProcessId,
            IsWindowVisible, PostMessageW, WM_PAINT,
        };

        let title: Vec<u16> = window_title.encode_utf16().collect();
        let process = unsafe { GetCurrentProcessId() };
        for _ in 0..100 {
            let mut window = unsafe { GetTopWindow(std::ptr::null_mut()) };
            while !window.is_null() {
                let mut owner = 0;
                unsafe { GetWindowThreadProcessId(window, &mut owner) };
                if owner == process {
                    let mut text = [0_u16; 128];
                    let length =
                        unsafe { GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32) };
                    if text[..length as usize] == title {
                        if unsafe { IsWindowVisible(window) } != 0 {
                            return;
                        }
                        // Queue on window's UI thread; Zgui keeps its normal first-frame reveal.
                        unsafe { PostMessageW(window, WM_PAINT, 0, 0) };
                    }
                }
                window = unsafe { GetWindow(window, GW_HWNDNEXT) };
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}

#[cfg(not(windows))]
pub fn force_first_paint(_window_title: &str) {}
