use crate::app::WorkbenchAppProps;
use plot::zgui_workspace::LinkControlsProps;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
use zgui::geom::{CssPx, DevicePx, Point, Size};
use zgui::platform::SurfaceEvent;
use zgui::prelude::*;
use zgui::render::{
    ExternalTexture, FrameOutcome, FrameStats, MemoryReport, RenderTarget, Renderer, TextureHandle,
};
use zgui::vocab::{Modifiers, PointerAction, PointerEvent, Timestamp};
use zgui_platform_headless::Harness;
use zgui_testkit_scene::CaptureRenderer;

#[derive(Default, Debug)]
struct Counts {
    peak_marks: usize,
    peak_vertices: usize,
    glyphs: usize,
    seen_steps: bool,
    seen_dots: bool,
    line_strokes: Vec<f32>,
    painted_boxes: Vec<([f32; 4], String)>,
    painted_radii: Vec<([f32; 4], [f32; 8])>,
}
struct Capture {
    renderer: CaptureRenderer,
    counts: Rc<RefCell<Counts>>,
}
impl Renderer for Capture {
    fn capabilities(&self) -> zgui::render::RenderCapabilities {
        self.renderer.capabilities()
    }
    fn configure(&mut self, target: RenderTarget) {
        self.renderer.configure(target);
    }
    fn target(&self) -> Option<RenderTarget> {
        self.renderer.target()
    }
    fn draw(&mut self, scene: &zgui::scene::Scene, damage: &zgui::bits::DamageSet) -> FrameOutcome {
        // Empty damage retains previous composed frame.
        if damage.is_empty() {
            return FrameOutcome::Presented(FrameStats::default());
        }
        let peak_marks = self
            .counts
            .borrow()
            .peak_marks
            .max(scene.primitives.marks.len());
        let seen_steps = self.counts.borrow().seen_steps
            || scene.primitives.mark_payloads.iter().any(|payload| {
                payload
                    .vertices
                    .iter()
                    .filter(|vertex| vertex[0].is_finite())
                    .count()
                    == 3
            });
        let seen_dots = self.counts.borrow().seen_dots
            || scene.primitives.marks.iter().any(|mark| mark.discs == 2);
        let peak_vertices = self.counts.borrow().peak_vertices.max(
            scene
                .primitives
                .mark_payloads
                .iter()
                .map(|p| p.vertices.len())
                .sum(),
        );
        // Render deltas contain changed quads; preserve unchanged button paints.
        let mut painted_boxes = std::mem::take(&mut self.counts.borrow_mut().painted_boxes);
        let mut painted_radii = std::mem::take(&mut self.counts.borrow_mut().painted_radii);
        for quad in &scene.primitives.quads {
            if let Some(color) = quad
                .fill
                .id()
                .and_then(|id| scene.paints.get(id))
                .and_then(|paint| paint.solid_color())
            {
                painted_boxes.retain(|(bounds, _)| *bounds != quad.bounds);
                painted_boxes.push((quad.bounds, format!("{color:?}")));
                painted_radii.retain(|(bounds, _)| *bounds != quad.bounds);
                painted_radii.push((quad.bounds, quad.radii));
            }
        }
        *self.counts.borrow_mut() = Counts {
            peak_marks,
            peak_vertices,
            seen_steps,
            seen_dots,
            line_strokes: Vec::new(),
            painted_boxes,
            painted_radii,
            glyphs: scene.primitives.mono_sprites.len() + scene.primitives.subpixel_sprites.len(),
        };
        self.renderer.draw(scene, damage);
        self.counts.borrow_mut().line_strokes =
            self.renderer
                .transcript()
                .map_or_else(Vec::new, |transcript| {
                    transcript
                        .to_string()
                        .lines()
                        .filter_map(|line| {
                            let width = line
                                .split(" half_width=")
                                .nth(1)?
                                .split_whitespace()
                                .next()?
                                .parse::<f32>()
                                .ok()?;
                            (width > 2.0).then_some(width)
                        })
                        .collect()
                });
        FrameOutcome::Presented(FrameStats::default())
    }
    fn register_external(&mut self, texture: ExternalTexture) -> TextureHandle {
        self.renderer.register_external(texture)
    }
    fn release_external(&mut self, texture: TextureHandle) {
        self.renderer.release_external(texture);
    }
    fn memory(&self) -> MemoryReport {
        self.renderer.memory()
    }
    fn texture_sink(&mut self) -> &mut dyn zgui::atlas::TextureSink {
        self.renderer.texture_sink()
    }
}
fn pointer(action: PointerAction, x: f32, y: f32) -> SurfaceEvent {
    let mut event = PointerEvent::mouse(Point::new(CssPx(x), CssPx(y)));
    event.button = matches!(action, PointerAction::Pressed | PointerAction::Released)
        .then_some(zgui::vocab::PointerButton::Primary);
    SurfaceEvent::Pointer {
        action,
        event,
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    }
}
fn click_class(harness: &mut Harness<zgui::runtime::Runtime>, class: &str) {
    let rect = harness.app().windows()[0].box_with_class(class).unwrap();
    let (x, y) = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
}

fn drag_filter_card(
    harness: &mut Harness<zgui::runtime::Runtime>,
    workbench: crate::state::Workbench,
    from: usize,
    to: usize,
    cancel: bool,
) {
    let original = workbench.filter_inputs.get_untracked();
    let window = &harness.app().windows()[0];
    let source = window.nodes_with_class("filter-input-handle")[from];
    let target = if workbench.filter_kind.get_untracked() == "rpy" {
        window.nodes_with_class("filter-quaternion-slot")[to]
    } else {
        window.nodes_with_class("filter-source-row")[to]
    };
    let a = zgui::view::ViewHost::window_box(window.host().as_ref(), source).unwrap();
    let b = zgui::view::ViewHost::window_box(window.host().as_ref(), target).unwrap();
    let (sx, sy) = (a.origin.x.0 + 3.0, a.origin.y.0 + 3.0);
    let (tx, ty) = (
        b.origin.x.0 + b.size.width.0 / 2.0,
        b.origin.y.0 + b.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, sx, sy));
    harness.deliver_to_first(pointer(PointerAction::Moved, tx, ty));
    harness.settle(16);
    assert_eq!(
        workbench.filter_inputs.get_untracked(),
        original,
        "Preview must not mutate assignments"
    );
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-input-dragging")
            .len(),
        1,
        "Drag {from} to {to}, source {a:?}, target {b:?}"
    );
    assert_ne!(workbench.filter_input_offset(from), "translateY(0px)");
    assert_ne!(
        workbench.filter_input_offset(to),
        "translateY(0px)",
        "Neighbor must slide into preview position"
    );
    harness.deliver_to_first(pointer(
        if cancel {
            PointerAction::Cancelled
        } else {
            PointerAction::Released
        },
        tx,
        ty,
    ));
    let deadline = Instant::now() + Duration::from_secs(3);
    while workbench.filter_input_ui.drag.get_untracked().is_some() {
        assert!(Instant::now() < deadline, "Input drag did not settle");
        harness.advance(Duration::from_millis(16));
        harness.settle(16);
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.advance(Duration::from_millis(32));
    harness.settle(128);
    if cancel {
        assert_eq!(workbench.filter_inputs.get_untracked(), original);
    }
}

#[test]
fn compact_saved_rows_toggle_editing_delete_with_x_and_fill_window_height() {
    use crate::{filter_workspace::FilterWorkspaceProps, state::Workbench};
    use zgui_ui_tokens::prelude::*;
    fn finish(harness: &mut Harness<zgui::runtime::Runtime>, workbench: Workbench) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while workbench.derived_busy.get_untracked() {
            assert!(Instant::now() < deadline);
            harness.pump();
            std::thread::sleep(Duration::from_millis(2));
        }
        harness.settle(128);
    }
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let log = std::sync::Arc::new(
        crate::log::LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap(),
    );
    let source = log.catalogue[0].id.clone();
    let handler=app().with_size(1200.0,800.0)
        .with_renderer(Box::new(|_,target| {let mut renderer=CaptureRenderer::new();renderer.configure(target);Ok(Box::new(renderer))}))
        .with_stylesheet(crate::theme::stylesheet())
        .into_handler(move || {
            let workbench=Workbench::new();workbench.log.set(Some(log.clone()));workbench.filters_open.set(true);*built.borrow_mut()=Some(workbench);
            view! {ThemeProvider(scheme = ColorScheme::Dark) {column(class = "workbench") {row(class = "body") {FilterWorkspace(workbench = workbench)}}}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.0),
        DevicePx(800.0),
    )));
    harness.settle(128);
    let workbench = model.borrow().unwrap();
    workbench.set_filter_kind("magnitude".into());
    workbench.accept_filter_sources(vec![source]).unwrap();
    harness.settle(128);
    click_class(&mut harness, "save-derived-field");
    finish(&mut harness, workbench);
    let first = workbench.filter_outputs.get_untracked()[0];
    let saved = workbench.derived.get_untracked()[0].clone();
    assert!(workbench.filter_mode.get_untracked().is_new());
    assert!(workbench.filter_inputs.get_untracked().is_empty());
    let window = &harness.app().windows()[0];
    assert!(window.nodes_with_class("filter-start-new").is_empty());
    assert!(window.nodes_with_class("delete-derived-field").is_empty());
    let row = window.box_with_class("derived-library-row").unwrap();
    assert!((30.0..=38.0).contains(&row.size.height.0), "{row:?}");
    let list = window.box_with_class("derived-library-list").unwrap();
    let new = window.box_with_class("new-derived-field").unwrap();
    assert!(
        list.size.height.0 > 650.0 && new.origin.y.0 >= row.origin.y.0 + row.size.height.0,
        "{list:?}, {new:?}"
    );
    assert!(
        new.origin.y.0 + new.size.height.0 < list.origin.y.0 + list.size.height.0 - 100.0,
        "New field follows short list instead of sticking to window bottom"
    );
    assert_eq!(window.nodes_with_class("new-field-selected").len(), 1);
    click_class(&mut harness, "derived-library-item");
    assert_eq!(
        workbench.filter_mode.get_untracked().selected(),
        Some(first)
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("filter-mode-edit")
            .is_some()
    );
    assert_eq!(
        harness.app().windows()[0].nodes_with_class("chosen").len(),
        1
    );
    click_class(&mut harness, "new-derived-field");
    assert!(workbench.filter_mode.get_untracked().is_new());
    assert_eq!(workbench.derived.get_untracked().len(), 1);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("new-field-selected")
            .len(),
        1
    );
    click_class(&mut harness, "derived-library-item");
    click_class(&mut harness, "derived-library-item");
    assert!(workbench.filter_mode.get_untracked().is_new());
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("chosen")
            .is_empty()
    );
    click_class(&mut harness, "derived-library-item");
    click_class(&mut harness, "filter-done-editing");
    assert!(workbench.filter_mode.get_untracked().is_new());
    workbench
        .accept_filter_sources(vec![crate::derived::key(first)])
        .unwrap();
    harness.settle(128);
    click_class(&mut harness, "save-derived-field");
    finish(&mut harness, workbench);
    assert_eq!(workbench.derived.get_untracked().len(), 2);
    assert!(workbench.filter_mode.get_untracked().is_new());
    click_class(&mut harness, "remove-saved-field");
    assert!(workbench.filter_error.get_untracked().contains("dependent"));
    assert!(
        workbench.filter_mode.get_untracked().is_new(),
        "X must not enter editing"
    );
    let window = &harness.app().windows()[0];
    let x = window.nodes_with_class("remove-saved-field")[1];
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), x).unwrap();
    let (x, y) = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    assert_eq!(workbench.derived.get_untracked().len(), 1);
    click_class(&mut harness, "remove-saved-field");
    assert!(workbench.derived.get_untracked().is_empty());
    let fields = (0..30)
        .map(|index| {
            let mut field = saved.clone();
            field.id = 1000 + index;
            field.group = field.id;
            field.name = format!("Magnitude {index}");
            field
        })
        .collect();
    workbench.derived.set(fields);
    harness.settle(128);
    let window = &harness.app().windows()[0];
    let list = window.box_with_class("derived-library-list").unwrap();
    let rows = window.nodes_with_class("derived-library-row");
    assert_eq!(rows.len(), 30);
    let last =
        zgui::view::ViewHost::window_box(window.host().as_ref(), *rows.last().unwrap()).unwrap();
    let new = window.box_with_class("new-derived-field").unwrap();
    assert!(
        new.origin.y.0 >= last.origin.y.0 + last.size.height.0,
        "New field follows final saved row"
    );
    assert!(
        new.origin.y.0 > list.origin.y.0 + list.size.height.0,
        "New field scrolls with long list"
    );
    assert!(
        last.origin.y.0 > list.origin.y.0 + list.size.height.0,
        "Saved list must scroll rather than grow window"
    );
}

#[test]
fn rust_filter_editor_fills_workspace_and_input_order_updates() {
    use crate::{derived::FilterInput, filter_workspace::FilterWorkspaceProps, state::Workbench};
    use zgui_ui_tokens::prelude::*;
    let model = Rc::new(RefCell::new(None));
    let saved = model.clone();
    let handler = app().with_size(1200.0, 800.0)
        .with_renderer(Box::new(|_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(renderer)) }))
        .with_stylesheet(crate::theme::stylesheet())
        .into_handler(move || {
            let workbench = Workbench::new();
            workbench.set_filter_kind("rust".into());
            workbench.filter_inputs.set(vec![FilterInput { source: "first".into(), name: "First".into() }, FilterInput { source: "second".into(), name: "Second".into() }, FilterInput { source: "third".into(), name: "Third".into() }]);
            *saved.borrow_mut() = Some(workbench);
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "workbench") { row(class = "body") { FilterWorkspace(workbench = workbench) } } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.0),
        DevicePx(800.0),
    )));
    harness.settle(128);
    let workbench = model.borrow().unwrap();
    let window = &harness.app().windows()[0];
    let source = window.box_with_class("script-source").unwrap();
    assert!(
        source.size.height.0 > 220.0 && source.size.width.0 > 400.0,
        "Code editor needs usable space: {source:?}"
    );
    assert!(
        source.origin.y.0 + source.size.height.0 <= 790.0,
        "{source:?}"
    );
    assert!(
        window
            .box_with_class("script-diagnostics-panel")
            .unwrap()
            .size
            .height
            .0
            >= 70.0
    );
    assert_eq!(window.nodes_with_class("script-diagnostics-empty").len(), 1);
    let divider = window.box_with_class("script-diagnostics-resizer").unwrap();
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        divider.origin.x.0 + divider.size.width.0 / 2.0,
        divider.origin.y.0 + divider.size.height.0 / 2.0,
    ));
    harness.settle(128);
    assert_eq!(
        harness.platform().offscreens()[0].last_cursor(),
        Some(zgui::platform::CursorStyle::ResizeNorthSouth)
    );
    let window = &harness.app().windows()[0];
    let actions = window.box_with_class("filter-actions").unwrap();
    let editor_panel = window.box_with_class("filter-editor").unwrap();
    let kind = window.box_with_class("filter-kind-picker").unwrap();
    assert!(
        actions.origin.y.0 < editor_panel.origin.y.0 + 40.0
            && actions.origin.y.0 + actions.size.height.0 <= kind.origin.y.0,
        "Actions belong directly below Filter heading, before type picker: {actions:?}, {editor_panel:?}, {kind:?}"
    );
    let create = window.box_with_class("save-derived-field").unwrap();
    let footer = window.box_with_class("filter-create-action").unwrap();
    assert!(
        create.size.height.0 >= 40.0 && create.size.width.0 >= 220.0,
        "Create action should be large and full width: {create:?}"
    );
    assert!(
        (footer.origin.y.0 + footer.size.height.0
            - editor_panel.origin.y.0
            - editor_panel.size.height.0)
            .abs()
            <= 1.0,
        "Create footer should stay at panel bottom: {footer:?}, {editor_panel:?}"
    );
    assert!(window.box_with_class("script-output-names").is_some());
    assert!(window.box_with_class("check-script").is_some());
    assert_eq!(window.nodes_with_class("filter-input-handle").len(), 3);
    assert!(window.nodes_with_class("script-input-down").is_empty());
    assert!(window.nodes_with_class("script-input-up").is_empty());
    drag_filter_card(&mut harness, workbench, 0, 2, false);
    assert_eq!(
        workbench
            .filter_inputs
            .get_untracked()
            .iter()
            .map(|input| input.source.as_str())
            .collect::<Vec<_>>(),
        ["second", "third", "first"]
    );
    drag_filter_card(&mut harness, workbench, 2, 0, true);
    assert_eq!(workbench.filter_inputs.get_untracked()[0].source, "second");
    let window = &harness.app().windows()[0];
    let rows = window.nodes_with_class("filter-source-row");
    let text = |node| window.dom().text_content(node);
    assert!(text(rows[0]).contains("inputs[0]") && text(rows[0]).contains("second"));
    assert!(text(rows[1]).contains("inputs[1]") && text(rows[1]).contains("third"));
    assert!(text(rows[2]).contains("inputs[2]") && text(rows[2]).contains("first"));
    click_class(&mut harness, "add-script-output");
    assert_eq!(workbench.script_names.get_untracked(), "Result\n");
    assert!(crate::script_state::output_names(&workbench.script_names.get_untracked()).is_err());
    workbench.rename_script_output(1, "Velocity".into());
    harness.settle(128);
    let window = &harness.app().windows()[0];
    let labels: Vec<_> = window
        .nodes_with_class("script-output-index")
        .iter()
        .map(|node| window.dom().text_content(*node))
        .collect();
    assert_eq!(labels, ["outputs[0]", "outputs[1]"]);
    assert_eq!(
        crate::script_state::output_names(&workbench.script_names.get_untracked()).unwrap(),
        ["Result", "Velocity"]
    );
    click_class(&mut harness, "remove-script-output");
    assert_eq!(workbench.script_names.get_untracked(), "Velocity");
    click_class(&mut harness, "remove-script-output");
    assert_eq!(
        workbench.script_names.get_untracked(),
        "Velocity",
        "Keep at least one output row"
    );
    workbench.set_filter_kind("magnitude".into());
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("script-editor")
            .is_empty()
    );
    click_class(&mut harness, "custom-rust-filter");
    assert_eq!(workbench.filter_kind.get_untracked(), "rust");
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("filter-type")
            .is_empty()
    );
    let rust_width = harness.app().windows()[0]
        .box_with_class("filter-editor")
        .unwrap()
        .size
        .width
        .0;
    click_class(&mut harness, "builtin-filter-mode");
    assert_eq!(workbench.filter_kind.get_untracked(), "magnitude");
    assert_eq!(
        harness.app().windows()[0]
            .box_with_class("filter-editor")
            .unwrap()
            .size
            .width
            .0,
        rust_width,
        "Built-in/Rust must keep same settings width"
    );
    workbench.set_filter_kind("low".into());
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .box_with_class("filter-zero-phase")
            .is_some()
    );
    click_class(&mut harness, "filter-zero-phase");
    assert!(workbench.filter_zero_phase.get_untracked());
    click_class(&mut harness, "filter-type");
    click_class(&mut harness, "filter-type-average");
    assert_eq!(workbench.filter_kind.get_untracked(), "average");
    assert!(
        harness.app().windows()[0]
            .box_with_class("filter-window")
            .is_some()
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("filter-zero-phase")
            .is_none()
    );
    click_class(&mut harness, "filter-type");
    click_class(&mut harness, "filter-type-integral");
    assert!(
        harness.app().windows()[0]
            .box_with_class("filter-integral-initial")
            .is_some()
    );
    click_class(&mut harness, "filter-type");
    click_class(&mut harness, "filter-type-derivative");
    assert!(
        harness.app().windows()[0]
            .box_with_class("filter-window")
            .is_none()
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("filter-integral-initial")
            .is_none()
    );
    assert_eq!(
        harness.app().windows()[0]
            .box_with_class("filter-editor")
            .unwrap()
            .size
            .width
            .0,
        rust_width
    );
    workbench.set_filter_kind("magnitude".into());
    harness.settle(128);
    click_class(&mut harness, "custom-rust-filter");
    let window = &harness.app().windows()[0];
    let output = window.box_with_class("script-output-name").unwrap();
    assert!(
        output.size.height.0 <= 22.0 && output.size.width.0 >= 110.0,
        "Output names should be short and wide: {output:?}"
    );
    let builtin = window.box_with_class("builtin-filter-mode").unwrap();
    let custom = window.box_with_class("custom-rust-filter").unwrap();
    assert!(custom.origin.x.0 - builtin.origin.x.0 - builtin.size.width.0 >= 5.0);
    assert!(
        custom.size.width.0 >= 110.0 && custom.size.width.0 > builtin.size.width.0,
        "Rust function needs room for its longer label and side padding: {custom:?}, {builtin:?}"
    );
    workbench.script_names.set(
        (0..16)
            .map(|index| format!("Result {index}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    harness.settle(128);
    let list = harness.app().windows()[0]
        .box_with_class("script-output-list")
        .unwrap();
    assert!(
        list.size.height.0 > 400.0,
        "Outputs should get more space before scrolling: {list:?}"
    );
    let create = harness.app().windows()[0]
        .box_with_class("save-derived-field")
        .unwrap();
    let panel = harness.app().windows()[0]
        .box_with_class("filter-editor")
        .unwrap();
    assert!(
        create.origin.y.0 + create.size.height.0 <= panel.origin.y.0 + panel.size.height.0,
        "Long outputs must scroll without hiding create button: {create:?}, {panel:?}"
    );
    let diagnostics = harness.app().windows()[0].nodes_with_class("script-diagnostics-panel")[0];
    workbench.filter_error.set(
        (0..40)
            .map(|index| format!("error {index}: example diagnostic"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    harness.settle(128);
    let window = &harness.app().windows()[0];
    assert_eq!(
        window.nodes_with_class("script-diagnostics-panel")[0],
        diagnostics
    );
    let before = window.box_with_class("script-diagnostics-panel").unwrap();
    let handle = window.box_with_class("script-diagnostics-resizer").unwrap();
    let (x, y) = (
        handle.origin.x.0 + handle.size.width.0 / 2.0,
        handle.origin.y.0 + handle.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Moved, x, y - 65.0));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y - 65.0));
    harness.settle(128);
    let after = harness.app().windows()[0]
        .box_with_class("script-diagnostics-panel")
        .unwrap();
    assert!(
        after.size.height.0 > before.size.height.0 + 40.0,
        "Diagnostics should resize: {before:?} -> {after:?}"
    );
    workbench.filter_error.set(String::new());
    harness.settle(128);
    let window = &harness.app().windows()[0];
    assert_eq!(
        window.nodes_with_class("script-diagnostics-panel")[0],
        diagnostics
    );
    assert_eq!(
        window.box_with_class("script-diagnostics-panel").unwrap(),
        after,
        "Clearing diagnostics keeps resized panel geometry"
    );
    assert_eq!(
        workbench.script_source.get_untracked(),
        crate::script_compile::EXAMPLE
    );
}

#[test]
fn rust_code_editor_syncs_source_undo_and_read_only_state() {
    use crate::script_code_editor::ScriptCodeEditorProps;
    use zgui_editor::{Command, EditorHandle};
    use zgui_ui_tokens::prelude::*;
    let model = Rc::new(RefCell::new(None));
    let saved = model.clone();
    let handler = app().with_size(900.0, 650.0)
        .with_renderer(Box::new(|_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(renderer)) }))
        .with_stylesheet(crate::theme::stylesheet())
        .into_handler(move || {
            let source = RwSignal::new_local("fn first() {}".to_owned());
            let locked = RwSignal::new_local(false);
            let saved = saved.clone();
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "workbench") {
                ScriptCodeEditor(source = source, read_only = Signal::derive_local(move || locked.get()), on_ready = UnsyncCallback::new(move |editor| { *saved.borrow_mut() = Some((source, locked, editor)); }))
            } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(650.0),
    )));
    harness.settle(128);
    let (source, locked, editor): (_, _, EditorHandle) = model.borrow().clone().unwrap();
    editor.commands([Command::SelectAll, Command::Insert("fn second() {}".into())]);
    harness.settle(128);
    assert_eq!(source.get_untracked(), "fn second() {}");
    editor.command(Command::Undo);
    harness.settle(128);
    assert_eq!(source.get_untracked(), "fn first() {}");
    source.set("fn external() {\n    let n = 42;\n}".into());
    harness.settle(128);
    assert_eq!(
        editor.query(|snapshot| snapshot.rope().to_string()),
        source.get_untracked()
    );
    // Click inside lower part of rendered second line. Root padding used to
    // shift hit testing into following line, even though pointer stayed on text.
    let window = &harness.app().windows()[0];
    let code = window.nodes_with_class("script-source")[0];
    let root = window.box_with_class("script-source").unwrap();
    let child = window
        .dom()
        .children(code)
        .into_iter()
        .find_map(|node| {
            zgui::view::ViewHost::window_box(window.host().as_ref(), node)
                .filter(|rect| rect.size.height.0 > 0.0)
        })
        .unwrap();
    assert_eq!(
        root.origin, child.origin,
        "Paint and pointer origins must match"
    );
    let caret = editor.point_for_byte(0).unwrap();
    let (x, y) = (caret.x + 1.0, child.origin.y.0 + caret.height * 1.8);
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    assert_eq!(editor.cursor_position().get_untracked().line, 1);
    locked.set(true);
    harness.settle(128);
    let before = source.get_untracked();
    editor.commands([Command::SelectAll, Command::Insert("blocked".into())]);
    harness.settle(128);
    assert_eq!(source.get_untracked(), before);
    locked.set(false);
    harness.settle(128);
    editor.commands([
        Command::SelectAll,
        Command::Insert("fn unlocked() {}".into()),
    ]);
    harness.settle(128);
    assert_eq!(source.get_untracked(), "fn unlocked() {}");
}

