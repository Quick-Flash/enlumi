use super::*;
use crate::state::Line;
use crate::{
    geometry::{Geometry, GeometryKind},
    log::LogStore,
    profile::{
        Metadata,
        prepare::{Prepared, PreparedView},
    },
};

fn prepared(workbench: Workbench) -> Prepared {
    let log = workbench.log.get_untracked().unwrap();
    let source = "esc_status/0/esc[5].esc_rpm".to_owned();
    let data = log.prepare(&source).unwrap();
    let id = workbench.id();
    let mut item = Geometry::new(id, GeometryKind::Band, kurbo::Point::ZERO);
    item.name = "Flight interval".into();
    item.coordinates = [0., 0., 1., 1.];
    Prepared {
        name: "Test setup".into(),
        warnings: Vec::new(),
        diagnostics: Vec::new(),
        groups: BTreeMap::new(),
        ids: Default::default(),
        symbols: BTreeMap::new(),
        fields: Vec::new(),
        metadata: BTreeMap::from([(
            source.clone(),
            Metadata {
                name: "ESC 6".into(),
                unit: "rpm".into(),
                axis: "motor6".into(),
            },
        )]),
        geometry: vec![item],
        views: vec![PreparedView {
            bindings: BTreeMap::new(),
            name: "ESC view".into(),
            fields: vec![(source, data)],
            geometry: vec![id],
            tab: false,
            stacked: true,
        }],
    }
}

#[test]
fn missing_geometry_does_not_create_panel() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    workbench.log.set(Some(Arc::new(
        LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap(),
    )));
    let prepared = prepared(workbench);
    assert!(
        workbench
            .place_profile_view(prepared.views[0].clone(), true)
            .is_err()
    );
    assert_eq!(workbench.panes().with_untracked(Vec::len), 1);
}

#[test]
fn reload_skips_invalid_geometry_without_removing_other_profiles_shared_resource() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let mut first = prepared(workbench_with_fixture(workbench));
    let id = first.geometry[0].id;
    first.ids.geometry.insert("limit".into(), id);
    let owner = ProfileKey::new(PathBuf::from("owner.json"), 1);
    let borrower = ProfileKey::new(PathBuf::from("borrower.json"), 1);
    let definition: profile::Profile =
        serde_json::from_value(serde_json::json!({"version":2,"name":"Owner"})).unwrap();
    let original = AppliedProfile::from_prepared(owner.clone(), definition.clone(), first);
    workbench
        .commit_instance(original.clone(), Vec::new(), Vec::new())
        .unwrap();
    let mut shared = original.clone();
    shared.key = borrower.clone();
    shared.geometry.clear();
    workbench
        .commit_instance(shared, Vec::new(), Vec::new())
        .unwrap();
    let mut reloaded = original;
    reloaded.ids.geometry.clear();
    reloaded.geometry.clear();
    reloaded.views.clear();
    workbench
        .commit_instance(reloaded, Vec::new(), Vec::new())
        .unwrap();
    assert!(workbench.geometry_item(id).is_some());
    assert_eq!(
        workbench
            .profiles
            .instances
            .with_untracked(|instances| instances[&borrower].ids.geometry["limit"]),
        id
    );
}

fn workbench_with_fixture(workbench: Workbench) -> Workbench {
    workbench.log.set(Some(Arc::new(
        LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap(),
    )));
    workbench
}

