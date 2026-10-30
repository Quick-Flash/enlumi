//! Generic draggable signal list item; application supplies identity and drop action.
use zgui::prelude::*;
use zgui::reactive::{Callable, UnsyncCallback};
use zgui::vocab::{Key, NamedKey, PointerButton};

/// Shared signal drag, in window CSS coordinates. Five-pixel threshold preserves clicks.
#[derive(Clone, Copy)]
pub struct SeriesDrag {
    pub state: RwSignal<Option<(String, f32, f32, bool)>, LocalStorage>,
    pub pointer: RwSignal<(f32, f32), LocalStorage>,
}

impl Default for SeriesDrag {
    fn default() -> Self {
        Self {
            state: RwSignal::new_local(None),
            pointer: RwSignal::new_local((0.0, 0.0)),
        }
    }
}

/// Focusable source item, usable with virtual lists and arbitrary data adapters.
#[derive(Clone, Copy)]
pub struct SeriesSourceActions {
    pub on_move: UnsyncCallback<(f32, f32)>,
    pub on_drop: UnsyncCallback<(String, f32, f32)>,
    pub on_cancel: UnsyncCallback<()>,
    /// Clear drop feedback on press and release, without changing status text.
    pub on_finish: UnsyncCallback<()>,
}

/// Drag source with caller-owned assignment and feedback callbacks.
#[component]
pub fn SeriesSource(
    id: Signal<Option<String>, LocalStorage>,
    label: Signal<String, LocalStorage>,
    count: Signal<String, LocalStorage>,
    selected: Signal<bool, LocalStorage>,
    drag: SeriesDrag,
    actions: SeriesSourceActions,
) -> impl IntoView {
    let SeriesSourceActions {
        on_move,
        on_drop,
        on_cancel,
        on_finish,
    } = actions;
    view! {
        control(class = "metric-option", tabindex = Focus::Sequential,
            class:metric-option-selected = move || selected.get(),
            a11y:label = move || zgui::vocab::SharedString::from(label.get()),
            on:pointer_down = move |ev| {
                if ev.button == Some(PointerButton::Primary) && let Some(key) = id.get_untracked() {
                    let position = (ev.position.x.0, ev.position.y.0);
                    on_finish.run(());
                    drag.pointer.set(position);
                    drag.state.set(Some((key, position.0, position.1, false)));
                    ev.capture_pointer();
                }
            },
            on:pointer_move = move |ev| {
                if drag.state.get_untracked().is_none() { return; }
                let position = (ev.position.x.0, ev.position.y.0);
                drag.pointer.set(position);
                drag.state.update(|drag| {
                    if let Some((_, x, y, moving)) = drag {
                        *moving |= (position.0 - *x).hypot(position.1 - *y) >= 5.0;
                    }
                });
                on_move.run(position);
            },
            on:key_down = move |ev| {
                if ev.key == Key::Named(NamedKey::Escape) && drag.state.get_untracked().is_some() {
                    drag.state.set(None);
                    on_cancel.run(());
                    ev.prevent_default(); ev.stop_propagation();
                }
            },
            on:pointer_cancel = move |ev| {
                drag.state.set(None); on_cancel.run(()); ev.release_pointer();
            },
            on:pointer_up = move |ev| {
                if let Some((key, _, _, true)) = drag.state.get_untracked() {
                    on_drop.run((key, ev.position.x.0, ev.position.y.0));
                }
                drag.state.set(None); on_finish.run(()); ev.release_pointer();
            }) {
            text(class = "signal-name") {{move || label.get()}}
            text(class = "metric-option-count") {{move || count.get()}}
        }
    }
}
