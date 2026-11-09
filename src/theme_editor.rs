//! Custom theme editor: a side panel that previews every change on the whole app, with an
//! example card that shows where each token is used.
use crate::action_button::ActionButtonProps;
use crate::preferences::ThemePreference;
use crate::theme::{CustomTheme, Palette, hex, parse_hex, resolved_tokens};
use plot::zgui_series::ColorPickerProps;
use std::rc::Rc;
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui::prelude::*;

/// Editing session shared by the theme picker, which starts it, and the panel.
#[derive(Clone, Copy)]
pub struct ThemeEditor {
    pub open: RwSignal<bool, LocalStorage>,
    name: RwSignal<String, LocalStorage>,
    tokens: RwSignal<Vec<(String, String)>, LocalStorage>,
    error: RwSignal<String, LocalStorage>,
    /// The theme to restore when the panel closes without saving, while a session is open.
    previous: StoredValue<Option<Palette>, LocalStorage>,
    /// The custom theme being edited, if the session started from one.
    editing: StoredValue<Option<String>, LocalStorage>,
}

impl ThemeEditor {
    pub fn new() -> Self {
        Self {
            open: RwSignal::new_local(false),
            name: RwSignal::new_local(String::new()),
            tokens: RwSignal::new_local(Vec::new()),
            error: RwSignal::new_local(String::new()),
            previous: StoredValue::new_local(None),
            editing: StoredValue::new_local(None),
        }
    }
    /// Starts from the current theme's values. A built-in theme is copied under a new name.
    pub fn begin(self, current: Palette) {
        let editing = matches!(current, Palette::Custom(_)).then(|| current.name().to_owned());
        self.name.set(
            editing
                .clone()
                .unwrap_or_else(|| format!("{} custom", current.name())),
        );
        self.tokens.set(resolved_tokens(current.declarations()));
        self.error.set(String::new());
        self.editing.set_value(editing);
        self.previous.set_value(Some(current));
        self.open.set(true);
    }
    #[cfg(test)]
    pub(crate) fn tokens(self) -> Vec<(String, String)> {
        self.tokens.get_untracked()
    }
    fn draft(self) -> Palette {
        let css: String = self.tokens.with(|tokens| {
            tokens
                .iter()
                .map(|(token, value)| format!("{token}: {value};\n"))
                .collect()
        });
        let name = self.name.with(|name| name.trim().to_owned());
        Palette::Custom(Rc::new(CustomTheme::new(&name, &css)))
    }
    pub(crate) fn set_token(self, index: usize, value: String) {
        self.tokens.update(|tokens| {
            if let Some(entry) = tokens.get_mut(index) {
                entry.1 = value;
            }
        });
    }
    fn index_of(self, token: &str) -> Option<usize> {
        self.tokens
            .with_untracked(|tokens| tokens.iter().position(|(name, _)| name == token))
    }
    /// Replaces every value with `palette`'s, keeping the name.
    fn start_from(self, palette: &Palette) {
        self.tokens.set(resolved_tokens(palette.declarations()));
    }
}

