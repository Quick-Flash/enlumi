use super::*;
use crate::trajectory::{Altitude, Pair};
#[test]
fn new_map_keeps_channel_source_when_dropping_first_pair() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let pane = workbench.pane();
    let builder = workbench.trajectory_builder();
    let (lines, mut spec, _) = fixture();
    spec.color_source = "battery".into();
    spec.map.as_mut().unwrap().coloring = ColorBy::Channel;
    builder.pane.trajectory.set(Some(spec));
    builder.pane.lines.set(lines);
    workbench.drop_trajectory_pair(0, pane.id);
    let spec = pane.trajectory.get_untracked().unwrap();
    assert_eq!(spec.color_source, "battery");
    assert_eq!(spec.map.unwrap().coloring, ColorBy::Channel);
    assert!(
        pane.lines
            .with_untracked(|lines| lines.iter().any(|line| line.source == "battery"))
    );
}
pub(crate) fn fixture() -> (Vec<Line>, Spec, Arc<Tracks>) {
    let columns = [
        ("x", vec![0., 10., 30., f64::NAN, 60., 90.]),
        ("y", vec![0.; 6]),
        ("z", vec![100., 110., 120., 130., 140., 150.]),
    ];
    let mut lines = columns
        .into_iter()
        .enumerate()
        .map(|(i, (source, values))| Line {
            id: i as u64 + 1,
            source: source.into(),
            name: source.into(),
            revision: 0,
            data: plot::time_series::SeriesData::new(
                values
                    .into_iter()
                    .enumerate()
                    .map(|(i, y)| PlotPoint { x: i as f64, y })
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        })
        .collect::<Vec<_>>();
    lines.push(Line {
        id: 4,
        source: "battery".into(),
        name: "voltage".into(),
        revision: 0,
        data: plot::time_series::SeriesData::new(vec![
            PlotPoint { x: 0., y: 10. },
            PlotPoint { x: 2., y: 14. },
            PlotPoint { x: 4., y: f64::NAN },
            PlotPoint { x: 5., y: 20. },
        ])
        .unwrap(),
    });
    let spec = Spec {
        x: "x".into(),
        y: "y".into(),
        local: Some(super::super::flight::LocalFrame {
            origin: Some([0., 0.]),
            at_first: false,
        }),
        map: Some(Default::default()),
        altitudes: BTreeMap::from([(
            0,
            Altitude {
                source: "z".into(),
                local: false,
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    let tracks = Arc::new(Tracks::prepare(&lines, &spec).unwrap());
    (lines, spec, tracks)
}
#[test]
fn speed_uses_ground_metres_and_preserves_position_gaps() {
    let (lines, mut spec, tracks) = fixture();
    spec.map.as_mut().unwrap().coloring = ColorBy::Speed;
    let colors = prepare(&tracks, &lines, &spec, &BTreeSet::new());
    let values = &colors.samples[&2];
    for (index, expected) in [(0, 10.), (1, 10.), (2, 20.), (4, 30.), (5, 30.)] {
        assert!((values[index] - expected).abs() < 1e-6, "{values:?}");
    }
    assert!(values[3].is_nan());
    assert_eq!(colors.range, Some([10., 30.]));
}
#[test]
fn channels_align_by_time_and_altitude_obeys_scale_and_reference() {
    let (lines, mut spec, tracks) = fixture();
    spec.map.as_mut().unwrap().coloring = ColorBy::Channel;
    spec.color_source = "battery".into();
    let colors = prepare(&tracks, &lines, &spec, &BTreeSet::new());
    assert_eq!(colors.samples[&2][1], 12.);
    assert_eq!(colors.range, Some([10., 20.]));
    assert!(colors.samples[&2][3].is_nan() && colors.samples[&2][4].is_nan());
    assert_eq!(colors.color(2, 4, [255; 4]), [145, 145, 145, 255]);
    spec.map.as_mut().unwrap().coloring = ColorBy::Altitude;
    let altitude = spec.altitudes.get_mut(&0).unwrap();
    altitude.local = true;
    altitude.at_first = true;
    altitude.scale = -1.;
    let colors = prepare(&tracks, &lines, &spec, &BTreeSet::new());
    assert_eq!(colors.range, Some([-50., 0.]));
    assert_eq!(colors.samples[&2][1], -10.);
}
#[test]
fn hidden_traces_leave_shared_scale_and_empty_channels_stay_missing() {
    let (mut lines, mut spec, _) = fixture();
    lines.push(Line {
        id: 5,
        source: "fast".into(),
        name: "fast".into(),
        revision: 0,
        data: plot::time_series::SeriesData::new(
            (0..6)
                .map(|i| PlotPoint {
                    x: i as f64,
                    y: i as f64 * 20.,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap(),
    });
    spec.extra.push(Pair {
        x: "x".into(),
        y: "fast".into(),
        local: spec.local,
        gps_scale: None,
        fix: String::new(),
    });
    spec.map.as_mut().unwrap().coloring = ColorBy::Speed;
    let tracks = Tracks::prepare(&lines, &spec).unwrap();
    let all = prepare(&tracks, &lines, &spec, &BTreeSet::new());
    assert!(all.range.unwrap()[1] > 30.);
    let hidden = prepare(&tracks, &lines, &spec, &BTreeSet::from(["5".into()]));
    assert_eq!(hidden.range, Some([10., 30.]));
    spec.map.as_mut().unwrap().coloring = ColorBy::Channel;
    let missing = prepare(&tracks, &lines, &spec, &BTreeSet::new());
    assert_eq!(missing.range, None);
    assert_eq!(
        Coloring {
            range: Some([7., 7.]),
            ..Default::default()
        }
        .bucket_for(2, 7.),
        Some(31)
    );
}
