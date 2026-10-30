use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trajectory::{Pair, Spec},
};
use plot::zgui_workspace::{SeriesSourceActions, SeriesSourceProps};
use zgui::prelude::*;

#[derive(Clone, Copy)]
pub struct Builder {
    pub open: RwSignal<bool, LocalStorage>,
    pub pane: Pane,
    pub target: RwSignal<Option<u64>, LocalStorage>,
    pub area: NodeRef,
}
impl Builder {
    pub fn new() -> Self {
        let pane = Pane::new(0);
        pane.trajectory.set(Some(Spec::default()));
        pane.trajectory_setup.set(true);
        Self {
            open: RwSignal::new_local(false),
            pane,
            target: RwSignal::new_local(None),
            area: NodeRef::new(),
        }
    }
}
impl Workbench {
    pub fn trajectory_builder(self) -> Builder {
        self.view().trajectory_builder
    }
    pub fn builder_slot_at(self, x: f32, y: f32) -> Option<(usize, bool)> {
        let builder = self.trajectory_builder();
        if !builder.open.get_untracked() || !Self::inside(builder.area, x, y) {
            return None;
        }
        self.trajectory_slot_at(builder.pane, x, y)
    }
    pub fn edit_trajectory(self, pane: Pane) {
        let builder = self.trajectory_builder();
        builder.pane.trajectory.set(pane.trajectory.get_untracked());
        builder.pane.lines.set(pane.lines.get_untracked());
        builder.target.set(Some(pane.id));
        builder.open.set(true);
        self.analysis().open.set(false);
        self.filters_open.set(false);
    }
    pub fn drop_trajectory_pair(self, index: usize, target: u64) {
        let builder = self.trajectory_builder();
        let Some(pair) = builder
            .pane
            .trajectory
            .get_untracked()
            .and_then(|spec| spec.pairs().get(index).cloned())
        else {
            return;
        };
        if pair.x.is_empty() || pair.y.is_empty() {
            return;
        }
        let Some(workbench) = self.for_plot(target) else {
            return;
        };
        let Some(original) = workbench.find(target) else {
            return;
        };
        let pane = if builder.target.get_untracked() == Some(target)
            || original.trajectory.with_untracked(Option::is_some)
            || (original.lines.with_untracked(Vec::is_empty)
                && original.analysis.with_untracked(Option::is_none))
        {
            original
        } else {
            workbench.add_plot(target, false, false)
        };
        let mut spec = pane.trajectory.get_untracked().unwrap_or_default();
        let incoming_y = pair.y.clone();
        let previous = spec.pairs();
        let new_trace = !previous.iter().any(|pair| pair.y == incoming_y);
        let replacing = (builder.target.get_untracked() == Some(target))
            .then(|| previous.get(index).map(|pair| pair.y.as_str()))
            .flatten();
        let occupied = pane.lines.with_untracked(|lines| {
            lines
                .iter()
                .filter(|line| {
                    previous.iter().any(|pair| pair.y == line.source)
                        && replacing != Some(line.source.as_str())
                })
                .map(|line| pane.plot.style(&line.id.to_string()).color)
                .collect::<Vec<_>>()
        });
        if pane.trajectory.with_untracked(Option::is_some) {
            if spec.pairs().contains(&pair) {
                workbench.activate(pane.id);
                return;
            }
            if builder.target.get_untracked() == Some(target) && index < spec.pairs().len() {
                let mut pairs = spec.pairs();
                pairs[index] = pair;
                let first = pairs.remove(0);
                spec.x = first.x;
                spec.y = first.y;
                spec.fix = first.fix;
                spec.gps_scale = first.gps_scale;
                spec.extra = pairs;
            } else if spec.x.is_empty() && spec.y.is_empty() {
                spec.x = pair.x;
                spec.y = pair.y;
                spec.fix = pair.fix;
                spec.gps_scale = pair.gps_scale;
            } else {
                spec.extra.push(pair);
            }
        } else {
            spec.x = pair.x;
            spec.y = pair.y;
            spec.fix = pair.fix;
            spec.gps_scale = pair.gps_scale;
        }
        if let Err(error) = spec.validate() {
            self.status.set(error);
            return;
        }
        pane.analysis.set(None);
        pane.trajectory.set(Some(spec.clone()));
        pane.trajectory_setup.set(false);
        workbench.activate(pane.id);
        workbench.profiles.titles.update(|titles| {
            titles.entry(pane.id).or_insert_with(|| "2D plot".into());
        });
        let available = workbench
            .panes()
            .get_untracked()
            .into_iter()
            .chain(std::iter::once(builder.pane))
            .flat_map(|pane| {
                pane.lines
                    .get_untracked()
                    .into_iter()
                    .map(move |line| (pane, line))
            })
            .collect::<Vec<_>>();
        let mut sources = spec
            .sources_mut()
            .filter(|source| !source.is_empty())
            .map(|source| source.clone())
            .collect::<Vec<_>>();
        sources.sort();
        sources.dedup();
        for source in sources {
            if pane
                .lines
                .with_untracked(|lines| lines.iter().any(|line| line.source == source))
            {
                continue;
            }
            if let Some((from, line)) = available.iter().find(|(_, line)| line.source == source) {
                let mut copy = line.clone();
                copy.id = workbench.id();
                let style = from.plot.style(&line.id.to_string());
                let draw = from
                    .drawing
                    .with_untracked(|drawing| drawing.get(&line.id).copied().unwrap_or_default());
                pane.plot.styles.update(|styles| {
                    styles.insert(copy.id.to_string(), style);
                });
                pane.drawing.update(|drawing| {
                    drawing.insert(copy.id, draw);
                });
                pane.lines.update(|lines| lines.push(copy));
            } else {
                workbench.add(source);
            }
        }
        // A pair may reuse a field already present as an X input. Choose colors
        // from visible Y traces, preserving appearance unless its color clashes.
        if new_trace
            && let Some(id) = pane.lines.with_untracked(|lines| {
                lines
                    .iter()
                    .find(|line| line.source == incoming_y)
                    .map(|line| line.id)
            })
        {
            let mut style = pane.plot.style(&id.to_string());
            if occupied.iter().any(|color| color[..3] == style.color[..3]) {
                style.color = crate::line_colors::next_color(&occupied);
                pane.plot.styles.update(|styles| {
                    styles.insert(id.to_string(), style);
                });
            }
        }
    }
}