impl Default for ThemeEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// (title, what it affects, [(token, label)]).
type Group = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);
/// Color tokens grouped by what they paint.
const GROUPS: &[Group] = &[
    (
        "Surfaces",
        "Window, panel and plot backgrounds.",
        &[
            ("--app-background", "Window"),
            ("--app-surface", "Panels and cards"),
            ("--app-inset", "Inputs and wells"),
            ("--app-plot-background", "Plot area"),
            ("--app-overlay", "Popup backdrop"),
        ],
    ),
    (
        "Text",
        "Labels and readouts, from most to least prominent.",
        &[
            ("--app-strong", "Headings"),
            ("--app-foreground", "Body text"),
            ("--app-muted", "Secondary text"),
            ("--app-dim", "Hints"),
            ("--app-control-text", "Button text"),
            ("--app-selected-text", "Selected text"),
            ("--app-accent-text", "Links"),
        ],
    ),
    (
        "Controls",
        "Buttons and menu items in each state.",
        &[
            ("--app-control", "Idle"),
            ("--app-hover", "Hover"),
            ("--app-pressed", "Pressed"),
            ("--app-selected", "Selected"),
            ("--app-selected-hover", "Selected hover"),
            ("--app-disabled", "Disabled"),
        ],
    ),
    (
        "Borders and focus",
        "Outlines around panels, fields and the focused control.",
        &[
            ("--app-border", "Border"),
            ("--app-border-strong", "Strong border"),
            ("--app-focus", "Focus ring"),
        ],
    ),
    (
        "Accents",
        "Highlights, the measurement band on plots, and warnings.",
        &[
            ("--app-accent", "Accent"),
            ("--app-measurement", "Measurement edge"),
            ("--app-measurement-fill", "Measurement band"),
            ("--app-warning", "Warning"),
        ],
    ),
    (
        "Danger",
        "Destructive actions and errors.",
        &[
            ("--app-danger", "Background"),
            ("--app-danger-text", "Text"),
            ("--app-danger-border", "Border"),
        ],
    ),
    (
        "Drafts",
        "Unsaved filters and pending edits.",
        &[
            ("--app-draft", "Background"),
            ("--app-draft-hover", "Hover"),
            ("--app-draft-text", "Text"),
        ],
    ),
    (
        "Dragging and docking",
        "Shown while dragging fields and plots.",
        &[
            ("--app-drop", "Drop target"),
            ("--app-dock-preview", "Dock preview"),
            ("--app-dock-source", "Dragged plot"),
        ],
    ),
    (
        "Code editor",
        "Rust filter editor.",
        &[
            ("--app-code-selection", "Selection"),
            ("--app-code-line", "Current line"),
        ],
    ),
];
const RADII: [(&str, &str); 2] = [
    ("--app-radius", "Corners"),
    ("--app-radius-sm", "Small corners"),
];

