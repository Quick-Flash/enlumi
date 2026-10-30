//! Interactive legend entries for zgui plots.
//!
//! Each entry toggles its series on click or Enter/Space and, while the pointer
//! is over it, publishes its key so the plot can emphasise that series. Keys are
//! any comparable type, so callers can use an enum instead of encoded strings.
//!
//! Styling hooks: `plot-legend-item` on every entry, `plot-legend-line` on line
//! entries, and `legend-hidden` while the series is hidden.

use std::collections::BTreeSet;

pub use crate::LegendMarker;
use crate::LegendTheme;

use zgui::prelude::*;
use zgui::reactive::{Callable, UnsyncCallback};
use zgui::{component, view};

/// CSS for [`LegendItem`]'s classes, from the same theme exports use: dot and
/// line entry text, with separate hidden-series opacity for each marker.
pub fn stylesheet(theme: &LegendTheme) -> String {
    format!(
        ".plot-legend-item {{ {} }}\n.plot-legend-line {{ {} }}\n.plot-legend-item.legend-hidden {{ opacity: {}; }}\n.plot-legend-line.legend-hidden {{ opacity: {}; }}\n",
        theme.dot.css(),
        theme.line.css(),
        theme.dot.hidden_opacity,
        theme.line.hidden_opacity
    )
}

/// Add `key` if absent, remove it if present.
pub fn toggle_member<K: Ord + Clone>(set: &mut BTreeSet<K>, key: &K) {
    if !set.remove(key) {
        set.insert(key.clone());
    }
}

/// One clickable, focusable legend entry.
/// Related LegendItem settings.
#[derive(Clone)]
pub struct LegendAppearance {
    /// CSS colour of the entry text.
    pub color: String,
    /// Optional live color, e.g. caller-owned series styling controls.
    pub color_signal: Option<Signal<String, LocalStorage>>,
    pub marker: LegendMarker,
}

#[component]
pub fn LegendItem<K: Clone + PartialEq + 'static>(
    label: String,
    visible: Signal<bool, LocalStorage>,
    on_toggle: UnsyncCallback<()>,
    key: K,
    #[prop(optional)] hovered: Option<RwSignal<Option<K>, LocalStorage>>,
    appearance: LegendAppearance,
) -> impl IntoView {
    let LegendAppearance {
        color,
        color_signal,
        marker,
    } = appearance;

    let line = marker == LegendMarker::Line;
    let text = marker.label(&label);
    view! {
        text(class = "plot-legend-item",
            class:plot-legend-line = move || line,
            class:legend-hidden = move || !visible.get(),
            tabindex = Focus::Sequential,
            style:color = move || Some(color_signal.map_or_else(|| color.clone(), |color| color.get())),
            on:pointer_enter = move |_| if let Some(hovered) = hovered { hovered.set(Some(key.clone())); },
            on:pointer_leave = move |_| if let Some(hovered) = hovered { hovered.set(None); },
            on:pointer_down = move |ev| { if ev.button == Some(PointerButton::Primary) { on_toggle.run(()); ev.stop_propagation(); } },
            on:key_down = move |ev| {
                if matches!(ev.key, Key::Named(NamedKey::Enter | NamedKey::Space)) {
                    on_toggle.run(()); ev.prevent_default(); ev.stop_propagation();
                }
            }) {{text}}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stylesheet_has_one_rule_per_line() {
        let entry = crate::LegendEntryStyle {
            font_size: 14.0,
            font_weight: 400,
            line_height: 18.0,
            hidden_opacity: 0.3,
        };
        let css = stylesheet(&LegendTheme {
            dot: entry,
            line: entry,
        });
        assert!(!css.contains('\\'));
        assert_eq!(css.lines().count(), 4);
        assert!(css.lines().all(|line| line.ends_with('}')));
        assert!(css.contains(".plot-legend-item.legend-hidden { opacity: 0.3; }"));
        assert!(css.contains(".plot-legend-line.legend-hidden { opacity: 0.3; }"));
    }

    #[test]
    fn toggle_member_flips_presence() {
        let mut set = BTreeSet::from(["a".to_owned()]);
        toggle_member(&mut set, &"a".to_owned());
        assert!(set.is_empty());
        toggle_member(&mut set, &"b".to_owned());
        assert!(set.contains("b"));
    }
}