fn blank_pair() -> Pair {
    Pair {
        x: String::new(),
        y: String::new(),
        gps_scale: None,
        fix: String::new(),
    }
}
fn store(pane: Pane, mut spec: Spec, mut pairs: Vec<Pair>) -> Result<(), String> {
    while pairs.len() > 1
        && pairs
            .last()
            .is_some_and(|pair| pair.x.is_empty() && pair.y.is_empty())
    {
        pairs.pop();
    }
    if pairs.is_empty() {
        pairs.push(blank_pair());
    }
    let first = pairs.remove(0);
    spec.x = first.x;
    spec.y = first.y;
    spec.fix = first.fix;
    spec.gps_scale = first.gps_scale;
    spec.extra = pairs;
    spec.validate()?;
    pane.trajectory.set(Some(spec));
    Ok(())
}
pub fn assign(pane: Pane, index: usize, y: bool, sources: &[String]) -> Result<(), String> {
    let Some(spec) = pane.trajectory.get_untracked() else {
        return Ok(());
    };
    let mut pairs = spec.pairs();
    for (offset, source) in sources.iter().enumerate() {
        let index = index + offset;
        if index >= 256 {
            return Err("A 2D plot supports at most 256 traces".into());
        }
        while pairs.len() <= index {
            pairs.push(blank_pair());
        }
        if y {
            pairs[index].y = source.clone();
        } else {
            pairs[index].x = source.clone();
        }
    }
    store(pane, spec, pairs)
}
fn swap_slots(pane: Pane, from: (usize, bool), to: (usize, bool)) -> Result<(), String> {
    let Some(spec) = pane.trajectory.get_untracked() else {
        return Ok(());
    };
    let mut pairs = spec.pairs();
    if from.0.max(to.0) >= 256 {
        return Err("A 2D plot supports at most 256 pairs".into());
    }
    while pairs.len() <= from.0.max(to.0) {
        pairs.push(blank_pair());
    }
    let value = |slot: (usize, bool)| {
        pairs.get(slot.0).map(|pair| {
            if slot.1 {
                pair.y.clone()
            } else {
                pair.x.clone()
            }
        })
    };
    let (Some(a), Some(b)) = (value(from), value(to)) else {
        return Err("Coordinate slot no longer exists".into());
    };
    if from.1 {
        pairs[from.0].y = b;
    } else {
        pairs[from.0].x = b;
    }
    if to.1 {
        pairs[to.0].y = a;
    } else {
        pairs[to.0].x = a;
    }
    store(pane, spec, pairs)
}

