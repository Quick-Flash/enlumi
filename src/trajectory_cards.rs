//! 2D plot and map setup as one collapsible card per trace.
use crate::{
    action_button::ActionButtonProps,
    mapping::settings::{AltitudeOptionsProps, target, update},
    state::{Pane, Workbench},
    trajectory::Pair,
    trajectory_axes::{
        AxisSlotProps, PairDragProps, SwapAxesProps, local_pair, map_pair_options, row_indices,
    },
};
use std::collections::BTreeSet;
use zgui::prelude::*;
use zgui_ui::prelude::*;

const CHEVRON_OPEN: &str = "M2 4 L6 8 L10 4";
const CHEVRON_CLOSED: &str = "M4 2 L8 6 L4 10";
const DEGREES_E7: f64 = 1e-7;
const RADIANS: f64 = 180. / std::f64::consts::PI;

fn pair(pane: Pane, index: usize) -> Option<Pair> {
    pane.trajectory
        .with(|s| s.as_ref().and_then(|s| s.pairs().get(index).cloned()))
}
fn unit_label(scale: Option<f64>, local: bool) -> &'static str {
    if local {
        return "local m";
    }
    match scale {
        Some(s) if s == DEGREES_E7 => "deg×10⁷",
        Some(s) if s == RADIANS => "radians",
        _ => "degrees",
    }
}

/// Coordinate scale suggested by the field's values: deg×10⁷ when they exceed ±360.
fn detect_scale(workbench: Workbench, source: &str) -> Option<f64> {
    if source.is_empty() {
        return None;
    }
    let data = workbench
        .log
        .with_untracked(|log| log.as_ref()?.prepare(source).ok())?;
    let peak = data
        .samples()
        .iter()
        .map(|p| p.y.abs())
        .filter(|v| v.is_finite())
        .fold(0_f64, f64::max);
    (peak > 0.).then_some(if peak > 360. { DEGREES_E7 } else { 1. })
}

fn summary(workbench: Workbench, pane: Pane, index: usize) -> (String, String) {
    let Some(pair) = pair(pane, index) else {
        return ("New trace".into(), "Drop a field to start".into());
    };
    let name = |source: &str| {
        if source.is_empty() {
            "—".to_owned()
        } else {
            workbench.field_name(source)
        }
    };
    let title = if pair.y.is_empty() && pair.x.is_empty() {
        "New trace".to_owned()
    } else if pair.y.is_empty() {
        name(&pair.x)
    } else {
        name(&pair.y)
    };
    let map = pane
        .trajectory
        .with(|s| s.as_ref().is_some_and(|s| s.map.is_some()));
    if !map {
        return (title, format!("{} vs {}", name(&pair.x), name(&pair.y)));
    }
    let local = pair.local.is_some();
    let mut detail = format!(
        "{} · {}",
        if local { "east/north" } else { "lon/lat" },
        unit_label(pair.gps_scale, local)
    );
    if let Some(origin) = pair.local.and_then(|l| l.origin) {
        detail.push_str(&format!(" · start {:.5}, {:.5}", origin[1], origin[0]));
    } else if local {
        detail.push_str(" · no start set");
    }
    let altitude = pane.trajectory.with(|s| {
        s.as_ref()
            .and_then(|s| s.altitudes.get(&index).cloned())
            .filter(|a| !a.source.is_empty())
    });
    if let Some(altitude) = altitude {
        detail.push_str(&format!(
            " · Z {} {}",
            name(&altitude.source),
            match (altitude.local, altitude.at_first) {
                (false, _) => "AMSL",
                (true, true) => "start on ground",
                (true, false) => "above terrain",
            }
        ));
    }
    (title, detail)
}

#[component]
pub fn TrajectorySetup(workbench: Workbench, pane: Pane) -> impl IntoView {
    let builder = workbench.trajectory_builder();
    let is_map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().is_some_and(|s| s.map.is_some()))
    });
    let closed = RwSignal::new_local(BTreeSet::<usize>::new());
    view! {column(class="trajectory-setup-page"){
        row(class="trajectory-setup-heading"){
            column(class="trajectory-heading-copy"){text(class="heading"){{move ||if is_map.get(){"Map traces"}else{"2D traces"}}}text(class="trajectory-setup-caption"){"Drag a finished trace onto a plot"}}
            ActionButton(icon=true,class="trajectory-close-builder",a11y:label="Close plot setup",on:click=move |_|builder.open.set(false)){
                vector(class="trajectory-control-icon",prop:viewBox="0 0 16 16",prop:d="M4 4 L12 12 M12 4 L4 12",a11y:hidden=true)
            }
        }
        scroll(class="trajectory-setup-pairs",node_ref=builder.area){
            for index in move ||row_indices(pane),key=|index|*index{
                TraceCard(workbench=workbench,pane=pane,index=index,closed=closed,map_mode=is_map)
            }
            if move ||is_map.get() {ColorFieldSlot(workbench=workbench,pane=pane)}
        }
        text(class="trajectory-setup-hint"){{move ||if is_map.get(){"Drop one field from a GPS or position topic; its partners fill in. Drop a card on a map to add the trace."}else{"Drop on a 2D plot to add a trace, or a time plot to create one."}}}
    }}
}