#[test]
fn ownership_reload_preserves_ids_and_manual_work_and_remove_blocks_dependents() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let log = Arc::new(
        LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap(),
    );
    workbench.log.set(Some(log.clone()));
    let path = PathBuf::from("/tmp/owned-profile.json");
    let key = ProfileKey::new(path, 1);
    let mut recipe = serde_json::json!({"version":1,"name":"Owned","filters":[{"id":"scaled","metadata":{"name":"Scaled","unit":"rpm"},"inputs":["esc_status/0/esc[5].esc_rpm"],"operation":{"kind":"affine","gain":2}}],"views":[{"name":"Motor","fields":["$scaled"]}]});
    let definition: profile::Profile = serde_json::from_value(recipe.clone()).unwrap();
    let prepared = profile::prepare::prepare(
        definition.clone(),
        &log,
        std::path::Path::new("/tmp"),
        100,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let ids = prepared.ids.clone();
    let fields = prepared.fields.clone();
    workbench
        .commit_instance(
            AppliedProfile::from_prepared(key.clone(), definition, prepared),
            fields,
            Vec::new(),
        )
        .unwrap();
    let raw = "esc_status/0/esc[5].esc_rpm".to_string();
    workbench.pane().lines.update(|lines| {
        lines.push(Line {
            id: 99,
            source: raw.clone(),
            name: raw.clone(),
            data: log.prepare(&raw).unwrap(),
            revision: 0,
        })
    });
    workbench.place_prepared_item(
        key.clone(),
        Item::View("Motor".into()),
        Destination::Plot(Some(1)),
    );
    workbench.place_prepared_item(
        key.clone(),
        Item::View("Motor".into()),
        Destination::Plot(Some(1)),
    );
    assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 2);
    let line_ids = workbench
        .pane()
        .lines
        .with_untracked(|lines| lines.iter().map(|l| l.id).collect::<Vec<_>>());
    recipe["filters"][0]["operation"]["gain"] = serde_json::json!(3);
    let definition: profile::Profile = serde_json::from_value(recipe).unwrap();
    let prepared = profile::prepare::prepare_with_ids(
        definition.clone(),
        &log,
        std::path::Path::new("/tmp"),
        ids,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let fields = prepared.fields.clone();
    workbench
        .commit_instance(
            AppliedProfile::from_prepared(key.clone(), definition, prepared),
            fields,
            Vec::new(),
        )
        .unwrap();
    assert_eq!(
        line_ids,
        workbench
            .pane()
            .lines
            .with_untracked(|lines| lines.iter().map(|l| l.id).collect::<Vec<_>>())
    );
    let manual = derived::DerivedField {
        id: 200,
        name: "Manual dependent".into(),
        sources: vec![derived::key(100)],
        group: 200,
        filter: derived::FilterSpec::Derivative,
        data: None,
    };
    workbench.derived.update(|fields| fields.push(manual));
    assert_eq!(
        workbench.profile_remove_blockers(&key),
        vec!["Filter: Manual dependent"]
    );
    workbench.remove_profile(key.clone());
    assert_eq!(
        workbench.profiles.instances.with_untracked(BTreeMap::len),
        1
    );
    workbench
        .derived
        .update(|fields| fields.retain(|field| field.id != 200));
    workbench.remove_profile(key);
    assert!(
        workbench
            .profiles
            .instances
            .with_untracked(BTreeMap::is_empty)
    );
    assert_eq!(
        workbench.pane().lines.with_untracked(Vec::len),
        1,
        "Manual raw line remains"
    );
}

#[test]
fn labels_restore_profile_default_and_raw_independently() {
    let window = zgui_testkit_view::Window::open();
    let workbench = window.scope.with(Workbench::new);
    let source = "raw/0/x".to_owned();
    workbench.profiles.overrides.update(|all| {
        all.insert(
            source.clone(),
            Some(profile::Metadata {
                name: "Roll".into(),
                unit: "deg".into(),
                axis: "Roll".into(),
            }),
        );
    });
    workbench.rebuild_profile_metadata();
    assert_eq!(workbench.field_name(&source), "Roll [deg]");
    workbench.profiles.overrides.update(|all| {
        all.insert(source.clone(), None);
    });
    workbench.rebuild_profile_metadata();
    assert!(
        !workbench
            .profiles
            .metadata
            .with_untracked(|all| all.contains_key(&source))
    );
}
