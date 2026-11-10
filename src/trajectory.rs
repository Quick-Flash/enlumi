//! Time-indexed arbitrary XY paths. Samples remain in recording order, including loops.
use plot::{
    PlotPoint,
    plot_data::{ScatterSeries, ScatterSet},
    time_series::SeriesData,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub altitudes: std::collections::BTreeMap<usize, Altitude>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<crate::mapping::flight::LocalFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<crate::mapping::flight::Config>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color_source: String,
    pub x: String,
    pub y: String,
    /// GPS: X is longitude, Y is latitude; scale converts wire values to degrees.
    pub gps_scale: Option<f64>,
    #[serde(default)]
    pub fix: String,
    pub connected: bool,
    pub equal: bool,
    pub points: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Pair>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Altitude {
    pub source: String,
    pub scale: f64,
    pub local: bool,
    /// Subtract first valid aligned altitude before anchoring to terrain.
    pub at_first: bool,
}
impl Default for Altitude {
    fn default() -> Self {
        Self {
            source: String::new(),
            scale: 1.,
            local: true,
            at_first: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<crate::mapping::flight::LocalFrame>,
    pub x: String,
    pub y: String,
    pub gps_scale: Option<f64>,
    #[serde(default)]
    pub fix: String,
}
impl Default for Spec {
    fn default() -> Self {
        Self {
            altitudes: Default::default(),
            local: None,
            map: None,
            color_source: String::new(),
            x: String::new(),
            y: String::new(),
            gps_scale: None,
            fix: String::new(),
            connected: true,
            equal: true,
            points: false,
            extra: Vec::new(),
        }
    }
}
impl Spec {
    pub fn pairs(&self) -> Vec<Pair> {
        std::iter::once(Pair {
            local: self.local,
            x: self.x.clone(),
            y: self.y.clone(),
            gps_scale: self.gps_scale,
            fix: self.fix.clone(),
        })
        .chain(self.extra.iter().cloned())
        .collect()
    }
    pub fn sources_mut(&mut self) -> impl Iterator<Item = &mut String> {
        [&mut self.x, &mut self.y, &mut self.fix]
            .into_iter()
            .chain(std::iter::once(&mut self.color_source))
            .chain(
                self.extra
                    .iter_mut()
                    .flat_map(|pair| [&mut pair.x, &mut pair.y, &mut pair.fix]),
            )
            .chain(
                self.altitudes
                    .values_mut()
                    .map(|altitude| &mut altitude.source),
            )
    }
    pub fn add_source(&mut self, source: &str) {
        if self.color_source == source
            || self
                .altitudes
                .values()
                .any(|altitude| altitude.source == source)
        {
            return;
        }
        if self
            .pairs()
            .iter()
            .any(|pair| pair.x == source || pair.y == source || pair.fix == source)
        {
            return;
        }
        if self.x.is_empty() {
            self.x = source.into();
        } else if self.y.is_empty() {
            self.y = source.into();
        } else if let Some(pair) = self
            .extra
            .iter_mut()
            .find(|pair| pair.x.is_empty() || pair.y.is_empty())
        {
            if pair.x.is_empty() {
                pair.x = source.into();
            } else {
                pair.y = source.into();
            }
        } else if self.extra.len() < 255 {
            self.extra.push(Pair {
                local: None,
                x: source.into(),
                y: String::new(),
                gps_scale: None,
                fix: String::new(),
            });
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self
            .altitudes
            .values()
            .any(|a| !a.scale.is_finite() || a.scale == 0.)
        {
            return Err("Altitude scale must be nonzero and finite".into());
        }
        if let Some(map) = &self.map {
            map.validate()?;
            if self
                .pairs()
                .iter()
                .any(|pair| pair.gps_scale.is_none() && pair.local.is_none())
            {
                return Err(
                    "Choose geographic coordinates or anchored local metres for each map trace"
                        .into(),
                );
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        if self.extra.len() > 255 {
            return Err("A 2D plot supports at most 256 traces".into());
        }
        for pair in self.pairs() {
            if let Some(local) = pair.local {
                local.validate()?;
                if self.map.is_none() || pair.gps_scale.is_some() {
                    return Err("Local anchors require a map trace with metre units".into());
                }
            }
            if !pair.y.is_empty() && !seen.insert(pair.y) {
                return Err("Each Y signal can appear only once in a 2D plot".into());
            }
            if pair
                .gps_scale
                .is_some_and(|scale| !scale.is_finite() || scale <= 0.)
            {
                return Err("GPS coordinate scale must be positive and finite".into());
            }
        }
        if self
            .gps_scale
            .is_some_and(|scale| !scale.is_finite() || scale <= 0.)
        {
            return Err("GPS coordinate scale must be positive and finite".into());
        }
        Ok(())
    }
}

pub struct Track {
    pub times: Arc<[f64]>,
    pub points: Arc<[PlotPoint]>,
    pub scatter: ScatterSet,
    pub vertices: Arc<[[f32; 2]]>,
    pub origin: PlotPoint,
    pub gps_origin: Option<(f64, f64)>,
    pub geographic: Option<Arc<[[f64; 2]]>>,
    pub endpoints: (PlotPoint, PlotPoint),
    max_gap: f64,
}
pub struct PreparedTrace {
    pub id: u64,
    pub track: Track,
}
#[derive(Default)]
pub struct Tracks {
    pub traces: Vec<PreparedTrace>,
    pub scatter: ScatterSet,
    pub warnings: Vec<String>,
    inputs: Inputs,
}
#[derive(Default, PartialEq)]
struct Inputs {
    map: bool,
    pairs: Vec<Pair>,
    lines: Vec<(u64, String, String, u64, usize, usize)>,
}
impl Inputs {
    fn new(lines: &[crate::state::Line], spec: &Spec) -> Self {
        let mut keys = lines
            .iter()
            .map(|line| {
                (
                    line.id,
                    line.source.clone(),
                    line.name.clone(),
                    line.revision,
                    line.data.samples().as_ptr() as usize,
                    line.data.samples().len(),
                )
            })
            .collect::<Vec<_>>();
        keys.sort_by_key(|line| line.0);
        Self {
            map: spec.map.is_some(),
            pairs: spec.pairs(),
            lines: keys,
        }
    }
}
impl Tracks {
    pub fn matches(&self, lines: &[crate::state::Line], spec: &Spec) -> bool {
        self.inputs == Inputs::new(lines, spec)
    }
    #[cfg(test)]
    pub fn prepare(lines: &[crate::state::Line], spec: &Spec) -> Result<Self, String> {
        Self::prepare_cancelled(lines, spec, &|| false)
    }
    pub fn prepare_cancelled(
        lines: &[crate::state::Line],
        spec: &Spec,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        spec.validate()?;
        let mut traces = Vec::new();
        let mut warnings = Vec::new();
        let mut anchor = None;
        for pair in spec.pairs() {
            if cancelled() {
                return Err("2D preparation superseded".into());
            }
            if pair.y.is_empty() {
                continue;
            }
            let result = (|| {
                let x = lines
                    .iter()
                    .find(|line| line.source == pair.x)
                    .ok_or_else(|| format!("Missing X input {}", pair.x))?;
                let y = lines
                    .iter()
                    .find(|line| line.source == pair.y)
                    .ok_or_else(|| format!("Missing Y input {}", pair.y))?;
                let fix = if pair.gps_scale.is_none() || pair.fix.is_empty() {
                    None
                } else {
                    Some(
                        &lines
                            .iter()
                            .find(|line| line.source == pair.fix)
                            .ok_or("Missing GPS fix input")?
                            .data,
                    )
                };
                let local = Spec {
                    local: pair.local,
                    map: spec.map,
                    x: pair.x,
                    y: pair.y,
                    gps_scale: pair.gps_scale,
                    fix: pair.fix,
                    ..Default::default()
                };
                let track = Track::named(
                    &x.data,
                    &y.data,
                    fix,
                    &local,
                    anchor,
                    (&y.id.to_string(), &format!("{} vs {}", y.name, x.name)),
                    cancelled,
                )?;
                if track.gps_origin.is_some() {
                    anchor = track.gps_origin;
                }
                Ok::<_, String>(PreparedTrace { id: y.id, track })
            })();
            match result {
                Ok(trace) => traces.push(trace),
                Err(error) => warnings.push(error),
            }
        }
        if cancelled() {
            return Err("2D preparation superseded".into());
        }
        if traces.is_empty() {
            return Err(if warnings.is_empty() {
                "Choose an X signal and at least one Y signal in 2D settings.".into()
            } else {
                warnings.join("; ")
            });
        }
        let scatter = ScatterSet::new(
            traces
                .iter()
                .flat_map(|trace| trace.track.scatter.series().iter().cloned())
                .collect(),
        )?;
        Ok(Self {
            traces,
            scatter,
            warnings,
            inputs: Inputs::new(lines, spec),
        })
    }
    #[cfg(test)]
    pub fn at(&self, time: f64) -> Option<PlotPoint> {
        self.traces.first()?.track.at(time)
    }
}
impl Track {
    #[cfg(test)]
    pub fn prepare(
        x: &SeriesData,
        y: &SeriesData,
        fix: Option<&SeriesData>,
        spec: &Spec,
    ) -> Result<Self, String> {
        Self::named(x, y, fix, spec, None, ("trajectory", "Trajectory"), &|| {
            false
        })
    }
    pub fn named(
        x: &SeriesData,
        y: &SeriesData,
        fix: Option<&SeriesData>,
        spec: &Spec,
        anchor: Option<(f64, f64)>,
        identity: (&str, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let (id, label) = identity;
        if cancelled() {
            return Err("2D preparation superseded".into());
        }
        spec.validate()?;
        let reference = x.samples();
        let mut aligned = crate::filter_multi::Aligned::new(y, reference);

        let mut gps_origin = anchor;
        let mut points = Vec::with_capacity(reference.len());
        let mut times = Vec::with_capacity(reference.len());
        let mut intervals = reference
            .windows(2)
            .map(|p| p[1].x - p[0].x)
            .filter(|dt| *dt > 0.)
            .collect::<Vec<_>>();
        intervals.sort_unstable_by(f64::total_cmp);
        if cancelled() {
            return Err("2D preparation superseded".into());
        }
        let max_gap = intervals.get(intervals.len() / 2).copied().unwrap_or(0.) * 10.;
        let mut previous_time = None;
        let mut geographic = (spec.gps_scale.is_some() || spec.local.is_some())
            .then(|| Vec::with_capacity(reference.len()));
        let local_frame = spec
            .local
            .map(|frame| {
                let origin = frame.origin.ok_or_else(|| {
                    "Choose the starting location for this local trace".to_string()
                })?;
                let mut world = crate::mapping::flight::project(origin[0], origin[1]).unwrap();
                if let Some((lon, lat)) = anchor {
                    let reference = crate::mapping::flight::project(lon, lat).unwrap()[0];
                    world[0] += ((reference - world[0]) / crate::mapping::flight::WORLD).round()
                        * crate::mapping::flight::WORLD;
                }
                Ok::<_, String>((frame, origin, world, origin[1].to_radians().cos()))
            })
            .transpose()?;
        let mut local_zero = None;
        let mut previous_x = None::<f64>;
        for (index, sample) in reference.iter().enumerate() {
            if index % 4096 == 0 && cancelled() {
                return Err("2D preparation superseded".into());
            }
            let mut point = PlotPoint {
                x: sample.y,
                y: aligned.at(index, sample.x),
            };
            let gap = previous_time.is_some_and(|time| sample.x - time > max_gap);
            if gap {
                previous_x = None;
            }
            let mut geo = [f64::NAN; 2];
            if let Some((frame, origin, world, scale)) = local_frame {
                if point.x.is_finite() && point.y.is_finite() {
                    let zero = if frame.at_first {
                        *local_zero.get_or_insert([point.x, point.y])
                    } else {
                        [0., 0.]
                    };
                    point = PlotPoint {
                        x: world[0] + (point.x - zero[0]) / scale,
                        y: world[1] + (point.y - zero[1]) / scale,
                    };
                    let coords = crate::mapping::flight::geographic([point.x, point.y]);
                    geo = [(coords[0] + 180.).rem_euclid(360.) - 180., coords[1]];
                    gps_origin = Some((origin[0], origin[1]));
                } else {
                    point = PlotPoint {
                        x: f64::NAN,
                        y: f64::NAN,
                    };
                }
            } else if let Some(scale) = spec.gps_scale {
                let (lon, lat) = (point.x * scale, point.y * scale);
                let valid_fix = fix.is_none_or(|fix| {
                    let values = fix.samples();
                    let right = values.partition_point(|value| value.x <= sample.x);
                    right.checked_sub(1).is_some_and(|index| {
                        let value = values[index];
                        value.y.is_finite()
                            && value.y >= (if spec.map.is_some() { 2. } else { 3. })
                            && sample.x - value.x <= max_gap
                    })
                });
                if !valid_fix
                    || !lon.is_finite()
                    || !lat.is_finite()
                    || !(-180. ..=180.).contains(&lon)
                    || !(-90. ..=90.).contains(&lat)
                {
                    point = PlotPoint {
                        x: f64::NAN,
                        y: f64::NAN,
                    };
                } else {
                    geo = [lon, lat];
                    let (lon0, lat0) = *gps_origin.get_or_insert((lon, lat));
                    // Local spherical East/North coordinates, with dateline wrapping.
                    let dlon = (lon - lon0 + 180.).rem_euclid(360.) - 180.;
                    point.x = 6_371_008.8 * dlon.to_radians() * lat0.to_radians().cos();
                    point.y = 6_371_008.8 * (lat - lat0).to_radians();
                    if spec.map.is_some() {
                        let [mut x, y] = crate::mapping::flight::project(lon, lat)
                            .expect("Validated geographic coordinates");
                        let reference = previous_x.unwrap_or_else(|| {
                            crate::mapping::flight::project(lon0, lat0).unwrap()[0]
                        });
                        x += ((reference - x) / crate::mapping::flight::WORLD).round()
                            * crate::mapping::flight::WORLD;
                        previous_x = Some(x);
                        point = PlotPoint { x, y };
                    }
                }
            }
            if !point.x.is_finite() {
                previous_x = None;
            }
            if gap {
                if let Some(geo) = &mut geographic {
                    geo.push([f64::NAN; 2]);
                }
                times.push(sample.x);
                points.push(PlotPoint {
                    x: f64::NAN,
                    y: f64::NAN,
                });
            }
            previous_time = Some(sample.x);
            if let Some(geographic) = &mut geographic {
                geographic.push(geo);
            }
            times.push(sample.x);
            points.push(point);
        }
        if !points
            .iter()
            .any(|point| point.x.is_finite() && point.y.is_finite())
        {
            return Err("No overlapping valid X/Y samples".into());
        }
        let endpoints = (
            *points
                .iter()
                .find(|p| p.x.is_finite() && p.y.is_finite())
                .unwrap(),
            *points
                .iter()
                .rev()
                .find(|p| p.x.is_finite() && p.y.is_finite())
                .unwrap(),
        );
        let points: Arc<[PlotPoint]> = points.into();
        let series = ScatterSeries::new(id, label, points.clone())?;
        let vertices = series.vertices();
        let origin = series.origin();
        let scatter = ScatterSet::new(vec![series])?;
        Ok(Self {
            times: times.into(),
            points,
            scatter,
            vertices,
            origin,
            gps_origin,
            geographic: geographic.map(Arc::from),
            endpoints,
            max_gap,
        })
    }
    pub fn at(&self, time: f64) -> Option<PlotPoint> {
        if !time.is_finite() {
            return None;
        }
        let right = self.times.partition_point(|value| *value <= time);
        let left = right.checked_sub(1)?;
        let a = self.points[left];
        if !a.x.is_finite() || !a.y.is_finite() {
            return None;
        }
        if self.times[left] == time {
            return Some(a);
        }
        let b = *self.points.get(right)?;
        let dt = self.times[right] - self.times[left];
        if dt <= 0. || dt > self.max_gap || !b.x.is_finite() || !b.y.is_finite() {
            return None;
        }
        let t = (time - self.times[left]) / dt;
        Some(PlotPoint {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
        })
    }
}

/// Pair only fields in the same message/instance, never different vehicles or GPS receivers.
pub fn gps_pair(ids: &[String]) -> Option<Spec> {
    gps_pairs(ids).into_iter().next()
}
pub fn gps_pairs(ids: &[String]) -> Vec<Spec> {
    let mut pairs = Vec::new();
    for id in ids {
        let (prefix, field, delimiter, scale) = if id.starts_with("mavlink/") {
            let Some((prefix, field)) = id.rsplit_once('.') else {
                continue;
            };
            if field != "lon"
                || !(prefix.ends_with("/GLOBAL_POSITION_INT")
                    || prefix.ends_with("/GPS_RAW_INT")
                    || prefix.ends_with("/GPS2_RAW"))
            {
                continue;
            }
            (prefix, field, '.', 1e-7)
        } else {
            let Some((prefix, field)) = id.rsplit_once('/') else {
                continue;
            };
            if !matches!(field, "lon" | "longitude_deg")
                || !(prefix.starts_with("vehicle_global_position/")
                    || prefix.starts_with("vehicle_gps_position/")
                    || prefix.starts_with("sensor_gps/"))
            {
                continue;
            }
            let scale =
                if field == "longitude_deg" || prefix.starts_with("vehicle_global_position/") {
                    1.
                } else {
                    1e-7
                };
            (prefix, field, '/', scale)
        };
        let lat = format!(
            "{prefix}{delimiter}{}",
            if field == "longitude_deg" {
                "latitude_deg"
            } else {
                "lat"
            }
        );
        if !ids.contains(&lat) {
            continue;
        }
        let fix = format!("{prefix}{delimiter}fix_type");
        pairs.push(Spec {
            x: id.clone(),
            y: lat,
            gps_scale: Some(scale),
            fix: if ids.contains(&fix) {
                fix
            } else {
                String::new()
            },
            ..Default::default()
        });
    }
    pairs
}

pub fn equal_ranges(mut ranges: plot::Ranges, width: f64, height: f64) -> plot::Ranges {
    if width <= 0. || height <= 0. {
        return ranges;
    }
    let scale = (ranges.x.span() / width).max(ranges.y.span() / height);
    let (cx, cy) = (
        (ranges.x.min + ranges.x.max) * 0.5,
        (ranges.y.min + ranges.y.max) * 0.5,
    );
    ranges.x = (cx - scale * width * 0.5, cx + scale * width * 0.5).into();
    ranges.y = (cy - scale * height * 0.5, cy + scale * height * 0.5).into();
    ranges
}

#[cfg(test)]
mod tests;