#[component]
pub fn ThemeEditorPanel(
    editor: ThemeEditor,
    palette: RwSignal<Palette, LocalStorage>,
    themes: RwSignal<Vec<Palette>, LocalStorage>,
    preferences: ThemePreference,
) -> impl IntoView {
    install_stylesheet("plot-series-controls", plot::zgui_series::stylesheet());
    install_stylesheet("zui-slider", zgui_ui::slider::SliderStyle::CSS);
    // Live preview while a session is open.
    let preview = RenderEffect::new(move |_: Option<()>| {
        editor.tokens.track();
        editor.name.track();
        if editor.open.get() && editor.previous.with_value(Option::is_some) {
            palette.set(editor.draft());
        }
    });
    on_cleanup_local(move || drop(preview));
    let preferences = StoredValue::new_local(preferences);
    let save = move || {
        let name = editor.name.with_untracked(|name| name.trim().to_owned());
        let tokens = editor.tokens.get_untracked();
        match preferences.with_value(|preferences| preferences.save_custom(&name, &tokens)) {
            Ok(saved) => {
                if let Some(old) = editor.editing.get_value()
                    && old != name
                    && let Err(error) =
                        preferences.with_value(|preferences| preferences.delete_custom(&old))
                {
                    eprintln!("Could not remove renamed theme {old}: {error}");
                }
                if let Err(error) = preferences.with_value(|preferences| preferences.save(&saved)) {
                    eprintln!("Could not save theme preference: {error}");
                }
                themes.set(preferences.with_value(ThemePreference::themes));
                editor.previous.set_value(None);
                palette.set(saved);
                editor.open.set(false);
            }
            Err(error) => editor.error.set(error.to_string()),
        }
    };
    let delete = move || {
        let Some(name) = editor.editing.get_value() else {
            return;
        };
        if let Err(error) = preferences.with_value(|preferences| preferences.delete_custom(&name)) {
            editor.error.set(format!("Could not delete theme: {error}"));
            return;
        }
        let fallback = Palette::default();
        if let Err(error) = preferences.with_value(|preferences| preferences.save(&fallback)) {
            eprintln!("Could not save theme preference: {error}");
        }
        themes.set(preferences.with_value(ThemePreference::themes));
        editor.previous.set_value(None);
        palette.set(fallback);
        editor.open.set(false);
    };
    view! {
        column(class = "theme-editor", a11y:label = "Theme editor") {
            row(class = "theme-editor-header") {
                text(class = "heading") { "Theme editor" }
                ActionButton(icon = true, class = "theme-editor-close", a11y:label = "Close theme editor without saving", on:click = move |_| editor.open.set(false)) { "X" }
            }
            ScrollArea(class = "theme-editor-body", label = "Theme settings") {
                column(class = "theme-editor-content") {
                    column(class = "theme-editor-field") {
                        text(class = "theme-editor-label") { "Name" }
                        Input(value = editor.name, class = "theme-editor-name-input", a11y:label = "Theme name")
                    }
                    column(class = "theme-editor-field") {
                        text(class = "theme-editor-label") { "Start from" }
                        Select(value = Binding::controlled(Signal::derive_local(String::new), move |id: String| {
                            if let Some(base) = themes.with_untracked(|themes| themes.iter().find(|theme| theme.id() == id).cloned()) { editor.start_from(&base); }
                        })) {
                            SelectTrigger(label = "Copy colors from", class = "theme-editor-base") { text {"Copy colors from a theme…"} }
                            SelectContent(class = "filter-type-surface theme-menu") {
                                for choice in move || themes.get(), key = |choice| choice.id().into_owned() {
                                    SelectItem(value = choice.id().into_owned(), class = format!("theme-base-{}", choice.id().replace([':', ' '], "-"))) { {choice.name().to_owned()} }
                                }
                            }
                        }
                    }
                    ThemeExample()
                    column(class = "theme-editor-group") {
                        text(class = "theme-editor-group-title") { "Shape" }
                        text(class = "theme-editor-group-note") { "Rounding of buttons, fields, cards and menus." }
                        for radius in move || RADII.to_vec(), key = |radius| radius.0 {
                            ThemeRadiusRow(editor = editor, token = radius.0, label = radius.1)
                        }
                    }
                    for group in move || GROUPS.to_vec(), key = |group| group.0 {
                        column(class = "theme-editor-group") {
                            text(class = "theme-editor-group-title") { {group.0} }
                            text(class = "theme-editor-group-note") { {group.1} }
                            for entry in move || group.2.to_vec(), key = |entry| entry.0 {
                                ThemeTokenRow(editor = editor, token = entry.0, label = entry.1)
                            }
                        }
                    }
                }
            }
            if move || !editor.error.with(String::is_empty) {
                text(class = "warnings theme-editor-error", a11y:role = Role::Alert) {{move || editor.error.get()}}
            }
            row(class = "theme-editor-footer") {
                if move || editor.editing.with_value(Option::is_some) && editor.open.get() {
                    ActionButton(class = "theme-editor-delete", on:click = move |_| delete()) { "Delete" }
                }
                box(class = "theme-editor-spacer")
                ActionButton(class = "theme-editor-cancel", on:click = move |_| editor.open.set(false)) { "Cancel" }
                ActionButton(class = "theme-editor-save", disabled = Signal::derive_local(move || !editor.name.with(|name| crate::preferences::valid_theme_name(name.trim()))), on:click = move |_| save()) { "Save theme" }
            }
        }
    }
}

/// Closing without saving, by any means, restores the theme the session started from. Lives
/// outside the panel so that it still runs when the panel unmounts.
#[component]
pub fn ThemeEditorRestore(
    editor: ThemeEditor,
    palette: RwSignal<Palette, LocalStorage>,
) -> impl IntoView {
    let restore = RenderEffect::new(move |_: Option<()>| {
        if !editor.open.get()
            && let Some(previous) = editor.previous.try_update_value(Option::take).flatten()
        {
            palette.set(previous);
        }
    });
    on_cleanup_local(move || drop(restore));
}

