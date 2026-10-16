//! Semantic palette owns colors; components own behavior and geometry.
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui_tokens::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Palette {
    #[default]
    Forest,
    Slate,
    Ember,
    Amethyst,
    Dune,
}
impl Palette {
    pub const ALL: [Self; 5] = [
        Self::Forest,
        Self::Slate,
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
    fn sheet(self) -> String {
        format!(":root {{ {} }}", self.declarations())
    }
}
pub fn stylesheet() -> String {
    format!(
        ":root {{ {} {} }}\n{}\n{}",
        include_str!("themes/common.css"),
        Palette::Forest.declarations(),
        include_str!("style.css"),
        include_str!("plot_theme.css")
    )
}
#[component]
pub fn AppTheme(
    #[prop(into)] palette: Signal<Palette, LocalStorage>,
    children: Children,
) -> impl IntoView {
    let sheet = Stylesheet::install("enlumi-palette", &palette.get_untracked().sheet());
    let effect = RenderEffect::new(move |_: Option<()>| {
        let css = palette.get().sheet();
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
