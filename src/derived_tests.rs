use crate::{
    derived::{self, FilterSpec},
    state::Workbench,
};
use plot::time_series::SeriesData;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use zgui::prelude::*;
use zgui_testkit_view::Window;

fn fixture() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/esc_status_log.ulg"
    ))
}
fn settle(window: &Window, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        window.frame();
        if ready() {
            return;
        }
        assert!(Instant::now() < deadline, "Worker did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn loaded(window: &Window) -> (Workbench, String) {
    let workbench = window.scope.with(Workbench::new);
    window.scope.with(|| workbench.open(fixture()));
    settle(window, || !workbench.loading.get_untracked());
    let source = workbench
        .log
        .get_untracked()
        .unwrap()
        .catalogue
        .iter()
        .find(|field| field.id.contains("esc_rpm"))
        .unwrap()
        .id
        .clone();
    (workbench, source)
}
fn same(a: &SeriesData, b: &SeriesData) {
    assert_eq!(a.samples().len(), b.samples().len());
    for (a, b) in a.samples().iter().zip(b.samples()) {
        assert_eq!(a.x, b.x);
        assert!(a.y == b.y || a.y.is_nan() && b.y.is_nan());
    }
}

#[test]
fn new_mode_can_use_saved_outputs_and_edit_mode_only_checks_its_own_targets() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    workbench.edit_filter(None, None);
    workbench.set_filter_kind("magnitude".into());
    workbench
        .accept_filter_sources(vec![source.clone()])
        .unwrap();
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let first = workbench.filter_outputs.get_untracked()[0];
    assert!(workbench.filter_mode.get_untracked().is_new());
    assert!(workbench.filter_inputs.get_untracked().is_empty());
    workbench.toggle_filter_editing(first);
    let before = workbench.filter_inputs.get_untracked();
    assert!(
        workbench
            .accept_filter_sources(vec![derived::key(first)])
            .is_err()
    );
    assert_eq!(workbench.filter_inputs.get_untracked(), before);
    workbench.edit_filter(None, None);
    workbench.set_filter_kind("magnitude".into());
    // Creation must ignore old result IDs, even if a caller retained them for plotting.
    workbench.filter_outputs.set(vec![first]);
    workbench
        .accept_filter_sources(vec![derived::key(first)])
        .unwrap();
    assert_eq!(workbench.filter_outputs.get_untracked(), [first]);
    assert!(workbench.filter_mode.get_untracked().is_new());
    assert_eq!(
        workbench.filter_inputs.get_untracked()[0].source,
        derived::key(first)
    );
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    let second = workbench.filter_outputs.get_untracked()[0];
    assert_ne!(first, second);
    workbench.next_filters();
    assert_eq!(workbench.filter_kind.get_untracked(), "magnitude");
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    let fields = workbench.derived.get_untracked();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].sources, [source]);
    assert_eq!(fields[1].sources, [derived::key(first)]);
    assert_eq!(fields[2].sources, [derived::key(second)]);
    assert!(
        workbench
            .upsert_filter(
                Some(first),
                "True cycle".into(),
                fields[2].key(),
                FilterSpec::LowPass(5.0)
            )
            .is_err()
    );
}

#[test]
fn quaternion_slots_keep_sparse_assignments_and_removal_does_not_shift_roles() {
    let window = Window::open();
    let (workbench, _) = loaded(&window);
    let sources: Vec<_> = workbench
        .log
        .get_untracked()
        .unwrap()
        .catalogue
        .iter()
        .take(4)
        .map(|field| field.id.clone())
        .collect();
    assert_eq!(sources.len(), 4);
    workbench.edit_filter(None, None);
    workbench.set_filter_kind("rpy".into());
    workbench
        .accept_filter_sources_at(vec![sources[3].clone()], Some(3))
        .unwrap();
    let inputs = workbench.filter_inputs.get_untracked();
    assert!(inputs[..3].iter().all(|input| input.source.is_empty()));
    assert_eq!(inputs[3].source, sources[3]);
    workbench
        .accept_filter_sources_at(vec![sources[0].clone()], Some(0))
        .unwrap();
    workbench
        .accept_filter_sources_at(vec![sources[1].clone()], Some(1))
        .unwrap();
    workbench.save_filter();
    assert!(workbench.filter_error.get_untracked().contains("fill W"));
    assert!(workbench.derived.get_untracked().is_empty());
    workbench
        .accept_filter_sources_at(vec![sources[2].clone()], Some(2))
        .unwrap();
    workbench.remove_filter_input(1);
    let inputs = workbench.filter_inputs.get_untracked();
    assert_eq!(inputs[0].source, sources[0]);
    assert!(inputs[1].source.is_empty());
    assert_eq!(inputs[2].source, sources[2]);
    assert_eq!(inputs[3].source, sources[3]);
    workbench
        .accept_filter_sources_at(vec![sources[1].clone()], Some(1))
        .unwrap();
    workbench
        .accept_filter_sources_at(vec![sources[3].clone()], Some(0))
        .unwrap();
    let inputs = workbench.filter_inputs.get_untracked();
    assert_eq!(inputs[0].source, sources[3]);
    assert_eq!(inputs[3].source, sources[0]);
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    assert_eq!(workbench.filter_outputs.get_untracked().len(), 3);
}