#[component]
fn TraceCard(
    workbench: Workbench,
    pane: Pane,
    index: usize,
    closed: RwSignal<BTreeSet<usize>, LocalStorage>,
    map_mode: Memo<bool>,
) -> impl IntoView {
    let open = Memo::new(move |_| closed.with(|c| !c.contains(&index)));
    let filled =
        Memo::new(move |_| pair(pane, index).is_some_and(|p| !p.x.is_empty() || !p.y.is_empty()));
    let local = Memo::new(move |_| pair(pane, index).is_some_and(|p| p.local.is_some()));
    let swatch = Signal::derive_local(move || {
        let source = pair(pane, index)?.y;
        let id = pane
            .lines
            .with(|lines| lines.iter().find(|l| l.source == source).map(|l| l.id))
            .or_else(|| {
                target(workbench, pane)
                    .lines
                    .with(|lines| lines.iter().find(|l| l.source == source).map(|l| l.id))
            })?;
        let [r, g, b, _] = target(workbench, pane).plot.style(&id.to_string()).color;
        Some(format!("#{r:02x}{g:02x}{b:02x}"))
    });
    view! {column(class="trace-card",class:trace-card-open=move ||open.get()) {
        row(class="trace-card-summary",on:click=move |_|closed.update(|c|{if !c.remove(&index){c.insert(index);}})) {
            vector(class="trace-card-chevron",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if open.get(){CHEVRON_OPEN}else{CHEVRON_CLOSED}),a11y:hidden=true)
            box(class="trace-card-swatch",class:trace-card-swatch-empty=move ||swatch.get().is_none(),style:background=move ||swatch.get()) {}
            column(class="trace-card-name") {
                text(class="trace-card-title") {{move ||summary(workbench,pane,index).0}}
                text(class="trace-card-detail") {{move ||summary(workbench,pane,index).1}}
            }
        }
        if move ||open.get() {
            column(class="trace-card-body") {
                row(class="trace-card-group-title") {
                    text {"Position"}
                    if move ||map_mode.get() && !local.get() {UnitBadge(workbench=workbench,pane=pane,index=index)}
                }
                row(class="trajectory-axis-headings"){text{{move ||if !map_mode.get(){"X axis"}else if local.get(){"East (X)"}else{"Longitude"}}}text{{move ||if !map_mode.get(){"Y axis"}else if local.get(){"North (Y)"}else{"Latitude"}}}}
                row(class="trajectory-pair-inputs"){
                    AxisSlot(workbench=workbench,pane=pane,index=index,y=false)
                    AxisSlot(workbench=workbench,pane=pane,index=index,y=true)
                    box(class="trajectory-column-divider"){}
                    if move ||filled.get() {
                        column(class="trajectory-pair-actions"){
                            SwapAxes(workbench=workbench,pane=pane,index=Signal::stored_local(index))
                        }
                    }
                }
                if move ||map_mode.get() && filled.get() {
                    MapUnits(workbench=workbench,pane=pane,index=index)
                    if move ||local.get() {StartLocation(workbench=workbench,pane=pane,index=index)}
                    AltitudeOptions(workbench=workbench,pane=pane,index=index,force=true)
                }
            }
        }
        if move ||pair(pane,index).is_some_and(|p|!p.x.is_empty() && !p.y.is_empty()) {
            PairDrag(workbench=workbench,pane=pane,index=index)
        }
    }}
}

