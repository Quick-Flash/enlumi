//! Semantic palette owns colors; components own behavior and geometry.
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui_tokens::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Palette {
    #[default]
    Slate,
    Forest,
    Ember,
    Amethyst,
    Dune,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Corners {
    #[default]
    Square,
    Subtle,
    Soft,
    Rounded,
    ExtraRounded,
}
impl Corners {
    pub const ALL: [Self; 5] = [
        Self::Square,
        Self::Subtle,
        Self::Soft,
        Self::Rounded,
        Self::ExtraRounded,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Square => "square",
            Self::Subtle => "subtle",
            Self::Soft => "soft",
            Self::Rounded => "rounded",
            Self::ExtraRounded => "extra-rounded",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Square => "Square",
            Self::Subtle => "Subtle",
            Self::Soft => "Soft",
            Self::Rounded => "Rounded",
            Self::ExtraRounded => "Extra rounded",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.id() == id)
    }
    pub fn radii(self) -> (u8, u8) {
        match self {
            Self::Square => (0, 0),
            Self::Subtle => (2, 1),
            Self::Soft => (4, 3),
            Self::Rounded => (6, 4),
            Self::ExtraRounded => (10, 6),
        }
    }
    fn declarations(self) -> String {
        let (outer, small) = self.radii();
        format!("--app-radius: {outer}px; --app-radius-sm: {small}px;")
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
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|palette| palette.id() == id)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Forest => "Forest",
            Self::Slate => "Slate",
            Self::Ember => "Ember",
            Self::Amethyst => "Amethyst",
            Self::Dune => "Dune",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Forest => "forest",
            Self::Slate => "slate",
            Self::Ember => "ember",
            Self::Amethyst => "amethyst",
            Self::Dune => "dune",
        }
    }
    pub fn declarations(self) -> &'static str {
        match self {
            Self::Forest => include_str!("themes/forest.css"),
            Self::Slate => include_str!("themes/slate.css"),
            Self::Ember => include_str!("themes/ember.css"),
            Self::Amethyst => include_str!("themes/amethyst.css"),
            Self::Dune => include_str!("themes/dune.css"),
        }
    }
    pub fn legacy_corners(self) -> Corners {
        match self {
            Self::Slate => Corners::Square,
            Self::Forest => Corners::Soft,
            Self::Ember => Corners::Subtle,
            Self::Dune => Corners::Rounded,
            Self::Amethyst => Corners::ExtraRounded,
        }
    }
    fn sheet(self, corners: Corners) -> String {
        format!(
            ":root {{ {} {} }}",
            self.declarations(),
            corners.declarations()
        )
    }
    pub fn measurement_fill(self) -> [u8; 4] {
        let hex = self
            .declarations()
            .lines()
            .find_map(|line| line.strip_prefix("--app-measurement-fill: #"))
            .and_then(|value| value.strip_suffix(';'))
            .expect("Palette defines measurement fill");
        u32::from_str_radix(hex, 16)
            .expect("Measurement fill is eight hex digits")
            .to_be_bytes()
    }
    pub fn plot_background(self) -> [u8; 3] {
        let hex = self
            .declarations()
            .lines()
            .find_map(|line| line.strip_prefix("--app-plot-background: #"))
            .and_then(|value| value.strip_suffix(';'))
            .expect("Palette defines plot background");
        let value = u32::from_str_radix(hex, 16).expect("Background is hexadecimal RGB");
        [(value >> 16) as u8, (value >> 8) as u8, value as u8]
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
    #[prop(into, default = Signal::stored_local(Corners::Square))] corners: Signal<
        Corners,
        LocalStorage,
    >,
    children: Children,
) -> impl IntoView {
    zgui::reactive::provide_local_context(palette);
    let sheet = Stylesheet::install(
        "enlumi-palette",
        &palette.get_untracked().sheet(corners.get_untracked()),
    );
    let effect = RenderEffect::new(move |_: Option<()>| {
        let css = palette.get().sheet(corners.get());
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
