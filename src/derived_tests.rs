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
    assert_eq!(workbench.derived.get_untracked()[0].source, source);
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
fn invalid_batch_is_atomic_and_edit_source_drop_rejects_cycles() {
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
    let before = workbench.filter_inputs.get_untracked()[0].source.clone();
    assert!(
        workbench
            .accept_filter_sources(vec![derived::key(child)])
            .is_err()
    );
    assert_eq!(workbench.filter_inputs.get_untracked()[0].source, before);
}
