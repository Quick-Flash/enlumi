use super::PlotSnapshotSource;
use std::path::Path;
use zgui::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenshotFormat {
    Svg,
    Png,
}
impl ScreenshotFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
        }
    }
    pub fn encode(self, svg: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Svg => Ok(svg.as_bytes().to_vec()),
            #[cfg(feature = "raster")]
            Self::Png => crate::raster::svg_to_png(svg),
            #[cfg(not(feature = "raster"))]
            Self::Png => Err("PNG screenshots require the raster feature".into()),
        }
    }
    /// Encode and write on a worker thread; PNG may load system fonts once.
    pub fn save(self, svg: &str, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.encode(svg)?)
            .map_err(|error| format!("Cannot save screenshot: {error}"))
    }
}

pub fn capture_plot(source: PlotSnapshotSource, include_overlay: bool) -> Result<String, String> {
    let capture = source.get_value().ok_or("Plot is not ready to export")?;
    let snapshot = capture()?;
    Ok(if include_overlay {
        snapshot.svg.with_overlay
    } else {
        snapshot.svg.clean
    })
}

#[cfg(all(test, feature = "raster"))]
mod tests {
    use super::*;
    #[test]
    fn screenshot_formats_share_geometry_and_png_is_twice_svg_size() {
        let svg =
            crate::zgui_svg::scenes_to_svg(&[], &crate::PlotDescription::new(40., 30., "#123456"))
                .unwrap();
        assert_eq!(ScreenshotFormat::Svg.encode(&svg).unwrap(), svg.as_bytes());
        let png = ScreenshotFormat::Png.encode(&svg).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 80);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 60);
    }
}
