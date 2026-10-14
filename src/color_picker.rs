use zgui::{
    canvas::{Brush, ShapeBuilder, zgui_color::Color},
    prelude::*,
    reactive::{Callable, RenderEffect, UnsyncCallback},
};
use zgui_ui::prelude::*;

pub fn rgb(hsv: (f64, f64, f64)) -> [u8; 4] {
    let (h, s, v) = hsv;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let byte = |value: f64| ((value + v - c).clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(r), byte(g), byte(b), 255]
}
pub fn hsv(color: [u8; 4]) -> (f64, f64, f64) {
    let [r, g, b, _] = color.map(|value| f64::from(value) / 255.0);
    let v = r.max(g).max(b);
    let low = r.min(g).min(b);
    let delta = v - low;
    let h = if delta == 0.0 {
        0.0
    } else if v == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if v == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    (h, if v == 0.0 { 0.0 } else { delta / v }, v)
}

fn hex(color: [u8; 4]) -> String {
    let [r, g, b, _] = color;
    format!("#{r:02X}{g:02X}{b:02X}")
}
fn parse_hex(text: &str) -> Option<[u8; 4]> {
    let text = text.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    Some([(value >> 16) as u8, (value >> 8) as u8, value as u8, 255])
}

#[component]
pub fn ColorPicker(
    color: Signal<[u8; 4], LocalStorage>,
    on_change: UnsyncCallback<[u8; 4]>,
) -> impl IntoView {
    let value = RwSignal::new_local(hsv(color.get_untracked()));
    let text = RwSignal::new_local(hex(color.get_untracked()));
    let sync = RenderEffect::new(move |_| {
        let color = color.get();
        if parse_hex(&text.get_untracked()) != Some(color) {
            text.set(hex(color));
        }
        if rgb(value.get_untracked()) != color {
            value.set(hsv(color));
        }
    });
    on_cleanup_local(move || drop(sync));
    let node = NodeRef::new();
    let hue_node = NodeRef::new();
    let hue_dragging = RwSignal::new_local(false);
    let choose_hue = move |position: zgui::geom::Point<zgui::geom::CssPx, zgui::geom::Css>| {
        let Some(bounds) = hue_node.window_bounds() else {
            return;
        };
        if bounds.size.width.0 <= 0.0 {
            return;
        }
        let h = (f64::from(
            (position.x.0 * hue_node.scale() - bounds.origin.x.0) / bounds.size.width.0,
        )
        .clamp(0.0, 1.0)
            * 359.0)
            .round();
        value.update(|value| value.0 = h);
        on_change.run(rgb(value.get_untracked()));
    };
    let dragging = RwSignal::new_local(false);
    let choose = move |position| {
        let Some((x, y)) = plot::zgui_renderer::local_position(&node, position) else {
            return;
        };

        value.update(|value| {
            value.1 = ((x - 6.0) / 168.0).clamp(0.0, 1.0);
            value.2 = (1.0 - (y - 6.0) / 168.0).clamp(0.0, 1.0);
        });
        on_change.run(rgb(value.get_untracked()));
    };
    let square = zgui::elements::canvas()
        .class("color-square-image")
        .draw(move |cx| {
            let (h, s, v) = value.get();
            let color = |hsv| {
                let [r, g, b, a] = rgb(hsv);
                Color::srgb_u8(r, g, b, a)
            };
            let square = kurbo::Rect::new(6.0, 6.0, 174.0, 174.0);
            cx.scene.push(
                ShapeBuilder::new(kurbo::Shape::to_path(&square, 0.1))
                    .fill(Brush::Linear {
                        start: (6.0, 6.0).into(),
                        end: (174.0, 6.0).into(),
                        stops: vec![(0.0, color((h, 0.0, 1.0))), (1.0, color((h, 1.0, 1.0)))],
                        repeating: false,
                    })
                    .build(),
            );
            cx.scene.push(
                ShapeBuilder::new(kurbo::Shape::to_path(&square, 0.1))
                    .fill(Brush::Linear {
                        start: (6.0, 6.0).into(),
                        end: (6.0, 174.0).into(),
                        stops: vec![
                            (0.0, Color::srgb_u8(0, 0, 0, 0)),
                            (1.0, Color::srgb_u8(0, 0, 0, 255)),
                        ],
                        repeating: false,
                    })
                    .build(),
            );
            let mut dot = kurbo::BezPath::new();
            plot::append_circle(&mut dot, (6.0 + 168.0 * s, 6.0 + 168.0 * (1.0 - v)), 5.0);
            cx.scene.push(
                ShapeBuilder::new(dot.clone())
                    .fill(Brush::Solid(color((h, s, v))))
                    .build(),
            );
            plot::zgui_renderer::push_stroke(
                cx.scene,
                dot.clone(),
                Color::srgb_u8(0, 0, 0, 255),
                3.0,
            );
            plot::zgui_renderer::push_stroke(
                cx.scene,
                dot,
                Color::srgb_u8(255, 255, 255, 255),
                1.0,
            );
        });
    view! {
        column(class = "color-picker",
            on:pointer_down:capture = move |event| {
                if event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                let Some(bounds) = hue_node.window_bounds() else { return; };
                let (x, y) = (event.position.x.0 * hue_node.scale(), event.position.y.0 * hue_node.scale());
                if x < bounds.origin.x.0 || x > bounds.origin.x.0 + bounds.size.width.0 || y < bounds.origin.y.0 || y > bounds.origin.y.0 + bounds.size.height.0 { return; }
                hue_dragging.set(true); dragging.set(false); choose_hue(event.position);
                event.capture_pointer(); event.prevent_default(); event.stop_propagation(); hue_node.focus();
            },
            on:pointer_move:capture = move |event| if hue_dragging.get_untracked() { choose_hue(event.position); event.stop_propagation(); },
            on:pointer_up:capture = move |event| if hue_dragging.get_untracked() {
                choose_hue(event.position); hue_dragging.set(false); event.release_pointer(); event.prevent_default(); event.stop_propagation();
            },
            on:pointer_cancel:capture = move |event| if hue_dragging.get_untracked() { hue_dragging.set(false); event.release_pointer(); event.stop_propagation(); }) {
            control(class = "color-square", node_ref = node, tabindex = Focus::Sequential,
                {..Attrs::new().a11y_from(A11yBinding::new(Role::Slider).label("Line color: saturation and brightness; arrows adjust saturation and brightness").numeric_value(move || value.get().1 * 100.0).step(|binding| binding.numeric_range(0.0, 100.0).numeric_step(5.0)))},
                on:pointer_down = move |event| { if event.button == Some(zgui::vocab::PointerButton::Primary) { dragging.set(true); choose(event.position); event.capture_pointer(); event.stop_propagation(); } },
                on:pointer_move = move |event| if dragging.get_untracked() { choose(event.position); event.stop_propagation(); },
                on:pointer_up = move |event| { dragging.set(false); event.release_pointer(); event.stop_propagation(); },
                on:pointer_cancel = move |event| { dragging.set(false); event.release_pointer(); event.stop_propagation(); },
                on:key_down = move |event| {
                    use zgui::vocab::{Key, NamedKey};
                    match event.key {
                        Key::Named(NamedKey::ArrowRight) => value.update(|value| value.1 = (value.1 + 0.05).min(1.0)),
                        Key::Named(NamedKey::ArrowLeft) => value.update(|value| value.1 = (value.1 - 0.05).max(0.0)),
                        Key::Named(NamedKey::ArrowUp) => value.update(|value| value.2 = (value.2 + 0.05).min(1.0)),
                        Key::Named(NamedKey::ArrowDown) => value.update(|value| value.2 = (value.2 - 0.05).max(0.0)),
                        _ => return,
                    }
                    on_change.run(rgb(value.get_untracked())); event.prevent_default(); event.stop_propagation();
                }) { {square.into_view()} }
            row(class = "plot-toolbar") {
                text { "Hue" }
                Slider(node_ref = hue_node, class = "color-hue", style:--picker-hue = move || Some(format!("hsl({}, 100%, 50%)", value.get().0)), min = 0.0, max = 359.0, step = 1.0, label = "Color hue", value = Binding::controlled(Signal::derive_local(move || value.get().0), move |h| { value.update(|value| value.0 = h); on_change.run(rgb(value.get_untracked())); }))
            }
            row(class = "color-hex-row") {
                text(class = "color-hex-label") { "Hex" }
                Input(class = "color-hex", value = text, label = "Line color hexadecimal RGB", placeholder = "#RRGGBB",
                    invalid = Signal::derive_local(move || parse_hex(&text.get()).is_none()),
                    on_change = UnsyncCallback::new(move |text: String| { if let Some(color) = parse_hex(&text) { on_change.run(color); } }))
            }
            text(class = "color-value", style:background-color = move || { let [r,g,b,_] = color.get(); Some(format!("rgb({r}, {g}, {b})")) }, style:color = move || { let [r,g,b,_] = color.get(); Some(if u32::from(r)*299 + u32::from(g)*587 + u32::from(b)*114 > 140000 { "#12181c" } else { "#ffffff" }.to_owned()) }) {{move || { let [r,g,b,_] = color.get(); hex([r,g,b,255]) }}}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_accepts_rgb_and_rejects_partial_or_invalid_input() {
        assert_eq!(parse_hex(" #75c58b "), Some([117, 197, 139, 255]));
        assert_eq!(parse_hex("FFFFFF"), Some([255, 255, 255, 255]));
        for invalid in ["", "#75C", "#GGGGGG", "#12345678", "12345"] {
            assert_eq!(parse_hex(invalid), None);
        }
        assert_eq!(hex([0, 17, 255, 255]), "#0011FF");
    }
    #[test]
    fn picker_spans_colors_white_and_black_and_round_trips() {
        for color in [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [117, 197, 139, 255],
            [255, 255, 255, 255],
            [0, 0, 0, 255],
        ] {
            assert_eq!(rgb(hsv(color)), color);
        }
        assert_eq!(rgb((120.0, 1.0, 1.0)), [0, 255, 0, 255]);
    }
}
