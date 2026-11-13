use crate::preferences::ThemePreference;
use crate::theme::{Corners, Palette};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn ThemePicker(
    palette: RwSignal<Palette, LocalStorage>,
    corners: RwSignal<Corners, LocalStorage>,
    preferences: ThemePreference,
) -> impl IntoView {
    let corner_preferences = preferences.clone();
    view! {
        Select(value = Binding::controlled(Signal::derive_local(move || palette.get().id().to_owned()), move |id: String| {
            if let Some(choice) = Palette::from_id(&id) { palette.set(choice); if let Err(error) = preferences.save(choice).and_then(|_|preferences.save_corners(corners.get_untracked())) { eprintln!("Could not save theme preference: {error}"); } }
        })) {
            SelectTrigger(label = "Theme colors", class = "theme-picker") { SelectValue() }
            SelectContent(class = "filter-type-surface theme-menu") {
                for choice in || Palette::ALL, key = |choice| *choice {
                    SelectItem(value = choice.id(), class = format!("theme-{}", choice.id())) { {choice.name()} }
                }
            }
        }
        Select(value=Binding::controlled(Signal::derive_local(move ||corners.get().id().to_owned()),move |id:String|{
            if let Some(choice)=Corners::from_id(&id){corners.set(choice);if let Err(error)=corner_preferences.save_corners(choice){eprintln!("Could not save corner preference: {error}");}}
        })) {
            SelectTrigger(label="Corner rounding",class="corner-picker") {{move ||format!("{} corners",corners.get().name())}}
            SelectContent(class="filter-type-surface theme-menu") {
                for choice in ||Corners::ALL,key=|choice|*choice {SelectItem(value=choice.id(),class=format!("corners-{}",choice.id())) {{choice.name()}}}
            }
        }
    }
}
