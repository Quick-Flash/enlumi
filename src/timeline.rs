use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
};
use plot::{PlotGeometry, zgui_renderer::push_stroke};
use zgui::{canvas::zgui_color::Color, prelude::*};
use zgui_ui::prelude::*;

#[derive(Clone, Copy)]
pub struct Timeline {
    pub time: RwSignal<Option<f64>, LocalStorage>,
    pub follow: RwSignal<bool, LocalStorage>,
    pub height: RwSignal<f32, LocalStorage>,
}
impl Timeline {
    pub fn new() -> Self {
        Self {
            time: RwSignal::new_local(Some(0.)),
            follow: RwSignal::new_local(false),
            height: RwSignal::new_local(34.),
        }
    }
    pub fn hover(self, time: f64) {
        if self.follow.get_untracked()
            && time.is_finite()
            && time >= 0.
            && self.time.get_untracked() != Some(time)
        {
            self.time.set(Some(time));
        }
    }
    pub fn pin(self, time: f64) -> Result<(), String> {
        if !time.is_finite() || time < 0. {
            return Err("Timeline time must be finite and nonnegative".into());
        }
        if self.follow.get_untracked() {
            self.follow.set(false);
        }
        if self.time.get_untracked() != Some(time) {
            self.time.set(Some(time));
        }
        Ok(())
    }
}
impl Workbench {
    pub fn step_timeline(self, forward: bool) {
        let time = self.timeline.time.get_untracked().unwrap_or(0.);
        let mut next = None::<f64>;
        let mut have_samples = false;
        self.pane().lines.with_untracked(|lines| {
            for line in lines {
                let samples = line.data.samples();
                have_samples |= !samples.is_empty();
                let candidate = if forward {
                    samples.get(samples.partition_point(|sample| sample.x <= time))
                } else {
                    samples
                        .partition_point(|sample| sample.x < time)
                        .checked_sub(1)
                        .and_then(|index| samples.get(index))
                };
                if let Some(candidate) = candidate
                    && candidate.x.is_finite()
                    && candidate.x >= 0.
                {
                    next = Some(next.map_or(candidate.x, |old| {
                        if forward {
                            old.min(candidate.x)
                        } else {
                            old.max(candidate.x)
                        }
                    }));
                }
            }
        });
        let _ = self.timeline.pin(next.unwrap_or_else(|| {
            if have_samples {
                time
            } else {
                (time + if forward { 0.1 } else { -0.1 }).max(0.)
            }
        }));
    }
    pub fn timeline_shortcut(
        self,
        event: &zgui::vocab::KeyEvent,
        modifiers: zgui::vocab::Modifiers,
    ) -> bool {
        if modifiers.control()
            || modifiers.alt()
            || modifiers.meta()
            || zgui::view::focus_is_text_entry()
        {
            return false;
        }
        match &event.key {
            zgui::vocab::Key::Character(key) if key.as_str() == "[" => self.step_timeline(false),
            zgui::vocab::Key::Character(key) if key.as_str() == "]" => self.step_timeline(true),
            _ => return false,
        };
        true
    }
}

fn pointer_time(
    host: NodeRef,
    pane: Pane,
    position: zgui::geom::Point<zgui::geom::CssPx, zgui::geom::Css>,
) -> Option<f64> {
    let (x, _) = plot::zgui_renderer::local_position(&host, position)?;
    let bounds = host.window_bounds()?;
    let width = f64::from(bounds.size.width.0 / host.scale());
    if width <= 0. {
        return None;
    }
    let range = pane.plot.ranges.get_untracked().x;
    Some((range.min + x.clamp(0., width) / width * range.span()).max(0.))
}

