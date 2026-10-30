//! Native image clipboard. Keep handle alive while pasted images remain available on Linux.
use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

/// Cloneable, lazy clipboard owner. Retain in application state and call on a worker thread.
#[derive(Clone, Default)]
pub struct ScreenshotClipboard {
    clipboard: Arc<Mutex<Option<arboard::Clipboard>>>,
}

impl ScreenshotClipboard {
    /// Copy current SVG as an image at the same resolution as PNG export.
    /// Rasterization, font loading, and clipboard I/O belong on a worker thread.
    pub fn copy(&self, svg: &str) -> Result<(), String> {
        let image = image_data(svg)?;
        let mut slot = self
            .clipboard
            .lock()
            .map_err(|_| "Screenshot clipboard lock poisoned")?;
        if slot.is_none() {
            *slot = Some(
                arboard::Clipboard::new()
                    .map_err(|error| format!("Cannot open screenshot clipboard: {error}"))?,
            );
        }
        slot.as_mut()
            .ok_or("Screenshot clipboard unavailable")?
            .set_image(image)
            .map_err(|error| format!("Cannot copy screenshot: {error}"))
    }
}

fn image_data(svg: &str) -> Result<arboard::ImageData<'static>, String> {
    let pixels = crate::raster::render_svg(svg)?;
    // Clipboard expects straight RGBA; tiny-skia renders premultiplied RGBA.
    let bytes = pixels
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    Ok(arboard::ImageData {
        width: pixels.width() as usize,
        height: pixels.height() as usize,
        bytes: Cow::Owned(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::image_data;

    #[test]
    fn clipboard_matches_png_dimensions_and_preserves_straight_alpha() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="3"><rect width="4" height="3" fill="red" fill-opacity="0.5"/></svg>"#;
        let image = image_data(svg).unwrap();
        let png = crate::raster::svg_to_png(svg).unwrap();
        assert_eq!(
            image.width,
            u32::from_be_bytes(png[16..20].try_into().unwrap()) as usize
        );
        assert_eq!(
            image.height,
            u32::from_be_bytes(png[20..24].try_into().unwrap()) as usize
        );
        assert_eq!(image.bytes.len(), 8 * 6 * 4);
        assert_eq!(&image.bytes[..3], &[255, 0, 0]);
        assert!((127..=128).contains(&image.bytes[3]));
        assert!(image_data("invalid svg").is_err());
    }
}
