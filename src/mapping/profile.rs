//! Altitude against distance flown, under 2D maps. Clicking pins the timeline.
use super::camera::geographic;
use crate::state::{Pane, Workbench};
use std::collections::BTreeMap;
use zgui::prelude::*;

const MAX_POINTS: usize = 800;
const LEFT: f64 = 46.;
const RIGHT: f64 = 10.;
const TOP: f64 = 18.;
const BOTTOM: f64 = 16.;

#[derive(Clone, PartialEq, Default)]
struct Profile {
    /// (distance m, altitude m, time s), finite samples in flight order.
    samples: Vec<[f64; 3]>,
    total: f64,
    low: f64,
    high: f64,
}

fn build(pane: Pane) -> Option<Profile> {
    let spec = pane.trajectory.get()?;
    let data = pane.trajectory_data.get()?;
    let lines = pane.lines.get();
    let hidden = pane.plot.hidden.get();
    let pairs = spec.pairs();
    let altitudes = pairs
        .iter()
        .enumerate()
        .filter_map(|(i, pair)| spec.altitudes.get(&i).map(|a| (pair.y.clone(), a.clone())))
        .collect::<BTreeMap<_, _>>();
    let trace = data
        .traces
        .iter()
        .find(|trace| !hidden.contains(&trace.id.to_string()))?;
    let source = lines
        .iter()
        .find(|line| line.id == trace.id)?
        .source
        .clone();
    if altitudes.get(&source).is_none_or(|a| a.source.is_empty()) {
        return None;
    }
    let paths =
        super::flight3::prepare(&data, &lines, &altitudes, &hidden, &BTreeMap::new()).ok()?;
    let path = paths.iter().find(|path| path.id == trace.id)?;
    let mut samples = Vec::new();
    let mut total = 0.;
    let mut previous = None::<[f64; 2]>;
    for (point, &time) in path.points.iter().zip(path.times.iter()) {
        if !point.iter().all(|v| v.is_finite()) {
            previous = None;
            continue;
        }
        let here = geographic([point[0], point[1]]);
        if let Some(before) = previous {
            let lat = ((here[1] + before[1]) / 2.).to_radians();
            let east = (here[0] - before[0]).to_radians() * lat.cos();
            let north = (here[1] - before[1]).to_radians();
            total += east.hypot(north) * 6_371_008.8;
        }
        previous = Some(here);
        samples.push([total, point[2], time]);
    }
    if samples.len() < 2 {
        return None;
    }
    let (low, high) = samples
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), s| {
            (lo.min(s[1]), hi.max(s[1]))
        });
    let pad = ((high - low) * 0.1).max(1.);
    let stride = samples.len().div_ceil(MAX_POINTS).max(1);
    let mut reduced: Vec<_> = samples.iter().step_by(stride).copied().collect();
    if reduced.last() != samples.last() {
        reduced.push(*samples.last()?);
    }
    Some(Profile {
        samples: reduced,
        total,
        low: low - pad,
        high: high + pad,
    })
}

