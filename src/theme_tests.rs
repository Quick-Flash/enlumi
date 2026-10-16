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
        assert_eq!(Palette::from_id(choice.id()), Some(choice));
        let available: BTreeSet<_> = palette.union(&common).copied().collect();
        for css in [
            include_str!("style.css"),
            include_str!("action_button.css"),
            include_str!("plot_theme.css"),
            include_str!("theme.css"),
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