#[test]
fn combined_filter_editor_labels_inputs_reorders_and_switches_output_names() {
    use crate::{derived::FilterInput, filter_workspace::FilterWorkspaceProps, state::Workbench};
    use zgui_ui_tokens::prelude::*;
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let handler = app().with_size(1200.0, 800.0)
        .with_renderer(Box::new(|_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(renderer)) }))
        .with_stylesheet(crate::theme::stylesheet())
        .into_handler(move || {
            let workbench = Workbench::new();
            workbench.filter_kind.set("rpy".into());
            *built.borrow_mut() = Some(workbench);
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "workbench") { row(class = "body") { FilterWorkspace(workbench = workbench) } } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.0),
        DevicePx(800.0),
    )));
    harness.settle(128);
    let workbench = model.borrow().unwrap();
    let window = &harness.app().windows()[0];
    let labels: Vec<_> = window
        .nodes_with_class("filter-input-role")
        .iter()
        .map(|node| window.dom().text_content(*node))
        .collect();
    assert_eq!(labels, ["W", "X", "Y", "Z"]);
    assert_eq!(window.nodes_with_class("filter-slot-empty").len(), 4);
    assert!(window.nodes_with_class("filter-input-grip").is_empty());
    assert!(window.nodes_with_class("filter-source-row").is_empty());
    let slot = window.nodes_with_class("filter-quaternion-slot")[3];
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), slot).unwrap();
    let log = std::sync::Arc::new(
        crate::log::LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap(),
    );
    let source = log.catalogue[0].id.clone();
    workbench.log.set(Some(log));
    workbench.filters_open.set(true);
    workbench.drop_fields(
        source.clone(),
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    harness.settle(128);
    let inputs = workbench.filter_inputs.get_untracked();
    assert_eq!(inputs[3].source, source);
    assert!(inputs[..3].iter().all(|input| input.source.is_empty()));
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-slot-empty")
            .len(),
        3
    );
    workbench.filter_inputs.set(
        (0..4)
            .map(|index| FilterInput {
                source: format!("q[{index}]"),
                name: format!("unused {index}"),
            })
            .collect(),
    );
    harness.settle(128);
    let window = &harness.app().windows()[0];
    assert_eq!(window.nodes_with_class("filter-output-name-row").len(), 3);
    assert!(
        window.nodes_with_class("filter-output-name").is_empty(),
        "Combined inputs must not show batch output names"
    );
    let quaternion_rows: Vec<_> = window
        .nodes_with_class("filter-source-row")
        .iter()
        .map(|node| zgui::view::ViewHost::window_box(window.host().as_ref(), *node).unwrap())
        .collect();
    for pair in quaternion_rows.windows(2) {
        let gap = pair[1].origin.y.0 - pair[0].origin.y.0 - pair[0].size.height.0;
        assert!(
            (2.5..=3.5).contains(&gap),
            "Quaternion rows should use same 3px gap as magnitude: {gap}"
        );
    }
    click_class(&mut harness, "filter-angle-unit");
    let window = &harness.app().windows()[0];
    let degrees = window
        .nodes_with_class("zui-select__item")
        .into_iter()
        .find(|node| window.dom().text_content(*node).contains("Degrees"))
        .unwrap();
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), degrees).unwrap();
    let (x, y) = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    assert_eq!(workbench.filter_angle_unit.get_untracked(), "deg");
    let window = &harness.app().windows()[0];
    for node in window.nodes_with_class("filter-output-name-row") {
        assert!(window.dom().text_content(node).contains("(deg)"));
    }
    assert!(window.nodes_with_class("reorder-filter-source").is_empty());
    drag_filter_card(&mut harness, workbench, 3, 2, false);
    drag_filter_card(&mut harness, workbench, 0, 3, true);
    assert_eq!(workbench.filter_inputs.get_untracked()[2].source, "q[3]");
    workbench
        .filter_names
        .update(|names| names[1] = "Bank".into());
    workbench.set_filter_kind("magnitude".into());
    harness.settle(128);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-output-name-row")
            .len(),
        1
    );
    workbench.set_filter_kind("rpy".into());
    harness.settle(128);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-output-name-row")
            .len(),
        3
    );
    assert_eq!(workbench.filter_names.get_untracked()[1], "Bank");
    click_class(&mut harness, "remove-filter-source");
    assert!(workbench.filter_input_ui.drag.get_untracked().is_none());
    assert_eq!(workbench.filter_inputs.get_untracked().len(), 4);
    assert!(workbench.filter_inputs.get_untracked()[0].source.is_empty());
    let window = &harness.app().windows()[0];
    let labels: Vec<_> = window
        .nodes_with_class("filter-input-role")
        .iter()
        .map(|node| window.dom().text_content(*node))
        .collect();
    assert_eq!(labels, ["W", "X", "Y", "Z"]);
    assert_eq!(window.nodes_with_class("filter-slot-empty").len(), 1);
    workbench.set_filter_kind("magnitude".into());
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("filter-input-handle")
            .is_empty()
    );
    let before = workbench.filter_inputs.get_untracked();
    workbench.start_filter_input_drag(0, 0.0, 0.0);
    assert!(workbench.filter_input_ui.drag.get_untracked().is_none());
    assert_eq!(workbench.filter_inputs.get_untracked(), before);
    workbench.set_filter_kind("affine".into());
    workbench.filter_offset.set("7".into());
    harness.settle(128);
    let window = &harness.app().windows()[0];
    assert!(window.nodes_with_class("filter-input-handle").is_empty());
    assert_eq!(window.nodes_with_class("filter-io-label").len(), 6);
    let row = window.box_with_class("filter-source-row").unwrap();
    assert!(
        row.size.height.0 <= 36.0,
        "Standard filter rows should be compact: {row:?}"
    );
    let before = workbench.filter_inputs.get_untracked();
    let (x, y) = (row.origin.x.0 + 3.0, row.origin.y.0 + 3.0);
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Moved, x, y + 50.0));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y + 50.0));
    harness.settle(128);
    assert!(workbench.filter_input_ui.drag.get_untracked().is_none());
    assert_eq!(workbench.filter_inputs.get_untracked(), before);
    click_class(&mut harness, "preset-rad-to-deg");
    assert_eq!(
        workbench
            .filter_gain
            .get_untracked()
            .parse::<f64>()
            .unwrap(),
        1.0_f64.to_degrees()
    );
    assert_eq!(workbench.filter_offset.get_untracked(), "0");
    workbench.filter_offset.set("4".into());
    harness.settle(128);
    click_class(&mut harness, "preset-deg-to-rad");
    assert_eq!(
        workbench
            .filter_gain
            .get_untracked()
            .parse::<f64>()
            .unwrap(),
        1.0_f64.to_radians()
    );
    assert_eq!(workbench.filter_offset.get_untracked(), "0");
}

#[test]
fn snapped_cross_links_and_readouts_match_and_checkboxes_toggle_links() {
    use crate::{
        plot_cursor::CursorPlotProps,
        state::{Line, Workbench},
    };
    use zgui_ui_tokens::prelude::*;
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let point = plot::PlotPoint { x: 0.25, y: 0.75 };
    let counts = Rc::new(RefCell::new(Counts::default()));
    let captured = Rc::clone(&counts);
    let handler = app().with_size(900.0, 600.0)
        .with_renderer(Box::new(move |_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(Capture { renderer, counts: Rc::clone(&captured) })) }))
        .with_stylesheet(format!("{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), crate::theme::stylesheet()))
        .into_handler(move || {
            let workbench = Workbench::new();
            let first = workbench.pane();
            let second = workbench.add_plot(first.id, false, false);
            let shared = RwSignal::new_local(None);
            let data = plot::time_series::SeriesData::new(vec![point, plot::PlotPoint { x: 0.8, y: 0.2 }]).unwrap();
            first.lines.set(vec![Line { id: 1, source: "sample".into(), name: "sample".into(), data: data.clone(),  revision: 0 }]);
            second.lines.set(vec![Line { id: 2, source: "other sample".into(), name: "other sample".into(), data,  revision: 0 }]);
            for pane in [first, second] { pane.plot.set_view(plot::Ranges::new((0.0, 1.0), (0.0, 1.0))).unwrap(); }
            *built.borrow_mut() = Some((first, second, shared));
            view! { ThemeProvider(scheme = ColorScheme::Dark) {
                column(class = "workbench") {
                    LinkControls(options = first.links)
                    row(class = "body") {
                        CursorPlot(workbench = workbench, pane = first, visible = Signal::stored_local(true), linked = shared)
                        CursorPlot(workbench = workbench, pane = second, visible = Signal::stored_local(true), linked = shared)
                    }
                }
            } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(600.0),
    )));
    harness.settle(256);
    let (first, second, shared) = model.borrow().unwrap();
    let graph = harness.app().windows()[0]
        .box_with_class("time-series-plot")
        .unwrap();
    let window = &harness.app().windows()[0];
    let source_layers = window.nodes_with_class("time-series-layer");
    assert!(
        zgui::view::ViewHost::window_box(window.host().as_ref(), source_layers[2])
            .is_some_and(|bounds| bounds.size.width.0 > 0.0),
        "Shared plot renderer must own series drawing"
    );
    let x = graph.origin.x.0 + graph.size.width.0 * 0.25 + 7.0;
    let y = graph.origin.y.0 + graph.size.height.0 * 0.25 + 8.0;
    harness.deliver_to_first(pointer(PointerAction::Moved, x, y));
    harness.settle(256);
    assert_eq!(first.plot.hover.get_untracked().unwrap().point, point);
    assert_eq!(
        shared.get_untracked(),
        Some((first.id, point)),
        "Publish snapped cross, not offset mouse"
    );
    let window = &harness.app().windows()[0];
    let values = window.nodes_with_class("snapped-cursor-values");
    for value in &values {
        assert_eq!(
            window.dom().text_content(*value),
            "X: 0.250000s | Y: 0.750000"
        );
    }
    let first_readout = harness.app().windows()[0]
        .box_with_class("snapped-cursor-values")
        .unwrap();
    let legend = harness.app().windows()[0]
        .box_with_class("workbench-legend")
        .unwrap();
    let gear = harness.app().windows()[0]
        .box_with_class("plot-settings-trigger")
        .unwrap();
    assert!(gear.origin.x.0 >= legend.origin.x.0 && gear.origin.y.0 >= legend.origin.y.0);
    assert!(gear.origin.x.0 + gear.size.width.0 <= legend.origin.x.0 + legend.size.width.0);
    assert!(gear.origin.y.0 + gear.size.height.0 <= legend.origin.y.0 + legend.size.height.0);
    assert!(
        first_readout.origin.y.0 >= legend.origin.y.0 + legend.size.height.0,
        "Coordinates must sit below legend: {first_readout:?} {legend:?}"
    );
    let checks = harness.app().windows()[0].nodes_with_class("link-option");
    assert_eq!(checks.len(), 4);
    // Labelled checkboxes expose existing state; pointer opt-out clears publication.
    first.links.update(|links| links.pointer = false);
    harness.settle(128);
    assert!(shared.get_untracked().is_none());
    first.links.update(|links| links.pointer = true);
    harness.settle(128);
    assert_eq!(shared.get_untracked(), Some((first.id, point)));
    second.links.update(|links| links.pointer = false);
    harness.settle(128);
    assert_eq!(harness.app().windows()[0].dom().text_content(values[1]), "");
    for index in [1, 2, 3] {
        let rect = zgui::view::ViewHost::window_box(
            harness.app().windows()[0].host().as_ref(),
            checks[index],
        )
        .unwrap();
        let (x, y) = (
            rect.origin.x.0 + rect.size.width.0 / 2.0,
            rect.origin.y.0 + rect.size.height.0 / 2.0,
        );
        harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
        harness.deliver_to_first(pointer(PointerAction::Released, x, y));
        harness.settle(128);
    }
    assert!(
        first.links.get_untracked().x
            && first.links.get_untracked().y
            && !first.links.get_untracked().pointer
    );
    assert!(shared.get_untracked().is_none());
    click_class(&mut harness, "plot-settings-trigger");
    harness.advance(Duration::from_millis(500));
    harness.settle(128);
    let info = harness.app().windows()[0]
        .box_with_class("line-selection-info")
        .unwrap();
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        info.origin.x.0 + info.size.width.0 / 2.0,
        info.origin.y.0 + info.size.height.0 / 2.0,
    ));
    harness.settle(128);
    harness.advance(Duration::from_millis(700));
    harness.settle(128);
    let window = &harness.app().windows()[0];
    let help = window.nodes_with_class("line-selection-help");
    assert_eq!(help.len(), 1, "Hover must show selection help");
    assert!(window.dom().text_content(help[0]).contains("Ctrl-click"));
    harness.deliver_to_first(pointer(PointerAction::Moved, 1.0, 1.0));
    harness.advance(Duration::from_millis(500));
    harness.settle(128);
    let window = &harness.app().windows()[0];
    assert!(
        window
            .dom()
            .semantics(window.nodes_with_class("plot-editor-surface")[0])
            .is_some_and(|semantics| semantics.role == Role::Dialog)
    );
    let before = first.plot.style("1").color;
    let picker_node = window.nodes_with_class("color-square")[0];
    let wheel = window.box_with_class("color-square").unwrap();
    let image = window.box_with_class("color-square-image").unwrap();
    assert!(
        image.origin.y.0 + image.size.height.0 <= wheel.origin.y.0 + wheel.size.height.0,
        "Square canvas must stay inside square: {image:?} {wheel:?}"
    );
    let (x, y) = (wheel.origin.x.0 + 176.0, wheel.origin.y.0 + 90.0);
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    assert_ne!(
        first.plot.style("1").color,
        before,
        "Wheel bounds: {wheel:?}; click ({x}, {y})"
    );
    let initial = first.plot.style("1").color;
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    for offset in [30.0, 60.0, 90.0] {
        harness.deliver_to_first(pointer(
            PointerAction::Moved,
            wheel.origin.x.0 + offset,
            wheel.origin.y.0 + offset,
        ));
        harness.settle(128);
        assert_eq!(
            harness.app().windows()[0].nodes_with_class("color-square"),
            [picker_node]
        );
        assert_eq!(
            harness.app().windows()[0]
                .box_with_class("color-square")
                .unwrap(),
            wheel
        );
    }
    harness.deliver_to_first(pointer(
        PointerAction::Released,
        wheel.origin.x.0 + 90.0,
        wheel.origin.y.0 + 90.0,
    ));
    harness.settle(128);
    assert_ne!(first.plot.style("1").color, initial);
    let [r, g, b, _] = first.plot.style("1").color;
    let window = &harness.app().windows()[0];
    assert_eq!(
        window
            .dom()
            .text_content(window.nodes_with_class("color-value")[0]),
        format!("#{r:02X}{g:02X}{b:02X}")
    );
    first.plot.styles.update(|styles| {
        styles.get_mut("1").unwrap().color = [255, 0, 0, 255];
    });
    harness.settle(128);
    let hue = harness.app().windows()[0]
        .box_with_class("color-hue")
        .unwrap();
    assert!(
        hue.origin.y.0 >= wheel.origin.y.0 + wheel.size.height.0,
        "Hue strip must sit beyond square clip"
    );
    let y = hue.origin.y.0 + hue.size.height.0 / 2.0;
    let x = hue.origin.x.0 + hue.size.width.0 * 0.2;
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.settle(128);
    let start_color = first.plot.style("1").color;
    let x = hue.origin.x.0 + hue.size.width.0 * 0.7;
    harness.deliver_to_first(pointer(PointerAction::Moved, x, y));
    harness.settle(128);
    let dragged_color = first.plot.style("1").color;
    assert_ne!(
        dragged_color, start_color,
        "Hue drag must update color: {hue:?}"
    );
    let (h, _, _) = plot::zgui_series::hsv(dragged_color);
    assert!(
        (h - 251.0).abs() < 3.0,
        "Hue must follow pointer along rainbow strip: {h}"
    );
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    let window = &harness.app().windows()[0];
    let dots = window
        .nodes_with_class("line-marker")
        .into_iter()
        .find(|node| window.dom().text_content(*node) == "Dots")
        .unwrap();
    counts.borrow_mut().seen_dots = false;
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), dots).unwrap();
    let (x, y) = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    let window = &harness.app().windows()[0];
    let post = window
        .nodes_with_class("line-connection")
        .into_iter()
        .find(|node| window.dom().text_content(*node) == "Steps post")
        .unwrap();
    counts.borrow_mut().seen_steps = false;
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), post).unwrap();
    let (x, y) = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    assert_eq!(
        first.drawing.get_untracked()[&1],
        plot::line_draw::LineDraw {
            connection: plot::line_draw::Connection::Post,
            markers: true,
            ..Default::default()
        }
    );
    click_class(&mut harness, "zui-dialog__dismiss");
    harness.advance(Duration::from_millis(500));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        harness.settle(128);
        if counts.borrow().seen_steps {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Step render buffer never appeared: {:?}",
            counts.borrow()
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        counts.borrow().seen_dots,
        "Dots must render at original samples"
    );
    assert_eq!(first.lines.get_untracked()[0].data.samples().len(), 2);
    let graph = harness.app().windows()[0]
        .box_with_class("time-series-plot")
        .unwrap();
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        graph.origin.x.0 + graph.size.width.0 * 0.25 + 7.0,
        graph.origin.y.0 + graph.size.height.0 * 0.25 + 8.0,
    ));
    harness.settle(128);
    assert_eq!(
        first.plot.hover.get_untracked().unwrap().point,
        point,
        "Steps must retain original sample snapping"
    );
    click_class(&mut harness, "legend-remove");
    assert!(first.lines.get_untracked().is_empty());
    assert_eq!(
        second.lines.get_untracked().len(),
        1,
        "Legend removal must affect chosen plot only"
    );
    assert!(!first.plot.hidden.get_untracked().contains("1"));
    assert!(!first.plot.styles.get_untracked().contains_key("1"));
    assert!(!first.drawing.get_untracked().contains_key(&1));
    let window = &harness.app().windows()[0];
    assert_eq!(window.nodes_with_class("workbench-legend").len(), 1);
    assert_eq!(window.nodes_with_class("plot-settings-trigger").len(), 1);
}