#[component]
fn UnitBadge(workbench: Workbench, pane: Pane, index: usize) -> impl IntoView {
    let source = Memo::new(move |_| pair(pane, index).map(|p| p.x).unwrap_or_default());
    let detected = Memo::new(move |_| detect_scale(workbench, &source.get()));
    let current = Memo::new(move |_| pair(pane, index).and_then(|p| p.gps_scale).unwrap_or(1.));
    view! {
        if move ||detected.get().is_some_and(|d|d==current.get()) {
            text(class="trace-card-badge") {{move ||format!("{} detected",unit_label(detected.get(),false))}}
        }
        if move ||detected.get().is_some_and(|d|d!=current.get()) {
            ActionButton(class="trace-card-badge-warn",on:click=move |_|{if let Some(scale)=detected.get_untracked() && let Err(e)=map_pair_options(pane,index,scale,None){workbench.status.set(e);}}) {
                {move ||format!("Values look like {} · use it",unit_label(detected.get(),false))}
            }
        }
    }
}

#[component]
fn MapUnits(workbench: Workbench, pane: Pane, index: usize) -> impl IntoView {
    let state = Memo::new(move |_| {
        pair(pane, index).map_or((Some(1.), false), |p| (p.gps_scale, p.local.is_some()))
    });
    let choose = move |scale: Option<f64>| {
        let result = match scale {
            Some(scale) => map_pair_options(pane, index, scale, None),
            None => local_pair(pane, index),
        };
        if let Err(error) = result {
            workbench.status.set(error);
        }
        sync_target(workbench, pane);
    };
    view! {row(class="trace-card-segment",a11y:role=Role::Group,a11y:label="Coordinate units") {
        ActionButton(class="map-unit-degrees",class:setting-active=move ||state.get()==(Some(1.),false),on:click=move |_|choose(Some(1.))) {"Degrees"}
        ActionButton(class="map-unit-e7",class:setting-active=move ||state.get()==(Some(DEGREES_E7),false),on:click=move |_|choose(Some(DEGREES_E7))) {"deg×10⁷"}
        ActionButton(class="map-unit-radians",class:setting-active=move ||state.get()==(Some(RADIANS),false),on:click=move |_|choose(Some(RADIANS))) {"Radians"}
        ActionButton(class="map-coordinate-local",class:setting-active=move ||state.get().1,on:click=move |_|choose(None)) {"Local m"}
    }}
}

/// Builder pair edits apply to the map being edited as well.
fn sync_target(workbench: Workbench, pane: Pane) {
    if pane.id != 0 {
        return;
    }
    let target = target(workbench, pane);
    if target.id == pane.id {
        return;
    }
    let Some(spec) = pane.trajectory.get_untracked() else {
        return;
    };
    let pairs = spec.pairs();
    target.trajectory.update(|t| {
        if let Some(t) = t
            .as_mut()
            .filter(|t| t.map.is_some() && t.pairs().len() == pairs.len())
        {
            t.local = spec.local;
            t.gps_scale = spec.gps_scale;
            for (mine, theirs) in t.extra.iter_mut().zip(spec.extra.iter()) {
                mine.local = theirs.local;
                mine.gps_scale = theirs.gps_scale;
            }
        }
    });
}

fn edit_local(
    workbench: Workbench,
    pane: Pane,
    index: usize,
    edit: impl Fn(&mut crate::mapping::flight::LocalFrame),
) {
    update(workbench, pane, |spec| {
        let local = if index == 0 {
            spec.local.as_mut()
        } else {
            spec.extra.get_mut(index - 1).and_then(|p| p.local.as_mut())
        };
        if let Some(local) = local {
            edit(local);
        }
    });
}

/// First valid fix of a geographic trace on the map being edited.
fn gps_start(workbench: Workbench, pane: Pane) -> Option<[f64; 2]> {
    target(workbench, pane).trajectory_data.with(|data| {
        data.as_ref()?.traces.iter().find_map(|trace| {
            trace
                .track
                .geographic
                .as_ref()?
                .iter()
                .find(|p| p.iter().all(|v| v.is_finite()))
                .copied()
        })
    })
}

fn parse_lat_lon(text: &str) -> Option<[f64; 2]> {
    let mut parts = text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(str::parse::<f64>);
    let lat = parts.next()?.ok()?;
    let lon = parts.next()?.ok()?;
    (parts.next().is_none() && lat.abs() <= 90. && lon.abs() <= 180.).then_some([lon, lat])
}

