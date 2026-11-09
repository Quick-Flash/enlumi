use crate::theme::Palette;
use std::collections::BTreeSet;

fn keys(css: &str) -> BTreeSet<&str> {
    css.lines()
        .filter_map(|line| line.split_once(':').map(|(key, _)| key.trim()))
        .collect()
}
#[test]
fn themes_cover_shared_component_tokens() {
    let forest = keys(Palette::Forest.declarations());

    let common = keys(include_str!("themes/common.css"));
    for choice in Palette::ALL {
        let palette = keys(choice.declarations());
        assert_eq!(
            forest,
            palette,
            "Theme {} must define same semantic roles",
            choice.name()
        );
        assert_eq!(Palette::from_id(&choice.id()), Some(choice.clone()));
        assert_eq!(choice.measurement_fill()[3], 56);
        let available: BTreeSet<_> = palette.union(&common).copied().collect();
        for css in [
            include_str!("style.css"),
            include_str!("action_button.css"),
            include_str!("plot_theme.css"),
            include_str!("theme.css"),
            include_str!("measurement.css"),
        ] {
            for reference in css.split("var(").skip(1) {
                let name = reference.split(')').next().unwrap();
                if name.starts_with("--app-") {
                    assert!(available.contains(name), "Undefined theme token: {name}");
                }
            }
            assert!(!css.contains('#'), "Palette literals belong in theme files");
        }
    }
}

#[test]
fn custom_themes_keep_known_safe_tokens_and_fall_back_for_the_rest() {
    use crate::theme::{CustomTheme, parse_hex, resolved_tokens};
    let css = "--app-accent: #ff7a59;\n\
               --app-unknown: #000000;\n\
               --app-surface: red; } body { color: blue;\n\
               --app-plot-background: not-a-color;\n";
    let theme = Palette::Custom(std::rc::Rc::new(CustomTheme::new("Test", css)));
    let values = resolved_tokens(theme.declarations());
    let value = |token: &str| {
        values
            .iter()
            .find(|(name, _)| name == token)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(value("--app-accent"), Some("#ff7a59"));
    assert_eq!(value("--app-unknown"), None, "Unknown tokens are dropped");
    assert_eq!(
        value("--app-surface"),
        tokens_of(Palette::Slate, "--app-surface").as_deref(),
        "Values that could escape the declaration block are dropped"
    );
    assert_eq!(
        theme.plot_background(),
        Palette::Slate.plot_background(),
        "A non-color value falls back to the default color"
    );
    assert_eq!(
        values.len(),
        resolved_tokens(Palette::Slate.declarations()).len()
    );
    assert_eq!(parse_hex("#11223344"), Some([0x11, 0x22, 0x33, 0x44]));
    assert_eq!(parse_hex("#abc"), None);
}

fn tokens_of(palette: Palette, token: &str) -> Option<String> {
    crate::theme::tokens(palette.declarations())
        .find(|(name, _)| *name == token)
        .map(|(_, value)| value.to_owned())
}
