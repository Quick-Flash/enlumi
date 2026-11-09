use crate::preferences::ThemePreference;
use crate::theme::Palette;
use crate::theme_editor::ThemeEditor;
use zgui::prelude::*;
use zgui_ui::prelude::*;

const CUSTOMIZE: &str = "__customize__";

#[component]
pub fn ThemePicker(
    palette: RwSignal<Palette, LocalStorage>,
    themes: RwSignal<Vec<Palette>, LocalStorage>,
    editor: ThemeEditor,
    preferences: ThemePreference,
) -> impl IntoView {
    view! {
        Select(value = Binding::controlled(Signal::derive_local(move || palette.with(|palette| palette.id().into_owned())), move |id: String| {
            if id == CUSTOMIZE { editor.begin(palette.get_untracked()); return; }
            let Some(choice) = themes.with_untracked(|themes| themes.iter().find(|theme| theme.id() == id).cloned()) else { return };
            if let Err(error) = preferences.save(&choice) { eprintln!("Could not save theme preference: {error}"); }
            palette.set(choice);
        })) {
            SelectTrigger(label = "Theme", class = "theme-picker") { SelectValue() }
            SelectContent(class = "filter-type-surface theme-menu") {
                for choice in move || themes.get(), key = |choice| choice.id().into_owned() {
                    SelectItem(value = choice.id().into_owned(), class = format!("theme-{}", choice.id().replace([':', ' '], "-"))) { {choice.name().to_owned()} }
                }
                SelectItem(value = CUSTOMIZE, class = "theme-customize") { "Customize…" }
            }
        }
    }
}
