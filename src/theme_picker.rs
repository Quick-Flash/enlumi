use crate::preferences::ThemePreference;
use crate::theme::Palette;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn ThemePicker(
    palette: RwSignal<Palette, LocalStorage>,
    preferences: ThemePreference,
) -> impl IntoView {
    view! {
        Select(value = Binding::controlled(Signal::derive_local(move || palette.get().id().to_owned()), move |id: String| {
            if let Some(choice) = Palette::from_id(&id) { palette.set(choice); if let Err(error) = preferences.save(choice) { eprintln!("Could not save theme preference: {error}"); } }
        })) {
            SelectTrigger(label = "Theme", class = "theme-picker") { SelectValue() }
            SelectContent(class = "filter-type-surface theme-menu") {
                for choice in || Palette::ALL, key = |choice| *choice {
                    SelectItem(value = choice.id(), class = format!("theme-{}", choice.id())) { {choice.name()} }
                }
            }
        }
    }
}