#[component]
fn StartLocation(workbench: Workbench, pane: Pane, index: usize) -> impl IntoView {
    let frame = Memo::new(move |_| pair(pane, index).and_then(|p| p.local).unwrap_or_default());
    let saved = RwSignal::new_local(crate::preferences::DefaultMapStart::user().load());
    let draft = RwSignal::new_local(String::new());
    let error = RwSignal::new_local(String::new());
    let gps = Memo::new(move |_| gps_start(workbench, pane));
    let map_target = Signal::derive_local(move || {
        let target = target(workbench, pane);
        (target.id != 0
            && target
                .trajectory
                .with(|s| s.as_ref().is_some_and(|s| s.map.is_some())))
        .then_some(target)
    });
    let set = move |origin: [f64; 2]| {
        let check = crate::mapping::flight::LocalFrame {
            origin: Some(origin),
            at_first: true,
        }
        .validate();
        match check {
            Ok(()) => {
                error.set(String::new());
                edit_local(workbench, pane, index, move |local| {
                    local.origin = Some(origin)
                });
            }
            Err(e) => error.set(e),
        }
    };
    let apply_draft = move || match parse_lat_lon(&draft.get_untracked()) {
        Some(origin) => set(origin),
        None => error.set("Enter latitude, longitude in degrees, e.g. 47.397742, 8.545594".into()),
    };
    let coords = |origin: Option<[f64; 2]>| {
        origin.map_or_else(
            || "none".to_owned(),
            |[lon, lat]| format!("{lat:.6}, {lon:.6}"),
        )
    };
    view! {column(class="trace-card-group local-start") {
        row(class="trace-card-group-title") {text {"Start location"}}
        row(class="trace-card-segment") {
            ActionButton(class="start-default",disabled=Signal::derive_local(move ||saved.get().is_none()),on:click=move |_|if let Some(origin)=saved.get_untracked(){set(origin);}) {"Default"}
            ActionButton(class="start-gps",disabled=Signal::derive_local(move ||gps.get().is_none()),on:click=move |_|if let Some(origin)=gps.get_untracked(){set(origin);}) {"From GPS"}
            ActionButton(class="start-map",disabled=Signal::derive_local(move ||map_target.with(Option::is_none)),on:click=move |_|if let (Some(target),Some(pair))=(map_target.get_untracked(),pair(pane,index)){target.map_origin_target.set(Some(pair.y));}) {"Click map"}
        }
        Input(class="start-lat-lon",value=draft,placeholder="Lat, lon — e.g. 47.397742, 8.545594",a11y:label="Start latitude and longitude",on:key_down=move |e|if e.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){apply_draft();})
        text(class="trace-card-detail") {{move ||format!("Start: {}",coords(frame.get().origin))}}
        if move ||!error.get().is_empty() {text(class="trace-gradient-error",a11y:role=Role::Alert){{move ||error.get()}}}
        row(class="trace-card-segment") {
            text(class="trace-card-label") {"Put here"}
            ActionButton(class="start-first-sample",class:setting-active=move ||frame.get().at_first,on:click=move |_|edit_local(workbench,pane,index,|l|l.at_first=true)) {"First sample"}
            ActionButton(class="start-origin",class:setting-active=move ||!frame.get().at_first,on:click=move |_|edit_local(workbench,pane,index,|l|l.at_first=false)) {"Local (0, 0)"}
        }
        row(class="trace-card-default") {
            text(class="trace-card-detail") {{move ||format!("★ Default start: {}",coords(saved.get()))}}
            ActionButton(class="start-save-default",disabled=Signal::derive_local(move ||frame.get().origin.is_none()),on:click=move |_|{
                if let Some(origin)=frame.get_untracked().origin {
                    match crate::preferences::DefaultMapStart::user().save(origin) {
                        Ok(())=>saved.set(Some(origin)),
                        Err(e)=>error.set(format!("Could not save default start: {e}")),
                    }
                }
            }) {"Save as default"}
        }
    }}
}

/// Map-wide field that traces in Field color mode are shaded by.
#[component]
fn ColorFieldSlot(workbench: Workbench, pane: Pane) -> impl IntoView {
    let builder = workbench.trajectory_builder();
    let source = Memo::new(move |_| {
        pane.trajectory.with(|s| {
            s.as_ref()
                .map(|s| s.color_source.clone())
                .unwrap_or_default()
        })
    });
    view! {column(class="trace-card trace-card-open color-field-card") {
        row(class="trace-card-group-title") {text {"Color field"} text(class="trace-card-detail") {"whole map"}}
        column(class="trajectory-axis-slot color-field-slot",class:axis-slot-filled=move ||!source.get().is_empty(),class:axis-drop-active=move ||builder.color_hover.get(),node_ref=builder.color_slot) {
            if move ||source.get().is_empty() {
                text(class="trajectory-drop-label") {"Drop any field to color traces by it"}
            }
            if move ||!source.get().is_empty() {
                row(class="trajectory-input-chip") {
                    text(class="signal-name") {{move ||workbench.field_name(&source.get())}}
                    ActionButton(icon=true,class="trajectory-remove-input",a11y:label="Remove color field",on:click=move |_|crate::mapping::settings::bind_color(workbench,pane,String::new())) {
                        vector(class="trajectory-control-icon",prop:viewBox="0 0 16 16",prop:d="M4 4 L12 12 M12 4 L4 12",a11y:hidden=true)
                    }
                }
            }
        }
        text(class="trace-card-detail") {"Traces use it when their legend color is set to the field."}
    }}
}