/// A small mock of the app drawn with the theme tokens, so each change shows where it lands.
#[component]
fn ThemeExample() -> impl IntoView {
    view! {
        column(class = "theme-example", a11y:label = "Theme example") {
            text(class = "theme-example-heading") { "Example" }
            column(class = "theme-example-card") {
                text(class = "theme-example-strong") { "Roll rate" }
                text(class = "theme-example-body") { "Body text on a panel" }
                text(class = "theme-example-muted") { "Secondary text · " text(class = "theme-example-dim") { "hint" } }
                row(class = "theme-example-buttons") {
                    row(class = "theme-example-button") { text { "Idle" } }
                    row(class = "theme-example-button theme-example-hover") { text { "Hover" } }
                    row(class = "theme-example-button theme-example-pressed") { text { "Pressed" } }
                }
                row(class = "theme-example-buttons") {
                    row(class = "theme-example-button theme-example-selected") { text { "Selected" } }
                    row(class = "theme-example-button theme-example-disabled") { text { "Disabled" } }
                    row(class = "theme-example-button theme-example-focus") { text { "Focused" } }
                }
                row(class = "theme-example-input") { text { "Input field" } }
                row(class = "theme-example-plot") {
                    column(class = "theme-example-plot-segment theme-example-plot-left") { box(class = "theme-example-trace") }
                    column(class = "theme-example-plot-segment theme-example-band") { box(class = "theme-example-trace") }
                    column(class = "theme-example-plot-segment theme-example-plot-right") { box(class = "theme-example-trace") }
                }
                row(class = "theme-example-chips") {
                    row(class = "theme-example-chip theme-example-accent") { text { "Accent" } }
                    row(class = "theme-example-chip theme-example-warning") { text { "Warning" } }
                    row(class = "theme-example-chip theme-example-danger") { text { "Delete" } }
                    row(class = "theme-example-chip theme-example-draft") { text { "Draft" } }
                }
            }
        }
    }
}

#[component]
fn ThemeTokenRow(editor: ThemeEditor, token: &'static str, label: &'static str) -> impl IntoView {
    let index = editor
        .index_of(token)
        .expect("Editor tokens come from the default palette");
    let value = Signal::derive_local(move || {
        editor.tokens.with(|tokens| {
            tokens
                .get(index)
                .map(|entry| entry.1.clone())
                .unwrap_or_default()
        })
    });
    let color = Signal::derive_local(move || value.with(|value| parse_hex(value)));
    view! {
        row(class = "theme-token-row") {
            Popover {
                PopoverTrigger(class = "theme-token-swatch", size = ButtonSize::IconXs, variant = ButtonVariant::Ghost, a11y:label = format!("Pick {label} color"), style:background = move || color.get().map(|_| value.get())) {""}
                PopoverContent(class = "band-color-popover") {
                    ColorPicker(color = Signal::derive_local(move || color.get().unwrap_or([0, 0, 0, 255])), on_change = UnsyncCallback::new(move |rgba: [u8; 4]| editor.set_token(index, hex(rgba))))
                }
            }
            text(class = "theme-token-name") { {label} }
            Input(class = "theme-token-value", a11y:label = format!("{label} color value"), value = Binding::controlled(value, move |text: String| editor.set_token(index, text)))
        }
    }
}

#[component]
fn ThemeRadiusRow(editor: ThemeEditor, token: &'static str, label: &'static str) -> impl IntoView {
    let index = editor
        .index_of(token)
        .expect("Editor tokens come from the default palette");
    let pixels = Signal::derive_local(move || {
        editor.tokens.with(|tokens| {
            tokens
                .get(index)
                .and_then(|entry| {
                    entry
                        .1
                        .trim()
                        .strip_suffix("px")?
                        .trim()
                        .parse::<f64>()
                        .ok()
                })
                .unwrap_or(0.)
        })
    });
    view! {
        row(class = "theme-token-row") {
            text(class = "theme-token-name") { {label} }
            Slider(class = "theme-radius-slider", min = 0., max = 16., step = 1., label = format!("{label} radius"), value = Binding::controlled(pixels, move |value: f64| editor.set_token(index, format!("{}px", value.round()))))
            text(class = "theme-radius-value") {{move || format!("{} px", pixels.get())}}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GROUPS, RADII};
    use crate::theme::{Palette, tokens};

    #[test]
    fn the_editor_shows_every_theme_token_exactly_once() {
        let shown: Vec<&str> = GROUPS
            .iter()
            .flat_map(|group| group.2.iter().map(|entry| entry.0))
            .chain(RADII.iter().map(|entry| entry.0))
            .collect();
        let default = Palette::default();
        let defined: Vec<&str> = tokens(default.declarations())
            .map(|(token, _)| token)
            .collect();
        for token in &defined {
            assert_eq!(
                shown.iter().filter(|shown| *shown == token).count(),
                1,
                "{token} must appear once in the editor"
            );
        }
        assert_eq!(
            shown.len(),
            defined.len(),
            "The editor lists only defined tokens"
        );
    }
}
