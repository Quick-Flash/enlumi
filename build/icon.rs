use image::{ImageFormat, imageops::FilterType};
use std::{env, fs, io::Cursor, path::PathBuf};

pub fn build() {
    println!("cargo:rerun-if-changed=build/icon.rs");
    println!("cargo:rerun-if-changed=assets/enlumi.png");
    let source = image::open("assets/enlumi.png").expect("decode assets/enlumi.png");
    assert_eq!(source.width(), source.height(), "app icon must be square");
    let directory = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"));
    let icon = rounded(&source, 256);
    fs::write(directory.join("enlumi.rgba"), icon.as_raw()).expect("write window icon");
    icon.save(directory.join("enlumi.png"))
        .expect("write desktop icon");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        rounded(&source, 1024)
            .save(directory.join("enlumi-dock.png"))
            .expect("write Dock icon");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // ICO directory with PNG images at native Windows shell/taskbar sizes.
    let sizes = [16_u32, 24, 32, 48, 64, 128, 256];
    let mut entries = Vec::new();
    for size in sizes {
        let mut png = Cursor::new(Vec::new());
        rounded(&source, size)
            .write_to(&mut png, ImageFormat::Png)
            .expect("encode ICO image");
        entries.push((size, png.into_inner()));
    }
    let mut ico = vec![0, 0, 1, 0];
    ico.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + entries.len() as u32 * 16;
    for (size, png) in &entries {
        ico.extend_from_slice(&[*size as u8, *size as u8, 0, 0]);
        ico.extend_from_slice(&1_u16.to_le_bytes());
        ico.extend_from_slice(&32_u16.to_le_bytes());
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in entries {
        ico.extend_from_slice(&png);
    }
    let path = directory.join("enlumi.ico");
    fs::write(&path, ico).expect("write executable icon");
    winresource::WindowsResource::new()
        .set_icon(path.to_str().expect("icon path is UTF-8"))
        .set("ProductName", "Enlumi")
        .set("FileDescription", "Enlumi log workbench")
        .compile()
        .expect("embed Windows executable icon");
}

// Preserve artwork; clip generated platform icons with antialiased corners.
fn rounded(source: &image::DynamicImage, size: u32) -> image::RgbaImage {
    let mut icon = source
        .resize_exact(size, size, FilterType::Lanczos3)
        .to_rgba8();
    let radius = size as f32 * 0.14;
    for (x, y, pixel) in icon.enumerate_pixels_mut() {
        let x = x as f32 + 0.5;
        let y = y as f32 + 0.5;
        let dx = x - x.clamp(radius, size as f32 - radius);
        let dy = y - y.clamp(radius, size as f32 - radius);
        let coverage = (radius + 0.5 - dx.hypot(dy)).clamp(0.0, 1.0);
        pixel[3] = (pixel[3] as f32 * coverage).round() as u8;
    }
    icon
}
