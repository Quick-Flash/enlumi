use crate::workspace::layout::{Divider, Layout};
use zgui::prelude::*;
use zgui::vocab::{Key, NamedKey};

/// Current splitter drag: ID, orientation, pointer origin, available span, initial ratio.
pub type DividerDrag = RwSignal<Option<(u64, bool, f32, f64, f64)>, LocalStorage>;

/// Keyboard and pointer accessible split resize grip.
#[component]
pub fn SplitDivider(
    id: u64,
    divider: Signal<Option<Divider>, LocalStorage>,
    layout: RwSignal<Layout, LocalStorage>,
    drag: DividerDrag,
    #[prop(default = Signal::stored_local((0.0, 0.0)))] canvas_size: Signal<
        (f64, f64),
        LocalStorage,
    >,
) -> impl IntoView {
    let current = move || divider.get();
    let initial = StoredValue::new_local(None::<Layout>);
    let viewport = StoredValue::new_local((0.0, 0.0));
    let resize = move |tree: &mut Layout, id, ratio, size: (f64, f64)| {
        if size.0 > 0.0 && size.1 > 0.0 {
            tree.resize_divider(id, ratio, size);
        } else {
            tree.resize(id, ratio);
        }
    };
    view! {
                            control(class = "split-grip", class:split-grip-stacked = move || current().is_some_and(|d| d.stacked),
                                tabindex = Focus::Sequential, a11y:label = "Resize split: arrows, Home/End limits, Enter equal sizes",
                                style:left = move || Some(format!("{}px", current().map_or(0.0, |d| d.rect.x))),
                                style:top = move || Some(format!("{}px", current().map_or(0.0, |d| d.rect.y))),
                                style:width = move || Some(format!("{}px", current().map_or(0.0, |d| d.rect.width))),
                                style:height = move || Some(format!("{}px", current().map_or(0.0, |d| d.rect.height))),
                                on:key_down = move |ev| {
                                    let Some(d) = current() else { return };
                                    let Some((ratio, _)) = keyboard_weights(&ev.key, d.stacked, d.ratio, 1.0 - d.ratio) else { return };
                                    drag.set(None); layout.update(|tree| resize(tree, id, ratio, canvas_size.get_untracked()));
                                    ev.prevent_default(); ev.stop_propagation();
                                },
                                on:pointer_down = move |ev| {
                                    if ev.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                                    let Some(d) = current() else { return };
                                    let start = if d.stacked { ev.position.y.0 } else { ev.position.x.0 };
                                    initial.set_value(Some(layout.get_untracked())); viewport.set_value(canvas_size.get_untracked());
                                    drag.set(Some((id, d.stacked, start, d.span, d.ratio))); ev.capture_pointer(); ev.stop_propagation();
                                },
                                on:pointer_move = move |ev| {
                                    if let Some((id, vertical, start, span, ratio)) = drag.get_untracked() {
                                        let pos = if vertical { ev.position.y.0 } else { ev.position.x.0 };
                                        if let Some(mut tree) = initial.get_value() {
                                            resize(&mut tree, id, ratio + f64::from(pos - start) / span.max(1.0), viewport.get_value());
                                            layout.set(tree);
                                        }
                                    }
                                },
                                on:pointer_up = move |ev| { drag.set(None); initial.set_value(None); ev.release_pointer(); },
                                on:pointer_cancel = move |ev| { drag.set(None); initial.set_value(None); ev.release_pointer(); }) {}
    }
}

pub fn keyboard_weights(key: &Key, stacked: bool, a: f64, b: f64) -> Option<(f64, f64)> {
    let total = a + b;
    let delta = match key {
        Key::Named(NamedKey::ArrowLeft) if !stacked => -0.02,
        Key::Named(NamedKey::ArrowRight) if !stacked => 0.02,
        Key::Named(NamedKey::ArrowUp) if stacked => -0.02,
        Key::Named(NamedKey::ArrowDown) if stacked => 0.02,
        Key::Named(NamedKey::Home) => -1.0,
        Key::Named(NamedKey::End) => 1.0,
        Key::Named(NamedKey::Enter) => return Some((total / 2.0, total / 2.0)),
        _ => return None,
    };
    Some(resize_weights(a, b, delta, 1.0))
}

pub fn resize_weights(a: f64, b: f64, delta: f64, pixels: f64) -> (f64, f64) {
    let total = a + b;
    let left = (a + delta / pixels.max(1.0) * total).clamp(total * 0.15, total * 0.85);
    (left, total - left)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_dividers_follow_orientation_and_preserve_total() {
        assert_eq!(
            keyboard_weights(&Key::Named(NamedKey::ArrowRight), false, 1.0, 1.0),
            Some((1.04, 0.96))
        );
        assert_eq!(
            keyboard_weights(&Key::Named(NamedKey::ArrowRight), true, 1.0, 1.0),
            None
        );
        assert_eq!(
            keyboard_weights(&Key::Named(NamedKey::ArrowDown), true, 1.0, 1.0),
            Some((1.04, 0.96))
        );
        assert_eq!(
            keyboard_weights(&Key::Named(NamedKey::Enter), false, 0.5, 1.5),
            Some((1.0, 1.0))
        );
        for key in [NamedKey::Home, NamedKey::End] {
            let (a, b) = keyboard_weights(&Key::Named(key), false, 1.0, 1.0).unwrap();
            assert_eq!(a + b, 2.0);
            assert!((0.3..=1.7).contains(&a));
            assert!((0.3 - 1e-9..=1.7 + 1e-9).contains(&b));
        }
    }
    #[test]
    fn divider_resize_preserves_total_and_bounds_each_pane() {
        assert_eq!(resize_weights(1.0, 1.0, 100.0, 1000.0), (1.2, 0.8));
        let (a, b) = resize_weights(1.0, 1.0, 10000.0, 1000.0);
        assert_eq!(a, 1.7);
        assert!((b - 0.3).abs() < 1e-9);
        assert_eq!(a + b, 2.0);
    }
}
