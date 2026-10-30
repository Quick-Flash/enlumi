use super::{DockPane, DockState, FrameHandle, finish_tab_after};
use crate::workspace::{TabStripSnapshot, layout::DropZone, tab_offsets};
use crate::zgui_svg::node_rect;
use std::{
    rc::Rc,
    time::{Duration, Instant},
};
use zgui::prelude::*;
use zgui::reactive::{Callable, UnsyncCallback};
use zgui_ui::prelude::*;

/// Animated tab strip. Caller supplies pane titles and application actions.
#[component]
pub fn PlotTabs(
    pane: DockPane,
    dock: DockState,
    title: UnsyncCallback<u64, String>,
    on_select: UnsyncCallback<u64>,
    on_close: UnsyncCallback<u64>,
    on_add: UnsyncCallback<u64>,
    /// Double-click a title or press F2 to edit it in place. Empty name restores caller default.
    #[prop(optional)]
    on_rename: Option<UnsyncCallback<(u64, String)>>,
) -> impl IntoView {
    let last_click = StoredValue::new_local(None::<(u64, Instant, (f64, f64))>);
    let editing = RwSignal::new_local(None::<u64>);
    let draft = RwSignal::new_local(String::new());
    let rename_input = NodeRef::new();
    let rename_frame = StoredValue::new_local(None::<FrameHandle>);
    // Wait for the input's binding, not just the editing flag: frame callbacks can
    // run before the reactive branch has mounted its input.
    let rename_focus = zgui::reactive::RenderEffect::new(move |_| {
        if editing.get().is_some() && rename_input.get().is_some() {
            rename_frame.set_value(Some(request_frame(move |_| {
                rename_input.focus();
                rename_input.select_all();
                rename_frame.set_value(None);
            })));
        }
    });
    on_cleanup_local(move || {
        drop(rename_focus);
        rename_frame.set_value(None);
    });
    let begin_rename = move |id| {
        if on_rename.is_none() {
            return;
        }
        last_click.set_value(None);
        draft.set(title.run(id));
        editing.set(Some(id));
    };
    let finish_rename = move |id| {
        if editing.get_untracked() != Some(id) {
            return;
        }
        let name = draft
            .get_untracked()
            .trim()
            .chars()
            .take(80)
            .collect::<String>();
        editing.set(None);
        if let Some(callback) = on_rename {
            callback.run((id, name));
        }
    };
    let slot = pane;
    let layout = dock.layout;
    let tab_drag = dock.drag;
    let tab_drop_target = dock.target;
    let tab_preview = dock.preview;
    let tab_nodes = dock.nodes;
    let tab_strips = dock.strips;
    let tab_pointer = dock.pointer;
    let tab_settling = dock.settling;
    let tab_rebase = dock.rebase;
    let tab_settle_frame = dock.settle_frame;
    let tab_settle_done = dock.settle_done;
    let suppress_tab_click = dock.suppress_click;
    let select_plot = move |id| on_select.run(id);
    let close_plot = move |id| on_close.run(id);
    let tab_hit_target = move |x, y, source| dock.hit_target(x, y, source);
    view! {
        row(class = "plot-tab-navigation", node_ref = slot.tabs_navigation) {
            if move || slot.tabs_overflow.get() {
            row(class = "plot-tab-arrow-guard",
                on:pointer_down = move |ev| ev.stop_propagation(),
                on:pointer_up = move |ev| ev.stop_propagation(),
                on:click = move |ev| ev.stop_propagation()) {
            Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost,
                class = "plot-tab-arrow", a11y:label = "Show earlier tabs",
                disabled = Signal::derive_local(move || slot.tab_offset.get().min(slot.tab_limit.get()) <= 0.0),
                on:click = move |ev| {
                    ev.stop_propagation();
                    let step = node_rect(slot.tabs_node).map_or(80.0, |rect| rect.width * 0.8);
                    slot.tab_offset.update(|offset| *offset = (offset.min(slot.tab_limit.get_untracked()) - step).max(0.0));
                }) {
                    vector(class = "plot-tab-arrow-icon", prop:viewBox = "0 0 12 12", prop:d = "M8 2.5 L4.5 6 L8 9.5", a11y:hidden = true)
                }
            }
            }
        row(class = "plot-tabs", node_ref = slot.tabs_node,
            style:width = move || Some(format!("{}px", slot.tabs_width.get()))) {
        row(class = "plot-tab-track", node_ref = slot.tabs_track,
            style:margin-left = move || Some(format!("-{}px", slot.tab_offset.get().min(slot.tab_limit.get())))) {
            for tab_id in move || layout.get().tab_ids(slot.id), key = |id| *id {
                row(class = "plot-tab",
                    node_ref = {
                        let node = NodeRef::new();
                        tab_nodes.update_value(|nodes| { nodes.insert((slot.id, tab_id), node); });
                        node
                    },
                    class:plot-tab-active = move || layout.get().is_visible(tab_id),
                    class:tab-drag-source = move || tab_drag.get().is_some_and(|drag| drag.0 == tab_id && drag.3),
                    class:tab-rebase = move || tab_rebase.get(),
                    class:tab-drop-before = move || tab_drop_target.get() == Some((tab_id, DropZone::TabBefore)),
                    class:tab-drop-after = move || tab_drop_target.get() == Some((tab_id, DropZone::TabAfter)),
                    style:transform = move || {
                        if let Some(offsets) = tab_settling.get() {
                            return Some(format!("translateX({}px)", offsets.get(&(slot.id, tab_id)).copied().unwrap_or(0.0)));
                        }
                        let Some((source, start_x, _, true, original)) = tab_drag.get() else { return Some("translateX(0px)".to_owned()); };
                        if tab_id == source {
                            let pointer = tab_pointer.get();
                            let inside_tabs = tab_strips.with_value(|strips| strips.iter().any(|strip| {
                                let rect = strip.rect;
                                f64::from(pointer.0) >= rect.x && f64::from(pointer.0) <= rect.x + rect.width
                                    && f64::from(pointer.1) >= rect.y && f64::from(pointer.1) <= rect.y + rect.height
                            }));
                            return Some(format!("translateX({}px)", if inside_tabs { pointer.0 - start_x } else { 0.0 }));
                        }
                        let (anchor, preview) = tab_preview.get()?;
                        if !original.tab_ids(anchor).contains(&slot.id) { return None; }
                        let order = preview.tab_ids(slot.id);
                        tab_strips.with_value(|strips| {
                            let strip = strips.iter().find(|strip| strip.pane == slot.id)?;
                            let old = strip.tabs.iter().find(|(id, _)| *id == tab_id)?.1;
                            let mut left = strip.tabs.first()?.1.x;
                            let gap = strip.tabs.windows(2).next().map_or(1.0, |tabs| (tabs[1].1.x - tabs[0].1.x - tabs[0].1.width).max(0.0));
                            for id in order {
                                if id == tab_id { return Some(format!("translateX({}px)", left - old.x)); }
                                let width = strips.iter().flat_map(|strip| &strip.tabs).find(|(tab, _)| *tab == id).map_or(0.0, |(_, rect)| rect.width);
                                left += width + gap;
                            }
                            None
                        })
                    },
                    on:pointer_down = move |ev| {
                        if editing.get_untracked() == Some(tab_id) { ev.stop_propagation(); return; }
                        if ev.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                        if let Some(done) = tab_settle_done.get_value() {
                            tab_settle_frame.set_value(None);
                            done();
                        }
                        suppress_tab_click.set(None);
                        let current = layout.get_untracked();
                        let snapshots = dock.panes.get_untracked().into_iter().filter(|pane| current.is_visible(pane.id)).filter_map(|pane| {
                            let rect = node_rect(pane.tabs_node)?;
                            let tabs = tab_nodes.with_value(|nodes| current.tab_ids(pane.id).into_iter().filter_map(|id| {
                                let bounds = node_rect(*nodes.get(&(pane.id, id))?)?;
                                Some((id, bounds))
                            }).collect());
                            Some(TabStripSnapshot { pane: pane.id, rect, tabs })
                        }).collect();
                        tab_strips.set_value(snapshots);
                        tab_pointer.set((ev.position.x.0, ev.position.y.0));
                        tab_drag.set(Some((tab_id, ev.position.x.0, ev.position.y.0, false, layout.get_untracked())));
                        dock.group_target.set(None);
                        ev.capture_pointer();
                        ev.stop_propagation();
                    },
                    on:pointer_move = move |ev| {
                        let Some((source, x, y, moving, _)) = tab_drag.get_untracked() else { return; };
                        if source != tab_id { return; }
                        tab_pointer.set((ev.position.x.0, ev.position.y.0));
                        if !moving && (ev.position.x.0 - x).hypot(ev.position.y.0 - y) >= 5.0 {
                            last_click.set_value(None);
                            tab_drag.update(|drag| { if let Some(drag) = drag { drag.3 = true; } });
                        }
                        if let Some((source, _, _, true, _)) = tab_drag.get_untracked() {
                            let previous_group=dock.group_target.get_untracked();
                            let target = tab_hit_target(ev.position.x.0, ev.position.y.0, source);
                            if target == tab_drop_target.get_untracked() && previous_group==dock.group_target.get_untracked() { return; }
                            tab_drop_target.set(target);
                            tab_preview.set(target.and_then(|(target,_)|dock.proposal(source).map(|tree|(target,tree))));

                        }
                    },
                    on:pointer_up = move |ev| {
                        let clicked = tab_drag.get_untracked().is_some_and(|drag| drag.0 == tab_id && !drag.3);
                        ev.release_pointer();
                        if let Some((source, _, _, true, original)) = tab_drag.get_untracked() {
                            suppress_tab_click.set(Some(source));
                            let target=tab_hit_target(ev.position.x.0,ev.position.y.0,source);
                            tab_drop_target.set(target);
                            let proposal=dock.proposal(source);
                            let applied=proposal.is_some();
                            let final_tree=proposal.unwrap_or(original.clone());
                            let local_move = target.is_none_or(|(target, zone)| {
                                matches!(zone, DropZone::TabBefore | DropZone::TabAfter) && original.tab_ids(source).contains(&target)
                            });
                            let settle_offsets = local_move.then(|| tab_strips.with_value(|strips| tab_offsets(strips, &final_tree)));
                            let done: Rc<dyn Fn()> = Rc::new(move || {
                                tab_settle_done.set_value(None);
                                tab_rebase.set(true);
                                if layout.get_untracked() != final_tree { layout.set(final_tree.clone()); }
                                tab_settling.set(None);
                                if applied { select_plot(source); }
                                tab_settle_frame.set_value(Some(request_frame(move |_| {
                                    tab_rebase.set(false);
                                    tab_settle_frame.set_value(None);
                                })));
                            });
                            if let Some(offsets) = settle_offsets {
                                tab_settling.set(Some(offsets));
                                tab_settle_done.set_value(Some(Rc::clone(&done)));
                                finish_tab_after(Instant::now() + Duration::from_millis(160), tab_settle_frame, done);
                            } else {
                                done();
                            }
                            ev.prevent_default(); ev.stop_propagation();
                        }
                        tab_drag.set(None); tab_drop_target.set(None); tab_preview.set(None); dock.group_target.set(None);
                        if clicked {
                            ev.prevent_default(); ev.stop_propagation();
                            select_plot(tab_id);
                            // Tab dragging owns pointer capture and suppresses framework activation,
                            // so its ordinary clicks must also recognize double-click here.
                            let now = Instant::now();
                            let position = (f64::from(ev.position.x.0), f64::from(ev.position.y.0));
                            let double = last_click.get_value().is_some_and(|(id, at, previous)| {
                                id == tab_id && crate::interaction::is_double_click(now.saturating_duration_since(at), previous, position)
                            });
                            last_click.set_value(if double { None } else { Some((tab_id, now, position)) });
                            if double { begin_rename(tab_id); }
                        }
                    },
                    on:pointer_cancel = move |ev| {
                        last_click.set_value(None);
                        tab_drag.set(None); tab_drop_target.set(None); tab_preview.set(None); dock.group_target.set(None); ev.release_pointer();
                    },
                    on:click = move |ev| {
                        ev.stop_propagation();
                        if suppress_tab_click.get_untracked() != Some(tab_id) { select_plot(tab_id); }
                    }) {
                    if move || editing.get() == Some(tab_id) {
                        Input(class = "plot-tab-name", node_ref = rename_input, value = draft, a11y:label = "Plot tab name",
                            on:pointer_down = move |ev| ev.stop_propagation(),
                            on:pointer_up = move |ev| ev.stop_propagation(),
                            on:click = move |ev| ev.stop_propagation(),
                            on:double_click = move |ev| ev.stop_propagation(),
                            on:focus_out = move |_| finish_rename(tab_id),
                            on:key_down = move |ev| {
                                match ev.key {
                                    Key::Named(NamedKey::Enter) => finish_rename(tab_id),
                                    Key::Named(NamedKey::Escape) => { editing.set(None); },
                                    _ => return,
                                }
                                ev.prevent_default(); ev.stop_propagation();
                            })
                    } else {
                        Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "plot-tab-select",
                            on:double_click = move |ev| { begin_rename(tab_id); ev.prevent_default(); ev.stop_propagation(); },
                            on:key_down = move |ev| {
                                last_click.set_value(None);
                                if ev.key == Key::Named(NamedKey::F2) { begin_rename(tab_id); ev.prevent_default(); ev.stop_propagation(); }
                            }) {{move || title.run(tab_id)}}
                    }
                    Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost,
                        class = "plot-tab-close", a11y:label = "Close plot tab",
                        on:pointer_down = move |ev| ev.stop_propagation(),
                        on:pointer_up = move |ev| ev.stop_propagation(),
                        on:click = move |ev| { ev.stop_propagation(); close_plot(tab_id); }) { "×" }
                    }
            }
        }
        }
            Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost,
                class = "plot-tab-add", a11y:label = "Add plot tab",
                on:click = move |_| on_add.run(slot.id)
                ) {
                    vector(class = "plot-tab-add-icon", prop:viewBox = "0 0 12 12", prop:d = "M6 2 L6 10 M2 6 L10 6", a11y:hidden = true)
                }
            if move || slot.tabs_overflow.get() {
            row(class = "plot-tab-arrow-guard",
                on:pointer_down = move |ev| ev.stop_propagation(),
                on:pointer_up = move |ev| ev.stop_propagation(),
                on:click = move |ev| ev.stop_propagation()) {
            Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost,
                class = "plot-tab-arrow", a11y:label = "Show later tabs",
                disabled = Signal::derive_local(move || slot.tab_offset.get() >= slot.tab_limit.get()),
                on:click = move |ev| {
                    ev.stop_propagation();
                    let step = node_rect(slot.tabs_node).map_or(80.0, |rect| rect.width * 0.8);
                    slot.tab_offset.update(|offset| *offset = (offset.min(slot.tab_limit.get_untracked()) + step).min(slot.tab_limit.get_untracked()));
                }) {
                    vector(class = "plot-tab-arrow-icon", prop:viewBox = "0 0 12 12", prop:d = "M4 2.5 L7.5 6 L4 9.5", a11y:hidden = true)
                }
            }
            }
        }

    }
}