#[component]
pub fn TimeSurface(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
    spectral: bool,
    children: Children,
) -> impl IntoView {
    let host = NodeRef::new();
    let dragging = RwSignal::new_local(false);
    let previous = StoredValue::new_local((None, false));
    view! {column(class="time-cursor-host",node_ref=host,
    on:pointer_down:capture=move |event| {if spectral || !pane.timeline_link.get_untracked() || !visible.get_untracked() || event.button!=Some(zgui::vocab::PointerButton::Primary) || !event.modifiers.alt(){return;}
        previous.set_value((workbench.timeline.time.get_untracked(),workbench.timeline.follow.get_untracked()));dragging.set(true);
        if let Some(time)=pointer_time(host,pane,event.position){let _=workbench.timeline.pin(time);}event.capture_pointer();event.prevent_default();event.stop_propagation();
    },
    on:pointer_move:capture=move |event| {if dragging.get_untracked(){if let Some(time)=pointer_time(host,pane,event.position){let _=workbench.timeline.pin(time);}event.prevent_default();event.stop_propagation();}},
    on:pointer_up:capture=move |event| {if dragging.get_untracked(){dragging.set(false);event.release_pointer();event.prevent_default();event.stop_propagation();}},
    on:pointer_cancel=move |event| {if dragging.get_untracked(){dragging.set(false);let (time,follow)=previous.get_value();workbench.timeline.time.set(time);workbench.timeline.follow.set(follow);event.release_pointer();}},
    on:pointer_move=move |event| {if pane.timeline_link.get_untracked() && !spectral && visible.get_untracked() && workbench.timeline.follow.get_untracked() && let Some(time)=pointer_time(host,pane,event.position){workbench.timeline.hover(time);}}) {{children.into_view_once()}}}
}

#[component]
pub fn TimelineStrip(workbench: Workbench) -> impl IntoView {
    install_stylesheet("enlumi-trajectory", include_str!("trajectory.css"));
    let input = RwSignal::new_local(String::new());
    let duration = Signal::derive_local(move || {
        workbench
            .log
            .with(|log| log.as_ref().map(|log| log.duration))
            .unwrap_or_else(|| {
                workbench.panes().with(|panes| {
                    panes
                        .iter()
                        .flat_map(|pane| pane.lines.get())
                        .filter_map(|line| line.data.samples().last().map(|point| point.x))
                        .fold(1., f64::max)
                })
            })
            .max(0.001)
    });
    let resize = StoredValue::new_local(None::<(f32, f32)>);
    let apply = UnsyncCallback::new(move |()| {
        let result = input
            .get_untracked()
            .trim()
            .parse::<f64>()
            .map_err(|_| "Enter a time in seconds".to_owned())
            .and_then(|time| workbench.timeline.pin(time));
        if let Err(error) = result {
            workbench.status.set(error);
        }
    });
    view! {column(class="timeline-strip",style:height=move ||Some(format!("{}px",workbench.timeline.height.get())),a11y:role=Role::Region,a11y:label="Shared timeline"){
        control(class="timeline-resize-handle",a11y:role=Role::Splitter,a11y:label="Resize timeline height",tabindex=Focus::Sequential,
            on:pointer_down=move |event|{if event.button==Some(zgui::vocab::PointerButton::Primary){resize.set_value(Some((event.position.y.0,workbench.timeline.height.get_untracked())));event.capture_pointer();event.prevent_default();event.stop_propagation();}},
            on:pointer_move=move |event|{if let Some((y,height))=resize.get_value(){workbench.timeline.height.set((height+y-event.position.y.0).clamp(30.,96.));event.prevent_default();event.stop_propagation();}},
            on:pointer_up=move |event|{if resize.get_value().is_some(){resize.set_value(None);event.release_pointer();event.stop_propagation();}},
            on:pointer_cancel=move |event|{if let Some((_,height))=resize.get_value(){workbench.timeline.height.set(height);resize.set_value(None);event.release_pointer();}},
            on:key_down=move |event|{let delta=match event.key{zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowUp)=>4.,zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowDown)=>-4.,_=>return};workbench.timeline.height.update(|height|*height=(*height+delta).clamp(30.,96.));event.prevent_default();event.stop_propagation();}){}
        row(class="timeline-fields"){
            text(class="timeline-label"){"Time"}
            ActionButton(class="timeline-previous",a11y:label="Previous sample",on:click=move |_|workbench.step_timeline(false)){"<"}
            Slider(class="timeline-scrubber",min=0.,max=1.,step=0.000001,label="Timeline time",value=Binding::controlled(Signal::derive_local(move ||(workbench.timeline.time.get().unwrap_or(0.)/duration.get()).clamp(0.,1.)),move |fraction|{let _=workbench.timeline.pin(fraction*duration.get_untracked());}))
            ActionButton(class="timeline-next",a11y:label="Next sample",on:click=move |_|workbench.step_timeline(true)){">"}
            text(class="timeline-readout"){{move ||workbench.timeline.time.get().map_or_else(||"-- s".to_owned(),|time|format!("{time:.6} s"))}}
            Input(class="timeline-time-input",value=input,placeholder="Seconds",a11y:label="Set timeline time in seconds",on:key_down=move |event|if event.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){apply.run(());event.prevent_default();event.stop_propagation();})
            ActionButton(class="timeline-set-time",on:click=move |_|apply.run(())){"Set"}
            ActionButton(class="timeline-follow-pointer",class:setting-active=move ||workbench.timeline.follow.get(),on:click=move |_|workbench.timeline.follow.update(|follow|*follow= !*follow)){"Follow pointer"}
        }
    }}
}