impl Workbench {
    pub fn swap_trajectory_pair(self, pane: Pane, index: usize) {
        let Some(pair) = pane
            .trajectory
            .get_untracked()
            .and_then(|spec| spec.pairs().get(index).cloned())
        else {
            return;
        };
        let ids = pane.lines.with_untracked(|lines| {
            Some((
                lines.iter().find(|line| line.source == pair.y)?.id,
                lines.iter().find(|line| line.source == pair.x)?.id,
            ))
        });
        if let Err(error) = swap_slots(pane, (index, false), (index, true)) {
            self.status.set(error);
            return;
        }
        // A trace uses its Y field's ID. Keep its appearance when the Y field changes.
        if let Some((old, new)) = ids
            && old != new
        {
            let style = pane.plot.style(&old.to_string());
            let draw = pane
                .drawing
                .with_untracked(|drawing| drawing.get(&old).copied().unwrap_or_default());
            pane.plot.styles.update(|styles| {
                styles.insert(new.to_string(), style);
            });
            pane.drawing.update(|drawing| {
                drawing.insert(new, draw);
            });
            pane.plot.hidden.update(|hidden| {
                let was_hidden = hidden.remove(&old.to_string());
                hidden.remove(&new.to_string());
                if was_hidden {
                    hidden.insert(new.to_string());
                }
            });
            pane.legend_order.update(|order| {
                for id in order {
                    if *id == old {
                        *id = new;
                    }
                }
            });
            if pane.selected.get_untracked() == Some(old) {
                pane.selected.set(Some(new));
            }
        }
    }
}

