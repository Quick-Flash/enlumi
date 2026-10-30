use super::*;

fn line(id: u64, source: &str, samples: &[(f64, f64)]) -> crate::state::Line {
    crate::state::Line {
        id,
        source: source.into(),
        name: source.into(),
        data: data(samples),
        revision: 0,
    }
}

#[test]
fn superseded_alignment_stops_between_sample_blocks() {
    let values = (0..10_000)
        .map(|index| (index as f64, index as f64))
        .collect::<Vec<_>>();
    let lines = vec![line(1, "x", &values), line(2, "y", &values)];
    let spec = Spec {
        x: "x".into(),
        y: "y".into(),
        ..Default::default()
    };
    let checks = std::cell::Cell::new(0);
    let result = Tracks::prepare_cancelled(&lines, &spec, &|| {
        checks.set(checks.get() + 1);
        checks.get() >= 5
    });
    assert!(result.is_err_and(|error| error.contains("superseded")));
    assert!(
        checks.get() >= 5,
        "Long alignment must check cancellation inside the sample loop"
    );
    assert!(Tracks::prepare(&lines, &spec).is_ok());
}

#[test]
fn multiple_pairs_keep_identity_share_gps_origin_and_recover_other_valid_traces() {
    let lines = vec![
        line(1, "lon", &[(0., 10.), (1., 10.001)]),
        line(2, "lat", &[(0., 45.), (1., 45.001)]),
        line(3, "other_lon", &[(0., 10.001), (1., 10.002)]),
        line(4, "other_lat", &[(0., 45.001), (1., 45.002)]),
    ];
    let mut spec = Spec {
        x: "lon".into(),
        y: "lat".into(),
        gps_scale: Some(1.),
        extra: vec![Pair {
            x: "other_lon".into(),
            y: "other_lat".into(),
            gps_scale: Some(1.),
            fix: String::new(),
        }],
        ..Default::default()
    };
    let tracks = Tracks::prepare(&lines, &spec).unwrap();
    assert_eq!(tracks.traces.len(), 2);
    assert_eq!(tracks.scatter.series()[1].id(), "4");
    assert_eq!(
        tracks.traces[0].track.gps_origin,
        tracks.traces[1].track.gps_origin
    );
    let start = tracks.traces[1].track.at(0.).unwrap();
    assert!(
        start.x > 70. && start.y > 100.,
        "GPS traces must preserve their physical offset"
    );
    spec.extra[0].x = "missing".into();
    let recovered = Tracks::prepare(&lines, &spec).unwrap();
    assert_eq!(recovered.traces.len(), 1);
    assert_eq!(recovered.warnings.len(), 1);
    spec.extra[0].y = "lat".into();
    assert!(spec.validate().is_err());
}

#[test]
fn adding_sources_builds_independent_pairs_and_remaps_every_pair_for_presets() {
    let mut spec = Spec::default();
    for source in ["x", "a", "b", "c", "x"] {
        spec.add_source(source);
    }
    assert_eq!(spec.pairs().len(), 2);
    assert_eq!(spec.x, "x");
    assert_eq!(spec.extra[0].x, "b");
    for source in spec.sources_mut() {
        if !source.is_empty() {
            *source = format!("${source}");
        }
    }
    assert_eq!(spec.extra[0].y, "$c");
    assert_eq!(spec.extra[0].x, "$b");
}

