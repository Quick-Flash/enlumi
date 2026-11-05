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

/// Marks the process started by [`retry_with_software_vulkan`], so a second failure cannot loop.
#[cfg(target_os = "linux")]
const SOFTWARE_RETRY_MARKER: &str = "ENLUMI_SOFTWARE_RETRY";

/// Value zgui itself writes to `VK_LOADER_DRIVERS_DISABLE` at startup to hide lavapipe.
#[cfg(target_os = "linux")]
const ZGUI_DISABLE_VALUE: &str = "*lvp*";

/// Driver-selection variables of the Vulkan loader and zgui. A set variable means the user chose.
#[cfg(target_os = "linux")]
const USER_DRIVER_VARIABLES: [&str; 5] = [
    "VK_LOADER_DRIVERS_SELECT",
    "VK_DRIVER_FILES",
    "VK_ICD_FILENAMES",
    "VK_ADD_DRIVER_FILES",
    "ZGUI_BACKENDS",
];

/// Whether a failed GPU start may retry on the software Vulkan driver (Mesa lavapipe).
///
/// zgui hides lavapipe when any hardware driver manifest exists. A hardware driver that cannot
/// present (for example Haswell Vulkan) then leaves no usable adapter.
#[cfg(target_os = "linux")]
fn software_retry_allowed(get: impl Fn(&str) -> Option<String>) -> bool {
    let disabled_by_user =
        get("VK_LOADER_DRIVERS_DISABLE").is_some_and(|value| value != ZGUI_DISABLE_VALUE);
    get(SOFTWARE_RETRY_MARKER).is_none()
        && !disabled_by_user
        && !USER_DRIVER_VARIABLES.iter().any(|name| get(name).is_some())
}

/// Replaces this process with a copy restricted to lavapipe. Returns only when the retry is
/// not possible or `exec` fails.
pub fn retry_with_software_vulkan() {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        if !software_retry_allowed(|name| std::env::var(name).ok()) {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        eprintln!("No hardware GPU can present. Retrying with the software Vulkan driver.");
        let mut args: Vec<_> = std::env::args_os().skip(1).collect();
        // The first start already consumed and deleted the update stage directory.
        if args
            .first()
            .is_some_and(|arg| arg == "--enlumi-finish-update")
        {
            args.clear();
        }
        let error = std::process::Command::new(exe)
            .args(args)
            .env(SOFTWARE_RETRY_MARKER, "1")
            .env_remove("VK_LOADER_DRIVERS_DISABLE")
            .env("VK_LOADER_DRIVERS_SELECT", "*lvp*")
            .env("ZGUI_BACKENDS", "vulkan")
            .exec();
        eprintln!("Software retry failed: {error}");
    }
}

#[cfg(all(test, target_os = "linux"))]
mod software_retry_tests {
    use super::software_retry_allowed;

    fn with(set: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            set.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn retry_runs_on_a_clean_environment() {
        assert!(software_retry_allowed(with(&[])));
    }

    #[test]
    fn retry_runs_when_only_zgui_hid_lavapipe() {
        assert!(software_retry_allowed(with(&[(
            "VK_LOADER_DRIVERS_DISABLE",
            "*lvp*"
        )])));
    }

    #[test]
    fn retry_stops_after_the_marker_is_set() {
        assert!(!software_retry_allowed(with(&[(
            "ENLUMI_SOFTWARE_RETRY",
            "1"
        )])));
    }

    #[test]
    fn retry_respects_a_user_driver_choice() {
        for name in [
            "VK_DRIVER_FILES",
            "VK_LOADER_DRIVERS_SELECT",
            "ZGUI_BACKENDS",
        ] {
            let set: &'static [(&str, &str)] = Box::leak(Box::new([(name, "x")]));
            assert!(!software_retry_allowed(with(set)), "{name}");
        }
        assert!(!software_retry_allowed(with(&[(
            "VK_LOADER_DRIVERS_DISABLE",
            "*intel*"
        )])));
    }
}
