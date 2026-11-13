use super::*;
#[test]
fn timeline_updates_keep_static_geometry_key_and_small_marker_layer() {
    let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
    let mut scene = Scene::default();
    scene.paths = prepare(
        &tracks,
        &lines,
        &BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    let camera = Camera {
        center: [0., 0.],
        zoom: 16.,
    };
    let orbit = super::super::terrain::Orbit::default();
    let size = [800., 600.];
    let key = geometry_key(&scene, camera, orbit, size);
    let mut ground = super::super::ground_path::Cache::default();
    ground.prepare(&scene.paths, &scene.tiles);
    scene.time = Some(1.);
    let mut detail = super::super::trace_detail::Cache::default();
    let before = vertices_layer(
        &scene,
        camera,
        orbit,
        size,
        Layer::Timeline,
        &mut ground,
        &mut detail,
    );
    assert_eq!(
        before.len(),
        6,
        "Marker-only update holds the marker and heading chevron, not flight or ground paths"
    );
    scene.time = Some(1.5);
    assert_eq!(key, geometry_key(&scene, camera, orbit, size));
    assert_ne!(
        before,
        vertices_layer(
            &scene,
            camera,
            orbit,
            size,
            Layer::Timeline,
            &mut ground,
            &mut detail
        )
    );
    let rotated = super::super::terrain::Orbit {
        bearing: orbit.bearing + 40.,
        ..orbit
    };
    assert_eq!(
        key,
        geometry_key(&scene, camera, rotated, size),
        "Solid orbit must reuse trace vertices"
    );
    Arc::make_mut(&mut scene.paths)[0].drawing.pattern = plot::line_draw::LinePattern::Dashed;
    assert_ne!(
        geometry_key(&scene, camera, orbit, size),
        geometry_key(&scene, camera, rotated, size),
        "Dash phase still tracks orbit projection"
    );
}
#[test]
fn state_tint_splits_3d_segments_without_blending_state_boundaries() {
    let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
    let mut tint = crate::state_tint::tests::fixture();
    let states = Arc::make_mut(&mut tint.states);
    let intervals = Arc::make_mut(&mut states.intervals);
    intervals[0].end = 1.5;
    intervals[1].start = 1.5;
    let colors = super::super::telemetry::prepare_styled(
        &tracks,
        &lines,
        &spec,
        &Default::default(),
        &Default::default(),
        vec![tint],
    );
    let mut scene = Scene::default();
    scene.paths = prepare(
        &tracks,
        &lines,
        &BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    scene.colors = Arc::new(colors);
    let geometry = vertices(
        &scene,
        Camera {
            center: [0., 0.],
            zoom: 16.,
        },
        super::super::terrain::Orbit::default(),
        [800., 600.],
    );
    assert_eq!(
        geometry.len(),
        20,
        "State transition creates eight path vertices; endpoint crosses retain twelve vertices"
    );
    assert_eq!(geometry[3][..3], geometry[4][..3]);
    assert_ne!(
        geometry[3][3..6],
        geometry[4][3..6],
        "State boundary must keep distinct colors on each side"
    );
    assert_eq!(geometry[2][3..6], geometry[3][3..6]);
    assert_eq!(geometry[4][3..6], geometry[5][3..6]);
}
#[test]
fn ground_projection_samples_terrain_breaks_gaps_and_connects_marker() {
    let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
    let paths = prepare(
        &tracks,
        &lines,
        &BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    let id = super::super::camera::TileId { z: 0, x: 0, y: 0 };
    let mut scene = Scene::default();
    scene.paths = paths;
    scene.time = Some(1.);
    scene.ground_projection = true;
    scene.tiles.push(super::super::terrain::Tile {
        id,
        dem_id: id,
        raster_id: id,
        provider: super::super::tiles::Provider::OpenStreetMap,
        raster: None,
        elevation: Some(Arc::new(super::super::tiles::Payload::Elevation(
            super::super::tiles::Elevation {
                version: 1,
                width: 2,
                height: 2,
                heights: vec![5.; 4],
            },
        ))),
    });
    let camera = Camera {
        center: [0., 0.],
        zoom: 16.,
    };
    let geometry = vertices(
        &scene,
        camera,
        super::super::terrain::Orbit::default(),
        [800., 600.],
    );
    let ground = geometry
        .iter()
        // Stored once as halos; the shader draws the core from each instance.
        .filter(|v| v[8] < 0. && v[7] == 3.5)
        .collect::<Vec<_>>();
    let ink = ground
        .chunks_exact(2)
        .map(|pair| f64::from((pair[1][0] - pair[0][0]).abs()))
        .sum::<f64>();
    assert!(
        (ink - 60.).abs() < 1e-4,
        "Ground display must keep complete path length"
    );
    assert!(
        ground.chunks_exact(2).all(
            |pair| pair[0][0].max(pair[1][0]) <= 30.001 || pair[0][0].min(pair[1][0]) >= 59.999
        ),
        "Ground display must preserve GPS gap"
    );
    assert!(ground.iter().all(|v| (v[1] - 5.5).abs() < 1e-6));
    let connector = geometry
        .iter()
        .filter(|v| v[7] == 1. && v[3..6] == [1., 1., 1.])
        .collect::<Vec<_>>();
    assert_eq!(connector.len(), 2);
    assert_eq!(connector[0][1], 5.5);
    assert_eq!(connector[1][1], 110.5);
    scene.ground_projection = false;
    assert!(
        !vertices(
            &scene,
            camera,
            super::super::terrain::Orbit::default(),
            [800., 600.]
        )
        .iter()
        .any(|v| v[7] == 1.5)
    );
}
#[test]
fn telemetry_colors_reach_both_ends_of_3d_segments() {
    let (lines, mut spec, tracks) = super::super::telemetry::tests::fixture();
    spec.map.as_mut().unwrap().coloring = super::super::telemetry::ColorBy::Speed;
    let colors = super::super::telemetry::prepare(&tracks, &lines, &spec, &Default::default());
    let paths = prepare(
        &tracks,
        &lines,
        &BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    let mut scene = Scene::default();
    scene.paths = paths;
    scene.colors = Arc::new(colors);
    let geometry = vertices(
        &scene,
        Camera {
            center: [0., 0.],
            zoom: 16.,
        },
        super::super::terrain::Orbit::default(),
        [800., 600.],
    );
    let pair = &geometry[2..4];
    assert_ne!(
        pair[0][3..6],
        pair[1][3..6],
        "Gradient must reach GPU as two endpoint colors"
    );
}

#[test]
fn timeline_heading_follows_climb_descent_and_vertical_travel() {
    let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
    let paths = prepare(
        &tracks,
        &lines,
        &BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    for (dx, dz) in [(20., 40.), (20., -40.), (20., 0.), (0., 40.), (0., -40.)] {
        let mut path = paths[0].clone();
        path.times = Arc::from([0., 1., 2.]);
        path.points = Arc::from([
            [0., 0., 100.],
            [dx, 0., 100. + dz],
            [dx * 2., 0., 100. + dz * 2.],
        ]);
        let point = path.at(1.).unwrap();
        let heading = heading_chevron(&path, 1., point, 1., 1.).unwrap();
        let tip = heading[1];
        let back = [
            (heading[0][0] + heading[2][0]) / 2.,
            (heading[0][1] + heading[2][1]) / 2.,
            (heading[0][2] + heading[2][2]) / 2.,
        ];
        assert!(((tip[0] - back[0]) * dz - (tip[2] - back[2]) * dx).abs() < 1e-8);
        assert_eq!((tip[2] - point[2]).total_cmp(&0.), dz.total_cmp(&0.));
        assert!(heading.iter().flatten().all(|v| v.is_finite()));
    }
    assert!(chevron([0.; 3], [0.; 3], 1., 8., 7.).is_none());
}

#[test]
fn direction_arrows_use_separate_color_and_toggle_without_changing_line() {
    let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
    let paths = prepare(
        &tracks,
        &lines,
        &BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    let mut path = paths[0].clone();
    path.points = Arc::from([[0., 0., 100.], [1000., 0., 200.]]);
    path.times = Arc::from([0., 10.]);
    path.color = [50, 80, 120, 255];
    path.arrow_color = [18, 171, 205, 255];
    let mut scene = Scene::default();
    scene.paths = Arc::from([path.clone()]);
    let camera = Camera {
        center: [0., 0.],
        zoom: 16.,
    };
    let orbit = super::super::terrain::Orbit::default();
    let size = [800., 600.];
    let draw = |scene: &Scene| {
        vertices_layer(
            scene,
            camera,
            orbit,
            size,
            Layer::Trace,
            &mut Default::default(),
            &mut Default::default(),
        )
    };
    let before = draw(&scene);
    let key = geometry_key(&scene, camera, orbit, size);
    path.arrows = true;
    scene.paths = Arc::from([path]);
    assert_ne!(key, geometry_key(&scene, camera, orbit, size));
    let after = draw(&scene);
    assert!(after.len() > before.len());
    let rgb = [18., 171., 205.].map(|v| v / 255.);
    assert!(
        after
            .iter()
            .any(|v| v[3..6].iter().zip(rgb).all(|(a, b)| (a - b).abs() < 1e-6))
    );
    assert!(
        !before
            .iter()
            .any(|v| v[3..6].iter().zip(rgb).all(|(a, b)| (a - b).abs() < 1e-6))
    );
}

#[test]
fn arrow_color_roundtrips_and_old_presets_get_default() {
    let spec = crate::trajectory::Spec {
        arrow_color: [18, 171, 205, 255],
        ..Default::default()
    };
    let mut json = serde_json::to_value(&spec).unwrap();
    assert_eq!(
        serde_json::from_value::<crate::trajectory::Spec>(json.clone()).unwrap(),
        spec
    );
    json.as_object_mut().unwrap().remove("arrow_color");
    assert_eq!(
        serde_json::from_value::<crate::trajectory::Spec>(json)
            .unwrap()
            .arrow_color,
        crate::trajectory::arrow_color()
    );
}

#[test]
fn each_3d_line_keeps_its_arrow_color_and_state_markers() {
    let _zone = zgui::reactive::enter_non_reactive_zone();
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let pane = workbench.pane();
    let (lines, spec, tracks) = super::super::telemetry::tests::fixture();
    let prepared = prepare(
        &tracks,
        &lines,
        &BTreeMap::from([("y".into(), spec.altitudes[&0].clone())]),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    pane.trajectory.set(Some(spec));
    let tint = crate::state_tint::tests::fixture();
    let mut band =
        crate::geometry::Geometry::new(99, crate::geometry::GeometryKind::Band, kurbo::Point::ZERO);
    band.states = Some(tint.states);
    workbench.geometry.items.update(|items| items.push(band));
    workbench.attach_geometry(99, pane, true);
    pane.trace_colors.set(BTreeMap::from([
        (
            2,
            crate::trace_color::TraceColor {
                arrow_color: Some([18, 171, 205, 255]),
                state_markers: false,
                ..Default::default()
            },
        ),
        (
            5,
            crate::trace_color::TraceColor {
                arrow_color: Some([238, 51, 68, 255]),
                ..Default::default()
            },
        ),
    ]));
    let mut first = prepared[0].clone();
    first.points = Arc::from([[0., 0., 100.], [1000., 0., 200.]]);
    first.times = Arc::from([0., 10.]);
    first.arrows = true;
    let mut second = first.clone();
    second.id = 5;
    let mut paths = Arc::from([first, second]);
    decorate(workbench, pane, &mut paths);
    assert_eq!(paths[0].arrow_color, [18, 171, 205, 255]);
    assert_eq!(paths[1].arrow_color, [238, 51, 68, 255]);
    assert!(paths[0].modes.is_empty());
    assert!(!paths[1].modes.is_empty());
    let mut scene = Scene::default();
    scene.paths = paths;
    let geometry = vertices_layer(
        &scene,
        Camera {
            center: [0., 0.],
            zoom: 16.,
        },
        super::super::terrain::Orbit::default(),
        [800., 600.],
        Layer::Trace,
        &mut Default::default(),
        &mut Default::default(),
    );
    for color in [[18., 171., 205.], [238., 51., 68.]] {
        let rgb = color.map(|c| c / 255.);
        assert!(
            geometry
                .iter()
                .any(|v| v[3..6].iter().zip(rgb).all(|(a, b)| (a - b).abs() < 1e-6))
        );
    }
}