fn data(samples: &[(f64, f64)]) -> SeriesData {
    SeriesData::new(
        samples
            .iter()
            .map(|(x, y)| PlotPoint { x: *x, y: *y })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn loops_keep_recording_order_and_interpolate_by_time_not_x() {
    let x = data(&[(0., 0.), (1., 2.), (2., 0.), (3., -2.), (4., 0.)]);
    let y = data(&[(0., 0.), (1., 2.), (2., 4.), (3., 2.), (4., 0.)]);
    let track = Track::prepare(&x, &y, None, &Spec::default()).unwrap();
    assert_eq!(track.points[3], PlotPoint { x: -2., y: 2. });
    assert_eq!(track.at(1.5), Some(PlotPoint { x: 1., y: 3. }));
    assert_eq!(track.at(4.), Some(PlotPoint { x: 0., y: 0. }));
    assert!(track.at(-1.).is_none());
    assert!(track.at(5.).is_none());
}

#[test]
fn alignment_does_not_extrapolate_or_bridge_invalid_values_and_long_gaps() {
    let x = data(&[
        (0., 0.),
        (1., 1.),
        (2., 2.),
        (3., 3.),
        (100., 4.),
        (101., 5.),
    ]);
    let y = data(&[
        (1., 10.),
        (2., f64::NAN),
        (3., 30.),
        (100., 40.),
        (101., 50.),
    ]);
    let track = Track::prepare(&x, &y, None, &Spec::default()).unwrap();
    assert!(track.points[0].y.is_nan());
    assert!(track.at(1.5).is_none());
    assert!(track.at(50.).is_none());
    assert!(track.at(100.).is_some());
    assert!(track.points[4].x.is_nan());
    let sparse = data(&[(0., 0.), (2., 20.)]);
    let aligned = Track::prepare(
        &data(&[(0., 1.), (1., 2.), (2., 3.), (3., 4.)]),
        &sparse,
        None,
        &Spec::default(),
    )
    .unwrap();
    assert_eq!(aligned.at(1.).unwrap().y, 10.);
    assert!(aligned.at(3.).is_none());
}

#[test]
fn gps_projection_respects_units_fixes_and_dateline_without_rejecting_zero_coordinates() {
    let spec = Spec {
        gps_scale: Some(1e-7),
        ..Default::default()
    };
    let lon = data(&[
        (0., 0.),
        (1., 1_799_990_000.),
        (2., -1_799_990_000.),
        (3., 1e20),
    ]);
    let lat = data(&[(0., 0.), (1., 0.), (2., 0.), (3., 0.)]);
    let fix = data(&[(0., 0.), (1., 3.), (2., 3.), (3., 3.)]);
    let track = Track::prepare(&lon, &lat, Some(&fix), &spec).unwrap();
    assert_eq!(track.gps_origin, Some((179.999, 0.)));
    assert!(track.at(0.).is_none());
    assert!((track.at(2.).unwrap().x - 222.39016).abs() < 0.01);
    assert!(track.at(3.).is_none());
    let zero = Track::prepare(&data(&[(0., 0.)]), &data(&[(0., 0.)]), None, &spec).unwrap();
    assert_eq!(zero.gps_origin, Some((0., 0.)));
    let track = Track::prepare(
        &data(&[(0., 0.), (0.5, 1.), (1., 2.)]),
        &data(&[(0., 0.), (0.5, 1.), (1., 2.)]),
        Some(&data(&[(0., 2.), (1., 4.)])),
        &Spec {
            gps_scale: Some(1.),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        track.at(0.5).is_none(),
        "Fix quality is a step state, not an interpolated numeric value"
    );
}

#[test]
fn gps_detection_pairs_one_sender_and_handles_px4_degrees_and_integer_gps() {
    let strings = |items: &[&str]| {
        items
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    };
    assert!(
        gps_pair(&strings(&[
            "mavlink/1.1/GPS_RAW_INT.lon",
            "mavlink/2.1/GPS_RAW_INT.lat"
        ]))
        .is_none()
    );
    let spec = gps_pair(&strings(&[
        "mavlink/1.1/GPS_RAW_INT.lon",
        "mavlink/1.1/GPS_RAW_INT.lat",
        "mavlink/1.1/GPS_RAW_INT.fix_type",
    ]))
    .unwrap();
    assert_eq!(spec.gps_scale, Some(1e-7));
    assert!(!spec.fix.is_empty());
    let spec = gps_pair(&strings(&[
        "vehicle_global_position/0/lon",
        "vehicle_global_position/0/lat",
    ]));
    // ULog identities use slash-separated fields.
    assert_eq!(spec.unwrap().gps_scale, Some(1.));
}

#[test]
fn equal_scale_expands_bounds_preserving_center_and_screen_distances() {
    let ranges = equal_ranges(plot::Ranges::new((0., 10.), (-10., 10.)), 800., 400.);
    assert!((ranges.x.span() / 800. - ranges.y.span() / 400.).abs() < 1e-12);
    assert_eq!((ranges.x.min + ranges.x.max) / 2., 5.);
    assert_eq!((ranges.y.min + ranges.y.max) / 2., 0.);
}