#[test]
fn real_layout_fills_workspace_and_accepts_field_and_tab_drops() {
    let counts = Rc::new(RefCell::new(Counts::default()));
    let captured = Rc::clone(&counts);
    let handler = app().with_size(1500.0, 850.0)
        .with_renderer(Box::new(move |_, target| {
            let mut renderer = CaptureRenderer::new(); renderer.configure(target);
            Ok(Box::new(Capture { renderer, counts: Rc::clone(&captured) }))
        }))
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .into_handler(|| view! { WorkbenchApp(preferences = crate::preferences::ThemePreference::disabled(), initial_path = Some(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esc_status_log.ulg")))) })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1500.0),
        DevicePx(850.0),
    )));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        harness.settle(128);
        if !harness.app().windows()[0]
            .nodes_with_class("topic-branch")
            .is_empty()
        {
            break;
        }
        assert!(Instant::now() < deadline, "Browser never loaded");
        std::thread::sleep(Duration::from_millis(2));
    }
    let window = &harness.app().windows()[0];
    let browser = window.box_with_class("signal-list").unwrap();
    let grid = window.box_with_class("plot-grid").unwrap();
    let pane = window.box_with_class("workbench-plot").unwrap();
    let graph = window.box_with_class("time-series-plot").unwrap();
    assert!(
        pane.origin.y.0 + pane.size.height.0 <= 850.0,
        "Pane exceeds visible workspace: {pane:?}"
    );
    assert!(
        browser.origin.y.0 + browser.size.height.0 <= 850.0,
        "Field list exceeds visible workspace: {browser:?}"
    );
    assert!(browser.size.height.0 > 500.0, "{browser:?}");
    assert!(
        grid.size.width.0 > 1150.0 && pane.size.width.0 > 1150.0,
        "{grid:?} {pane:?}"
    );
    assert!(
        (pane.size.height.0 - graph.size.height.0).abs() < 40.0,
        "Graph must fill pane beneath tab strip: {pane:?} {graph:?}"
    );
    assert!((pane.size.width.0 - graph.size.width.0).abs() < 5.0);
    assert!(window.nodes_with_class("editor").is_empty());
    assert!(window.nodes_with_class("workbench-legend").is_empty());
    assert!(window.nodes_with_class("plot-settings-trigger").is_empty());
    let fps = window.box_with_class("fps-readout").unwrap();
    assert!(
        fps.origin.x.0 > 1400.0 && fps.origin.y.0 < 40.0,
        "FPS must sit top-right: {fps:?}"
    );
    let browser_node = window.nodes_with_class("browser")[0];
    click_class(&mut harness, "sidebar-toggle");
    let full = harness.app().windows()[0]
        .box_with_class("workbench-plot")
        .unwrap();
    assert!(
        full.origin.x.0 < 2.0 && full.size.width.0 > 1495.0,
        "Hidden sidebar must give plot full width: {full:?}"
    );
    assert_eq!(
        harness.app().windows()[0].nodes_with_class("browser"),
        [browser_node],
        "Sidebar state must survive hiding"
    );
    click_class(&mut harness, "sidebar-toggle");
    assert_eq!(
        harness.app().windows()[0]
            .box_with_class("workbench-plot")
            .unwrap(),
        pane
    );
    let window = &harness.app().windows()[0];
    let links = window.box_with_class("plot-links-trigger").unwrap();
    assert_eq!(
        window
            .dom()
            .text_content(window.nodes_with_class("plot-links-label")[0]),
        "Links: pointer"
    );
    let actions = window.box_with_class("duplicate-plot-button").unwrap();
    let maximize = window.box_with_class("maximize-button").unwrap();
    assert!(links.origin.x.0 < actions.origin.x.0 && actions.origin.x.0 < maximize.origin.x.0);
    assert!(
        links.origin.y.0 < pane.origin.y.0 + 28.0,
        "Links dropdown must live in plot top bar: {links:?}"
    );
    click_class(&mut harness, "plot-links-trigger");
    let window = &harness.app().windows()[0];
    let checks = window.nodes_with_class("link-option");
    assert_eq!(checks.len(), 4);
    let popup = window.box_with_class("plot-links-surface").unwrap();
    assert!(
        (popup.size.width.0 - links.size.width.0).abs() < 2.0 && popup.size.height.0 <= 120.0,
        "Links popup should use compact AFLR proportions: {popup:?}"
    );
    assert!(
        ((popup.origin.x.0 + popup.size.width.0 / 2.0)
            - (links.origin.x.0 + links.size.width.0 / 2.0))
            .abs()
            < 2.0,
        "Dropdown should align beneath trigger: {popup:?} {links:?}"
    );
    assert!(popup.origin.y.0 >= links.origin.y.0 + links.size.height.0);
    assert!(links.size.height.0 <= 22.0);
    assert!(checks.iter().all(|node| {
        window
            .dom()
            .semantics(*node)
            .is_some_and(|semantics| semantics.role == Role::MenuItemCheckBox)
    }));
    let state = zgui::view::AttrName::new("data-state");
    assert_eq!(
        checks
            .iter()
            .map(|node| window.dom().attribute(*node, state))
            .collect::<Vec<_>>(),
        [
            Some("unchecked".into()),
            Some("unchecked".into()),
            Some("unchecked".into()),
            Some("checked".into())
        ]
    );
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), checks[1]).unwrap();
    let (x, y) = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    for expected in ["checked", "unchecked"] {
        harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
        harness.deliver_to_first(pointer(PointerAction::Released, x, y));
        harness.settle(128);
        let window = &harness.app().windows()[0];
        assert_eq!(
            window.dom().attribute(checks[1], state).as_deref(),
            Some(expected)
        );
        assert_eq!(
            window.nodes_with_class("link-controls").len(),
            1,
            "Dropdown must stay open while toggling links"
        );
        assert_eq!(
            window
                .dom()
                .text_content(window.nodes_with_class("plot-links-label")[0]),
            if expected == "checked" {
                "Links: X, pointer"
            } else {
                "Links: pointer"
            }
        );
    }
    for (index, expected, label) in [
        (0, [true, true, true, true], "Links: all"),
        (2, [false, true, false, true], "Links: X, pointer"),
        (0, [true, true, true, true], "Links: all"),
        (0, [false, false, false, false], "Links: off"),
        (3, [false, false, false, true], "Links: pointer"),
    ] {
        let window = &harness.app().windows()[0];
        let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), checks[index]).unwrap();
        let (x, y) = (
            rect.origin.x.0 + rect.size.width.0 / 2.0,
            rect.origin.y.0 + rect.size.height.0 / 2.0,
        );
        harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
        harness.deliver_to_first(pointer(PointerAction::Released, x, y));
        harness.settle(128);
        let window = &harness.app().windows()[0];
        for (node, checked) in checks.iter().zip(expected) {
            assert_eq!(
                window.dom().attribute(*node, state).as_deref(),
                Some(if checked { "checked" } else { "unchecked" })
            );
        }
        assert_eq!(
            window
                .dom()
                .text_content(window.nodes_with_class("plot-links-label")[0]),
            label
        );
    }
    click_class(&mut harness, "plot-links-trigger");
    harness.advance(Duration::from_millis(500));
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("link-controls")
            .is_empty()
    );
    let window = &harness.app().windows()[0];
    let topic = window.box_with_class("topic-branch").unwrap();
    let (x, y) = (
        topic.origin.x.0 + 30.0,
        topic.origin.y.0 + topic.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    let instance = harness.app().windows()[0]
        .box_with_class("field-branch")
        .unwrap();
    let stem = harness.app().windows()[0]
        .box_with_class("tree-stem")
        .unwrap();
    // Parameters may precede raw topics; inspect arrow belonging to topic we opened.
    let window = &harness.app().windows()[0];
    let topic_node = window.nodes_with_class("topic-branch")[0];
    let arrow_node = window
        .dom()
        .children(topic_node)
        .into_iter()
        .find(|node| window.nodes_with_class("tree-arrow").contains(node))
        .unwrap();
    let arrow = zgui::view::ViewHost::window_box(window.host().as_ref(), arrow_node).unwrap();
    assert!(
        (arrow.origin.x.0 + arrow.size.width.0 / 2.0 - stem.origin.x.0).abs() < 1.0,
        "Chevron must align with connector trunk: {arrow:?} {stem:?}"
    );
    assert!(
        (arrow.origin.y.0 + arrow.size.height.0 / 2.0
            - topic.origin.y.0
            - topic.size.height.0 / 2.0)
            .abs()
            < 1.0,
        "Chevron must center vertically in row: arrow={arrow:?}, topic={topic:?}"
    );
    let guide = harness.app().windows()[0]
        .box_with_class("tree-guide")
        .unwrap();
    assert_eq!(
        stem.origin.x, guide.origin.x,
        "Parent and child trunks must align"
    );
    assert_eq!(
        stem.origin.y.0 + stem.size.height.0,
        guide.origin.y.0,
        "Parent stem must meet next virtual row"
    );
    assert_eq!(guide.size.width.0, 1.0);
    let (x, y) = (
        instance.origin.x.0 + 30.0,
        instance.origin.y.0 + instance.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    let source = harness.app().windows()[0]
        .box_with_class("metric-option")
        .unwrap();
    let source_x = source.origin.x.0 + 20.0;
    let source_y = source.origin.y.0 + source.size.height.0 / 2.0;
    let target_x = pane.origin.x.0 + pane.size.width.0 / 2.0;
    let target_y = pane.origin.y.0 + pane.size.height.0 / 2.0;
    harness.deliver_to_first(pointer(PointerAction::Pressed, source_x, source_y));
    harness.deliver_to_first(pointer(PointerAction::Moved, target_x, target_y));
    harness.settle(128);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("signal-drop-active")
            .len(),
        1
    );
    harness.deliver_to_first(pointer(PointerAction::Released, target_x, target_y));
    loop {
        harness.settle(128);
        if !harness.app().windows()[0]
            .nodes_with_class("legend-entry")
            .is_empty()
        {
            break;
        }
        assert!(Instant::now() < deadline, "Field drag never added line");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(counts.borrow().peak_marks > 0 && counts.borrow().glyphs > 0);
    let settings = harness.app().windows()[0]
        .box_with_class("plot-settings-trigger")
        .unwrap();
    let (x, y) = (
        settings.origin.x.0 + 15.0,
        settings.origin.y.0 + settings.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("line-button")
            .len(),
        1
    );
    let window = &harness.app().windows()[0];
    assert_eq!(
        window.nodes_with_class("line-style").len(),
        1,
        "Edit only selected line"
    );
    let editor = window.box_with_class("plot-editor-surface").unwrap();
    assert!(
        editor.size.width.0 <= 750.0 && editor.origin.y.0 + editor.size.height.0 <= 850.0,
        "Editor must fit window: {editor:?}"
    );
    assert!(
        window.nodes_with_class("link-controls").is_empty(),
        "Link controls belong in plot header dropdown"
    );
    click_class(&mut harness, "settings-done");
    harness.advance(Duration::from_millis(500));
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("add-field-button")
            .is_empty()
    );
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("derive-selected-field")
            .is_empty()
    );
    let window = &harness.app().windows()[0];
    let source = zgui::view::ViewHost::window_box(
        window.host().as_ref(),
        window.nodes_with_class("metric-option")[0],
    )
    .unwrap();
    let button = window.box_with_class("open-filters").unwrap();
    let (sx, sy) = (
        source.origin.x.0 + 20.0,
        source.origin.y.0 + source.size.height.0 / 2.0,
    );
    let (tx, ty) = (
        button.origin.x.0 + button.size.width.0 / 2.0,
        button.origin.y.0 + button.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, sx, sy));
    harness.deliver_to_first(pointer(PointerAction::Moved, tx, ty));
    harness.settle(128);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-workspace")
            .len(),
        1
    );
    harness.deliver_to_first(pointer(PointerAction::Cancelled, tx, ty));
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("filter-source-row")
            .is_empty()
    );

    let window = &harness.app().windows()[0];
    let filter_window = window.box_with_class("filter-workspace").unwrap();
    let browser = window.box_with_class("signal-list").unwrap();
    assert!(filter_window.origin.x.0 >= browser.origin.x.0 + browser.size.width.0);
    assert!(filter_window.origin.y.0 + filter_window.size.height.0 <= 850.0);
    assert!(window.nodes_with_class("field-select-checkbox").is_empty());
    assert_eq!(window.nodes_with_class("new-derived-field").len(), 1);
    let collapse = window.box_with_class("collapse-fields").unwrap();
    let filters = window.box_with_class("open-filters").unwrap();
    assert!((collapse.origin.y.0 - filters.origin.y.0).abs() < 1.0);
    assert!(collapse.origin.x.0 >= filters.origin.x.0 + filters.size.width.0);
    let node = window.nodes_with_class("metric-option")[1];
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), node).unwrap();
    let (x, y) = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    for action in [PointerAction::Pressed, PointerAction::Released] {
        let mut event = pointer(action, x, y);
        if let SurfaceEvent::Pointer { modifiers, .. } = &mut event {
            *modifiers = Modifiers::CONTROL;
        }
        harness.deliver_to_first(event);
    }
    harness.settle(128);
    let window = &harness.app().windows()[0];
    assert_eq!(window.nodes_with_class("metric-option-selected").len(), 2);
    let source = zgui::view::ViewHost::window_box(
        window.host().as_ref(),
        window.nodes_with_class("metric-option")[0],
    )
    .unwrap();
    let basket = window.box_with_class("filter-sources").unwrap();
    let (sx, sy) = (
        source.origin.x.0 + 20.0,
        source.origin.y.0 + source.size.height.0 / 2.0,
    );
    let (tx, ty) = (
        basket.origin.x.0 + basket.size.width.0 / 2.0,
        basket.origin.y.0 + basket.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, sx, sy));
    harness.deliver_to_first(pointer(PointerAction::Moved, tx, ty));
    harness.settle(128);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-sources-hover")
            .len(),
        1
    );
    harness.deliver_to_first(pointer(PointerAction::Released, tx, ty));
    harness.settle(128);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-source-row")
            .len(),
        2
    );
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-output-name")
            .len(),
        2
    );
    let opened = Instant::now();
    click_class(&mut harness, "filter-type");
    eprintln!("Filter dropdown first open: {:?}", opened.elapsed());
    let window = &harness.app().windows()[0];
    let popup = window.box_with_class("filter-type-surface").unwrap();
    assert!(popup.size.height.0 < 260.0 && popup.size.width.0 > 100.0);
    let item = window
        .nodes_with_class("zui-select__item")
        .into_iter()
        .find(|node| window.dom().text_content(*node).contains("High-pass"))
        .unwrap();
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), item).unwrap();
    let (x, y) = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Released, x, y));
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .box_with_class("filter-type-surface")
            .is_none()
    );
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("filter-output-name")
            .len(),
        2
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    click_class(&mut harness, "save-derived-field");
    loop {
        harness.settle(128);
        let window = &harness.app().windows()[0];
        if window
            .nodes_with_class("filter-result-status")
            .iter()
            .any(|node| window.dom().text_content(*node) == "Created 2 fields")
        {
            break;
        }
        assert!(Instant::now() < deadline, "Derived batch never computed");
        std::thread::sleep(Duration::from_millis(2));
    }
    click_class(&mut harness, "plot-derived-field");
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("legend-entry")
            .len(),
        3
    );
    assert!(counts.borrow().peak_marks >= 3);
    click_class(&mut harness, "duplicate-plot-button");
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("legend-entry")
            .len(),
        6
    );
    let second = harness.app().windows()[0]
        .box_with_class("workbench-plot-active")
        .unwrap();
    assert!(
        second.size.width.0 > 500.0 && second.size.width.0 < 800.0,
        "{second:?}"
    );
    click_class(&mut harness, "equalize-panels");
    let restored = harness.app().windows()[0]
        .box_with_class("workbench-plot-active")
        .unwrap();
    assert!(
        (restored.size.width.0 - second.size.width.0).abs() < 2.0,
        "Equalizer must restore balanced panes: {restored:?}"
    );
    let first = harness.app().windows()[0]
        .box_with_class("workbench-plot")
        .unwrap();
    click_class(&mut harness, "open-filters");
    harness.advance(Duration::from_millis(500));
    harness.settle(128);
    click_class(&mut harness, "chain-derived-field");
    click_class(&mut harness, "save-derived-field");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        harness.settle(128);
        let window = &harness.app().windows()[0];
        if window
            .nodes_with_class("filter-result-status")
            .iter()
            .any(|node| window.dom().text_content(*node) == "Created 2 fields")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Second filter stage never computed"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("derived-library-item")
            .len(),
        4
    );
    click_class(&mut harness, "clear-filter-sources");
    let window = &harness.app().windows()[0];
    assert!(window.nodes_with_class("filter-source-row").is_empty());
    assert!(!window.nodes_with_class("chain-derived-field").is_empty());
    assert!(
        window.box_with_class("filter-mode-edit").is_none(),
        "Creating fields must leave a new draft"
    );
    assert_eq!(window.nodes_with_class("derived-library-item").len(), 4);
    click_class(&mut harness, "save-derived-field");
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("filter-error")
            .is_empty()
    );
    click_class(&mut harness, "filters-done");
    harness.advance(Duration::from_millis(500));
    harness.settle(128);
    for _ in 0..8 {
        let window = &harness.app().windows()[0];
        if window.nodes_with_class("metric-option").len() >= 2 {
            break;
        }
        let branch = window
            .nodes_with_class("tree-branch")
            .into_iter()
            .find(|node| {
                window
                    .dom()
                    .semantics(*node)
                    .is_some_and(|semantics| semantics.expanded == Some(false))
            })
            .unwrap();
        let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), branch).unwrap();
        let (x, y) = (
            rect.origin.x.0 + 20.0,
            rect.origin.y.0 + rect.size.height.0 / 2.0,
        );
        harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
        harness.deliver_to_first(pointer(PointerAction::Released, x, y));
        harness.settle(128);
    }
    let window = &harness.app().windows()[0];
    let source = zgui::view::ViewHost::window_box(
        window.host().as_ref(),
        window.nodes_with_class("metric-option")[2],
    )
    .unwrap();
    let (sx, sy) = (
        source.origin.x.0 + 20.0,
        source.origin.y.0 + source.size.height.0 / 2.0,
    );
    let (tx, ty) = (
        second.origin.x.0 + second.size.width.0 / 2.0,
        second.origin.y.0 + second.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, sx, sy));
    harness.deliver_to_first(pointer(PointerAction::Moved, tx, ty));
    harness.deliver_to_first(pointer(PointerAction::Released, tx, ty));
    loop {
        harness.settle(128);
        if harness.app().windows()[0]
            .nodes_with_class("legend-entry")
            .len()
            == 7
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Second plot did not receive dropped field"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    // Drag second plot's tab into first pane's center, creating one tab group.
    let (sx, sy) = (second.origin.x.0 + 30.0, second.origin.y.0 + 12.0);
    let (tx, ty) = (
        first.origin.x.0 + first.size.width.0 / 2.0,
        first.origin.y.0 + first.size.height.0 / 2.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, sx, sy));
    harness.deliver_to_first(pointer(PointerAction::Moved, tx, ty));
    harness.settle(128);
    assert!(
        !harness.app().windows()[0]
            .nodes_with_class("tab-drag-source")
            .is_empty()
    );
    harness.deliver_to_first(pointer(PointerAction::Released, tx, ty));
    std::thread::sleep(Duration::from_millis(180));
    harness.advance(Duration::from_millis(500));
    harness.settle(256);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("workbench-plot")
            .iter()
            .filter(|node| harness.app().windows()[0]
                .nodes_with_class("hidden")
                .contains(node))
            .count(),
        1
    );
    let grouped = harness.app().windows()[0]
        .box_with_class("workbench-plot-active")
        .unwrap();
    assert!(
        grouped.size.width.0 > 1150.0,
        "Docked tabs must fill workspace: {grouped:?}"
    );
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(350.0),
    )));
    harness.settle(256);
    let window = &harness.app().windows()[0];
    let list = window.box_with_class("signal-list").unwrap();
    let pane = window.box_with_class("workbench-plot-active").unwrap();
    let grid = window.box_with_class("plot-grid").unwrap();
    assert!(
        (grid.origin.y.0 + grid.size.height.0 - 350.0).abs() < 1.0,
        "Workspace must reach window bottom: {grid:?}"
    );
    assert!(
        pane.origin.y.0 + pane.size.height.0 <= 350.0,
        "Resize must fit plot: {pane:?}"
    );
    assert!((pane.size.height.0 - grid.size.height.0).abs() < 2.0);
    assert!(list.size.height.0 > 0.0 && list.origin.y.0 + list.size.height.0 < 350.0);
    let first_row = window.nodes_with_class("tree-row")[0];
    harness.deliver_to_first(SurfaceEvent::Wheel {
        event: zgui::vocab::WheelEvent {
            delta: zgui::vocab::ScrollDelta::Pixels(Size::new(CssPx(0.0), CssPx(500.0))),
            phase: zgui::vocab::ScrollPhase::Moved,
            position: Point::new(CssPx(list.origin.x.0 + 20.0), CssPx(list.origin.y.0 + 10.0)),
            id: zgui::vocab::PointerId::MOUSE,
            kind: zgui::vocab::PointerKind::Mouse,
        },
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(256);
    let window = &harness.app().windows()[0];
    assert!(
        !window.nodes_with_class("tree-row").contains(&first_row),
        "Wheel must scroll field rows out of view"
    );
    assert_eq!(window.box_with_class("signal-list").unwrap(), list);
    assert_eq!(window.box_with_class("plot-grid").unwrap(), grid);
    // Stacked docking must shrink panes instead of growing canvas below window.
    let (sx, sy) = (pane.origin.x.0 + 30.0, pane.origin.y.0 + 12.0);
    let (tx, ty) = (
        pane.origin.x.0 + pane.size.width.0 / 2.0,
        pane.origin.y.0 + pane.size.height.0 - 4.0,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, sx, sy));
    harness.deliver_to_first(pointer(PointerAction::Moved, tx, ty));
    harness.settle(128);
    harness.deliver_to_first(pointer(PointerAction::Released, tx, ty));
    std::thread::sleep(Duration::from_millis(180));
    harness.advance(Duration::from_millis(500));
    harness.settle(256);
    let window = &harness.app().windows()[0];
    assert_eq!(window.nodes_with_class("split-grip-stacked").len(), 1);
    assert!(
        window
            .nodes_with_class("workbench-plot")
            .iter()
            .all(|node| !window.nodes_with_class("hidden").contains(node))
    );
    let first = window.box_with_class("workbench-plot").unwrap();
    let second = window.box_with_class("workbench-plot-active").unwrap();
    for rect in [first, second] {
        assert!(
            rect.size.height.0 > 80.0 && rect.size.height.0 < 200.0,
            "{rect:?}"
        );
        assert!(
            rect.origin.y.0 + rect.size.height.0 <= 350.0,
            "Stacked pane exceeds viewport: {rect:?}"
        );
    }
    assert_eq!(window.box_with_class("plot-grid").unwrap(), grid);
}