#[derive(Debug, PartialEq)]
struct Siblings {
    x: String,
    y: String,
    z: Option<String>,
    geographic: bool,
}

const LONGITUDE: [&str; 4] = ["lon", "longitude_deg", "longitude", "lng"];
const LATITUDE: [&str; 3] = ["lat", "latitude_deg", "latitude"];
const ALTITUDE: [&str; 6] = [
    "alt",
    "altitude_msl_m",
    "alt_msl",
    "altitude",
    "altitude_amsl",
    "altitude_ellipsoid_m",
];

/// Partners of `source` in the same message/instance: longitude, latitude and
/// altitude, or local x/y/z (NED: east is `y`, north is `x`). No cross-topic guesses.
fn siblings(ids: &[String], source: &str) -> Option<Siblings> {
    let delimiter = if source.starts_with("mavlink/") {
        '.'
    } else {
        '/'
    };
    let (prefix, leaf) = source.rsplit_once(delimiter)?;
    let find = |names: &[&str]| {
        names.iter().find_map(|name| {
            let id = format!("{prefix}{delimiter}{name}");
            ids.contains(&id).then_some(id)
        })
    };
    if LONGITUDE.contains(&leaf) || LATITUDE.contains(&leaf) || ALTITUDE.contains(&leaf) {
        return Some(Siblings {
            x: find(&LONGITUDE)?,
            y: find(&LATITUDE)?,
            z: find(&ALTITUDE),
            geographic: true,
        });
    }
    if ["x", "y", "z"].contains(&leaf) {
        return Some(Siblings {
            x: find(&["y"])?,
            y: find(&["x"])?,
            z: find(&["z"]),
            geographic: false,
        });
    }
    None
}

