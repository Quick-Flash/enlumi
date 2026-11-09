//! Semantic palette owns colors; components own behavior and geometry.
use std::borrow::Cow;
use std::rc::Rc;
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui_tokens::prelude::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Palette {
    #[default]
    Slate,
    Forest,
    Ember,
    Amethyst,
    Dune,
    Custom(Rc<CustomTheme>),
}

/// A user theme: overrides of the built-in `--app-*` tokens, on top of the default palette.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CustomTheme {
    name: String,
    /// The default palette's declarations followed by the accepted overrides, so a later
    /// declaration of a token wins and a token the theme leaves out keeps its default.
    declarations: String,
}

impl CustomTheme {
    /// Builds a theme from `--token: value;` lines. Lines naming an unknown token, or carrying a
    /// value that could escape the declaration block, are dropped.
    pub fn new(name: &str, css: &str) -> Self {
        let mut declarations = Palette::default().declarations().to_owned();
        for (token, value) in tokens(css) {
            if is_known_token(token) && is_safe_value(value) {
                declarations.push('\n');
                declarations.push_str(&format!("{token}: {value};"));
            }
        }
        Self {
            name: name.to_owned(),
            declarations,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// `(token, value)` pairs of `--token: value;` lines, in order.
pub fn tokens(css: &str) -> impl Iterator<Item = (&str, &str)> {
    css.lines().filter_map(|line| {
        let (token, value) = line.trim().split_once(':')?;
        let value = value
            .trim()
            .strip_suffix(';')
            .unwrap_or(value.trim())
            .trim();
        Some((token.trim(), value))
    })
}

/// The current value of every token, last declaration winning, in the default palette's order.
pub fn resolved_tokens(declarations: &str) -> Vec<(String, String)> {
    let mut values: Vec<(String, String)> = tokens(Palette::default().declarations())
        .map(|(token, value)| (token.to_owned(), value.to_owned()))
        .collect();
    for (token, value) in tokens(declarations) {
        if let Some(entry) = values.iter_mut().find(|(known, _)| known == token) {
            entry.1 = value.to_owned();
        }
    }
    values
}

fn is_known_token(token: &str) -> bool {
    tokens(Palette::default().declarations()).any(|(known, _)| known == token)
}

fn is_safe_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '#' | '.' | '-' | ' ' | '%' | '(' | ')' | ',')
        })
}

/// `#rrggbb` or `#rrggbbaa` as RGBA.
pub fn parse_hex(value: &str) -> Option<[u8; 4]> {
    let hex = value.strip_prefix('#')?;
    if !matches!(hex.len(), 6 | 8) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).ok();
    Some([
        byte(0)?,
        byte(2)?,
        byte(4)?,
        if hex.len() == 8 { byte(6)? } else { 255 },
    ])
}

pub fn hex([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

impl Palette {
    pub const ALL: [Self; 5] = [
        Self::Slate,
        Self::Forest,
        Self::Ember,
        Self::Amethyst,
        Self::Dune,
    ];
    pub const CUSTOM_PREFIX: &str = "custom:";
    #[cfg(test)]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|palette| palette.id() == id)
    }
    pub fn name(&self) -> &str {
        match self {
            Self::Forest => "Forest",
            Self::Slate => "Slate",
            Self::Ember => "Ember",
            Self::Amethyst => "Amethyst",
            Self::Dune => "Dune",
            Self::Custom(theme) => theme.name(),
        }
    }

    pub fn id(&self) -> Cow<'static, str> {
        match self {
            Self::Forest => "forest".into(),
            Self::Slate => "slate".into(),
            Self::Ember => "ember".into(),
            Self::Amethyst => "amethyst".into(),
            Self::Dune => "dune".into(),
            Self::Custom(theme) => format!("{}{}", Self::CUSTOM_PREFIX, theme.name()).into(),
        }
    }
    pub fn declarations(&self) -> &str {
        match self {
            Self::Forest => include_str!("themes/forest.css"),
            Self::Slate => include_str!("themes/slate.css"),
            Self::Ember => include_str!("themes/ember.css"),
            Self::Amethyst => include_str!("themes/amethyst.css"),
            Self::Dune => include_str!("themes/dune.css"),
            Self::Custom(theme) => &theme.declarations,
        }
    }
    fn sheet(&self) -> String {
        format!(":root {{ {} }}", self.declarations())
    }
    /// The last declared color of `token`, or the default palette's when this palette's value is
    /// not a hex color.
    fn color(&self, token: &str) -> [u8; 4] {
        let find = |declarations: &str| {
            tokens(declarations)
                .filter(|(name, _)| *name == token)
                .last()
                .and_then(|(_, value)| parse_hex(value))
        };
        find(self.declarations())
            .or_else(|| find(Self::default().declarations()))
            .expect("Default palette defines every color token")
    }
    pub fn measurement_fill(&self) -> [u8; 4] {
        self.color("--app-measurement-fill")
    }
    pub fn plot_background(&self) -> [u8; 3] {
        let [r, g, b, _] = self.color("--app-plot-background");
        [r, g, b]
    }
}
pub fn stylesheet() -> String {
    format!(
        ":root {{ {} {} }}\n{}\n{}",
        include_str!("themes/common.css"),
        Palette::default().declarations(),
        include_str!("style.css"),
        include_str!("plot_theme.css")
    )
}
#[component]
pub fn AppTheme(
    #[prop(into)] palette: Signal<Palette, LocalStorage>,
    children: Children,
) -> impl IntoView {
    zgui::reactive::provide_local_context(palette);
    let sheet = Stylesheet::install("enlumi-palette", &palette.get_untracked().sheet());
    let effect = RenderEffect::new(move |_: Option<()>| {
        let css = palette.with(Palette::sheet);
        if let Some(sheet) = sheet.as_ref() {
            sheet.replace(&css);
        }
    });
    on_cleanup_local(move || drop(effect));
    view! {
        ThemeProvider(scheme = ColorScheme::Dark, dark = Theme::dark().with_css(include_str!("theme.css"))) {
            {children.into_view_once()}
        }
    }
}