#[test]
fn magnitude_tracks_all_parents_and_rejects_nonfirst_dependency_cycles() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    let first = workbench
        .upsert_filter(
            None,
            "First input".into(),
            source.clone(),
            FilterSpec::Affine {
                gain: 1.0,
                offset: 0.0,
            },
        )
        .unwrap();
    let second = workbench
        .upsert_filter(
            None,
            "Second input".into(),
            source.clone(),
            FilterSpec::Affine {
                gain: 2.0,
                offset: 0.0,
            },
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    workbench.edit_filter(None, None);
    workbench.filter_kind.set("magnitude".into());
    workbench
        .accept_filter_sources(vec![derived::key(first), derived::key(second)])
        .unwrap();
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    let magnitude = workbench.filter_outputs.get_untracked()[0];
    workbench.add(derived::key(magnitude));
    let before = workbench.pane().selection().unwrap();
    workbench
        .upsert_filter(
            Some(second),
            "Changed second input".into(),
            source.clone(),
            FilterSpec::Affine {
                gain: 3.0,
                offset: 0.0,
            },
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let fields = workbench.derived.get_untracked();
    let parents: Vec<_> = [first, second]
        .iter()
        .map(|id| {
            fields
                .iter()
                .find(|field| field.id == *id)
                .unwrap()
                .data
                .as_ref()
                .unwrap()
        })
        .collect();
    same(
        &workbench.pane().selection().unwrap().data,
        &FilterSpec::Magnitude.apply_inputs(&parents).unwrap(),
    );
    assert!(workbench.pane().selection().unwrap().revision > before.revision);
    assert!(
        workbench
            .upsert_filter(
                Some(second),
                "Cycle".into(),
                derived::key(magnitude),
                FilterSpec::LowPass(5.0)
            )
            .is_err()
    );
    assert!(workbench.delete_filter(second).is_err());
    workbench.edit_filter(Some(magnitude), None);
    workbench.filter_names.update(|names| names[0] = " ".into());
    workbench.save_filter();
    assert!(!workbench.filter_error.get_untracked().is_empty());
    assert_eq!(workbench.derived.get_untracked().len(), 3);
    assert_eq!(workbench.filter_outputs.get_untracked(), [magnitude]);
}

#[test]
fn quaternion_creates_named_group_edits_in_place_and_chains_all_outputs() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    let mut sources = Vec::new();
    for (index, offset) in [0.8, 0.2, -0.4, 0.4].into_iter().enumerate() {
        let id = workbench
            .upsert_filter(
                None,
                format!("q{index}"),
                source.clone(),
                FilterSpec::Affine { gain: 0.0, offset },
            )
            .unwrap();
        sources.push(derived::key(id));
    }
    settle(&window, || !workbench.derived_busy.get_untracked());
    workbench.edit_filter(None, None);
    workbench.filter_kind.set("rpy".into());
    workbench
        .accept_filter_sources(sources[..3].to_vec())
        .unwrap();
    workbench.save_filter();
    assert!(workbench.filter_error.get_untracked().contains("four"));
    assert_eq!(workbench.derived.get_untracked().len(), 4);
    workbench
        .accept_filter_sources(vec![sources[3].clone()])
        .unwrap();
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    let ids = workbench.filter_outputs.get_untracked();
    assert_eq!(ids.len(), 3);
    let fields = workbench.derived.get_untracked();
    for (index, expected) in [0.0, (-0.8_f64).asin(), 0.48_f64.atan2(0.36)]
        .into_iter()
        .enumerate()
    {
        let field = fields.iter().find(|field| field.id == ids[index]).unwrap();
        assert_eq!(field.sources, sources);
        let value = field
            .data
            .as_ref()
            .unwrap()
            .samples()
            .iter()
            .find(|point| point.y.is_finite())
            .unwrap()
            .y;
        assert!((value - expected).abs() < 1e-12);
    }
    workbench.edit_created_filters();
    workbench.filter_angle_unit.set("deg".into());
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    assert_eq!(workbench.filter_outputs.get_untracked(), ids);
    let fields = workbench.derived.get_untracked();
    for (index, expected) in [0.0, (-0.8_f64).asin(), 0.48_f64.atan2(0.36)]
        .into_iter()
        .enumerate()
    {
        let field = fields.iter().find(|field| field.id == ids[index]).unwrap();
        assert!(field.filter.label().contains("deg"));
        let value = field
            .data
            .as_ref()
            .unwrap()
            .samples()
            .iter()
            .find(|point| point.y.is_finite())
            .unwrap()
            .y;
        assert!((value - expected.to_degrees()).abs() < 1e-12);
    }
    workbench.filter_angle_unit.set("rad".into());
    workbench.edit_filter(Some(ids[2]), None);
    assert_eq!(workbench.filter_angle_unit.get_untracked(), "deg");
    assert_eq!(workbench.filter_outputs.get_untracked(), ids);
    assert_eq!(workbench.filter_inputs.get_untracked().len(), 4);
    workbench
        .filter_names
        .update(|names| names[3] = "Heading".into());
    workbench.move_filter_input(&sources[3], -1);
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    assert_eq!(workbench.filter_outputs.get_untracked(), ids);
    let fields = workbench.derived.get_untracked();
    let yaw = fields.iter().find(|field| field.id == ids[2]).unwrap();
    assert_eq!(yaw.name, "Heading");
    assert_eq!(
        yaw.sources,
        [
            sources[0].clone(),
            sources[1].clone(),
            sources[3].clone(),
            sources[2].clone()
        ]
    );
    assert_eq!(fields.len(), 7);
    assert!(
        workbench
            .upsert_filter(
                Some(ids[0]),
                "Cycle".into(),
                derived::key(ids[0]),
                FilterSpec::LowPass(5.0)
            )
            .is_err()
    );
    workbench.next_filters();
    assert_eq!(workbench.filter_kind.get_untracked(), "low");
    assert_eq!(workbench.filter_inputs.get_untracked().len(), 3);
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    assert_eq!(workbench.derived.get_untracked().len(), 10);
    assert!(workbench.delete_filter(ids[0]).is_err());
}
#[test]
fn named_chains_recompute_plotted_copies_without_changing_raw_data_or_appearance() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    window.scope.with(|| workbench.add(source.clone()));
    settle(&window, || {
        !workbench.pane().lines.get_untracked().is_empty()
    });
    let raw = workbench.pane().selection().unwrap();
    let scaled = workbench
        .upsert_filter(
            None,
            "Scaled RPM".into(),
            source.clone(),
            FilterSpec::Affine {
                gain: 2.0,
                offset: 1.0,
            },
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let smooth = workbench
        .upsert_filter(
            None,
            "Smooth RPM".into(),
            derived::key(scaled),
            FilterSpec::LowPass(5.0),
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let high = workbench
        .upsert_filter(
            None,
            "Fast RPM".into(),
            derived::key(smooth),
            FilterSpec::HighPass(2.0),
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    workbench.add(derived::key(high));
    let first = workbench.pane();
    let plotted = first.selection().unwrap();
    first.plot.styles.update(|styles| {
        let mut style = plot::plot_data::SeriesStyle::for_id(&plotted.id.to_string());
        style.width = 7.0;
        styles.insert(plotted.id.to_string(), style);
    });
    let second = workbench.add_plot(first.id, false, false);
    workbench.add(derived::key(high));
    assert_eq!(
        second.selection().unwrap().data.samples().as_ptr(),
        plotted.data.samples().as_ptr()
    );
    workbench
        .upsert_filter(
            Some(scaled),
            "Calibrated RPM".into(),
            source.clone(),
            FilterSpec::Affine {
                gain: 3.0,
                offset: -2.0,
            },
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let expected = FilterSpec::HighPass(2.0)
        .apply(
            &FilterSpec::LowPass(5.0)
                .apply(
                    &FilterSpec::Affine {
                        gain: 3.0,
                        offset: -2.0,
                    }
                    .apply(&raw.data)
                    .unwrap(),
                )
                .unwrap(),
        )
        .unwrap();
    same(&first.selection().unwrap().data, &expected);
    same(&second.selection().unwrap().data, &expected);
    assert_eq!(first.selection().unwrap().label(), "Fast RPM");
    assert_eq!(first.plot.style(&plotted.id.to_string()).width, 7.0);
    assert!(first.selection().unwrap().revision > plotted.revision);
    assert_eq!(
        first.lines.get_untracked()[0].data.samples().as_ptr(),
        raw.data.samples().as_ptr()
    );
    workbench
        .upsert_filter(
            Some(high),
            "High-frequency RPM".into(),
            derived::key(smooth),
            FilterSpec::HighPass(2.0),
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert_eq!(first.selection().unwrap().label(), "High-frequency RPM");
    assert_eq!(second.selection().unwrap().label(), "High-frequency RPM");
    first.remove_line(plotted.id);
    assert_eq!(workbench.derived.get_untracked().len(), 3);
}
#[test]
fn invalid_names_parameters_and_cycles_leave_graph_intact_and_deletion_respects_dependencies() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    let first = workbench
        .upsert_filter(
            None,
            "First".into(),
            source.clone(),
            FilterSpec::LowPass(5.0),
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let second = workbench
        .upsert_filter(
            None,
            "Second".into(),
            derived::key(first),
            FilterSpec::HighPass(1.0),
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(
        workbench
            .upsert_filter(
                None,
                " first ".into(),
                source.clone(),
                FilterSpec::LowPass(2.0)
            )
            .is_err()
    );
    assert!(
        workbench
            .upsert_filter(None, " ".into(), source.clone(), FilterSpec::LowPass(2.0))
            .is_err()
    );
    assert!(
        workbench
            .upsert_filter(None, "Bad".into(), source.clone(), FilterSpec::LowPass(0.0))
            .is_err()
    );
    assert!(
        workbench
            .upsert_filter(
                Some(first),
                "First".into(),
                derived::key(second),
                FilterSpec::LowPass(2.0)
            )
            .is_err()
    );
    assert_eq!(workbench.derived.get_untracked()[0].sources, [source]);
    assert_eq!(workbench.derived.get_untracked().len(), 2);
    assert!(workbench.delete_filter(first).is_err());
    workbench.add(derived::key(second));
    workbench.delete_filter(second).unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.pane().lines.get_untracked().is_empty());
    workbench.delete_filter(first).unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.derived.get_untracked().is_empty());
}
#[test]
fn latest_filter_edit_wins_and_reload_discards_old_chain_workers() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    let id = workbench
        .upsert_filter(
            None,
            "Output".into(),
            source.clone(),
            FilterSpec::LowPass(5.0),
        )
        .unwrap();
    workbench
        .upsert_filter(
            Some(id),
            "Latest".into(),
            source.clone(),
            FilterSpec::Affine {
                gain: 4.0,
                offset: 3.0,
            },
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let raw = workbench
        .log
        .get_untracked()
        .unwrap()
        .prepare(&source)
        .unwrap();
    let field = workbench.derived.get_untracked().remove(0);
    assert_eq!(field.name, "Latest");
    same(
        field.data.as_ref().unwrap(),
        &FilterSpec::Affine {
            gain: 4.0,
            offset: 3.0,
        }
        .apply(&raw)
        .unwrap(),
    );
    workbench
        .upsert_filter(
            Some(id),
            "Old worker".into(),
            source,
            FilterSpec::LowPass(2.0),
        )
        .unwrap();
    workbench.open(fixture());
    settle(&window, || !workbench.loading.get_untracked());
    for _ in 0..10 {
        window.frame();
    }
    assert!(workbench.derived.get_untracked().is_empty());
    assert!(!workbench.derived_busy.get_untracked());
}
#[test]
fn high_pass_and_affine_preserve_timestamps_and_gaps() {
    let raw = SeriesData::new(vec![
        plot::PlotPoint { x: 0.0, y: 4.0 },
        plot::PlotPoint { x: 0.1, y: 6.0 },
        plot::PlotPoint {
            x: 0.2,
            y: f64::NAN,
        },
        plot::PlotPoint { x: 0.3, y: 8.0 },
    ])
    .unwrap();
    let high = FilterSpec::HighPass(2.0).apply(&raw).unwrap();
    assert_eq!(high.samples()[0].y, 0.0);
    assert!(high.samples()[1].y > 0.0);
    assert!(high.samples()[2].y.is_nan());
    assert_eq!(high.samples()[3].y, 0.0);
    let affine = FilterSpec::Affine {
        gain: 2.0,
        offset: 1.0,
    }
    .apply(&raw)
    .unwrap();
    for (a, b) in raw.samples().iter().zip(affine.samples()) {
        assert_eq!(a.x, b.x);
    }
    assert_eq!(affine.samples()[0].y, 9.0);
    assert!(affine.samples()[2].y.is_nan());
    assert!(
        FilterSpec::Affine {
            gain: f64::INFINITY,
            offset: 0.0
        }
        .apply(&raw)
        .is_err()
    );
}

#[test]
fn failed_log_load_keeps_pending_derived_field_work_and_unrelated_stages_cached() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    let first = workbench
        .upsert_filter(
            None,
            "First".into(),
            source.clone(),
            FilterSpec::LowPass(5.0),
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let before = workbench.derived.get_untracked()[0].data.clone().unwrap();
    let second = workbench
        .upsert_filter(
            None,
            "Second".into(),
            derived::key(first),
            FilterSpec::HighPass(2.0),
        )
        .unwrap();
    workbench.open(PathBuf::from("missing-filter-test.ulg"));
    settle(&window, || {
        !workbench.loading.get_untracked() && !workbench.derived_busy.get_untracked()
    });
    assert!(workbench.log.get_untracked().is_some());
    let fields = workbench.derived.get_untracked();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields[0].data.as_ref().unwrap().samples().as_ptr(),
        before.samples().as_ptr()
    );
    same(
        fields[1].data.as_ref().unwrap(),
        &FilterSpec::HighPass(2.0).apply(&before).unwrap(),
    );
    let second_before = fields[1].data.clone().unwrap();
    workbench
        .upsert_filter(
            None,
            "Unrelated".into(),
            source,
            FilterSpec::Affine {
                gain: 1.0,
                offset: 2.0,
            },
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert_eq!(
        workbench
            .derived
            .get_untracked()
            .iter()
            .find(|field| field.id == second)
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .samples()
            .as_ptr(),
        second_before.samples().as_ptr()
    );
}

#[test]
fn batches_share_settings_keep_names_and_ids_and_chain_all_outputs() {
    let window = Window::open();
    let (workbench, first) = loaded(&window);
    let second = workbench
        .log
        .get_untracked()
        .unwrap()
        .catalogue
        .iter()
        .find(|field| field.id != first)
        .unwrap()
        .id
        .clone();
    workbench.edit_filter(None, None);
    assert!(workbench.filter_inputs.get_untracked().is_empty());
    workbench
        .accept_filter_sources(vec![first.clone(), second.clone(), first.clone()])
        .unwrap();
    assert_eq!(workbench.filter_inputs.get_untracked().len(), 2);
    workbench.filter_inputs.update(|inputs| {
        inputs[0].name = "First calibrated".into();
        inputs[1].name = "Second calibrated".into();
    });
    workbench.filter_kind.set("affine".into());
    workbench.filter_gain.set("2".into());
    workbench.filter_offset.set("3".into());
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    let ids = workbench.filter_outputs.get_untracked();
    assert_eq!(ids.len(), 2);
    let fields = workbench.derived.get_untracked();
    for (field, source) in fields.iter().zip([&first, &second]) {
        let raw = workbench
            .log
            .get_untracked()
            .unwrap()
            .prepare(source)
            .unwrap();
        same(
            field.data.as_ref().unwrap(),
            &FilterSpec::Affine {
                gain: 2.0,
                offset: 3.0,
            }
            .apply(&raw)
            .unwrap(),
        );
    }
    workbench.edit_created_filters();
    workbench.filter_gain.set("4".into());
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert_eq!(workbench.filter_outputs.get_untracked(), ids);
    assert_eq!(workbench.derived.get_untracked().len(), 2);
    assert!(
        workbench
            .derived
            .get_untracked()
            .iter()
            .all(|field| field.filter
                == FilterSpec::Affine {
                    gain: 4.0,
                    offset: 3.0
                })
    );
    workbench.next_filters();
    assert_eq!(
        workbench
            .filter_inputs
            .get_untracked()
            .iter()
            .map(|input| input.source.clone())
            .collect::<Vec<_>>(),
        ids.iter().map(|id| derived::key(*id)).collect::<Vec<_>>()
    );
    workbench.filter_kind.set("low".into());
    workbench.filter_cutoff.set("5".into());
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let children = workbench.filter_outputs.get_untracked();
    assert_eq!(children.len(), 2);
    let fields = workbench.derived.get_untracked();
    for (child, parent) in children.iter().zip(&ids) {
        let parent = fields.iter().find(|field| field.id == *parent).unwrap();
        let child = fields.iter().find(|field| field.id == *child).unwrap();
        same(
            child.data.as_ref().unwrap(),
            &FilterSpec::LowPass(5.0)
                .apply(parent.data.as_ref().unwrap())
                .unwrap(),
        );
    }
}

#[test]
fn invalid_batch_is_atomic_and_edit_cycle_does_not_change_mode() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    let result = workbench.apply_filter_batch(
        None,
        vec![
            derived::FilterInput {
                source: source.clone(),
                name: "Same".into(),
            },
            derived::FilterInput {
                source: source.clone(),
                name: "same".into(),
            },
        ],
        FilterSpec::LowPass(5.0),
    );
    assert!(result.is_err());
    assert!(workbench.derived.get_untracked().is_empty());
    assert!(!workbench.derived_busy.get_untracked());
    let parent = workbench
        .upsert_filter(None, "Parent".into(), source, FilterSpec::LowPass(5.0))
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    let child = workbench
        .upsert_filter(
            None,
            "Child".into(),
            derived::key(parent),
            FilterSpec::LowPass(2.0),
        )
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    workbench.edit_filter(Some(parent), None);
    let mode = workbench.filter_mode.get_untracked();
    let before = workbench.filter_inputs.get_untracked();
    assert!(
        workbench
            .accept_filter_sources(vec![derived::key(child)])
            .is_err()
    );
    assert_eq!(workbench.filter_mode.get_untracked(), mode);
    assert_eq!(workbench.filter_inputs.get_untracked(), before);
    workbench.clear_filter_sources();
    workbench
        .accept_filter_sources(vec![before[0].source.clone()])
        .unwrap();
    assert_eq!(workbench.filter_inputs.get_untracked()[0].name, "Parent");
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert_eq!(workbench.filter_outputs.get_untracked(), [parent]);
    workbench.edit_filter(None, None);
    workbench
        .accept_filter_sources(vec![derived::key(child)])
        .unwrap();
    assert!(workbench.filter_mode.get_untracked().is_new());
    assert!(workbench.filter_outputs.get_untracked().is_empty());
    assert_eq!(
        workbench.filter_inputs.get_untracked()[0].source,
        derived::key(child)
    );
    workbench.save_filter();
    settle(&window, || !workbench.derived_busy.get_untracked());
    assert!(workbench.filter_error.get_untracked().is_empty());
    assert_eq!(workbench.derived.get_untracked().len(), 3);
}

#[test]
fn legend_filter_handoff_starts_new_batch_with_raw_and_derived_sources() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    let derived = window
        .scope
        .with(|| {
            workbench.upsert_filter(
                None,
                "Stage one".into(),
                source.clone(),
                FilterSpec::LowPass(5.0),
            )
        })
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    window.scope.with(|| workbench.add(source.clone()));
    settle(&window, || {
        workbench
            .pane()
            .lines
            .with_untracked(|lines| lines.len() == 1)
    });
    window.scope.with(|| workbench.add(derived::key(derived)));
    let pane = workbench.pane();
    assert_eq!(pane.lines.get_untracked().len(), 2);
    workbench.edit_filter(Some(derived), None);
    let closed = window.scope.with(|| RwSignal::new_local(false));
    use crate::editor::EditorProps;
    use zgui_ui_tokens::prelude::*;
    let mut built = window.scope.with(|| view! { ThemeProvider(scheme = ColorScheme::Dark) {
        Editor(workbench = workbench, pane = pane, on_filters = UnsyncCallback::new(move |()| closed.set(true)))
    } }.into_view().build(&mut window.cx.cx()));
    built.mount(&window.dom_handle, window.root, None);
    window.frame();
    let mut pending = vec![window.root];
    let button = loop {
        let node = pending.pop().expect("Add filters button");
        if window
            .dom
            .tree()
            .classes(node)
            .contains(&zgui::view::ClassName::new("legend-add-filters"))
        {
            break node;
        }
        pending.extend(window.dom.tree().children(node));
    };
    window.dispatcher().key(
        button,
        zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter),
    );
    window.frame();
    assert!(
        closed.get_untracked(),
        "Filter action must close line settings"
    );
    assert!(workbench.filters_open.get_untracked());
    assert!(workbench.filter_mode.get_untracked().is_new());
    assert!(workbench.filter_outputs.get_untracked().is_empty());
    assert!(workbench.filter_error.get_untracked().is_empty());
    let inputs = workbench.filter_inputs.get_untracked();
    assert_eq!(
        inputs
            .iter()
            .map(|input| input.source.clone())
            .collect::<Vec<_>>(),
        [source, derived::key(derived)]
    );
    assert_eq!(inputs[1].name, "Stage one filtered");
    assert_eq!(
        workbench.derived.get_untracked().len(),
        1,
        "Opening editor must not apply filter yet"
    );
}