#[component]
pub fn TimeMarker(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    install_stylesheet("enlumi-trajectory", include_str!("trajectory.css"));
    let host = NodeRef::new();
    let size = host.observe_border_box();
    let dragging = RwSignal::new_local(false);
    let previous = StoredValue::new_local((None, false));
    let handle = zgui::elements::canvas::CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Foreground);
    // Keep an offscreen marker's empty canvas unchanged during pan/zoom. Only
    // projected marker geometry, rather than every bounds update, owes a redraw.
    let marker = Memo::new(move |_| {
        if !visible.get() || !pane.timeline_link.get() {
            return None;
        }
        let time = workbench.timeline.time.get()?;
        let size = size.get()?;
        let (w, h) = (
            f64::from(size.size.width.0 / host.scale()),
            f64::from(size.size.height.0 / host.scale()),
        );
        let g = PlotGeometry::with_bounds(0., 0., w, h, w, h)?;
        let ranges = pane.plot.ranges.get();
        ranges
            .x
            .contains(time)
            .then(|| (g.x(time, ranges.x), g.top, g.bottom))
    });
    let drawing = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        let marker = marker.get();
        drawing.draw(|scene| {
            scene.clear();
            let Some((x, top, bottom)) = marker else {
                return;
            };
            let mut path = kurbo::BezPath::new();
            path.move_to((x, top));
            path.line_to((x, bottom));
            push_stroke(scene, path, Color::srgb_u8(255, 190, 60, 255), 1.5);
        })
    });
    let canvas = zgui::elements::canvas()
        .class("time-marker-canvas")
        .scene(&handle);
    view! {column(class="time-marker",node_ref=host) {
        {canvas.into_view()}
        if move ||pane.timeline_link.get() && visible.get() && !workbench.timeline.follow.get() && !workbench.geometry.editing.get() && !workbench.measurement.active.get() && !workbench.analysis().picking_range.get() && workbench.timeline.time.get().is_some_and(|time|pane.plot.ranges.get().x.contains(time)) {
            control(class="timeline-drag-handle",tabindex=Focus::Sequential,a11y:label="Global timeline marker: drag or use Left and Right to step samples",
                style:left=move || {let bounds=size.get()?;let range=pane.plot.ranges.get().x;let time=workbench.timeline.time.get()?;Some(format!("{}px",(time-range.min)/range.span()*f64::from(bounds.size.width.0/host.scale())-5.))},
                on:pointer_down=move |event| {if event.button!=Some(zgui::vocab::PointerButton::Primary){return;}previous.set_value((workbench.timeline.time.get_untracked(),workbench.timeline.follow.get_untracked()));dragging.set(true);workbench.timeline.follow.set(false);event.capture_pointer();event.prevent_default();event.stop_propagation();},
                on:pointer_move=move |event| {if dragging.get_untracked(){if let Some(time)=pointer_time(host,pane,event.position){let _=workbench.timeline.pin(time);}event.prevent_default();event.stop_propagation();}},
                on:pointer_up=move |event| {if dragging.get_untracked(){dragging.set(false);event.release_pointer();event.prevent_default();event.stop_propagation();}},
                on:pointer_cancel=move |event| {if dragging.get_untracked(){dragging.set(false);let (time,follow)=previous.get_value();workbench.timeline.time.set(time);workbench.timeline.follow.set(follow);event.release_pointer();}},
                on:key_down=move |event| {match event.key{zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowLeft)=>workbench.step_timeline(false),zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight)=>workbench.step_timeline(true),_=>return};event.prevent_default();event.stop_propagation();}) {}
        }
    }}
}