#[component]
pub fn AltitudeProfile(workbench: Workbench, pane: Pane) -> impl IntoView {
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let size = Memo::new(move |_| {
        bounds.get().map_or([0., 0.], |b| {
            [
                f64::from(b.size.width.0 / host.scale()),
                f64::from(b.size.height.0 / host.scale()),
            ]
        })
    });
    let profile = Memo::new(move |_| build(pane));
    let hover = RwSignal::new_local(None::<usize>);
    let x = move |p: &Profile, d: f64| {
        LEFT + d / p.total.max(1e-9) * (size.get()[0] - LEFT - RIGHT).max(1.)
    };
    let y = move |p: &Profile, z: f64| {
        TOP + (1. - (z - p.low) / (p.high - p.low)) * (size.get()[1] - TOP - BOTTOM).max(1.)
    };
    let window = Memo::new(move |_| super::flight::highlight_window(workbench, pane));
    let path_in = move |range: Option<[f64; 2]>| {
        profile.with(|p| {
            p.as_ref().map_or_else(String::new, |p| {
                p.samples
                    .iter()
                    .filter(|s| range.is_none_or(|[a, b]| s[2] >= a && s[2] <= b))
                    .enumerate()
                    .map(|(i, s)| {
                        format!(
                            "{} {:.1} {:.1}",
                            if i == 0 { "M" } else { "L" },
                            x(p, s[0]),
                            y(p, s[1])
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
        })
    };
    let path = move || path_in(None);
    let index_at = move |px: f64| {
        profile.with(|p| {
            let p = p.as_ref()?;
            let d = (px - LEFT) / (size.get_untracked()[0] - LEFT - RIGHT).max(1.) * p.total;
            let i = p.samples.partition_point(|s| s[0] < d);
            Some(i.min(p.samples.len() - 1))
        })
    };
    let at = move |index: Option<usize>| {
        profile.with(|p| {
            let p = p.as_ref()?;
            let s = p.samples.get(index?)?;
            Some((x(p, s[0]), y(p, s[1]), s[1], s[2]))
        })
    };
    let marker = Memo::new(move |_| {
        let time = workbench.timeline.time.get()?;
        let index = profile.with(|p| {
            let p = p.as_ref()?;
            Some(
                p.samples
                    .partition_point(|s| s[2] < time)
                    .min(p.samples.len() - 1),
            )
        })?;
        at(Some(index))
    });
    let local = move |position: zgui::geom::Point<zgui::geom::CssPx, zgui::geom::Css>| {
        plot::zgui_renderer::local_position(&host, position).map(|(x, _)| x)
    };
    view! {column(class="map-profile",node_ref=host,
        on:pointer_move=move |e|hover.set(local(e.position).and_then(index_at)),
        on:pointer_leave=move |_|hover.set(None),
        on:pointer_down=move |e|{if let Some((_, _, _, time))=at(local(e.position).and_then(index_at)){let _=workbench.timeline.pin(time);}}) {
        text(class="map-profile-title") {"Altitude profile · bound Z (m) vs distance flown"}
        if move ||profile.with(Option::is_none) {
            text(class="map-profile-empty muted") {"Bind a Z / altitude field to this trace to see its profile."}
        }
        if move ||profile.with(Option::is_some) {
            text(class="map-profile-axis map-profile-high") {{move ||profile.with(|p|p.as_ref().map(|p|format!("{:.0}",p.high)).unwrap_or_default())}}
            text(class="map-profile-axis map-profile-low") {{move ||profile.with(|p|p.as_ref().map(|p|format!("{:.0}",p.low)).unwrap_or_default())}}
            text(class="map-profile-axis map-profile-distance") {{move ||profile.with(|p|p.as_ref().map(|p|if p.total<1000.{format!("{:.0} m",p.total)}else{format!("{:.2} km",p.total/1000.)}).unwrap_or_default())}}
            vector(class="map-profile-line",class:map-profile-faded=move ||window.get().is_some(),prop:viewBox=move ||{let [w,h]=size.get();PropValue::from(format!("0 0 {w} {h}"))},prop:d=move ||PropValue::from(path()),a11y:hidden=true)
            if move ||window.get().is_some() {
                vector(class="map-profile-line",prop:viewBox=move ||{let [w,h]=size.get();PropValue::from(format!("0 0 {w} {h}"))},prop:d=move ||PropValue::from(path_in(window.get())),a11y:hidden=true)
            }
            if move ||marker.get().is_some() {
                box(class="map-profile-cursor",style:left=move ||marker.get().map(|m|format!("{}px",m.0))) {}
                box(class="map-profile-dot",style:left=move ||marker.get().map(|m|format!("{}px",m.0)),style:top=move ||marker.get().map(|m|format!("{}px",m.1))) {}
            }
            if move ||at(hover.get()).is_some() {
                box(class="map-profile-hover",style:left=move ||at(hover.get()).map(|m|format!("{}px",m.0))) {}
                text(class="map-profile-readout") {{move ||at(hover.get()).map(|m|format!("{:.1} m · t {:.1} s",m.2,m.3)).unwrap_or_default()}}
            }
        }
    }}
}