#[test]
fn legend_drag_reorders_moves_appends_and_cancels_without_toggling_visibility() {
    use crate::{
        state::{Line, Workbench},
        workspace::PlotWorkspaceProps,
    };
    use zgui_ui_tokens::prelude::*;
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let counts = Rc::new(RefCell::new(Counts::default()));
    let handler = app().with_size(1200.0, 700.0)
        .with_renderer(Box::new(move |_, target| {
            let mut renderer = CaptureRenderer::new(); renderer.configure(target);
            Ok(Box::new(Capture { renderer, counts: Rc::clone(&counts) }))
        }))
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .into_handler(move || {
            let workbench = Workbench::new();
            let first = workbench.pane();
            let second = workbench.add_plot(first.id, false, false);
            let data = plot::time_series::SeriesData::new(vec![plot::PlotPoint { x: 0.0, y: 1.0 }, plot::PlotPoint { x: 1.0, y: 2.0 }]).unwrap();
            let line = |id| Line { id, source: format!("line {id}"), name: format!("line {id}"), data: data.clone(),  revision: 0 };
            first.lines.set(vec![line(1), line(2), line(3)]);
            second.lines.set(vec![line(4)]);
            first.plot.hidden.update(|hidden| { hidden.insert("3".into()); });
            first.plot.styles.update(|styles| { let mut style = plot::plot_data::SeriesStyle::for_id("3"); style.color = [10, 160, 240, 255]; style.width = 4.0; styles.insert("3".into(), style); });
            first.drawing.update(|drawing| { drawing.insert(3, plot::line_draw::LineDraw { connection: plot::line_draw::Connection::Post, markers: true, ..Default::default() }); });
            *built.borrow_mut() = Some((workbench, first, second));
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "page workbench") { row(class = "body") { PlotWorkspace(workbench = workbench) } } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.0),
        DevicePx(700.0),
    )));
    harness.settle(256);
    let (workbench, first, second) = model.borrow().unwrap();
    let ids = |pane: crate::state::Pane| {
        pane.lines
            .with_untracked(|lines| lines.iter().map(|line| line.id).collect::<Vec<_>>())
    };
    let label_point = |harness: &Harness<zgui::runtime::Runtime>, id, bottom: bool| {
        let window = &harness.app().windows()[0];
        let node = window
            .nodes_with_class("legend-drag-source")
            .into_iter()
            .find(|node| {
                window
                    .dom()
                    .text_content(*node)
                    .contains(&format!("line {id}"))
            })
            .unwrap();
        let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), node).unwrap();
        (
            rect.origin.x.0 + 20.0,
            rect.origin.y.0 + rect.size.height.0 * if bottom { 0.8 } else { 0.2 },
        )
    };
    let drag = |harness: &mut Harness<zgui::runtime::Runtime>, id, target: (f32, f32)| {
        let source = label_point(harness, id, false);
        harness.deliver_to_first(pointer(PointerAction::Pressed, source.0, source.1));
        harness.deliver_to_first(pointer(PointerAction::Moved, target.0, target.1));
        harness.settle(128);
        harness.deliver_to_first(pointer(PointerAction::Released, target.0, target.1));
        harness.settle(256);
    };
    let target = label_point(&harness, 1, false);
    drag(&mut harness, 3, target);
    assert_eq!(ids(first), [3, 1, 2]);
    assert!(first.plot.hidden.get_untracked().contains("3"));
    let original = first.lines.get_untracked()[0].clone();
    let style = first.plot.styles.get_untracked()["3"];
    let drawing = first.drawing.get_untracked()[&3];
    let rect = second.dock.node.window_bounds().unwrap();
    let center = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    drag(&mut harness, 3, center);
    assert_eq!(ids(first), [1, 2]);
    assert_eq!(ids(second), [4, 3], "Plot area appends moved line");
    assert_eq!(second.plot.styles.get_untracked()["3"], style);
    assert_eq!(second.drawing.get_untracked()[&3], drawing);
    assert!(second.plot.hidden.get_untracked().contains("3"));
    assert!(!first.plot.hidden.get_untracked().contains("3"));
    assert!(!first.plot.styles.get_untracked().contains_key("3"));
    assert!(!first.drawing.get_untracked().contains_key(&3));
    let moved = second.lines.get_untracked()[1].clone();
    assert_eq!(moved.name, original.name);
    assert_eq!(
        moved.data.samples().as_ptr(),
        original.data.samples().as_ptr()
    );
    assert_eq!(
        moved.data.samples().as_ptr(),
        original.data.samples().as_ptr()
    );
    let target = label_point(&harness, 4, false);
    drag(&mut harness, 2, target);
    assert_eq!(ids(first), [1]);
    assert_eq!(ids(second), [2, 4, 3], "Legend drop inserts before row");
    let target = label_point(&harness, 3, true);
    drag(&mut harness, 2, target);
    assert_eq!(ids(second), [4, 3, 2]);
    assert!(second.plot.hidden.get_untracked().contains("3"));
    assert!(!second.plot.hidden.get_untracked().contains("2"));
    drag(&mut harness, 1, (-20.0, -20.0));
    assert_eq!(ids(first), [1]);
    let source = label_point(&harness, 1, false);
    harness.deliver_to_first(pointer(PointerAction::Pressed, source.0, source.1));
    harness.deliver_to_first(pointer(PointerAction::Moved, center.0, center.1));
    harness.deliver_to_first(pointer(PointerAction::Cancelled, center.0, center.1));
    harness.settle(128);
    assert!(workbench.line_drag.state.get_untracked().is_none());
    assert!(workbench.line_drag.target.get_untracked().is_none());
    assert_eq!(ids(first), [1]);
    let source = label_point(&harness, 1, false);
    harness.deliver_to_first(pointer(PointerAction::Pressed, source.0, source.1));
    harness.deliver_to_first(pointer(PointerAction::Moved, center.0, center.1));
    harness.deliver_to_first(SurfaceEvent::Key {
        state: zgui::vocab::KeyState::Pressed,
        event: zgui::vocab::KeyEvent::named(
            zgui::vocab::NamedKey::Escape,
            zgui::vocab::PhysicalKey::Unidentified(0),
        ),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.deliver_to_first(pointer(PointerAction::Released, center.0, center.1));
    harness.settle(128);
    assert_eq!(ids(first), [1], "Escape cancels legend move");
    drag(&mut harness, 1, center);
    assert!(ids(first).is_empty());
    assert_eq!(ids(second), [4, 3, 2, 1]);
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("workbench-legend")
            .len(),
        1
    );
    assert!(workbench.line_drag.state.get_untracked().is_none());
    assert!(workbench.line_drag.target.get_untracked().is_none());
    let point = label_point(&harness, 1, false);
    harness.deliver_to_first(pointer(PointerAction::Pressed, point.0, point.1));
    harness.deliver_to_first(pointer(PointerAction::Released, point.0, point.1));
    harness.settle(128);
    assert!(
        second.plot.hidden.get_untracked().contains("1"),
        "Click still toggles visibility"
    );
    harness.deliver_to_first(SurfaceEvent::Key {
        state: zgui::vocab::KeyState::Pressed,
        event: zgui::vocab::KeyEvent::named(
            zgui::vocab::NamedKey::ArrowUp,
            zgui::vocab::PhysicalKey::Unidentified(0),
        ),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(128);
    assert_eq!(
        ids(second),
        [4, 3, 1, 2],
        "Focused legend label supports keyboard reorder"
    );
}

#[test]
fn shift_release_cancels_box_zoom_before_mouse_release() {
    use plot::{
        interaction::{Axes, Gesture, HitZones},
        zgui_pointer::PlotPointer,
        zgui_surface::PlotSurfaceProps,
    };
    use zgui::reactive::UnsyncCallback;
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let counts = Rc::new(RefCell::new(Counts::default()));
    let handler = app().with_size(900.0, 600.0)
        .with_renderer(Box::new(move |_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(Capture { renderer, counts: Rc::clone(&counts) })) }))
        .with_stylesheet(format!("{}\n.zoom-test {{ width: 900px; height: 600px; }}", plot::zgui_surface::stylesheet()))
        .into_handler(move || {
            let pointer = PlotPointer::new(HitZones::default(), Axes::BOTH);
            let ranges = RwSignal::new_local(plot::Ranges::new((0.0, 10.0), (0.0, 10.0)));
            *built.borrow_mut() = Some((pointer, ranges));
            view! { PlotSurface(node_ref = NodeRef::new(), class = "zoom-test", label = "Zoom test".into(), pointer = pointer,
                geometry = UnsyncCallback::new(|()| plot::PlotGeometry::with_bounds(0.0, 0.0, 900.0, 600.0, 900.0, 600.0)),
                ranges = UnsyncCallback::new(move |()| ranges.get_untracked()),
                on_update = UnsyncCallback::new(move |update: plot::interaction::RangeUpdate| ranges.update(|ranges| *ranges = update.apply(*ranges))),
                on_cancel = UnsyncCallback::new(move |initial| ranges.set(initial)), on_reset = UnsyncCallback::new(|()| {}),
                on_hover = UnsyncCallback::new(|_| {}), ticks = Memo::new(|_| Vec::new()), background = "#12181c".into()) { box {} } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(600.0),
    )));
    harness.settle(256);
    let (pointer_state, ranges) = model.borrow().unwrap();
    let original = ranges.get_untracked();
    let shifted = |action, x, y| {
        let mut event = pointer(action, x, y);
        if let SurfaceEvent::Pointer { modifiers, .. } = &mut event {
            *modifiers = Modifiers::SHIFT;
        }
        event
    };
    harness.deliver_to_first(shifted(PointerAction::Pressed, 200.0, 150.0));
    harness.deliver_to_first(shifted(PointerAction::Moved, 400.0, 300.0));
    harness.advance(Duration::from_millis(20));
    harness.settle(128);
    assert_eq!(pointer_state.active(), Some(Gesture::Select));
    assert!(pointer_state.selection().is_some());
    harness.deliver_to_first(SurfaceEvent::Key {
        state: zgui::vocab::KeyState::Released,
        event: zgui::vocab::KeyEvent::named(
            zgui::vocab::NamedKey::Shift,
            zgui::vocab::PhysicalKey::Unidentified(0),
        ),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.advance(Duration::from_millis(20));
    harness.settle(128);
    assert_eq!(pointer_state.active(), None);
    assert_eq!(pointer_state.selection(), None);
    assert_eq!(ranges.get_untracked(), original);
    harness.deliver_to_first(shifted(PointerAction::Moved, 500.0, 350.0));
    harness.deliver_to_first(pointer(PointerAction::Released, 500.0, 350.0));
    harness.settle(128);
    assert_eq!(
        ranges.get_untracked(),
        original,
        "Cancelled zoom must not restart or commit on release"
    );
    harness.deliver_to_first(shifted(PointerAction::Pressed, 200.0, 150.0));
    harness.deliver_to_first(shifted(PointerAction::Moved, 400.0, 300.0));
    harness.deliver_to_first(shifted(PointerAction::Released, 400.0, 300.0));
    harness.settle(128);
    assert_ne!(
        ranges.get_untracked(),
        original,
        "Holding Shift through release still commits zoom"
    );
}

#[test]
fn splitter_drag_pushes_other_edge_only_at_minimum_and_restores_it_on_reverse() {
    use crate::{state::Workbench, workspace::PlotWorkspaceProps};
    use zgui_ui_tokens::prelude::*;
    let counts = Rc::new(RefCell::new(Counts::default()));
    let handler = app().with_size(1800.0, 900.0)
        .with_renderer(Box::new(move |_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(Capture { renderer, counts: Rc::clone(&counts) })) }))
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .into_handler(move || {
            let workbench = Workbench::new(); let first = workbench.pane();
            let second = workbench.add_plot(first.id, false, false); let third = workbench.add_plot(second.id, false, false);
            workbench.layout.set(plot::workspace::layout::Layout::flat(&[(first.id, 1.0), (second.id, 1.0), (third.id, 1.0)], false));
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "page workbench") { row(class = "body") { PlotWorkspace(workbench = workbench) } } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1800.0),
        DevicePx(900.0),
    )));
    harness.settle(256);
    let boxes = |harness: &Harness<zgui::runtime::Runtime>| {
        let window = &harness.app().windows()[0];
        window
            .nodes_with_class("split-grip")
            .iter()
            .map(|node| zgui::view::ViewHost::window_box(window.host().as_ref(), *node).unwrap())
            .collect::<Vec<_>>()
    };
    let before = boxes(&harness);
    assert_eq!(before.len(), 2);
    let (x, y) = (before[0].origin.x.0 + 3.0, before[0].origin.y.0 + 80.0);
    harness.deliver_to_first(pointer(PointerAction::Pressed, x, y));
    harness.deliver_to_first(pointer(PointerAction::Moved, x + 100.0, y));
    harness.settle(128);
    harness.deliver_to_first(pointer(PointerAction::Moved, x + 130.0, y));
    harness.settle(128);
    harness.deliver_to_first(pointer(PointerAction::Moved, x + 500.0, y));
    harness.settle(128);
    let pushed = boxes(&harness);
    assert!((pushed[0].origin.x.0 - before[0].origin.x.0 - 500.0).abs() < 2.0);
    assert!((pushed[1].origin.x.0 - pushed[0].origin.x.0 - 6.0 - 220.0).abs() < 2.0);
    assert!(pushed[1].origin.x.0 > before[1].origin.x.0 + 1.0);
    harness.deliver_to_first(pointer(PointerAction::Moved, x + 130.0, y));
    harness.settle(128);
    harness.deliver_to_first(pointer(PointerAction::Released, x + 130.0, y));
    harness.settle(128);
    let after = boxes(&harness);
    assert!(
        (after[0].origin.x.0 - before[0].origin.x.0 - 130.0).abs() < 2.0,
        "Dragged splitter must follow pointer: {before:?} {after:?}"
    );
    assert!(
        (after[1].origin.x.0 - before[1].origin.x.0).abs() < 1.0,
        "Other splitter must stay pinned"
    );
}

#[test]
fn workspace_arrow_guide_previews_and_docks_beside_stacked_panels() {
    use crate::{state::Workbench, workspace::PlotWorkspaceProps};
    use plot::workspace::layout::DropZone;
    use zgui_ui_tokens::prelude::*;
    for zone in [DropZone::Left, DropZone::Right] {
        let saved = Rc::new(RefCell::new(None));
        let captured = Rc::clone(&saved);
        let counts = Rc::new(RefCell::new(Counts::default()));
        let handler = app().with_size(1600.0, 1000.0)
            .with_renderer(Box::new(move |_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(Capture { renderer, counts: Rc::clone(&counts) })) }))
            .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
            .into_handler(move || {
                let workbench = Workbench::new(); let first = workbench.pane();
                let second = workbench.add_plot(first.id, true, false);
                let third = workbench.add_plot(first.id, false, true);
                *captured.borrow_mut() = Some((workbench,first,second,third));
                view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "page workbench") { row(class = "body") { PlotWorkspace(workbench = workbench) } } } }
            }).unwrap();
        let mut harness = Harness::new(handler.into_runtime());
        harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
            DevicePx(1600.0),
            DevicePx(1000.0),
        )));
        harness.settle(256);
        let (workbench, first, second, third) = saved.borrow().unwrap();
        assert!(
            !third.links.get_untracked().x
                && !third.links.get_untracked().y
                && third.links.get_untracked().pointer
        );
        let guide = workbench
            .dock
            .group_guides()
            .into_iter()
            .max_by(|a, b| {
                (a.rect.width * a.rect.height).total_cmp(&(b.rect.width * b.rect.height))
            })
            .unwrap();
        let target = guide.button(zone);
        let (x, y) = ((target.x + 11.0) as f32, (target.y + 11.0) as f32);
        let tab = workbench
            .dock
            .nodes
            .with_value(|nodes| *nodes.get(&(third.id, third.id)).unwrap());
        let bounds = tab.window_bounds().unwrap();
        let scale = tab.scale();
        let start = (
            (bounds.origin.x.0 + bounds.size.width.0 / 2.0) / scale,
            (bounds.origin.y.0 + bounds.size.height.0 / 2.0) / scale,
        );
        harness.deliver_to_first(pointer(PointerAction::Pressed, start.0, start.1));
        harness.deliver_to_first(pointer(PointerAction::Moved, x, y));
        harness.settle(128);
        assert_eq!(
            workbench.dock.target.get_untracked(),
            Some((third.id, zone))
        );
        let window = &harness.app().windows()[0];
        assert!(window.nodes_with_class("workspace-edge").is_empty());
        assert!(
            window.nodes_with_class("dock-drop-zone").is_empty(),
            "Region guide must hide individual-pane arrows"
        );
        let previews: Vec<_> = window
            .nodes_with_class("dock-preview-source")
            .iter()
            .filter_map(|node| zgui::view::ViewHost::window_box(window.host().as_ref(), *node))
            .filter(|rect| rect.size.height.0 > 0.0)
            .collect();
        assert!(
            previews
                .iter()
                .any(|rect| (rect.size.height.0 - 1000.0).abs() < 1.0
                    && (rect.size.width.0 - 797.0).abs() < 1.0),
            "Workspace preview must span stacked panes: {previews:?}"
        );
        harness.deliver_to_first(pointer(PointerAction::Released, x, y));
        harness.settle(256);
        let a = first.dock.node.window_bounds().unwrap();
        let b = second.dock.node.window_bounds().unwrap();
        let c = third.dock.node.window_bounds().unwrap();
        assert!((c.size.height.0 - 1000.0).abs() < 1.0);
        assert_eq!(a.origin.x, b.origin.x);
        assert_eq!(a.size.width, b.size.width);
        assert!(if zone == DropZone::Left {
            c.origin.x.0 < a.origin.x.0
        } else {
            c.origin.x.0 > a.origin.x.0
        });
        assert!(
            harness.app().windows()[0]
                .nodes_with_class("workspace-dock-overlay")
                .is_empty()
        );
    }
}

