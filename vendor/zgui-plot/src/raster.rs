//! Rasterize exported SVG so PNG and SVG share geometry, labels, legends, and overlays.
use std::sync::{Arc, OnceLock};

/// Render vector geometry at twice the SVG width and height for crisp PNG exports.
pub fn svg_to_png(svg: &str) -> Result<Vec<u8>, String> {
    render_svg(svg)?
        .encode_png()
        .map_err(|error| format!("cannot encode plot PNG: {error}"))
}

pub(crate) fn render_svg(svg: &str) -> Result<resvg::tiny_skia::Pixmap, String> {
    const SCALE: u32 = 2;
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        let mut fonts = resvg::usvg::fontdb::Database::new();
        fonts.load_system_fonts();
        Arc::new(fonts)
    });
    let options = resvg::usvg::Options {
        fontdb: Arc::clone(fonts),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options)
        .map_err(|error| format!("cannot parse plot SVG for PNG: {error}"))?;
    let size = tree.size().to_int_size();
    let width = size
        .width()
        .checked_mul(SCALE)
        .ok_or_else(|| "scaled plot PNG width is too large".to_owned())?;
    let height = size
        .height()
        .checked_mul(SCALE)
        .ok_or_else(|| "scaled plot PNG height is too large".to_owned())?;
    let mut pixels = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "cannot allocate plot PNG pixels".to_owned())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(SCALE as f32, SCALE as f32),
        &mut pixels.as_mut(),
    );
    Ok(pixels)
}
