pub fn window() -> zgui::prelude::WindowIcon {
    zgui::prelude::WindowIcon::from_rgba(
        include_bytes!(concat!(env!("OUT_DIR"), "/enlumi.rgba")).to_vec(),
        256,
        256,
    )
    .expect("build-generated RGBA icon has matching dimensions")
}

pub fn set_dock_icon() {
    #[cfg(target_os = "macos")]
    {
        use objc2::{AllocAnyThread, MainThreadMarker};
        use objc2_app_kit::{NSApplication, NSImage};
        use objc2_foundation::NSData;
        let main = MainThreadMarker::new().expect("app constructed on main thread");
        let data = NSData::with_bytes(include_bytes!(concat!(env!("OUT_DIR"), "/enlumi-dock.png")));
        let image =
            NSImage::initWithData(NSImage::alloc(), &data).expect("valid embedded PNG icon");
        // Main thread; valid NSImage is retained by NSApplication.
        unsafe {
            NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image));
        }
    }
}
