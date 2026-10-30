use std::fmt::Write as _;

/// One standalone plot SVG placed in workspace coordinates.
pub struct SvgPanel {
    pub x: f64,
    pub y: f64,
    pub svg: String,
}

/// Compose snapshots produced by `scenes_to_svg`, preserving vector text and paths.
/// Definition IDs are isolated per panel so gradients cannot reference another plot.
pub fn compose_workspace_svg(
    width: f64,
    height: f64,
    background: &str,
    panels: &[SvgPanel],
) -> Result<String, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("workspace dimensions must be finite and positive".into());
    }
    if panels.is_empty() {
        return Err("workspace has no visible plots".into());
    }
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"{width}\" height=\"{height}\" fill=\"{}\"/>",
        super::escape_xml(background)
    );
    for (index, panel) in panels.iter().enumerate() {
        if !panel.x.is_finite() || !panel.y.is_finite() || !panel.svg.starts_with("<svg ") {
            return Err("workspace panel must have finite position and a plot SVG snapshot".into());
        }
        let prefix = format!("panel{index}-");
        let nested = panel
            .svg
            .replace("id=\"", &format!("id=\"{prefix}"))
            .replace("url(#", &format!("url(#{prefix}"))
            .replace("href=\"#", &format!("href=\"#{prefix}"));
        write!(
            svg,
            "<g transform=\"translate({} {})\">{nested}</g>",
            panel.x, panel.y
        )
        .expect("writing to String cannot fail");
    }
    svg.push_str("</svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_preserves_placement_and_isolates_gradient_ids() {
        let snapshot = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"30\"><defs><linearGradient id=\"paint0\"><stop stop-color=\"red\"/></linearGradient></defs><rect width=\"40\" height=\"30\" fill=\"url(#paint0)\"/><text x=\"2\" y=\"14\">X &amp; Y</text></svg>";
        let svg = compose_workspace_svg(
            90.0,
            30.0,
            "#000000",
            &[
                SvgPanel {
                    x: 0.0,
                    y: 0.0,
                    svg: snapshot.into(),
                },
                SvgPanel {
                    x: 50.0,
                    y: 0.0,
                    svg: snapshot.replace("red", "blue"),
                },
            ],
        )
        .unwrap();
        for index in 0..2 {
            assert!(svg.contains(&format!("id=\"panel{index}-paint0\"")));
            assert!(svg.contains(&format!("url(#panel{index}-paint0)")));
        }
        assert!(svg.contains("translate(50 0)"));
        assert_eq!(svg.matches("X &amp; Y").count(), 2);
        #[cfg(feature = "raster")]
        {
            let png = crate::raster::svg_to_png(&svg).unwrap();
            assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 180);
            assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 60);
            let pixels = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            let rgb = |x| {
                let pixel = pixels.pixel(x, 50).unwrap();
                (pixel.red(), pixel.green(), pixel.blue())
            };
            assert_eq!(rgb(10), (255, 0, 0));
            assert_eq!(rgb(90), (0, 0, 0));
            assert_eq!(rgb(110), (0, 0, 255));
        }
    }

    #[test]
    fn workspace_rejects_empty_or_invalid_snapshots() {
        assert!(compose_workspace_svg(10.0, 10.0, "black", &[]).is_err());
        let panels = [SvgPanel {
            x: f64::NAN,
            y: 0.0,
            svg: "<svg ></svg>".into(),
        }];
        assert!(compose_workspace_svg(10.0, 10.0, "black", &panels).is_err());
        assert!(compose_workspace_svg(f64::INFINITY, 10.0, "black", &panels).is_err());
    }
}