#[test]
#[ignore = "requires rustc with wasm32-unknown-unknown installed"]
fn script_e2e_editor_saves_groups_atomically_and_recomputes_descendants() {
    let window = Window::open();
    let (workbench, source) = loaded(&window);
    workbench.edit_filter(None, None);
    workbench.set_filter_kind("rust".into());
    workbench
        .accept_filter_sources(vec![source.clone()])
        .unwrap();
    workbench.script_names.set("Double\nTriple".into());
    let code = r#"pub const OUTPUTS: usize = 2;
#[derive(Default)]
pub struct State;
pub fn filter(time: &[f64], inputs: &[&[f64]], outputs: &mut [&mut [f64]], _state: &mut State) {
    for i in 0..time.len() {
        outputs[0][i] = inputs[0][i] * 2.0; outputs[1][i] = inputs[0][i] * 3.0;
    }
}
"#
    .to_owned();
    workbench.script_source.set(code.clone());
    window.scope.with(|| workbench.save_filter());
    assert!(workbench.script_busy.get_untracked());
    assert!(
        workbench.derived.get_untracked().is_empty(),
        "No partial save before worker finishes"
    );
    settle(&window, || !workbench.script_busy.get_untracked());
    assert!(
        workbench.filter_error.get_untracked().is_empty(),
        "{}",
        workbench.filter_error.get_untracked()
    );
    let ids = workbench.filter_outputs.get_untracked();
    assert_eq!(ids.len(), 2);
    assert!(workbench.filter_inputs.get_untracked().is_empty());
    let raw = workbench
        .log
        .get_untracked()
        .unwrap()
        .prepare(&source)
        .unwrap();
    let fields = workbench.derived.get_untracked();
    assert_eq!(fields[0].group, fields[1].group);
    same(
        fields[1].data.as_ref().unwrap(),
        &FilterSpec::Affine {
            gain: 3.0,
            offset: 0.0,
        }
        .apply(&raw)
        .unwrap(),
    );

    let child = window
        .scope
        .with(|| {
            workbench.upsert_filter(
                None,
                "Child".into(),
                derived::key(ids[0]),
                FilterSpec::Affine {
                    gain: 10.0,
                    offset: 0.0,
                },
            )
        })
        .unwrap();
    settle(&window, || !workbench.derived_busy.get_untracked());
    workbench.edit_filter(Some(ids[1]), None);
    assert_eq!(workbench.script_source.get_untracked(), code);
    assert_eq!(workbench.script_names.get_untracked(), "Double\nTriple");
    assert_eq!(workbench.filter_mode.get_untracked().ids(), ids);
    assert!(
        workbench
            .accept_filter_sources(vec![derived::key(child)])
            .is_err()
    );
    let before = workbench.derived.get_untracked();
    workbench.script_source.set("invalid Rust".into());
    window.scope.with(|| workbench.save_filter());
    settle(&window, || !workbench.script_busy.get_untracked());
    assert!(
        workbench
            .filter_error
            .get_untracked()
            .contains("compilation failed")
    );
    let after = workbench.derived.get_untracked();
    for (before, after) in before.iter().zip(&after) {
        assert_eq!(before.filter, after.filter);
        same(before.data.as_ref().unwrap(), after.data.as_ref().unwrap());
    }
    workbench.script_source.set(code.replace("* 2.0", "* 4.0"));
    window.scope.with(|| workbench.save_filter());
    settle(&window, || !workbench.script_busy.get_untracked());
    assert!(
        workbench.filter_error.get_untracked().is_empty(),
        "{}",
        workbench.filter_error.get_untracked()
    );
    assert_eq!(workbench.filter_outputs.get_untracked(), ids);
    let fields = workbench.derived.get_untracked();
    let child = fields.iter().find(|field| field.id == child).unwrap();
    same(
        child.data.as_ref().unwrap(),
        &FilterSpec::Affine {
            gain: 40.0,
            offset: 0.0,
        }
        .apply(&raw)
        .unwrap(),
    );

    // Edit inputs and grow saved group without changing existing IDs.
    let extra_source = workbench
        .log
        .get_untracked()
        .unwrap()
        .catalogue
        .iter()
        .find(|field| field.id != source)
        .unwrap()
        .id
        .clone();
    workbench
        .accept_filter_sources(vec![extra_source.clone()])
        .unwrap();
    workbench.add_script_output();
    workbench.rename_script_output(2, "Extra".into());
    let three = code
        .replace("OUTPUTS: usize = 2", "OUTPUTS: usize = 3")
        .replace("* 2.0", "* 4.0")
        .replace(
            "outputs[1][i] = inputs[0][i] * 3.0;",
            "outputs[1][i] = inputs[0][i] * 3.0; outputs[2][i] = inputs[1][i];",
        );
    // Mismatched declaration fails synchronously; compiler never starts.
    window.scope.with(|| workbench.run_script(true));
    assert!(!workbench.script_busy.get_untracked());
    assert!(
        workbench
            .filter_error
            .get_untracked()
            .contains("named rows")
    );
    assert_eq!(workbench.derived.get_untracked().len(), 3);
    workbench.script_source.set(three.clone());
    window.scope.with(|| workbench.save_filter());
    settle(&window, || !workbench.script_busy.get_untracked());
    assert!(
        workbench.filter_error.get_untracked().is_empty(),
        "{}",
        workbench.filter_error.get_untracked()
    );
    let grown = workbench.filter_outputs.get_untracked();
    assert_eq!(&grown[..2], &ids);
    assert_eq!(grown.len(), 3);
    assert!(
        workbench
            .derived
            .get_untracked()
            .iter()
            .filter(|field| grown.contains(&field.id))
            .all(|field| field.sources == [source.clone(), extra_source.clone()])
    );
    // Remove middle row; last output retains ID when its output index changes.
    workbench.remove_script_output(1);
    workbench.filter_inputs.update(|inputs| {
        inputs.remove(0);
    });
    let two = code.replace("* 2.0", "* 1.0").replace("* 3.0", "* 5.0");
    workbench.script_source.set(two.clone());
    window.scope.with(|| workbench.save_filter());
    settle(&window, || !workbench.script_busy.get_untracked());
    assert!(
        workbench.filter_error.get_untracked().is_empty(),
        "{}",
        workbench.filter_error.get_untracked()
    );
    assert_eq!(workbench.filter_outputs.get_untracked(), [ids[0], grown[2]]);
    workbench.edit_filter(Some(grown[2]), None);
    assert_eq!(workbench.script_names.get_untracked(), "Double\nExtra");
    assert_eq!(workbench.filter_inputs.get_untracked().len(), 1);
    // Referenced output cannot be deleted; graph and descendants remain intact.
    workbench.remove_script_output(0);
    workbench
        .script_source
        .set(crate::script_compile::EXAMPLE.into());
    window.scope.with(|| workbench.save_filter());
    assert!(!workbench.script_busy.get_untracked());
    assert!(
        workbench
            .filter_error
            .get_untracked()
            .contains("Child depends on it")
    );
    assert_eq!(workbench.derived.get_untracked().len(), 3);

    workbench.finish_filter_editing();
    workbench.set_filter_kind("rust".into());
    workbench.script_names.set("Cancelled output".into());
    workbench
        .script_source
        .set(crate::script_compile::EXAMPLE.into());
    workbench.accept_filter_sources(vec![source]).unwrap();
    window.scope.with(|| workbench.save_filter());
    workbench.cancel_script();
    for _ in 0..20 {
        window.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(workbench.derived.get_untracked().len(), 3);
    assert!(!workbench.script_busy.get_untracked());
}
