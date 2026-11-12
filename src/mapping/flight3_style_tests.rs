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
        2,
        "Marker-only update must not include flight or ground paths"
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