#[test]
fn nested_group_guides_dock_in_middle_without_moving_outer_siblings() {
    use crate::{state::Workbench, workspace::PlotWorkspaceProps};
    use plot::workspace::layout::{DropZone, Layout};
    use zgui_ui_tokens::prelude::*;
    for zone in [DropZone::Left, DropZone::Bottom] {
        let saved = Rc::new(RefCell::new(None));
        let captured = Rc::clone(&saved);
        let counts = Rc::new(RefCell::new(Counts::default()));
        let handler=app().with_size(2400.0,1200.0)
            .with_renderer(Box::new(move |_,target| { let mut renderer=CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(Capture{renderer,counts:Rc::clone(&counts)})) }))
            .with_stylesheet(format!("{}\n{}\n{}\n{}",plot::zgui_surface::stylesheet(),plot::zgui_time_series::stylesheet(),plot::zgui_workspace::stylesheet(),crate::theme::stylesheet()))
            .into_handler(move || {
                let workbench=Workbench::new(); let first=workbench.pane(); let second=workbench.add_plot(first.id,false,false); let third=workbench.add_plot(second.id,false,false); let fourth=workbench.add_plot(second.id,true,false); let source=workbench.add_plot(first.id,false,true);
                let mut layout=Layout::flat(&[(first.id,1.0),(second.id,1.0),(third.id,1.0)],false); assert!(layout.split(second.id,fourth.id,true)); assert!(layout.add_plot_tab(first.id,source.id)); workbench.layout.set(layout);
                *captured.borrow_mut()=Some((workbench,first,second,third,fourth,source));
                view! { ThemeProvider(scheme=ColorScheme::Dark) { column(class="page workbench") { row(class="body") { PlotWorkspace(workbench=workbench) } } } }
            }).unwrap();
        let mut harness = Harness::new(handler.into_runtime());
        harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
            DevicePx(2400.0),
            DevicePx(1200.0),
        )));
        harness.settle(256);
        let (workbench, first, second, third, fourth, source) = saved.borrow().unwrap();
        let siblings = [
            source.dock.node.window_bounds().unwrap(),
            third.dock.node.window_bounds().unwrap(),
        ];
        let guide = workbench
            .dock
            .group_guides()
            .into_iter()
            .find(|guide| guide.members == vec![second.id, fourth.id])
            .unwrap();
        let target = guide.button(zone);
        let (x, y) = ((target.x + 11.0) as f32, (target.y + 11.0) as f32);
        let tab = workbench
            .dock
            .nodes
            .with_value(|nodes| *nodes.get(&(source.id, source.id)).unwrap());
        let bounds = tab.window_bounds().unwrap();
        let scale = tab.scale();
        harness.deliver_to_first(pointer(
            PointerAction::Pressed,
            (bounds.origin.x.0 + bounds.size.width.0 / 2.0) / scale,
            (bounds.origin.y.0 + bounds.size.height.0 / 2.0) / scale,
        ));
        harness.deliver_to_first(pointer(PointerAction::Moved, x, y));
        harness.settle(128);
        assert_eq!(
            workbench.dock.group_target.get_untracked(),
            Some((vec![second.id, fourth.id], zone))
        );
        assert!(
            harness.app().windows()[0]
                .nodes_with_class("dock-group-guide")
                .len()
                >= 12
        );
        let window = &harness.app().windows()[0];
        let predicted = window.box_with_class("dock-preview-source").unwrap();
        assert_eq!(window.nodes_with_class("dock-preview-pane").len(), 5);
        harness.deliver_to_first(pointer(PointerAction::Released, x, y));
        harness.settle(256);
        assert_eq!(first.dock.node.window_bounds().unwrap(), siblings[0]);
        assert_eq!(third.dock.node.window_bounds().unwrap(), siblings[1]);
        let a = second.dock.node.window_bounds().unwrap();
        let b = fourth.dock.node.window_bounds().unwrap();
        let c = source.dock.node.window_bounds().unwrap();
        assert_eq!(c, predicted);
        if zone == DropZone::Left {
            assert_eq!(a.origin.x, b.origin.x);
            assert!(c.origin.x.0 < a.origin.x.0);
            assert!((c.size.height.0 - 1200.0).abs() < 1.0);
        } else {
            assert!(c.origin.y.0 > b.origin.y.0);
            assert_eq!(c.size.width, a.size.width);
        }
    }
}

#[test]
fn dense_step_lines_and_markers_use_bounded_render_payloads_after_zoom() {
    use crate::{
        plot_cursor::CursorPlotProps,
        state::{Line, Workbench},
    };
    use plot::line_draw::{Connection, LineDraw};
    use zgui_ui_tokens::prelude::*;
    let saved = Rc::new(RefCell::new(None));
    let captured = Rc::clone(&saved);
    let counts = Rc::new(RefCell::new(Counts::default()));
    let rendered = Rc::clone(&counts);
    let data = plot::time_series::SeriesData::new(
        (0..1_000_000)
            .map(|i| plot::PlotPoint {
                x: i as f64,
                y: (i % 100) as f64,
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let handler=app().with_size(1000.0,700.0)
        .with_renderer(Box::new(move |_,target| { let mut renderer=CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(Capture{renderer,counts:Rc::clone(&rendered)})) }))
        .with_stylesheet(format!("{}\n{}\n{}",plot::zgui_surface::stylesheet(),plot::zgui_time_series::stylesheet(),crate::theme::stylesheet()))
        .into_handler(move || {
            let workbench=Workbench::new(); let pane=workbench.pane();
            pane.lines.set((1..=3).map(|id|Line{id,source:format!("dense {id}"),name:format!("dense {id}"),data:data.clone(),revision:0}).collect());
            pane.drawing.set([(1,LineDraw{connection:Connection::Linear,markers:true,..Default::default()}),(2,LineDraw{connection:Connection::Pre,markers:true,..Default::default()}),(3,LineDraw{connection:Connection::Post,markers:true,..Default::default()})].into_iter().collect());
            *captured.borrow_mut()=Some(pane);
            view! { ThemeProvider(scheme=ColorScheme::Dark) { column(class="page workbench") { row(class="body") { CursorPlot(workbench=workbench,pane=pane,visible=Signal::stored_local(true),linked=RwSignal::new_local(None)) } } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1000.0),
        DevicePx(700.0),
    )));
    let deadline = Instant::now() + Duration::from_secs(30);
    while counts.borrow().peak_vertices < 1000 {
        harness.settle(256);
        assert!(Instant::now() < deadline, "Dense lines never rendered");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        counts.borrow().peak_vertices < 250_000,
        "Payload must scale with viewport: {:?}",
        counts.borrow()
    );
    let pane = saved.borrow().unwrap();
    assert!(pane.lines.with_untracked(|lines| {
        lines
            .iter()
            .all(|line| line.data.samples().len() == 1_000_000)
    }));
    for offset in [1000.0, 2000.0, 3000.0, 4000.0] {
        pane.plot
            .set_view(plot::Ranges::new((offset, offset + 100.0), (0.0, 100.0)))
            .unwrap();
        harness.settle(32);
    }
    harness.settle(256);
    assert!(counts.borrow().peak_vertices < 250_000);
}

#[test]
fn three_rows_offer_top_two_and_bottom_two_region_guides() {
    use crate::{state::Workbench, workspace::PlotWorkspaceProps};
    use plot::workspace::layout::{DropZone, Layout};
    use zgui_ui_tokens::prelude::*;
    for top in [true, false] {
        let saved = Rc::new(RefCell::new(None));
        let captured = Rc::clone(&saved);
        let handler=app().with_size(1200.0,900.0)
            .with_renderer(Box::new(|_,target|{let mut renderer=CaptureRenderer::new();renderer.configure(target);Ok(Box::new(renderer))}))
            .with_stylesheet(format!("{}\n{}\n{}\n{}",plot::zgui_surface::stylesheet(),plot::zgui_time_series::stylesheet(),plot::zgui_workspace::stylesheet(),crate::theme::stylesheet()))
            .into_handler(move || {
                let workbench=Workbench::new();let first=workbench.pane();let second=workbench.add_plot(first.id,true,false);let third=workbench.add_plot(second.id,true,false);let outside=if top {third}else{first};let source=workbench.add_plot(outside.id,false,true);
                let mut layout=Layout::flat(&[(first.id,1.0),(second.id,1.0),(third.id,1.0)],true);assert!(layout.add_plot_tab(outside.id,source.id));workbench.layout.set(layout);
                let members=if top {vec![first.id,second.id]}else{vec![second.id,third.id]};*captured.borrow_mut()=Some((workbench,outside,source,members,first,second,third));
                view!{ThemeProvider(scheme=ColorScheme::Dark){column(class="page workbench"){row(class="body"){PlotWorkspace(workbench=workbench)}}}}
            }).unwrap();
        let mut harness = Harness::new(handler.into_runtime());
        harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
            DevicePx(1200.0),
            DevicePx(900.0),
        )));
        harness.settle(256);
        let (workbench, outside, source, members, first, second, third) =
            saved.borrow().clone().unwrap();
        let original = source.dock.node.window_bounds().unwrap();
        let guide = workbench
            .dock
            .group_guides()
            .into_iter()
            .find(|guide| guide.members == members)
            .unwrap();
        assert_eq!(guide.count, 2);
        let target = guide.button(DropZone::Right);
        let (x, y) = ((target.x + 11.0) as f32, (target.y + 11.0) as f32);
        let tab = workbench
            .dock
            .nodes
            .with_value(|nodes| *nodes.get(&(source.id, source.id)).unwrap());
        let rect = tab.window_bounds().unwrap();
        let scale = tab.scale();
        harness.deliver_to_first(pointer(
            PointerAction::Pressed,
            (rect.origin.x.0 + rect.size.width.0 / 2.0) / scale,
            (rect.origin.y.0 + rect.size.height.0 / 2.0) / scale,
        ));
        harness.deliver_to_first(pointer(PointerAction::Moved, x, y));
        harness.settle(128);
        assert_eq!(
            workbench.dock.group_target.get_untracked(),
            Some((members.clone(), DropZone::Right))
        );
        let window = &harness.app().windows()[0];
        assert!(
            window.nodes_with_class("dock-drop-zone").is_empty(),
            "Region guide must hide individual-pane arrows"
        );
        assert!(window.nodes_with_class("workspace-edge").is_empty());
        assert_eq!(window.nodes_with_class("dock-group-active").len(), 1);
        let first_preview = harness.app().windows()[0]
            .box_with_class("dock-preview-source")
            .unwrap();
        let opposite = workbench
            .dock
            .group_guides()
            .into_iter()
            .find(|guide| {
                guide.count == 2
                    && guide.members.contains(&second.id)
                    && guide.members.contains(&outside.id)
            })
            .unwrap();
        let other = opposite.button(DropZone::Right);
        let other_point = ((other.x + 11.0) as f32, (other.y + 11.0) as f32);
        let generic_target = workbench.dock.target.get_untracked();
        harness.deliver_to_first(pointer(PointerAction::Moved, other_point.0, other_point.1));
        harness.settle(128);
        assert_eq!(workbench.dock.target.get_untracked(), generic_target);
        let other_preview = harness.app().windows()[0]
            .box_with_class("dock-preview-source")
            .unwrap();
        assert!(
            (other_preview.origin.y.0 - first_preview.origin.y.0).abs() > 100.0,
            "Changing region must refresh preview even when direction and source stay same"
        );
        harness.deliver_to_first(pointer(PointerAction::Moved, x, y));
        harness.settle(128);
        let window = &harness.app().windows()[0];
        assert_eq!(window.nodes_with_class("dock-preview-pane").len(), 4);
        let predicted = [first, second, third, source].map(|pane| {
            (
                pane,
                window
                    .box_with_class(&format!("dock-preview-pane-{}", pane.id))
                    .unwrap(),
            )
        });
        harness.deliver_to_first(pointer(PointerAction::Released, x, y));
        harness.settle(256);
        for (pane, rect) in predicted {
            assert_eq!(
                pane.dock.node.window_bounds().unwrap(),
                rect,
                "Each dock must land at its previewed position"
            );
        }
        let untouched = outside.dock.node.window_bounds().unwrap();
        assert!(
            (untouched.origin.y.0 - original.origin.y.0).abs() < 1.0
                && (untouched.size.height.0 - original.size.height.0).abs() < 1.0
        );
        assert_eq!(untouched.size.width, original.size.width);
        let selected = if top {
            [first, second]
        } else {
            [second, third]
        };
        let a = selected[0].dock.node.window_bounds().unwrap();
        let b = selected[1].dock.node.window_bounds().unwrap();
        let added = source.dock.node.window_bounds().unwrap();
        assert_eq!(a.origin.x, b.origin.x);
        assert!(added.origin.x.0 > a.origin.x.0);
        assert!((added.origin.y.0 - a.origin.y.0).abs() < 1.0);
        assert!(
            (added.size.height.0 - (b.origin.y.0 + b.size.height.0 - a.origin.y.0)).abs() < 1.0
        );
    }
}

#[test]
fn per_dock_targets_keep_edge_and_center_drops_and_preview_neighbor_movement() {
    use crate::{state::Workbench, workspace::PlotWorkspaceProps};
    use plot::workspace::layout::DropZone;
    use zgui_ui_tokens::prelude::*;
    for zone in [DropZone::Right, DropZone::Center] {
        let saved = Rc::new(RefCell::new(None));
        let captured = Rc::clone(&saved);
        let handler=app().with_size(1400.0,900.0)
            .with_renderer(Box::new(|_,target|{let mut renderer=CaptureRenderer::new();renderer.configure(target);Ok(Box::new(renderer))}))
            .with_stylesheet(format!("{}\n{}\n{}\n{}",plot::zgui_surface::stylesheet(),plot::zgui_time_series::stylesheet(),plot::zgui_workspace::stylesheet(),crate::theme::stylesheet()))
            .into_handler(move || {
                let workbench=Workbench::new();let first=workbench.pane();let second=workbench.add_plot(first.id,false,false);*captured.borrow_mut()=Some((workbench,first,second));
                view!{ThemeProvider(scheme=ColorScheme::Dark){column(class="page workbench"){row(class="body"){PlotWorkspace(workbench=workbench)}}}}
            }).unwrap();
        let mut harness = Harness::new(handler.into_runtime());
        harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
            DevicePx(1400.0),
            DevicePx(900.0),
        )));
        harness.settle(256);
        let (workbench, first, second) = saved.borrow().unwrap();
        let original = second.dock.node.window_bounds().unwrap();
        let source = workbench
            .dock
            .nodes
            .with_value(|nodes| *nodes.get(&(first.id, first.id)).unwrap());
        let bounds = source.window_bounds().unwrap();
        let scale = source.scale();
        let x = if zone == DropZone::Right {
            original.origin.x.0 + original.size.width.0 - 4.0
        } else {
            original.origin.x.0 + original.size.width.0 / 2.0
        };
        let y = original.origin.y.0 + original.size.height.0 / 2.0;
        harness.deliver_to_first(pointer(
            PointerAction::Pressed,
            (bounds.origin.x.0 + bounds.size.width.0 / 2.0) / scale,
            (bounds.origin.y.0 + bounds.size.height.0 / 2.0) / scale,
        ));
        harness.deliver_to_first(pointer(PointerAction::Moved, -20.0, -20.0));
        harness.settle(128);
        assert!(
            harness.app().windows()[0]
                .nodes_with_class("dock-drop-zone")
                .is_empty(),
            "Outside workspace must hide docking arrows"
        );
        harness.deliver_to_first(pointer(PointerAction::Moved, x, y));
        harness.settle(128);
        assert_eq!(
            workbench.dock.target.get_untracked(),
            Some((second.id, zone))
        );
        assert!(workbench.dock.group_target.get_untracked().is_none());
        let window = &harness.app().windows()[0];
        for class in [
            "dock-zone-top",
            "dock-zone-bottom",
            "dock-zone-left",
            "dock-zone-right",
            "dock-zone-center",
        ] {
            assert_eq!(window.nodes_with_class(class).len(), 1);
        }
        let overlay = window.box_with_class("dock-drop-overlay").unwrap();
        assert!(overlay.origin.x.0 >= original.origin.x.0);
        let predicted = window.box_with_class("dock-preview-source").unwrap();
        let neighbor = window.box_with_class(&format!("dock-preview-pane-{}", second.id));
        assert_eq!(
            second.dock.node.window_bounds().unwrap(),
            original,
            "Preview cannot move actual docks before release"
        );
        if zone == DropZone::Right {
            assert!(neighbor.unwrap().origin.x.0 < original.origin.x.0);
            assert_eq!(window.nodes_with_class("dock-preview-pane").len(), 2);
        } else {
            assert_eq!(window.nodes_with_class("dock-preview-pane").len(), 1);
            assert!((predicted.size.width.0 - 1400.0).abs() < 1.0);
        }
        harness.deliver_to_first(pointer(PointerAction::Released, x, y));
        harness.settle(256);
        assert_eq!(first.dock.node.window_bounds().unwrap(), predicted);
        if let Some(neighbor) = neighbor {
            assert_eq!(second.dock.node.window_bounds().unwrap(), neighbor);
        } else {
            assert_eq!(workbench.layout.get_untracked().tab_ids(first.id).len(), 2);
        }
        assert!(
            harness.app().windows()[0]
                .nodes_with_class("dock-layout-preview")
                .is_empty()
        );
    }
}

#[test]
fn hidden_legend_hover_previews_on_top_without_changing_visibility_or_bounds() {
    use crate::{
        state::{Line, Workbench},
        workspace::PlotWorkspaceProps,
    };
    use zgui_ui_tokens::prelude::*;
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let counts = Rc::new(RefCell::new(Counts::default()));
    let captured = Rc::clone(&counts);
    let handler = app().with_size(900.0, 600.0)
        .with_renderer(Box::new(move |_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(Capture { renderer, counts: Rc::clone(&captured) })) }))
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .into_handler(move || {
            let workbench = Workbench::new();
            let pane = workbench.pane();
            let data = plot::time_series::SeriesData::new(vec![plot::PlotPoint { x: 0.0, y: 1.0 }, plot::PlotPoint { x: 1.0, y: 2.0 }]).unwrap();
            pane.lines.set((1..=2).map(|id| Line { id, source: format!("line {id}"), name: format!("line {id}"), data: data.clone(), revision: 0 }).collect());
            pane.plot.hidden.update(|hidden| { hidden.insert("1".into()); });
            pane.plot.styles.update(|styles| {
                for (id, width) in [(1, 11.0), (2, 5.0)] { let mut style = plot::plot_data::SeriesStyle::for_id(&id.to_string()); style.width = width; styles.insert(id.to_string(), style); }
            });
            pane.plot.set_view(plot::Ranges::new((0.0, 1.0), (0.0, 3.0))).unwrap();
            *built.borrow_mut() = Some(pane);
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "page workbench") { row(class = "body") { PlotWorkspace(workbench = workbench) } } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(600.0),
    )));
    harness.settle(256);
    let pane = model.borrow().unwrap();
    let widths = || counts.borrow().line_strokes.clone();
    let wait = |harness: &mut Harness<zgui::runtime::Runtime>, expected: &[f32]| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            harness.settle(128);
            if widths() == expected {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Expected {expected:?}, got {:?}; scene {:?}",
                widths(),
                counts.borrow()
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    wait(&mut harness, &[2.5]);
    let bounds = pane.plot.ranges.get_untracked();
    let window = &harness.app().windows()[0];
    let label = window.nodes_with_class("legend-drag-source")[0];
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), label).unwrap();
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        rect.origin.x.0 + 20.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    ));
    wait(&mut harness, &[2.5, 6.25]);
    assert_eq!(pane.plot.emphasized.get_untracked().as_deref(), Some("1"));
    assert!(pane.plot.hidden.get_untracked().contains("1"));
    assert_eq!(pane.plot.ranges.get_untracked(), bounds);
    assert_eq!(
        pane.lines
            .with_untracked(|lines| lines.iter().map(|line| line.id).collect::<Vec<_>>()),
        vec![1, 2]
    );
    // Pointer capture during clicks must not clear hover and flash other lines.
    for hidden in [false, true] {
        for action in [PointerAction::Pressed, PointerAction::Released] {
            harness.deliver_to_first(pointer(
                action,
                rect.origin.x.0 + 20.0,
                rect.origin.y.0 + rect.size.height.0 / 2.0,
            ));
            harness.settle(128);
            assert_eq!(
                pane.plot.emphasized.get_untracked().as_deref(),
                Some("1"),
                "Hover lost on {action:?}"
            );
            assert_eq!(
                widths(),
                vec![2.5, 6.25],
                "Click must retain dimming and preview"
            );
        }
        assert_eq!(pane.plot.hidden.get_untracked().contains("1"), hidden);
        assert_eq!(pane.plot.ranges.get_untracked(), bounds);
    }
    harness.deliver_to_first(pointer(PointerAction::Moved, 800.0, 500.0));
    wait(&mut harness, &[2.5]);
    assert!(pane.plot.emphasized.get_untracked().is_none());
    assert!(pane.plot.hidden.get_untracked().contains("1"));
    assert_eq!(pane.plot.ranges.get_untracked(), bounds);
}