#[component]
pub fn SwapAxes(
    workbench: Workbench,
    pane: Pane,
    index: Signal<usize, LocalStorage>,
) -> impl IntoView {
    view! {ActionButton(icon=true,class="trajectory-swap-axes",a11y:label="Swap X and Y for this pair",on:click=move |_|workbench.swap_trajectory_pair(pane,index.get_untracked())){
        vector(class="trajectory-swap-icon",prop:viewBox="0 0 16 16",prop:d="M2 5 H13 M10 2 L13 5 L10 8 M14 11 H3 M6 8 L3 11 L6 14",a11y:hidden=true)
    }}
}
impl Workbench {
    pub fn trajectory_slot_at(self, pane: Pane, x: f32, y: f32) -> Option<(usize, bool)> {
        if !pane.trajectory_setup.get_untracked() {
            return None;
        }
        pane.trajectory_slots
            .with_value(|slots| {
                slots
                    .iter()
                    .find_map(|(key, node)| Self::inside(*node, x, y).then_some(*key))
            })
            .or_else(|| {
                let builder = self.trajectory_builder();
                if pane.id != 0 || !Self::inside(builder.area, x, y) {
                    return None;
                }
                let bounds = builder.area.window_bounds()?;
                let axis = x * builder.area.scale() >= bounds.origin.x.0 + bounds.size.width.0 / 2.;
                let pairs = pane.trajectory.get_untracked()?.pairs();
                let index = pairs
                    .iter()
                    .position(|pair| {
                        if axis {
                            pair.y.is_empty()
                        } else {
                            pair.x.is_empty()
                        }
                    })
                    .unwrap_or(pairs.len());
                Some((index, axis))
            })
    }
    pub fn update_trajectory_drop(self, x: f32, y: f32) {
        let builder = self.trajectory_builder();
        builder.pane.trajectory_drop.set(self.builder_slot_at(x, y));
        let target = self.hit_any_plot(x, y);
        for pane in self.panes().get_untracked() {
            let slot = if target == Some(pane.id) {
                self.trajectory_slot_at(pane, x, y)
            } else {
                None
            };
            if pane.trajectory_drop.get_untracked() != slot {
                pane.trajectory_drop.set(slot);
            }
        }
    }
}
#[component]
fn AxisSlot(workbench: Workbench, pane: Pane, index: usize, y: bool) -> impl IntoView {
    let node = NodeRef::new();
    pane.trajectory_slots.update_value(|slots| {
        slots.insert((index, y), node);
    });
    on_cleanup_local(move || {
        pane.trajectory_slots.update_value(|slots| {
            slots.remove(&(index, y));
        })
    });
    let source = Signal::derive_local(move || {
        pane.trajectory
            .with(|spec| {
                spec.as_ref()?
                    .pairs()
                    .get(index)
                    .map(|pair| if y { pair.y.clone() } else { pair.x.clone() })
            })
            .filter(|source| !source.is_empty())
    });
    let actions = SeriesSourceActions {
        on_move: UnsyncCallback::new(move |(x, y)| workbench.field_drag_move(x, y)),
        on_drop: UnsyncCallback::new(move |(source, x, py)| {
            if ((pane.id == 0 && workbench.trajectory_builder().open.get_untracked())
                || workbench.hit_any_plot(x, py) == Some(pane.id))
                && let Some(to) = workbench.trajectory_slot_at(pane, x, py)
            {
                if let Err(error) = swap_slots(pane, (index, y), to) {
                    workbench.status.set(error);
                }
                workbench.finish_field_drag();
            } else {
                workbench.drop_fields(source, x, py);
            }
        }),
        on_cancel: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
        on_finish: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
    };
    view! {column(class="trajectory-axis-slot",class:axis-slot-filled=move ||source.get().is_some(),class:axis-drop-active=move ||pane.trajectory_drop.get()==Some((index,y)),node_ref=node){
        if move ||source.get().is_some(){
            row(class="trajectory-input-chip"){SeriesSource(id=source,label=Signal::derive_local(move ||source.get().map_or_else(String::new,|source|pane.lines.with(|lines|lines.iter().find(|line|line.source==source).map_or_else(||workbench.field_name(&source),|line|line.name.clone())))),count=Signal::stored_local(String::new()),selected=Signal::stored_local(false),drag=workbench.drag,actions=actions)
                ActionButton(icon=true,class="trajectory-remove-input",a11y:label=if y{"Remove Y input"}else{"Remove X input"},on:click=move |_|{if let Err(error)=assign(pane,index,y,&[String::new()]){workbench.status.set(error);}}){
                    vector(class="trajectory-control-icon",prop:viewBox="0 0 16 16",prop:d="M4 4 L12 12 M12 4 L4 12",a11y:hidden=true)
                }
            }
        }else{text(class="trajectory-drop-label"){{if y{"Drop Y field"}else{"Drop X field"}}}}
    }}
}
#[component]
pub fn TrajectoryWorkspace(workbench: Workbench) -> impl IntoView {
    view! {for current in move ||vec![workbench.view()],key=|view|view.id{
        TrajectorySetup(workbench=workbench.for_view(current.id).expect("View exists"),pane=current.trajectory_builder.pane)
    }}
}
#[component]
fn PairDrag(workbench: Workbench, pane: Pane, index: usize) -> impl IntoView {
    let actions = SeriesSourceActions {
        on_move: UnsyncCallback::new(move |(x, y)| workbench.field_drag_move(x, y)),
        on_drop: UnsyncCallback::new(move |(source, x, y)| workbench.drop_fields(source, x, y)),
        on_cancel: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
        on_finish: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
    };
    view! {column(class="trajectory-pair-drag",class:pair-dragging=move ||workbench.drag.state.get().is_some_and(|drag|drag.0==format!("@trajectory/pair/{index}")),a11y:label="Drag X/Y pair onto a plot"){
        SeriesSource(id=Signal::derive_local(move ||pane.trajectory.with(|spec|spec.as_ref()?.pairs().get(index).filter(|pair|!pair.x.is_empty() && !pair.y.is_empty()).map(|_|format!("@trajectory/pair/{index}")))),label=Signal::stored_local("Drag X/Y pair onto a plot".to_owned()),count=Signal::stored_local(String::new()),selected=Signal::stored_local(false),drag=workbench.drag,actions=actions)
        vector(class="trajectory-grip-icon",prop:viewBox="0 0 16 20",prop:d="M5 4 L5 6 M11 4 L11 6 M5 9 L5 11 M11 9 L11 11 M5 14 L5 16 M11 14 L11 16",a11y:hidden=true)
        text(class="trajectory-pair-drag-label",a11y:hidden=true) {{move ||if workbench.drag.state.get().is_some_and(|drag|drag.0==format!("@trajectory/pair/{index}")){"Drop on a plot"}else{"Drag pair to a plot"}}}
        vector(class="trajectory-pair-arrow",prop:viewBox="0 0 16 16",prop:d="M13 8 H3 M7 4 L3 8 L7 12",a11y:hidden=true)
    }}
}
fn row_indices(pane: Pane) -> Vec<usize> {
    pane.trajectory.with(|spec| {
        let pairs = spec.as_ref().map(|spec| spec.pairs()).unwrap_or_default();
        let count = pairs.len()
            + usize::from(
                pairs
                    .last()
                    .is_some_and(|pair| !pair.x.is_empty() || !pair.y.is_empty())
                    && pairs.len() < 256,
            );
        (0..count).collect()
    })
}
#[component]
pub fn TrajectorySetup(workbench: Workbench, pane: Pane) -> impl IntoView {
    let builder = workbench.trajectory_builder();
    view! {column(class="trajectory-setup-page"){
        row(class="trajectory-setup-heading"){
            column(class="trajectory-heading-copy"){text(class="heading"){"2D plots"}text(class="trajectory-setup-caption"){"Pair fields, then drag to a plot"}}
            ActionButton(icon=true,class="trajectory-close-builder",a11y:label="Close 2D setup",on:click=move |_|builder.open.set(false)){
                vector(class="trajectory-control-icon",prop:viewBox="0 0 16 16",prop:d="M4 4 L12 12 M12 4 L4 12",a11y:hidden=true)
            }
        }
        row(class="trajectory-axis-headings"){text{"X axis"}text{"Y axis"}}
        scroll(class="trajectory-setup-pairs",node_ref=builder.area){
            for index in move ||row_indices(pane),key=|index|*index{
                column(class="trajectory-axis-pair"){
                row(class="trajectory-pair-inputs"){
                    AxisSlot(workbench=workbench,pane=pane,index=index,y=false)
                    AxisSlot(workbench=workbench,pane=pane,index=index,y=true)
                    box(class="trajectory-column-divider"){}
                    if move ||pane.trajectory.with(|spec|spec.as_ref().and_then(|spec|spec.pairs().get(index).map(|pair|!pair.x.is_empty() || !pair.y.is_empty())).unwrap_or(false)){
                        column(class="trajectory-pair-actions"){
                            SwapAxes(workbench=workbench,pane=pane,index=Signal::stored_local(index))
                        }
                    }
                }
                if move ||pane.trajectory.with(|spec|spec.as_ref().and_then(|spec|spec.pairs().get(index).map(|pair|!pair.x.is_empty() && !pair.y.is_empty())).unwrap_or(false)){
                    PairDrag(workbench=workbench,pane=pane,index=index)
                }
                }
            }
        }
        text(class="trajectory-setup-hint"){"Drop on a 2D plot to add a trace, or a time plot to create one."}
    }}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_drops_choose_distinct_colors_without_changing_other_appearance() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let original = workbench.pane();
        let data = plot::time_series::SeriesData::new(vec![
            plot::PlotPoint { x: 0., y: 0. },
            plot::PlotPoint { x: 1., y: 1. },
        ])
        .unwrap();
        let lines = ["x1", "y1", "x2", "y2", "x3", "y3"]
            .into_iter()
            .map(|source| {
                let id = workbench.id();
                let mut style = plot::plot_data::SeriesStyle::for_id(&id.to_string());
                style.color = crate::line_colors::PALETTE[0];
                style.width = 7.;
                style.radius = 4.;
                original.plot.styles.update(|styles| {
                    styles.insert(id.to_string(), style);
                });
                crate::state::Line {
                    id,
                    source: source.into(),
                    name: source.into(),
                    revision: 0,
                    data: data.clone(),
                }
            })
            .collect::<Vec<_>>();
        original.lines.set(lines);
        workbench.new_trajectory(false);
        let draft = workbench.trajectory_builder().pane;
        assign(draft, 0, false, &["x1".into(), "x2".into(), "x3".into()]).unwrap();
        assign(draft, 0, true, &["y1".into(), "y2".into(), "y3".into()]).unwrap();
        window
            .scope
            .with(|| workbench.drop_trajectory_pair(0, original.id));
        let xy = workbench.pane();
        for index in 1..3 {
            workbench.drop_trajectory_pair(index, xy.id);
        }
        let colors = xy
            .view_lines
            .get_untracked()
            .iter()
            .map(|line| {
                let style = xy.plot.style(&line.id.to_string());
                assert_eq!(style.width, 7.);
                assert_eq!(style.radius, 4.);
                style.color
            })
            .collect::<Vec<_>>();
        assert_eq!(colors, &crate::line_colors::PALETTE[..3]);
        // An existing X input becoming a Y trace must also receive a free color.
        assign(draft, 3, false, &["x3".into()]).unwrap();
        assign(draft, 3, true, &["x1".into()]).unwrap();
        workbench.drop_trajectory_pair(3, xy.id);
        let x = xy
            .lines
            .with_untracked(|lines| lines.iter().find(|line| line.source == "x1").unwrap().id);
        assert_eq!(
            xy.plot.style(&x.to_string()).color,
            crate::line_colors::PALETTE[3]
        );
        let y = xy
            .lines
            .with_untracked(|lines| lines.iter().find(|line| line.source == "y1").unwrap().id);
        xy.plot.styles.update(|styles| {
            styles.get_mut(&y.to_string()).unwrap().color = [10, 20, 30, 255];
        });
        workbench.drop_trajectory_pair(0, xy.id);
        assert_eq!(
            xy.plot.style(&y.to_string()).color,
            [10, 20, 30, 255],
            "Repeated drops preserve custom colors"
        );
    }

    #[test]
    fn swapping_one_pair_preserves_its_appearance_and_other_pairs() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let pane = workbench.pane();
        pane.trajectory.set(Some(Spec {
            x: "x".into(),
            y: "y".into(),
            extra: vec![Pair {
                x: "x".into(),
                y: "z".into(),
                gps_scale: None,
                fix: String::new(),
            }],
            ..Default::default()
        }));
        let data = plot::time_series::SeriesData::new(vec![
            plot::PlotPoint { x: 0., y: 1. },
            plot::PlotPoint { x: 1., y: 2. },
        ])
        .unwrap();
        let ids = [workbench.id(), workbench.id(), workbench.id()];
        pane.lines.set(
            ids.into_iter()
                .zip(["x", "y", "z"])
                .map(|(id, source)| crate::state::Line {
                    id,
                    source: source.into(),
                    name: source.into(),
                    data: data.clone(),
                    revision: 0,
                })
                .collect(),
        );
        let mut style = pane.plot.style(&ids[1].to_string());
        style.width = 7.;
        style.color = [15, 65, 200, 255];
        pane.plot.styles.update(|styles| {
            styles.insert(ids[1].to_string(), style);
        });
        pane.plot.hidden.update(|hidden| {
            hidden.insert(ids[1].to_string());
        });
        pane.legend_order.set(vec![ids[2], ids[1]]);
        pane.selected.set(Some(ids[1]));
        let before = pane.trajectory.get_untracked().unwrap();
        workbench.swap_trajectory_pair(pane, 0);
        let swapped = pane.trajectory.get_untracked().unwrap();
        assert_eq!((&*swapped.x, &*swapped.y), ("y", "x"));
        assert_eq!(swapped.extra, before.extra);
        assert_eq!(pane.plot.style(&ids[0].to_string()), style);
        assert!(
            pane.plot
                .hidden
                .get_untracked()
                .contains(&ids[0].to_string())
        );
        assert_eq!(pane.legend_order.get_untracked(), vec![ids[2], ids[0]]);
        assert_eq!(pane.selected.get_untracked(), Some(ids[0]));
        workbench.swap_trajectory_pair(pane, 0);
        assert_eq!(pane.trajectory.get_untracked(), Some(before));
        assert!(
            pane.plot
                .hidden
                .get_untracked()
                .contains(&ids[1].to_string())
        );
    }

    #[test]
    fn dropping_one_pair_ignores_unfinished_rows_and_appends_to_existing_xy_plots() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let original = workbench.pane();
        let data = plot::time_series::SeriesData::new(vec![
            plot::PlotPoint { x: 0., y: 0. },
            plot::PlotPoint { x: 1., y: 1. },
        ])
        .unwrap();
        original.lines.set(
            ["x1", "y1", "x2", "y2"]
                .into_iter()
                .map(|source| crate::state::Line {
                    id: workbench.id(),
                    source: source.into(),
                    name: source.into(),
                    data: data.clone(),
                    revision: 0,
                })
                .collect(),
        );
        workbench.new_trajectory(false);
        let draft = workbench.trajectory_builder().pane;
        assign(draft, 0, false, &["x1".into(), "x2".into()]).unwrap();
        assign(draft, 0, true, &["y1".into()]).unwrap();
        window
            .scope
            .with(|| workbench.drop_trajectory_pair(0, original.id));
        let xy = workbench.pane();
        assert_ne!(xy.id, original.id);
        assert_eq!(xy.trajectory.get_untracked().unwrap().pairs().len(), 1);
        assert_eq!(original.lines.with_untracked(Vec::len), 4);
        assert!(xy.links.get_untracked().pointer && xy.timeline_link.get_untracked());
        workbench.drop_trajectory_pair(1, xy.id);
        assert_eq!(xy.trajectory.get_untracked().unwrap().pairs().len(), 1);
        xy.trajectory
            .update(|spec| spec.as_mut().unwrap().equal = false);
        workbench.profiles.titles.update(|titles| {
            titles.insert(xy.id, "Flight path".into());
        });
        assign(draft, 1, true, &["y2".into()]).unwrap();
        workbench.drop_trajectory_pair(1, xy.id);
        assert_eq!(workbench.panes().with_untracked(Vec::len), 2);
        let spec = xy.trajectory.get_untracked().unwrap();
        assert_eq!((&*spec.extra[0].x, &*spec.extra[0].y), ("x2", "y2"));
        assert!(!spec.equal);
        assert_eq!(
            workbench.profiles.titles.get_untracked()[&xy.id],
            "Flight path"
        );
        workbench.drop_trajectory_pair(1, xy.id);
        assert_eq!(xy.trajectory.get_untracked().unwrap().pairs().len(), 2);
    }

    #[test]
    fn trajectory_slots_accept_y_first_and_exchange_coordinates() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        workbench.new_trajectory(false);
        let pane = workbench.trajectory_builder().pane;
        assert!(pane.lines.with_untracked(Vec::is_empty));
        assign(pane, 0, true, &["north".into(), "target".into()]).unwrap();
        assert!(pane.trajectory.get_untracked().unwrap().x.is_empty());
        assign(pane, 0, false, &["east".into()]).unwrap();
        assert!(
            pane.trajectory.get_untracked().unwrap().extra[0]
                .x
                .is_empty()
        );
        assign(pane, 1, false, &["other_east".into()]).unwrap();
        assign(pane, 0, false, &["changed_east".into()]).unwrap();
        assert_eq!(
            pane.trajectory.get_untracked().unwrap().extra[0].x,
            "other_east"
        );
        swap_slots(pane, (0, false), (0, true)).unwrap();
        let spec = pane.trajectory.get_untracked().unwrap();
        assert_eq!((&*spec.x, &*spec.y), ("north", "changed_east"));
        assert_eq!(spec.extra[0].x, "other_east");
        let before = spec.clone();
        assert!(assign(pane, 256, true, &["overflow".into()]).is_err());
        assert_eq!(pane.trajectory.get_untracked().unwrap(), before);
    }

    #[test]
    fn trajectory_legend_transfers_keep_scalar_data_and_appearance() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let original = workbench.pane();
        let line = crate::state::Line {
            id: workbench.id(),
            source: "north".into(),
            name: "North".into(),
            data: plot::time_series::SeriesData::new(vec![plot::PlotPoint { x: 1., y: 7. }])
                .unwrap(),
            revision: 0,
        };
        let id = line.id;
        let mut style = original.plot.style(&id.to_string());
        style.width = 7.;
        style.color = [20, 50, 80, 255];
        original.plot.styles.update(|styles| {
            styles.insert(id.to_string(), style);
        });
        original.lines.set(vec![line]);
        let xy = window
            .scope
            .with(|| workbench.add_plot(original.id, false, false));
        xy.trajectory.set(Some(Spec::default()));
        xy.trajectory_drop.set(Some((0, true)));
        assert!(workbench.can_place_line(original.id, id, xy.id));
        workbench.place_line(original.id, id, xy.id, None);
        assert!(original.lines.with_untracked(Vec::is_empty));
        assert_eq!(xy.trajectory.get_untracked().unwrap().y, "north");
        assert_eq!(xy.plot.style(&id.to_string()), style);
        assert_eq!(xy.lines.get_untracked()[0].data.samples()[0].x, 1.);
        workbench.place_line(xy.id, id, original.id, None);
        assert!(xy.view_lines.with_untracked(Vec::is_empty));
        assert_eq!(original.lines.get_untracked()[0].data.samples()[0].y, 7.);
        assert_eq!(original.plot.style(&id.to_string()), style);
    }
}
