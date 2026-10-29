use crate::state::{Line, Workbench};
use plot::{PlotPoint, Ranges, time_series::SeriesData, workspace::layout::Layout};
use zgui::prelude::*;

#[test]
fn views_restore_bounds_and_duplicate_independent_plot_state() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let first = workbench.bind_view();
    let pane = first.pane();
    let id = workbench.id();
    let data =
        SeriesData::new(vec![PlotPoint { x: 0., y: 1. }, PlotPoint { x: 1., y: 2. }]).unwrap();
    pane.lines.set(vec![Line {
        id,
        source: "sensor/0/x".into(),
        name: "X".into(),
        data,
        revision: 0,
    }]);
    pane.legend_order.set(vec![id]);
    pane.selected.set(Some(id));
    pane.plot
        .hidden
        .set(std::collections::BTreeSet::from([id.to_string()]));
    pane.plot
        .set_view(Ranges::new((0.2, 0.8), (-2., 4.)))
        .unwrap();
    let second = first.add_plot(pane.id, true, false);
    first.maximize(second.id);
    first.analysis().start.set("0.25".into());
    let layout = first.layout().get_untracked();
    let other = workbench.new_view();
    assert_eq!(workbench.panes().with_untracked(Vec::len), 1);
    workbench
        .pane()
        .plot
        .set_view(Ranges::new((10., 20.), (30., 40.)))
        .unwrap();
    workbench.rename_view("Frequency".into());
    workbench.switch_view(first.view().id);
    assert_eq!(workbench.layout().get_untracked(), layout);
    assert_eq!(workbench.maximized().get_untracked(), Some(second.id));
    assert_eq!(workbench.pane().id, second.id);
    assert_eq!(
        pane.plot.ranges.get_untracked(),
        Ranges::new((0.2, 0.8), (-2., 4.))
    );
    assert_eq!(workbench.analysis().start.get_untracked(), "0.25");
    let copy = workbench.duplicate_view();
    let duplicated = workbench.panes().get_untracked();
    assert_eq!(duplicated.len(), 2);
    assert!(
        duplicated
            .iter()
            .all(|copy| copy.id != pane.id && copy.id != second.id)
    );
    let copied = duplicated[0];
    let copied_id = copied.lines.get_untracked()[0].id;
    assert_ne!(copied_id, id);
    assert_eq!(copied.legend_order.get_untracked(), vec![copied_id]);
    assert_eq!(copied.selected.get_untracked(), Some(copied_id));
    assert!(
        copied
            .plot
            .hidden
            .with_untracked(|hidden| hidden.contains(&copied_id.to_string()))
    );
    assert_eq!(
        copied.plot.ranges.get_untracked(),
        pane.plot.ranges.get_untracked()
    );
    assert!(matches!(
        workbench.layout().get_untracked(),
        Layout::Split { stacked: true, .. }
    ));
    copied
        .plot
        .set_view(Ranges::new((50., 60.), (70., 80.)))
        .unwrap();
    assert_eq!(
        pane.plot.ranges.get_untracked(),
        Ranges::new((0.2, 0.8), (-2., 4.))
    );
    workbench.close_view(copy);
    assert!(workbench.find(copied.id).is_none());
    assert!(workbench.find(pane.id).is_some());
    workbench.switch_view(other);
    assert_eq!(workbench.view().name.get_untracked(), "Frequency");
    assert_eq!(
        workbench.pane().plot.ranges.get_untracked(),
        Ranges::new((10., 20.), (30., 40.))
    );
    workbench.close_view(other);
    workbench.close_view(first.view().id);
    assert_eq!(workbench.views.items.with_untracked(Vec::len), 1);
    assert_eq!(workbench.view().name.get_untracked(), "View 1");
    assert!(workbench.pane().lines.with_untracked(Vec::is_empty));
}

#[test]
fn blank_workspaces_and_views_use_numbered_names() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let first_workspace = workbench.workspace_id();
    assert_eq!(workbench.workspace().name.get_untracked(), "Workspace 1");
    assert_eq!(workbench.view().name.get_untracked(), "View 1");
    workbench.new_view();
    assert_eq!(workbench.view().name.get_untracked(), "View 2");
    workbench.new_view();
    assert_eq!(workbench.view().name.get_untracked(), "View 3");

    workbench.new_blank_workspace();
    assert_eq!(workbench.workspace().name.get_untracked(), "Workspace 2");
    assert_eq!(workbench.view().name.get_untracked(), "View 1");
    workbench.new_view();
    assert_eq!(workbench.view().name.get_untracked(), "View 2");
    workbench.new_blank_workspace();
    assert_eq!(workbench.workspace().name.get_untracked(), "Workspace 3");

    workbench.switch_workspace(first_workspace);
    workbench.new_view();
    assert_eq!(workbench.view().name.get_untracked(), "View 4");
}

#[test]
fn numbered_names_skip_existing_custom_names() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.rename_view("View 2".into());
    workbench.new_view();
    assert_eq!(workbench.view().name.get_untracked(), "View 1");
    workbench.new_view();
    assert_eq!(workbench.view().name.get_untracked(), "View 3");

    workbench.new_workspace("Workspace 2".into());
    workbench.new_blank_workspace();
    assert_eq!(workbench.workspace().name.get_untracked(), "Workspace 3");
}