#[test]
fn legend_toggles_keep_bounds_and_axis_menu_double_click_and_full_height_drag_work() {
    use crate::{
        state::{Line, Workbench},
        workspace::PlotWorkspaceProps,
    };
    use zgui_ui_tokens::prelude::*;
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let handler = app().with_size(900.0, 600.0)
        .with_renderer(Box::new(|_, target| { let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(renderer)) }))
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .into_handler(move || {
            let workbench = Workbench::new();
            let pane = workbench.pane();
            let data = |x, y| plot::time_series::SeriesData::new(vec![plot::PlotPoint { x, y }, plot::PlotPoint { x: x + 10.0, y: y + 10.0 }]).unwrap();
            pane.lines.set(vec![Line { id: 1, source: "normal".into(), name: "normal".into(), data: data(0.0, 0.0), revision: 0 }, Line { id: 2, source: "outlier".into(), name: "outlier".into(), data: data(100.0, 200.0), revision: 0 }]);
            *built.borrow_mut() = Some(pane);
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "page workbench") { row(class = "body") { PlotWorkspace(workbench = workbench) } } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(600.0),
    )));
    harness.settle(256);
    let pane = model.borrow().unwrap();
    let fitted = pane.plot.ranges.get_untracked();
    assert!(fitted.x.max >= 110.0 && fitted.y.max >= 210.0);
    let click_at = |harness: &mut Harness<zgui::runtime::Runtime>, x, y| {
        for action in [PointerAction::Pressed, PointerAction::Released] {
            harness.deliver_to_first(pointer(action, x, y));
        }
        harness.settle(128);
    };
    let window = &harness.app().windows()[0];
    let row = window.nodes_with_class("legend-drag-source")[1];
    let rect = zgui::view::ViewHost::window_box(window.host().as_ref(), row).unwrap();
    let legend = (
        rect.origin.x.0 + 20.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    click_at(&mut harness, legend.0, legend.1);
    assert!(pane.plot.hidden.get_untracked().contains("2"));
    assert_eq!(pane.plot.ranges.get_untracked(), fitted);
    click_at(&mut harness, legend.0, legend.1);
    assert!(!pane.plot.hidden.get_untracked().contains("2"));
    assert_eq!(pane.plot.ranges.get_untracked(), fitted);
    let graph = harness.app().windows()[0]
        .box_with_class("cursor-plot")
        .unwrap();
    let open_menu = |harness: &mut Harness<zgui::runtime::Runtime>| {
        let (x, y) = (
            graph.origin.x.0 + graph.size.width.0 / 2.0,
            graph.origin.y.0 + graph.size.height.0 / 2.0,
        );
        for action in [PointerAction::Pressed, PointerAction::Released] {
            let mut event = pointer(action, x, y);
            if let SurfaceEvent::Pointer { event, .. } = &mut event {
                event.button = Some(zgui::vocab::PointerButton::Secondary);
            }
            harness.deliver_to_first(event);
        }
        harness.settle(128);
        assert!(
            harness.app().windows()[0]
                .box_with_class("plot-fit-menu")
                .is_some()
        );
    };
    let zoom = plot::Ranges::new((2.0, 4.0), (50.0, 70.0));
    pane.plot.set_view(zoom).unwrap();
    harness.settle(128);
    open_menu(&mut harness);
    click_class(&mut harness, "plot-fit-x");
    assert_eq!(
        pane.plot.ranges.get_untracked(),
        plot::Ranges {
            x: fitted.x,
            y: zoom.y
        }
    );
    pane.plot.set_view(zoom).unwrap();
    harness.settle(128);
    open_menu(&mut harness);
    click_class(&mut harness, "plot-fit-y");
    assert_eq!(
        pane.plot.ranges.get_untracked(),
        plot::Ranges {
            x: zoom.x,
            y: (1.9, 4.1).into()
        }
    );
    pane.plot.set_view(zoom).unwrap();
    harness.settle(128);
    open_menu(&mut harness);
    click_class(&mut harness, "plot-line-settings");
    assert!(
        harness.app().windows()[0]
            .box_with_class("plot-editor-surface")
            .is_some()
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("plot-fit-menu")
            .is_none()
    );
    assert_eq!(pane.plot.ranges.get_untracked(), zoom);
    click_class(&mut harness, "settings-done");
    let x_axis = (
        graph.origin.x.0 + graph.size.width.0 / 2.0,
        graph.origin.y.0 + graph.size.height.0 - 8.0,
    );
    click_at(&mut harness, x_axis.0, x_axis.1);
    click_at(&mut harness, x_axis.0, x_axis.1);
    assert_eq!(
        pane.plot.ranges.get_untracked(),
        plot::Ranges {
            x: fitted.x,
            y: zoom.y
        }
    );
    pane.plot.set_view(zoom).unwrap();
    harness.settle(128);
    let y_axis = (
        graph.origin.x.0 + 10.0,
        graph.origin.y.0 + graph.size.height.0 * 0.4,
    );
    click_at(&mut harness, y_axis.0, y_axis.1);
    click_at(&mut harness, y_axis.0, y_axis.1);
    assert_eq!(
        pane.plot.ranges.get_untracked(),
        plot::Ranges {
            x: zoom.x,
            y: (1.9, 4.1).into()
        }
    );
    pane.plot.set_view(zoom).unwrap();
    harness.settle(128);
    harness.deliver_to_first(pointer(PointerAction::Pressed, y_axis.0, y_axis.1));
    harness.deliver_to_first(pointer(PointerAction::Moved, y_axis.0, y_axis.1 - 30.0));
    harness.deliver_to_first(pointer(PointerAction::Released, y_axis.0, y_axis.1 - 30.0));
    harness.settle(128);
    let dragged = pane.plot.ranges.get_untracked();
    assert_eq!(dragged.x, zoom.x);
    assert_eq!(dragged.y.min, zoom.y.min);
    assert!(
        dragged.y.max > zoom.y.max,
        "Full-height Y band must drag outside old corner: {dragged:?}"
    );
    // Removing a visible outlier must not shrink bounds either.
    pane.remove_line(2);
    harness.settle(128);
    assert_eq!(pane.plot.ranges.get_untracked(), dragged);
}

#[test]
fn own_tab_group_keeps_docking_guides_visible_over_center() {
    use crate::{state::Workbench, workspace::PlotWorkspaceProps};
    use zgui_ui_tokens::prelude::*;
    let saved = Rc::new(RefCell::new(None));
    let captured = Rc::clone(&saved);
    let handler = app()
        .with_size(1200.0, 700.0)
        .with_renderer(Box::new(|_, target| {
            let mut renderer = CaptureRenderer::new();
            renderer.configure(target);
            Ok(Box::new(renderer))
        }))
        .with_stylesheet(format!(
            "{}\n{}\n{}\n{}",
            plot::zgui_surface::stylesheet(),
            plot::zgui_time_series::stylesheet(),
            plot::zgui_workspace::stylesheet(),
            crate::theme::stylesheet()
        ))
        .into_handler(move || {
            let workbench = Workbench::new();
            let first = workbench.pane();
            let second = workbench.add_plot(first.id, false, true);
            workbench.add_plot(second.id, false, false);
            *captured.borrow_mut() = Some((workbench, first, second));
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "page workbench") {
                row(class = "body") { PlotWorkspace(workbench = workbench) }
            } } }
        })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.0),
        DevicePx(700.0),
    )));
    harness.settle(256);
    let (workbench, first, second) = saved.borrow().unwrap();
    let original = workbench.layout.get_untracked();
    let bounds = second.dock.node.window_bounds().unwrap();
    let scale = second.dock.node.scale();
    let center = (
        (bounds.origin.x.0 + bounds.size.width.0 / 2.0) / scale,
        (bounds.origin.y.0 + bounds.size.height.0 / 2.0) / scale,
    );
    for source in [first, second] {
        let tab = workbench
            .dock
            .nodes
            .with_value(|nodes| nodes[&(second.id, source.id)]);
        let bounds = tab.window_bounds().unwrap();
        harness.deliver_to_first(pointer(
            PointerAction::Pressed,
            (bounds.origin.x.0 + bounds.size.width.0 / 2.0) / tab.scale(),
            (bounds.origin.y.0 + bounds.size.height.0 / 2.0) / tab.scale(),
        ));
        harness.deliver_to_first(pointer(PointerAction::Moved, center.0, center.1));
        harness.settle(128);
        assert!(
            workbench.dock.target.get_untracked().is_none(),
            "Self-center remains no-op drop"
        );
        let window = &harness.app().windows()[0];
        for class in [
            "dock-zone-top",
            "dock-zone-bottom",
            "dock-zone-left",
            "dock-zone-right",
            "dock-zone-center",
        ] {
            assert_eq!(
                window.nodes_with_class(class).len(),
                1,
                "Own tab group must retain {class}"
            );
        }
        assert!(window.nodes_with_class("dock-layout-preview").is_empty());
        harness.deliver_to_first(pointer(PointerAction::Moved, -20.0, -20.0));
        harness.settle(128);
        assert!(
            harness.app().windows()[0]
                .nodes_with_class("dock-drop-overlay")
                .is_empty()
        );
        harness.deliver_to_first(pointer(PointerAction::Moved, center.0, center.1));
        harness.settle(128);
        assert_eq!(
            harness.app().windows()[0]
                .nodes_with_class("dock-drop-overlay")
                .len(),
            1
        );
        harness.deliver_to_first(pointer(PointerAction::Released, center.0, center.1));
        std::thread::sleep(Duration::from_millis(180));
        harness.advance(Duration::from_millis(500));
        harness.settle(256);
        assert_eq!(workbench.layout.get_untracked(), original);
        assert!(
            harness.app().windows()[0]
                .nodes_with_class("dock-drop-overlay")
                .is_empty()
        );
    }
}

#[test]
fn line_settings_keep_geometry_fixed_during_selection_changes() {
    use crate::{
        editor::EditorProps,
        state::{Line, Workbench},
    };
    use zgui_ui_tokens::prelude::*;
    let handler = app().with_size(900.0, 700.0)
        .with_renderer(Box::new(|_, target| {
            let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(renderer))
        }))
        .with_stylesheet(format!("{}\n.test-editor-root {{width:680px;}}", crate::theme::stylesheet()))
        .into_handler(|| {
            let workbench = Workbench::new();
            let pane = workbench.pane();
            let data = plot::time_series::SeriesData::new(vec![]).unwrap();
            pane.lines.set((1..=2).map(|id| Line { id, source: id.to_string(), name: format!("Line {id}"), data: data.clone(), revision: 0 }).collect());
            workbench.select(pane.id, 1);
            pane.drawing.update(|drawing| { drawing.entry(2).or_default().markers = true; });
            view! { ThemeProvider(scheme = ColorScheme::Dark) {
                column(class = "plot-settings-surface test-editor-root") { Editor(workbench = workbench, pane = pane) }
            } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(700.0),
    )));
    harness.settle(256);
    let classes = [
        "editor",
        "appearance-row",
        "color-picker",
        "color-square",
        "appearance-controls",
        "line-width",
        "line-radius",
        "legend-add-filters",
    ];
    let before: Vec<_> = classes
        .iter()
        .map(|class| harness.app().windows()[0].box_with_class(class).unwrap())
        .collect();
    let rows = harness.app().windows()[0].nodes_with_class("line-button");
    let rect =
        zgui::view::ViewHost::window_box(harness.app().windows()[0].host().as_ref(), rows[1])
            .unwrap();
    let point = (
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
    );
    assert!(
        rect.size.height.0 <= 26.0,
        "Rows should stay compact: {rect:?}"
    );
    for modifiers in [Modifiers::CONTROL, Modifiers::NONE, Modifiers::CONTROL] {
        for action in [PointerAction::Pressed, PointerAction::Released] {
            let mut event = pointer(action, point.0, point.1);
            if let SurfaceEvent::Pointer {
                modifiers: held, ..
            } = &mut event
            {
                *held = modifiers;
            }
            harness.deliver_to_first(event);
        }
        harness.settle(128);
        for (class, before) in classes.iter().zip(&before) {
            assert_eq!(
                harness.app().windows()[0].box_with_class(class).unwrap(),
                *before,
                "Selection must not move {class}"
            );
        }
    }
}

#[test]
fn app_action_buttons_share_rendered_states_across_toolbar_and_filters() {
    fn fill(
        harness: &Harness<zgui::runtime::Runtime>,
        paints: &Rc<RefCell<Counts>>,
        class: &str,
    ) -> String {
        let window = &harness.app().windows()[0];
        let rect = window.box_with_class(class).unwrap();
        let paints = paints.borrow();
        paints
            .painted_boxes
            .iter()
            .filter(|(bounds, _)| {
                (bounds[2] - rect.size.width.0).abs() < 0.1
                    && (bounds[3] - rect.size.height.0).abs() < 0.1
            })
            .min_by(|(a, _), (b, _)| {
                let distance = |bounds: &[f32; 4]| {
                    (bounds[0] - rect.origin.x.0).abs() + (bounds[1] - rect.origin.y.0).abs()
                };
                distance(a).total_cmp(&distance(b))
            })
            .unwrap_or_else(|| {
                panic!(
                    "No painted button for {class}: {rect:?}, {:?}",
                    paints.painted_boxes
                )
            })
            .1
            .clone()
    }
    fn radii(
        harness: &Harness<zgui::runtime::Runtime>,
        paints: &Rc<RefCell<Counts>>,
        class: &str,
    ) -> [f32; 8] {
        let rect = harness.app().windows()[0].box_with_class(class).unwrap();
        let paints = paints.borrow();
        paints
            .painted_radii
            .iter()
            .find(|(bounds, _)| {
                (bounds[0] - rect.origin.x.0).abs() < 0.1
                    && (bounds[1] - rect.origin.y.0).abs() < 0.1
                    && (bounds[2] - rect.size.width.0).abs() < 0.1
                    && (bounds[3] - rect.size.height.0).abs() < 0.1
            })
            .expect("Rendered button radii")
            .1
    }
    let paints = Rc::new(RefCell::new(Counts::default()));
    let captured = Rc::clone(&paints);
    let handler = app().with_size(1500.0, 850.0)
        .with_renderer(Box::new(move |_, target| {
            let mut renderer = CaptureRenderer::new(); renderer.configure(target);
            Ok(Box::new(Capture { renderer, counts: Rc::clone(&captured) }))
        }))
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .into_handler(|| view! { WorkbenchApp(preferences = crate::preferences::ThemePreference::disabled(), initial_path = Some(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esc_status_log.ulg")))) }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1500.0),
        DevicePx(850.0),
    )));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        harness.settle(128);
        if !harness.app().windows()[0]
            .nodes_with_class("topic-branch")
            .is_empty()
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let disabled = fill(&harness, &paints, "equalize-panels");
    click_class(&mut harness, "duplicate-plot-button");
    click_class(&mut harness, "open-filters");
    click_class(&mut harness, "sidebar-toggle");
    harness.deliver_to_first(pointer(PointerAction::Moved, 1490.0, 840.0));
    harness.settle(128);
    click_class(&mut harness, "theme-picker");
    click_class(&mut harness, "theme-forest");
    harness.deliver_to_first(pointer(PointerAction::Moved, 1490.0, 840.0));
    harness.settle(128);
    let idle = fill(&harness, &paints, "open-log");
    assert_ne!(disabled, idle);
    for class in [
        "open-log",
        "equalize-panels",
        "sidebar-toggle",
        "filters-done",
    ] {
        assert_eq!(
            fill(&harness, &paints, class),
            idle,
            "Idle mismatch: {class}"
        );
        let rect = harness.app().windows()[0].box_with_class(class).unwrap();
        assert_eq!(rect.size.height.0, 24.0, "Size mismatch: {class}");
    }
    let mut hover = None;
    for class in [
        "open-log",
        "equalize-panels",
        "sidebar-toggle",
        "filters-done",
    ] {
        let rect = harness.app().windows()[0].box_with_class(class).unwrap();
        harness.deliver_to_first(pointer(
            PointerAction::Moved,
            rect.origin.x.0 + rect.size.width.0 / 2.0,
            rect.origin.y.0 + rect.size.height.0 / 2.0,
        ));
        harness.settle(128);
        let color = fill(&harness, &paints, class);
        assert_ne!(color, idle, "Missing hover: {class}");
        assert_eq!(
            *hover.get_or_insert(color.clone()),
            color,
            "Hover mismatch: {class}"
        );
    }
    click_class(&mut harness, "custom-rust-filter");
    let editor = harness.app().windows()[0].nodes_with_class("script-source")[0];
    click_class(&mut harness, "theme-picker");
    click_class(&mut harness, "theme-slate");
    harness.deliver_to_first(pointer(PointerAction::Moved, 1490.0, 840.0));
    harness.settle(128);
    let slate = fill(&harness, &paints, "open-log");
    assert_ne!(slate, idle, "Theme must update rendered button paint");
    assert_eq!(fill(&harness, &paints, "filters-done"), slate);
    assert_eq!(
        harness.app().windows()[0].nodes_with_class("script-source")[0],
        editor,
        "Theme changes must preserve editor instance"
    );
    assert_eq!(
        radii(&harness, &paints, "open-log"),
        [0.0; 8],
        "Slate should be sharp"
    );
    for (theme, radius) in [("ember", 2.0), ("amethyst", 10.0), ("dune", 6.0)] {
        click_class(&mut harness, "theme-picker");
        click_class(&mut harness, &format!("theme-{theme}"));
        harness.deliver_to_first(pointer(PointerAction::Moved, 1490.0, 840.0));
        harness.settle(128);
        assert_ne!(
            fill(&harness, &paints, "open-log"),
            slate,
            "Palette must change: {theme}"
        );
        assert_eq!(
            fill(&harness, &paints, "open-log"),
            fill(&harness, &paints, "filters-done"),
            "Buttons must stay uniform: {theme}"
        );
        assert!(
            radii(&harness, &paints, "open-log")
                .iter()
                .all(|actual| (*actual - radius).abs() < 0.1),
            "Theme radii must update: {theme}"
        );
        assert_eq!(
            harness.app().windows()[0].nodes_with_class("script-source")[0],
            editor
        );
    }
    click_class(&mut harness, "theme-picker");
    click_class(&mut harness, "theme-forest");
    harness.deliver_to_first(pointer(PointerAction::Moved, 1490.0, 840.0));
    harness.settle(128);
    assert_eq!(
        fill(&harness, &paints, "open-log"),
        idle,
        "Forest palette must restore original button paint"
    );
    assert_eq!(
        radii(&harness, &paints, "open-log"),
        [4.0; 8],
        "Forest must restore its rounding"
    );
    click_class(&mut harness, "sidebar-toggle");
    harness.deliver_to_first(pointer(PointerAction::Moved, 1490.0, 840.0));
    harness.settle(128);
    assert_ne!(
        fill(&harness, &paints, "sidebar-toggle"),
        idle,
        "Toggle must retain selected state"
    );
}

fn geometry_harness() -> (Harness<zgui::runtime::Runtime>, crate::state::Workbench) {
    use crate::{browser::BrowserProps, theme::AppThemeProps, workspace::PlotWorkspaceProps};
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let counts = Rc::new(RefCell::new(Counts::default()));
    let captured = Rc::clone(&counts);
    let handler=app().with_size(1200.,800.)
        .with_renderer(Box::new(move |_,target| {let mut renderer=CaptureRenderer::new();renderer.configure(target);Ok(Box::new(Capture{renderer,counts:Rc::clone(&captured)}))}))
        .with_stylesheet(format!("{}\n{}\n{}\n{}",plot::zgui_surface::stylesheet(),plot::zgui_time_series::stylesheet(),plot::zgui_workspace::stylesheet(),crate::theme::stylesheet()))
        .into_handler(move || {let workbench=crate::state::Workbench::new();*built.borrow_mut()=Some(workbench);
            view!{AppTheme(palette=crate::theme::Palette::Forest){column(class="workbench page"){row(class="body"){Browser(workbench=workbench) PlotWorkspace(workbench=workbench)}}}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1200.),
        DevicePx(800.),
    )));
    harness.settle(256);
    let workbench = model.borrow().unwrap();
    (harness, workbench)
}