/// One field dropped into an empty map pair binds its partners from the same topic.
pub(crate) fn fill_siblings(workbench: Workbench, pane: Pane, index: usize, source: &str) {
    let Some(mut spec) = pane.trajectory.get_untracked() else {
        return;
    };
    let mut pairs = spec.pairs();
    let Some(current) = pairs.get(index) else {
        return;
    };
    if spec.map.is_none() || current.x.is_empty() == current.y.is_empty() {
        return;
    }
    let ids = workbench.log.with_untracked(|log| {
        log.as_ref()
            .map(|log| {
                log.catalogue
                    .iter()
                    .map(|s| s.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let Some(found) = siblings(&ids, source) else {
        return;
    };
    let pair = &mut pairs[index];
    pair.x = found.x.clone();
    pair.y = found.y.clone();
    if found.geographic {
        let known = crate::trajectory::gps_pairs(&ids)
            .into_iter()
            .find(|s| s.x == found.x);
        pair.local = None;
        pair.gps_scale = Some(
            known
                .as_ref()
                .and_then(|s| s.gps_scale)
                .or_else(|| detect_scale(workbench, &found.x))
                .unwrap_or(1.),
        );
        pair.fix = known.map(|s| s.fix).unwrap_or_default();
    } else {
        pair.gps_scale = None;
        pair.fix.clear();
        pair.local.get_or_insert_with(Default::default);
    }
    if let Some(z) = &found.z {
        spec.altitudes.insert(
            index,
            crate::trajectory::Altitude {
                source: z.clone(),
                scale: if found.geographic { 1. } else { -1. },
                local: !found.geographic,
                at_first: !found.geographic,
            },
        );
    }
    if let Err(error) = crate::trajectory_axes::store(pane, spec, pairs) {
        workbench.status.set(error);
        return;
    }
    let leaf = |id: &str| id.rsplit(['/', '.']).next().unwrap_or(id).to_owned();
    let mut filled = vec![leaf(&found.x), leaf(&found.y)];
    filled.extend(found.z.as_deref().map(leaf));
    let topic = source
        .rsplit_once(['/', '.'])
        .map_or(source, |(prefix, _)| prefix);
    workbench
        .status
        .set(format!("Filled {} from {topic}", filled.join(", ")));
}

/// Add the trace `source` belongs to onto an existing map; returns the fields to load.
pub(crate) fn add_map_trace(workbench: Workbench, pane: Pane, source: &str) -> Option<Vec<String>> {
    let ids = workbench.log.with_untracked(|log| {
        log.as_ref()
            .map(|log| {
                log.catalogue
                    .iter()
                    .map(|s| s.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let found = siblings(&ids, source)?;
    let mut spec = pane.trajectory.get_untracked()?;
    spec.map?;
    let pair = if found.geographic {
        let known = crate::trajectory::gps_pairs(&ids)
            .into_iter()
            .find(|s| s.x == found.x);
        Pair {
            local: None,
            x: found.x.clone(),
            y: found.y.clone(),
            gps_scale: Some(
                known
                    .as_ref()
                    .and_then(|s| s.gps_scale)
                    .or_else(|| detect_scale(workbench, &found.x))
                    .unwrap_or(1.),
            ),
            fix: known.map(|s| s.fix).unwrap_or_default(),
        }
    } else {
        Pair {
            local: Some(crate::mapping::flight::LocalFrame {
                origin: crate::preferences::DefaultMapStart::user().load(),
                at_first: true,
            }),
            x: found.x.clone(),
            y: found.y.clone(),
            gps_scale: None,
            fix: String::new(),
        }
    };
    let mut pairs = spec.pairs();
    if pairs.iter().any(|p| p.y == pair.y) {
        return None;
    }
    let index = match pairs.iter().position(|p| p.x.is_empty() && p.y.is_empty()) {
        Some(index) => {
            pairs[index] = pair;
            index
        }
        None => {
            pairs.push(pair);
            pairs.len() - 1
        }
    };
    if let Some(z) = &found.z {
        spec.altitudes.insert(
            index,
            crate::trajectory::Altitude {
                source: z.clone(),
                scale: if found.geographic { 1. } else { -1. },
                local: !found.geographic,
                at_first: !found.geographic,
            },
        );
    }
    if let Err(error) = crate::trajectory_axes::store(pane, spec, pairs) {
        workbench.status.set(error);
        return None;
    }
    let mut sources = vec![found.x, found.y];
    sources.extend(found.z);
    Some(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn siblings_fill_from_the_same_topic_only() {
        let ids = [
            "vehicle_gps_position/0/latitude_deg",
            "vehicle_gps_position/0/longitude_deg",
            "vehicle_gps_position/0/altitude_msl_m",
            "vehicle_global_position/0/lat",
            "vehicle_local_position/0/x",
            "vehicle_local_position/0/y",
            "mavlink/1.1/GLOBAL_POSITION_INT.lat",
            "mavlink/1.1/GLOBAL_POSITION_INT.lon",
        ]
        .map(String::from);
        let gps = siblings(&ids, "vehicle_gps_position/0/altitude_msl_m").unwrap();
        assert_eq!(
            gps,
            Siblings {
                x: "vehicle_gps_position/0/longitude_deg".into(),
                y: "vehicle_gps_position/0/latitude_deg".into(),
                z: Some("vehicle_gps_position/0/altitude_msl_m".into()),
                geographic: true,
            }
        );
        // No longitude in the same instance: nothing is guessed elsewhere.
        assert_eq!(siblings(&ids, "vehicle_global_position/0/lat"), None);
        let local = siblings(&ids, "vehicle_local_position/0/x").unwrap();
        assert_eq!(
            (local.x.as_str(), local.y.as_str(), local.z),
            (
                "vehicle_local_position/0/y",
                "vehicle_local_position/0/x",
                None
            )
        );
        let mavlink = siblings(&ids, "mavlink/1.1/GLOBAL_POSITION_INT.lat").unwrap();
        assert_eq!(mavlink.x, "mavlink/1.1/GLOBAL_POSITION_INT.lon");
    }

    #[test]
    fn start_location_accepts_pasted_lat_lon() {
        assert_eq!(
            parse_lat_lon("47.397742, 8.545594"),
            Some([8.545594, 47.397742])
        );
        assert_eq!(parse_lat_lon("47.4 8.5"), Some([8.5, 47.4]));
        assert_eq!(parse_lat_lon("95, 8"), None);
        assert_eq!(parse_lat_lon("47.4"), None);
    }
}