#[test]
fn sidebar_profiles_and_view_buttons_work_in_real_layout() {
    let (mut harness, workbench) = geometry_harness();
    let log = std::sync::Arc::new(
        crate::log::LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap(),
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("esc.json");
    std::fs::write(&path,serde_json::json!({"version":1,"name":"ESC setup","views":[{"name":"ESC view","fields":["esc_status/0/esc[5].esc_rpm"]}]}).to_string()).unwrap();
    workbench
        .profiles
        .directory
        .set(Some(directory.path().to_path_buf()));
    workbench.log.set(Some(log));
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("log-settings-trigger")
            .is_empty()
    );
    click_class(&mut harness, "fields-menu");
    if harness.app().windows()[0]
        .nodes_with_class("profiles-trigger")
        .is_empty()
    {
        click_class(&mut harness, "fields-menu");
    }
    click_class(&mut harness, "profiles-trigger");
    harness.advance(Duration::from_millis(500));
    harness.settle(128);
    let library = harness.app().windows()[0]
        .box_with_class("profile-library")
        .unwrap();
    assert!(
        library.size.width.0 > 200.0 && library.size.height.0 > 300.0,
        "{library:?}"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.app().windows()[0]
        .nodes_with_class("profile-file-name")
        .is_empty()
    {
        assert!(Instant::now() < deadline, "Profile scan did not finish");
        harness.settle(128);
        std::thread::sleep(Duration::from_millis(2));
    }
    click_class(&mut harness, "profile-file-name");
    harness.settle(128);
    let fields_before = harness.app().windows()[0]
        .box_with_class("profiles-trigger")
        .unwrap();
    click_class(&mut harness, "fields-menu");
    harness.settle(128);
    let fields_after = harness.app().windows()[0]
        .box_with_class("profiles-trigger")
        .unwrap();
    assert_eq!(
        fields_before.origin, fields_after.origin,
        "Header spacing must stay fixed"
    );
    click_class(&mut harness, "profiles-trigger");
    harness.settle(128);
    assert!(
        !harness.app().windows()[0]
            .nodes_with_class("profile-view-source")
            .is_empty(),
        "Expansion survives mode switch"
    );
    let profile_output = crate::profile::identity::Drag {
        profile: crate::profile::identity::ProfileKey::new(path.canonicalize().unwrap(), 1),
        item: crate::profile::identity::Item::View("ESC view".into()),
    };
    workbench.use_profile_items(
        vec![profile_output.clone()],
        crate::profile_state::Destination::Plot(None),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while workbench.profiles.busy.get_untracked() {
        assert!(
            Instant::now() < deadline,
            "{}",
            workbench.profiles.error.get_untracked()
        );
        harness.settle(128);
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.settle(128);
    assert_eq!(
        workbench.panes.with_untracked(Vec::len),
        2,
        "{}",
        workbench.profiles.error.get_untracked()
    );
    assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 1);
    workbench.activate(1);
    for _ in 0..2 {
        workbench.use_profile_items(
            vec![profile_output.clone()],
            crate::profile_state::Destination::Plot(Some(1)),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while workbench.profiles.busy.get_untracked() {
            assert!(Instant::now() < deadline);
            harness.settle(128);
            std::thread::sleep(Duration::from_millis(2));
        }
        harness.settle(128);
    }
    assert_eq!(workbench.panes.with_untracked(Vec::len), 2);
    assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 1);
    assert_eq!(
        workbench
            .profiles
            .instances
            .with_untracked(std::collections::BTreeMap::len),
        1
    );
    click_class(&mut harness, "profile-menu");
    harness.advance(Duration::from_millis(200));
    harness.settle(128);
    assert!(
        !harness.app().windows()[0]
            .nodes_with_class("profile-auto")
            .is_empty()
    );
    click_class(&mut harness, "profile-remove-open");
    harness.settle(128);
    assert!(workbench.profiles.removal.get_untracked().is_some());
    assert_eq!(
        workbench.pane().lines.with_untracked(Vec::len),
        1,
        "Preview must not remove outputs"
    );
    click_class(&mut harness, "profile-remove-cancel");
    assert!(workbench.profiles.removal.get_untracked().is_none());
}

#[test]
fn geometry_draw_edit_reuse_and_cancel_keep_plot_bounds() {
    use crate::geometry::GeometryKind;
    let (mut harness, workbench) = geometry_harness();
    let pane = workbench.pane();
    pane.plot
        .set_view(plot::Ranges::new((0., 10.), (0., 10.)))
        .unwrap();
    click_class(&mut harness, "draw-rectangle");
    let bounds = harness.app().windows()[0]
        .box_with_class("geometry-overlay")
        .unwrap();
    assert!(
        bounds.size.width.0 > 700. && bounds.size.height.0 > 700.,
        "{bounds:?}"
    );
    let x = bounds.origin.x.0;
    let y = bounds.origin.y.0;
    let w = bounds.size.width.0;
    let h = bounds.size.height.0;
    harness.deliver_to_first(pointer(PointerAction::Pressed, x + w * 0.2, y + h * 0.2));
    harness.deliver_to_first(pointer(PointerAction::Moved, x + w * 0.6, y + h * 0.6));
    harness.settle(128);
    harness.deliver_to_first(pointer(PointerAction::Released, x + w * 0.6, y + h * 0.6));
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .pointer_capture(zgui::vocab::PointerId::MOUSE)
            .is_none(),
        "Drawing release clears pointer capture"
    );
    let items = workbench.geometry.items.get_untracked();
    assert_eq!(items.len(), 1, "Pointer drag creates geometry");
    let item = items[0].clone();
    assert!(item.nonempty());
    for (v, expected) in item.coordinates.iter().zip([2., 8., 6., 4.]) {
        assert!((v - expected).abs() < 0.03, "{:?}", item.coordinates);
    }
    assert!(pane.geometry.with_untracked(|ids| ids.contains(&item.id)));
    assert_eq!(
        pane.plot.ranges.get_untracked(),
        plot::Ranges::new((0., 10.), (0., 10.))
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("geometry-inspector")
            .is_some()
    );
    let legend = harness.app().windows()[0]
        .box_with_class("workbench-legend")
        .unwrap();
    let geom_row = harness.app().windows()[0]
        .box_with_class("geometry-legend-row")
        .unwrap();
    assert!(
        geom_row.origin.x.0 >= legend.origin.x.0
            && geom_row.origin.y.0 >= legend.origin.y.0
            && geom_row.origin.y.0 + geom_row.size.height.0
                <= legend.origin.y.0 + legend.size.height.0,
        "Geometry belongs inside graph legend"
    );
    let original_bounds = pane.plot.ranges.get_untracked();
    click_class(&mut harness, "geometry-legend-toggle");
    assert!(
        pane.hidden_geometry
            .with_untracked(|ids| ids.contains(&item.id))
    );
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("geometry-interaction")
            .is_empty()
    );
    assert_eq!(pane.plot.ranges.get_untracked(), original_bounds);
    click_class(&mut harness, "geometry-legend-toggle");
    assert!(
        !pane
            .hidden_geometry
            .with_untracked(|ids| ids.contains(&item.id))
    );
    click_class(&mut harness, "geometry-legend-edit");
    click_class(&mut harness, "geometry-appearance-tab");
    assert!(
        harness.app().windows()[0]
            .box_with_class("geometry-opacity")
            .is_some()
    );
    assert!(
        harness.app().windows()[0]
            .box_with_class("color-square")
            .is_some()
    );
    click_class(&mut harness, "geometry-coordinates-tab");
    click_class(&mut harness, "geometry-edit-toggle");
    click_class(&mut harness, "geometry-legend-remove");
    assert!(!pane.geometry.with_untracked(|ids| ids.contains(&item.id)));
    assert!(
        workbench.geometry_item(item.id).is_some(),
        "Legend X preserves reusable definition"
    );
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("workbench-legend")
            .is_empty(),
        "No empty legend after removing final geometry"
    );
    assert_eq!(pane.plot.ranges.get_untracked(), original_bounds);
    workbench.attach_geometry(item.id, pane, true);
    harness.settle(128);
    click_class(&mut harness, "geometry-legend-edit");
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("geometry-handle")
            .len(),
        5
    );
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("geometry-interaction")
            .is_empty(),
        "Editing must not cover plot with blocking surface"
    );
    // Interior background remains available for pan and wheel zoom during editing.
    let before_navigation = pane.plot.ranges.get_untracked();
    harness.deliver_to_first(pointer(PointerAction::Pressed, x + w * 0.3, y + h * 0.3));
    harness.deliver_to_first(pointer(PointerAction::Moved, x + w * 0.32, y + h * 0.32));
    harness.deliver_to_first(pointer(PointerAction::Released, x + w * 0.32, y + h * 0.32));
    harness.settle(128);
    assert_ne!(
        pane.plot.ranges.get_untracked(),
        before_navigation,
        "Pan remains active inside rectangle while editing"
    );
    assert_eq!(
        workbench.geometry_item(item.id).unwrap().coordinates,
        item.coordinates
    );
    assert!(workbench.geometry.editing.get_untracked());
    let before_wheel = pane.plot.ranges.get_untracked();
    harness.deliver_to_first(SurfaceEvent::Wheel {
        event: zgui::vocab::WheelEvent {
            delta: zgui::vocab::ScrollDelta::Pixels(Size::new(CssPx(0.), CssPx(-100.))),
            phase: zgui::vocab::ScrollPhase::Moved,
            position: Point::new(CssPx(x + w * 0.3), CssPx(y + h * 0.3)),
            id: zgui::vocab::PointerId::MOUSE,
            kind: zgui::vocab::PointerKind::Mouse,
        },
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    harness.settle(128);
    assert_ne!(
        pane.plot.ranges.get_untracked().x.span(),
        before_wheel.x.span(),
        "Wheel zoom stays active during editing"
    );
    assert_eq!(
        workbench.geometry_item(item.id).unwrap().coordinates,
        item.coordinates
    );
    pane.plot.set_view(before_navigation).unwrap();
    harness.settle(128);
    let editor_before = harness.app().windows()[0]
        .box_with_class("geometry-inspector")
        .unwrap();
    let title = harness.app().windows()[0]
        .box_with_class("geometry-editor-heading")
        .unwrap();
    harness.deliver_to_first(pointer(
        PointerAction::Pressed,
        title.origin.x.0 + 20.,
        title.origin.y.0 + 10.,
    ));
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        title.origin.x.0 + 120.,
        title.origin.y.0 - 40.,
    ));
    harness.deliver_to_first(pointer(
        PointerAction::Released,
        title.origin.x.0 + 120.,
        title.origin.y.0 - 40.,
    ));
    harness.settle(128);
    let editor_after = harness.app().windows()[0]
        .box_with_class("geometry-inspector")
        .unwrap();
    assert!(
        editor_after.origin.x.0 > editor_before.origin.x.0 + 90.,
        "Editor title drag moves controls away from data"
    );
    assert_eq!(pane.plot.ranges.get_untracked(), before_navigation);
    // Drag rectangle body; cancellation restores original coordinates.
    harness.deliver_to_first(pointer(PointerAction::Pressed, x + w * 0.4, y + h * 0.4));
    harness.deliver_to_first(pointer(PointerAction::Moved, x + w * 0.5, y + h * 0.5));
    harness.settle(128);
    assert_ne!(
        workbench.geometry_item(item.id).unwrap().coordinates,
        item.coordinates
    );
    assert_eq!(
        harness.platform().offscreens()[0].last_cursor(),
        Some(zgui::platform::CursorStyle::Grabbing),
        "Active move uses native grabbing cursor even under pointer capture"
    );
    workbench.cancel_geometry_drag();
    harness.deliver_to_first(pointer(PointerAction::Released, x + w * 0.5, y + h * 0.5));
    harness.settle(128);
    assert_eq!(
        workbench.geometry_item(item.id).unwrap().coordinates,
        item.coordinates
    );
    workbench.finish_geometry_editing();
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("geometry-interaction")
            .is_empty()
    );
    let before_pan = pane.plot.ranges.get_untracked();
    harness.deliver_to_first(pointer(PointerAction::Pressed, x + w * 0.4, y + h * 0.4));
    harness.deliver_to_first(pointer(PointerAction::Moved, x + w * 0.5, y + h * 0.5));
    harness.deliver_to_first(pointer(PointerAction::Released, x + w * 0.5, y + h * 0.5));
    harness.settle(128);
    assert_ne!(
        pane.plot.ranges.get_untracked(),
        before_pan,
        "Done restores normal plot panning"
    );
    assert_eq!(
        workbench.geometry_item(item.id).unwrap().coordinates,
        item.coordinates,
        "Panning must not move data-space bounds"
    );
    let second = workbench.add_plot(pane.id, false, false);
    harness.settle(128);
    // Drag library definition onto second pane, preserving first membership.
    let row = harness.app().windows()[0]
        .box_with_class("geometry-row")
        .unwrap();
    let start = (
        row.origin.x.0 + row.size.width.0 / 2.,
        row.origin.y.0 + row.size.height.0 / 2.,
    );
    let host = workbench
        .geometry
        .hosts
        .with_value(|hosts| hosts[&second.id]);
    let target = host.window_bounds().unwrap();
    let end = (
        target.origin.x.0 + target.size.width.0 * 0.5,
        target.origin.y.0 + target.size.height.0 * 0.5,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, start.0, start.1));
    harness.deliver_to_first(pointer(PointerAction::Moved, end.0, end.1));
    harness.settle(128);
    assert_eq!(workbench.geometry.target.get_untracked(), Some(second.id));
    harness.deliver_to_first(pointer(PointerAction::Released, end.0, end.1));
    harness.settle(128);
    assert!(second.geometry.with_untracked(|ids| ids.contains(&item.id)));
    assert!(pane.geometry.with_untracked(|ids| ids.contains(&item.id)));
    workbench.toggle_geometry(second, item.id);
    assert!(
        second
            .hidden_geometry
            .with_untracked(|ids| ids.contains(&item.id))
    );
    assert!(
        !pane
            .hidden_geometry
            .with_untracked(|ids| ids.contains(&item.id))
    );
    workbench.remove_geometry_from(second, item.id);
    assert!(workbench.geometry_item(item.id).is_some());
    assert!(pane.geometry.with_untracked(|ids| ids.contains(&item.id)));
    assert!(!second.geometry.with_untracked(|ids| ids.contains(&item.id)));
    assert!(
        !second
            .hidden_geometry
            .with_untracked(|ids| ids.contains(&item.id))
    );
    workbench.attach_geometry(item.id, second, true);
    workbench.select_geometry(item.id);
    workbench.geometry_coordinates(0, "3.0".into());
    assert_eq!(workbench.geometry_item(item.id).unwrap().coordinates[0], 3.);
    workbench.geometry_coordinates(0, "NaN".into());
    assert_eq!(workbench.geometry_item(item.id).unwrap().coordinates[0], 3.);
    workbench.duplicate_geometry(item.id);
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), 2);
    let copy = workbench.geometry.selected.get_untracked().unwrap();
    assert_ne!(copy, item.id);
    workbench.delete_geometry(copy);
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), 1);
    workbench.arm_geometry(GeometryKind::Line);
    let g = plot::PlotGeometry::with_bounds(0., 0., 100., 100., 100., 100.).unwrap();
    assert!(workbench.begin_geometry(pane, kurbo::Point::new(20., 20.), g));
    workbench.move_geometry(pane, kurbo::Point::new(80., 80.), g);
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), 2);
    workbench.cancel_geometry_drag();
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), 1);
    workbench.delete_geometry(item.id);
    assert!(pane.geometry.with_untracked(|ids| ids.is_empty()));
    assert!(second.geometry.with_untracked(|ids| ids.is_empty()));
    workbench.arm_geometry(GeometryKind::Line);
    assert!(workbench.begin_geometry(pane, kurbo::Point::new(20., 20.), g));
    workbench.move_geometry(pane, kurbo::Point::new(80., 80.), g);
    workbench.end_geometry();
    harness.settle(128);
    let line = workbench.geometry.selected.get_untracked().unwrap();
    click_class(&mut harness, "geometry-horizontal");
    let horizontal = workbench.geometry_item(line).unwrap();
    assert_eq!(
        horizontal.orientation,
        crate::geometry::LineOrientation::Horizontal
    );
    assert_eq!(horizontal.coordinates[1], horizontal.coordinates[3]);
    assert!(
        harness.app().windows()[0]
            .box_with_class("geometry-level")
            .is_some()
    );
    workbench.geometry_coordinates(1, "4.25".into());
    assert_eq!(workbench.geometry_item(line).unwrap().coordinates[3], 4.25);
    click_class(&mut harness, "geometry-vertical");
    let vertical = workbench.geometry_item(line).unwrap();
    assert_eq!(
        vertical.orientation,
        crate::geometry::LineOrientation::Vertical
    );
    assert_eq!(vertical.coordinates[0], vertical.coordinates[2]);
    workbench.geometry_coordinates(0, "5.0".into());
    assert_eq!(workbench.geometry_item(line).unwrap().coordinates[2], 5.0);
    harness.settle(128);
    click_class(&mut harness, "geometry-editor-collapse");
    let handle = harness.app().windows()[0]
        .box_with_class("geometry-move-handle")
        .unwrap();
    let center = (
        handle.origin.x.0 + handle.size.width.0 / 2.,
        handle.origin.y.0 + handle.size.height.0 / 2.,
    );
    let original_line = workbench.geometry_item(line).unwrap();
    let original_view = pane.plot.ranges.get_untracked();
    harness.deliver_to_first(pointer(PointerAction::Pressed, center.0, center.1));
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        center.0 + 30.,
        center.1 + 60.,
    ));
    harness.deliver_to_first(pointer(
        PointerAction::Released,
        center.0 + 30.,
        center.1 + 60.,
    ));
    harness.settle(128);
    let moved = workbench.geometry_item(line).unwrap();
    assert_ne!(moved.coordinates[0], original_line.coordinates[0]);
    assert_eq!(moved.coordinates[0], moved.coordinates[2]);
    assert_eq!(moved.coordinates[1], original_line.coordinates[1]);
    assert_eq!(moved.coordinates[3], original_line.coordinates[3]);
    assert_eq!(pane.plot.ranges.get_untracked(), original_view);
    let handle = harness.app().windows()[0]
        .box_with_class("geometry-move-handle")
        .unwrap();
    let center = (
        handle.origin.x.0 + handle.size.width.0 / 2.,
        handle.origin.y.0 + handle.size.height.0 / 2.,
    );
    harness.deliver_to_first(pointer(PointerAction::Pressed, center.0, center.1));
    harness.deliver_to_first(pointer(PointerAction::Moved, center.0 + 20., center.1));
    harness.settle(128);
    assert_ne!(
        workbench.geometry_item(line).unwrap().coordinates,
        moved.coordinates
    );
    workbench.finish_geometry_editing();
    harness.settle(128);
    harness.deliver_to_first(pointer(PointerAction::Released, center.0 + 20., center.1));
    harness.settle(128);
    assert_eq!(
        workbench.geometry_item(line).unwrap().coordinates,
        moved.coordinates,
        "Cancel restores coordinates while unmounting handles"
    );
    assert!(
        harness.app().windows()[0]
            .pointer_capture(zgui::vocab::PointerId::MOUSE)
            .is_none(),
        "Canceled handle release must clear persistent capture"
    );
}

#[test]
fn state_bands_map_sources_render_labels_refresh_and_keep_manual_highlights() {
    use crate::{
        geometry::{Geometry, GeometryKind},
        state_bands::StateBands,
    };
    use plot::{PlotPoint, time_series::SeriesData};
    let (mut harness, workbench) = geometry_harness();
    let pane = workbench.pane();
    let log = crate::log::LogStore::read(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/esc_status_log.ulg"
    )))
    .unwrap();
    let source = log.catalogue[0].id.clone();
    let end = log.duration;
    assert!(end > 0.);
    workbench.log.set(Some(std::sync::Arc::new(log)));
    let data = SeriesData::new(vec![
        PlotPoint { x: 0., y: 0. },
        PlotPoint {
            x: end * 0.3,
            y: 3.,
        },
        PlotPoint {
            x: end * 0.6,
            y: 4.,
        },
    ])
    .unwrap();
    workbench.cache.update(|cache| {
        cache.insert(source.clone(), data.clone());
    });
    pane.plot
        .set_view(plot::Ranges::new((0., end), (-10., 10.)))
        .unwrap();
    harness.settle(128);
    click_class(&mut harness, "open-state-bands");
    let drop = harness.app().windows()[0]
        .box_with_class("band-source-drop")
        .unwrap();
    workbench.drop_fields(source.clone(), drop.origin.x.0 + 10., drop.origin.y.0 + 10.);
    let deadline = Instant::now() + Duration::from_secs(10);
    while workbench.bands.busy.get_untracked() {
        harness.settle(128);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.settle(256);
    let item = workbench
        .geometry
        .items
        .get_untracked()
        .into_iter()
        .find(|item| item.states.is_some())
        .unwrap();
    let id = item.id;
    assert_eq!(item.states.as_ref().unwrap().intervals.len(), 3);
    assert!(pane.geometry.with_untracked(|ids| ids.contains(&id)));
    let window = &harness.app().windows()[0];
    assert_eq!(window.nodes_with_class("band-label").len(), 3);
    assert_eq!(window.nodes_with_class("band-rule-row").len(), 3);
    assert!(
        window.nodes_with_class("geometry-handle").is_empty(),
        "Generated intervals have no manual handles"
    );
    let bg = window.box_with_class("band-background").unwrap();
    let plot = window.box_with_class("time-series-plot").unwrap();
    assert_eq!(bg, plot, "Background spans data area");
    crate::geometry_inspector::change(workbench, id, |item| {
        std::sync::Arc::make_mut(item.states.as_mut().unwrap()).rules[0].name = "Pilot".into()
    });
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("band-label")
            .iter()
            .any(|node| harness.app().windows()[0].dom().text_content(*node) == "Pilot"),
        "Label reacts without recreating interval"
    );
    workbench.geometry_coordinates(0, "9".into());
    assert!(
        workbench
            .geometry
            .error
            .get_untracked()
            .contains("Source controls")
    );
    workbench.toggle_geometry(pane, id);
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("band-label")
            .is_empty()
    );
    workbench.toggle_geometry(pane, id);
    harness.settle(128);
    let mut manual = Geometry::new(workbench.id(), GeometryKind::Band, kurbo::Point::ZERO);
    manual.name = "Review".into();
    manual.coordinates = [end * 0.1, 0., end * 0.2, 0.];
    let manual_id = manual.id;
    workbench.geometry.items.update(|items| items.push(manual));
    workbench.attach_geometry(manual_id, pane, true);
    harness.settle(128);
    assert!(
        harness.app().windows()[0]
            .nodes_with_class("band-label")
            .iter()
            .any(|node| harness.app().windows()[0].dom().text_content(*node) == "Review")
    );
    // Derived state changes regenerate intervals and preserve mappings; invalid output hides stale shading.
    let derived_id = workbench.id();
    let key = crate::derived::key(derived_id);
    workbench.derived.set(vec![crate::derived::DerivedField {
        id: derived_id,
        name: "Discrete".into(),
        sources: vec![source.clone()],
        group: derived_id,
        filter: crate::derived::FilterSpec::Affine {
            gain: 1.,
            offset: 0.,
        },
        data: Some(data.clone()),
    }]);
    crate::geometry_inspector::change(workbench, id, |item| {
        let states = std::sync::Arc::make_mut(item.states.as_mut().unwrap());
        states.source = key.clone();
    });
    assert!(
        workbench
            .delete_filter(derived_id)
            .unwrap_err()
            .contains("state bands")
    );
    let changed = SeriesData::new(vec![
        PlotPoint { x: 0., y: 0. },
        PlotPoint {
            x: end * 0.5,
            y: 4.,
        },
    ])
    .unwrap();
    let mut fields = workbench.derived.get_untracked();
    fields[0].data = Some(changed);
    workbench.publish_filter_fields(fields);
    let deadline = Instant::now() + Duration::from_secs(10);
    while workbench
        .bands
        .jobs
        .with_value(|jobs| jobs.contains_key(&id))
    {
        harness.settle(128);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.settle(128);
    let updated = workbench.geometry_item(id).unwrap();
    assert_eq!(updated.states.as_ref().unwrap().intervals.len(), 2);
    assert_eq!(updated.states.as_ref().unwrap().rules[0].name, "Pilot");
    let mut fields = workbench.derived.get_untracked();
    fields[0].data = Some(SeriesData::new(vec![PlotPoint { x: 0., y: 0.5 }]).unwrap());
    workbench.publish_filter_fields(fields);
    let deadline = Instant::now() + Duration::from_secs(10);
    while workbench
        .bands
        .jobs
        .with_value(|jobs| jobs.contains_key(&id))
    {
        harness.settle(128);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.settle(128);
    assert!(
        workbench
            .geometry_item(id)
            .unwrap()
            .states
            .unwrap()
            .error
            .is_some()
    );
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("band-label")
            .len(),
        1,
        "Invalid derived source leaves only manual highlight"
    );
    // Recovery must also work when undo restores exact previous input allocation.
    let restored = updated.states.as_ref().unwrap().input.clone();
    let mut fields = workbench.derived.get_untracked();
    fields[0].data = Some(restored);
    workbench.publish_filter_fields(fields);
    let deadline = Instant::now() + Duration::from_secs(10);
    while workbench
        .bands
        .jobs
        .with_value(|jobs| jobs.contains_key(&id))
    {
        harness.settle(128);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.settle(128);
    assert!(
        workbench
            .geometry_item(id)
            .unwrap()
            .states
            .unwrap()
            .error
            .is_none()
    );
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("band-label")
            .len(),
        3
    );
    // Canceling creation invalidates queued work; missing sources do not create geometry.
    let count = workbench.geometry.items.with_untracked(Vec::len);
    workbench.bands.open.set(true);
    workbench.generate_bands(source.clone(), None);
    workbench.cancel_band_source();
    for _ in 0..20 {
        harness.settle(128);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), count);
    workbench.clear_generated_bands();
    assert!(workbench.geometry_item(id).is_none());
    assert!(workbench.geometry_item(manual_id).is_some());
    workbench.generate_bands("missing/0/state".into(), None);
    let deadline = Instant::now() + Duration::from_secs(10);
    while workbench.bands.busy.get_untracked() {
        harness.settle(128);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!workbench.bands.error.get_untracked().is_empty());
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), 1);
    workbench.generate_bands(source, None);
    workbench.open(std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/no-such-band-test.ulg"
    )));
    let deadline = Instant::now() + Duration::from_secs(10);
    while workbench.loading.get_untracked() {
        harness.settle(128);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.settle(128);
    assert!(!workbench.bands.busy.get_untracked());
    assert_eq!(
        workbench.geometry.items.with_untracked(Vec::len),
        1,
        "Stale generation cannot attach bands after log switch"
    );
    workbench.edit_geometry_on(pane, manual_id);
    harness.settle(128);
    let handle = harness.app().windows()[0]
        .box_with_class("geometry-move-handle")
        .unwrap();
    let center = (
        handle.origin.x.0 + handle.size.width.0 / 2.,
        handle.origin.y.0 + handle.size.height.0 / 2.,
    );
    harness.deliver_to_first(pointer(PointerAction::Moved, center.0, center.1));
    harness.settle(128);
    assert_eq!(
        harness.platform().offscreens()[0].last_cursor(),
        Some(zgui::platform::CursorStyle::ResizeEastWest)
    );
    let previous = workbench.geometry_item(manual_id).unwrap().coordinates;
    harness.deliver_to_first(pointer(PointerAction::Pressed, center.0, center.1));
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        center.0 + 30.,
        center.1 + 40.,
    ));
    harness.deliver_to_first(pointer(
        PointerAction::Released,
        center.0 + 30.,
        center.1 + 40.,
    ));
    harness.settle(128);
    let moved = workbench.geometry_item(manual_id).unwrap().coordinates;
    assert_ne!(moved[0], previous[0]);
    assert_eq!(moved[1], previous[1]);
    assert_eq!(moved[3], previous[3]);
    workbench.finish_geometry_editing();
    harness.settle(128);
    click_class(&mut harness, "draw-time-band");
    let plot = harness.app().windows()[0]
        .box_with_class("time-series-plot")
        .unwrap();
    let (x, y, w, h) = (
        plot.origin.x.0,
        plot.origin.y.0,
        plot.size.width.0,
        plot.size.height.0,
    );
    let before = pane.plot.ranges.get_untracked();
    harness.deliver_to_first(pointer(PointerAction::Pressed, x + w * 0.7, y + h * 0.3));
    harness.deliver_to_first(pointer(PointerAction::Moved, x + w * 0.85, y + h * 0.6));
    harness.settle(128);
    assert_eq!(
        harness.platform().offscreens()[0].last_cursor(),
        Some(zgui::platform::CursorStyle::Crosshair)
    );
    harness.deliver_to_first(pointer(PointerAction::Released, x + w * 0.85, y + h * 0.6));
    harness.settle(128);
    let drawn = workbench
        .geometry_item(workbench.geometry.selected.get_untracked().unwrap())
        .unwrap();
    assert_eq!(drawn.kind, GeometryKind::Band);
    assert!(drawn.states.is_none() && drawn.nonempty());
    assert_eq!(pane.plot.ranges.get_untracked(), before);
    assert!(StateBands::build("bad".into(), SeriesData::new(vec![]).unwrap(), end).is_err());
}

#[test]
fn mapping_builder_adds_custom_values_reports_duplicates_and_removes_rows() {
    let (mut harness, workbench) = geometry_harness();
    click_class(&mut harness, "open-state-bands");
    click_class(&mut harness, "blank-state-mapping");
    let id = workbench.geometry.selected.get_untracked().unwrap();
    assert!(workbench.log.get_untracked().is_none());
    click_class(&mut harness, "band-add-state");
    assert!(
        !workbench.geometry.error.get_untracked().is_empty(),
        "add {:?}, editor {:?}",
        harness.app().windows()[0].box_with_class("band-add-state"),
        harness.app().windows()[0].box_with_class("geometry-inspector")
    );
    for (class, text) in [("band-new-value", "42"), ("band-new-name", "Custom hover")] {
        click_class(&mut harness, class);
        harness.deliver_to_first(SurfaceEvent::Key {
            state: zgui::vocab::KeyState::Pressed,
            event: zgui::vocab::KeyEvent::character(text),
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        harness.settle(128);
    }
    click_class(&mut harness, "band-add-state");
    assert!(
        workbench.geometry.error.get_untracked().is_empty(),
        "{}",
        workbench.geometry.error.get_untracked()
    );
    let item = workbench.geometry_item(id).unwrap();
    assert_eq!(item.states.as_ref().unwrap().rules[0].name, "Custom hover");
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("band-rule-row")
            .len(),
        1
    );
    click_class(&mut harness, "band-rule-value");
    for (text, modifiers) in [("a", Modifiers::CONTROL), ("-2", Modifiers::NONE)] {
        harness.deliver_to_first(SurfaceEvent::Key {
            state: zgui::vocab::KeyState::Pressed,
            event: zgui::vocab::KeyEvent::character(text),
            modifiers,
            timestamp: Timestamp::ORIGIN,
        });
        harness.settle(128);
    }
    click_class(&mut harness, "band-set-value");
    assert_eq!(
        workbench
            .geometry_item(id)
            .unwrap()
            .states
            .as_ref()
            .unwrap()
            .rules[0]
            .value,
        -2
    );
    assert!(!workbench.edit_mapping_rule(id, None, "-2", "Duplicate"));
    assert!(
        workbench
            .geometry
            .error
            .get_untracked()
            .contains("already exists")
    );
    click_class(&mut harness, "band-remove-rule");
    assert_eq!(
        workbench
            .geometry_item(id)
            .unwrap()
            .states
            .as_ref()
            .unwrap()
            .rules
            .len(),
        0
    );
    assert_eq!(
        harness.app().windows()[0]
            .nodes_with_class("band-rule-row")
            .len(),
        0
    );
}

#[test]
fn plot_tab_pointer_rename_and_keyboard_commit_cancel_blur() {
    let (mut harness, workbench) = geometry_harness();
    let key = |harness: &mut Harness<zgui::runtime::Runtime>, key: zgui::vocab::NamedKey| {
        harness.deliver_to_first(SurfaceEvent::Key {
            state: zgui::vocab::KeyState::Pressed,
            event: zgui::vocab::KeyEvent::named(key, zgui::vocab::PhysicalKey::Unidentified(0)),
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        harness.settle(128);
    };
    let text = |harness: &mut Harness<zgui::runtime::Runtime>, value: &str| {
        harness.deliver_to_first(SurfaceEvent::Key {
            state: zgui::vocab::KeyState::Pressed,
            event: zgui::vocab::KeyEvent::character(value),
            modifiers: Modifiers::NONE,
            timestamp: Timestamp::ORIGIN,
        });
        harness.settle(128);
    };
    click_class(&mut harness, "plot-tab-select");
    click_class(&mut harness, "plot-tab-select");
    harness.advance(Duration::from_millis(20));
    harness.settle(128);
    assert!(
        !harness.app().windows()[0]
            .nodes_with_class("plot-tab-name")
            .is_empty(),
        "Pointer double-click opens tab editor"
    );
    text(&mut harness, "Roll PID");
    key(&mut harness, zgui::vocab::NamedKey::Enter);
    assert_eq!(
        workbench
            .profiles
            .titles
            .get_untracked()
            .get(&1)
            .map(String::as_str),
        Some("Roll PID")
    );
    click_class(&mut harness, "plot-tab-select");
    key(&mut harness, zgui::vocab::NamedKey::F2);
    harness.advance(Duration::from_millis(20));
    harness.settle(128);
    assert!(
        !harness.app().windows()[0]
            .nodes_with_class("plot-tab-name")
            .is_empty()
    );
    text(&mut harness, "Discard");
    key(&mut harness, zgui::vocab::NamedKey::Escape);
    assert_eq!(workbench.profiles.titles.get_untracked()[&1], "Roll PID");
    click_class(&mut harness, "plot-tab-select");
    key(&mut harness, zgui::vocab::NamedKey::F2);
    harness.advance(Duration::from_millis(20));
    harness.settle(128);
    text(&mut harness, "Pitch PID");
    click_class(&mut harness, "fields-menu");
    assert_eq!(
        workbench.profiles.titles.get_untracked()[&1],
        "Pitch PID",
        "Blur saves rename"
    );
    assert!(
        harness.app().windows()[0]
            .pointer_capture(zgui::vocab::PointerId::MOUSE)
            .is_none()
    );
}

#[test]
fn profile_reload_recovers_optional_output_and_failed_reload_keeps_live_lines() {
    let (mut harness, workbench) = geometry_harness();
    let log = std::sync::Arc::new(
        crate::log::LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/esc_status_log.ulg"
        )))
        .unwrap(),
    );
    workbench.log.set(Some(log));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reload.json");
    let mut recipe = serde_json::json!({"version":1,"name":"Reloadable","filters":[
        {"id":"base","metadata":{"name":"Base"},"inputs":["esc_status/0/esc[5].esc_rpm"],"operation":{"kind":"affine","gain":1}},
        {"id":"optional","metadata":{"name":"Optional"},"inputs":["missing/0/source"],"operation":{"kind":"affine","gain":2}}],
        "views":[{"name":"Motor","fields":["$base","$optional"]}]});
    std::fs::write(&path, recipe.to_string()).unwrap();
    workbench.apply_profile(path.clone());
    let wait = |harness: &mut Harness<zgui::runtime::Runtime>| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while workbench.profiles.busy.get_untracked() {
            assert!(Instant::now() < deadline, "Profile task stuck");
            harness.settle(128);
            std::thread::sleep(Duration::from_millis(2));
        }
        harness.settle(128);
    };
    wait(&mut harness);
    let profile_key = crate::profile::identity::ProfileKey::new(path.canonicalize().unwrap(), 1);
    let before = workbench.profiles.instances.get_untracked()[&profile_key]
        .ids
        .clone();
    workbench.place_prepared_item(
        profile_key.clone(),
        crate::profile::identity::Item::View("Motor".into()),
        crate::profile_state::Destination::Plot(Some(1)),
    );
    assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 1);
    let line = workbench.pane().lines.get_untracked()[0].clone();
    recipe["filters"][0]["operation"]["gain"] = serde_json::json!(3);
    recipe["filters"][1]["inputs"] = serde_json::json!(["esc_status/0/esc[5].esc_rpm"]);
    std::fs::write(&path, recipe.to_string()).unwrap();
    workbench.reload_profile(profile_key.clone());
    wait(&mut harness);
    assert!(
        workbench.profiles.error.get_untracked().is_empty(),
        "{}",
        workbench.profiles.error.get_untracked()
    );
    assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 2);
    assert_eq!(workbench.pane().lines.get_untracked()[0].id, line.id);
    assert_eq!(
        workbench.profiles.instances.get_untracked()[&profile_key]
            .ids
            .fields,
        before.fields
    );
    let revision = workbench.pane().lines.get_untracked()[0].revision;
    std::fs::write(&path, b"broken JSON").unwrap();
    workbench.reload_profile(profile_key.clone());
    wait(&mut harness);
    assert!(!workbench.profiles.error.get_untracked().is_empty());
    assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 2);
    assert_eq!(workbench.pane().lines.get_untracked()[0].revision, revision);
    recipe["filters"][0]["inputs"] = serde_json::json!(["missing/0/source"]);
    std::fs::write(&path, recipe.to_string()).unwrap();
    workbench.reload_profile(profile_key);
    wait(&mut harness);
    assert!(
        workbench
            .profiles
            .error
            .get_untracked()
            .contains("previous setup")
    );
    assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 2);
}

#[test]
fn multi_flight_profile_instances_use_separate_ids_names_settings_and_bands() {
    let (mut harness, workbench) = geometry_harness();
    let log = std::sync::Arc::new(
        crate::log::LogStore::read(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/synthetic-multi-flight.BBL"
        )))
        .unwrap(),
    );
    assert_eq!(log.settings.len(), 2);
    for flight in [1, 2] {
        let parameter = log.settings[&flight]
            .iter()
            .position(|s| s.name == "motor_poles")
            .unwrap();
        assert_eq!(
            log.prepare(&format!("@setting/{flight}/{parameter}"))
                .unwrap()
                .samples()
                .last()
                .unwrap()
                .x,
            3. * flight as f64
        );
    }
    assert_eq!(
        log.prepare("blackbox/2/gyroADC[0]").unwrap().samples()[0].y,
        20.
    );
    workbench.log.set(Some(log));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("scope.json");
    std::fs::write(&path,serde_json::json!({"version":1,"name":"Scoped","requires_settings":[{"setting":{"name":"Firmware revision"},"starts_with":"Betaflight 2026.6."}],
        "filters":[{"id":"gyro","metadata":{"name":"Gyro","unit":"deg/s"},"inputs":["blackbox/1/gyroADC[0]"],"operation":{"kind":"affine","gain":1}}],
        "geometry":[{"id":"flags","name":"Modes","shape":{"kind":"flags","source":"blackbox/1/flightModeFlags","rules":[{"mask":1,"name":"Example A","color":[1,2,3]},{"mask":4,"name":"Example B","color":[4,5,6]}]}}],
        "views":[{"name":"Gyro","fields":["$gyro"],"geometry":["flags"]}]}).to_string()).unwrap();
    for flight in [1, 2] {
        workbench.profiles.flight.set(flight);
        workbench.apply_profile(path.clone());
        let deadline = Instant::now() + Duration::from_secs(10);
        while workbench.profiles.busy.get_untracked() {
            assert!(Instant::now() < deadline);
            harness.settle(128);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            workbench.profiles.error.get_untracked().is_empty(),
            "{}",
            workbench.profiles.error.get_untracked()
        );
    }
    let instances = workbench.profiles.instances.get_untracked();
    assert_eq!(instances.len(), 2);
    let ids = instances
        .values()
        .map(|p| p.ids.fields["gyro"])
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 2);
    assert_eq!(workbench.derived.with_untracked(Vec::len), 2);
    assert_eq!(workbench.geometry.items.with_untracked(Vec::len), 2);
    for instance in instances.values() {
        assert!(
            instance.fields[0].sources[0]
                .starts_with(&format!("blackbox/{}/", instance.key.flight))
        );
        assert!(
            instance.fields[0]
                .name
                .contains(&format!("flight {}", instance.key.flight))
        );
        let bands = instance.geometry[0].states.as_ref().unwrap();
        assert!(
            bands
                .rules
                .iter()
                .any(|rule| rule.name == "Example A + Example B")
        );
    }
}
