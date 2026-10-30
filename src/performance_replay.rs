//! Opt-in replay against local logs/profiles; no private recordings are checked in.
use super::*;
use crate::{
    profile::identity::{ProfileKey, ResourceIds},
    profile_instance::AppliedProfile,
};
use std::{
    collections::BTreeSet,
    io::Write,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

// The pinned backend exposes mutable downcasting but not shared downcasting.
// Forward every renderer operation and add the read-only seam for image validation.
struct ReadbackRenderer(zgui_render_wgpu::WgpuRenderer);
impl Renderer for ReadbackRenderer {
    fn capabilities(&self) -> zgui::render::RenderCapabilities {
        self.0.capabilities()
    }
    fn configure(&mut self, target: RenderTarget) {
        self.0.configure(target);
    }
    fn target(&self) -> Option<RenderTarget> {
        self.0.target()
    }
    fn draw(&mut self, scene: &zgui::scene::Scene, damage: &zgui::bits::DamageSet) -> FrameOutcome {
        self.0.draw(scene, damage)
    }
    fn shifts_composed_pixels(&self) -> bool {
        self.0.shifts_composed_pixels()
    }
    fn composed_survives_configure(&self) -> bool {
        self.0.composed_survives_configure()
    }
    fn shift_composed(&mut self, shift: zgui::render::ScrollShift) {
        self.0.shift_composed(shift);
    }
    fn register_external(&mut self, texture: ExternalTexture) -> TextureHandle {
        self.0.register_external(texture)
    }
    fn release_external(&mut self, handle: TextureHandle) {
        self.0.release_external(handle);
    }
    fn memory(&self) -> MemoryReport {
        self.0.memory()
    }
    fn vector_status(&self) -> zgui::render::VectorStatus {
        self.0.vector_status()
    }
    fn target_pool(&self) -> zgui::render::TargetPoolReport {
        self.0.target_pool()
    }
    fn release_cached_targets(&mut self) -> u64 {
        self.0.release_cached_targets()
    }
    fn release_idle_resources(&mut self) -> u64 {
        self.0.release_idle_resources()
    }
    fn acquire_block(&self) -> Duration {
        self.0.acquire_block()
    }
    fn texture_sink(&mut self) -> &mut dyn zgui::atlas::TextureSink {
        self.0.texture_sink()
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        self.0.as_any_mut()
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(&self.0)
    }
}

#[derive(Default)]
struct Samples {
    frames: Vec<(f64, f64, f64, u32, u64, usize)>,
    steps: Vec<f64>,
    cpu_steps: Vec<f64>,
}
#[cfg(windows)]
fn thread_cpu() -> Option<Duration> {
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::Threading::{GetCurrentThread, GetThreadTimes},
    };
    let mut times = [FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    }; 4];
    let [created, exited, kernel, user] = &mut times;
    // These are output buffers and the pseudo-handle names this live thread.
    if unsafe { GetThreadTimes(GetCurrentThread(), created, exited, kernel, user) } == 0 {
        return None;
    }
    let ticks =
        |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    Some(Duration::from_nanos((ticks(*kernel) + ticks(*user)) * 100))
}
#[cfg(not(windows))]
fn thread_cpu() -> Option<Duration> {
    None
}
struct ReplayProbe {
    samples: Rc<RefCell<Option<Samples>>>,
    gpu: Rc<RefCell<Option<Arc<zgui_render_wgpu::Gpu>>>>,
}
impl zgui::runtime::FrameProbe for ReplayProbe {
    fn frame_ended(&self, _window: &zgui::runtime::Window) {}
    fn frame_reported(
        &self,
        _window: &zgui::runtime::Window,
        report: &zgui::runtime::window::frame::FrameReport,
    ) {
        let wait = Instant::now();
        if let Some(gpu) = self.gpu.borrow().as_ref() {
            gpu.wait();
        }
        let waited = wait.elapsed().as_secs_f64() * 1000.;
        if let Some(samples) = self.samples.borrow_mut().as_mut() {
            let stats = match report.outcome {
                FrameOutcome::Presented(stats) => stats,
                _ => FrameStats::default(),
            };
            samples.frames.push((
                report.elapsed.as_secs_f64() * 1000.,
                report.paint_elapsed.as_secs_f64() * 1000.,
                waited,
                stats.draw_calls,
                stats.bytes_uploaded,
                report.restyled,
            ));
        }
    }
}
fn p95(values: impl Iterator<Item = f64>) -> f64 {
    let mut values = values.collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        0.
    } else {
        values[(values.len() * 95).div_ceil(100).saturating_sub(1)]
    }
}
fn replay_pointer(
    harness: &Harness<zgui::runtime::Runtime>,
    epoch: Instant,
    action: PointerAction,
    x: f32,
    y: f32,
) -> SurfaceEvent {
    let mut event = pointer(action, x, y);
    if let SurfaceEvent::Pointer { timestamp, .. } = &mut event {
        *timestamp = Timestamp::from_origin(harness.now().saturating_duration_since(epoch));
    }
    event
}
fn references(value: &serde_json::Value, needed: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::String(value) => {
            if let Some(id) = value.strip_prefix('$') {
                needed.insert(id.into());
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                references(value, needed);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                references(value, needed);
            }
        }
        _ => {}
    }
}

#[test]
#[ignore = "local recording/profile and optional real GPU; see docs/PERFORMANCE.md"]
fn replay_attitude_drag() {
    zgui_profile::latency::start_epoch();
    use crate::{
        browser::BrowserProps, fps::FpsProps, theme::AppThemeProps, workspace::PlotWorkspaceProps,
    };
    let path = PathBuf::from(std::env::var_os("ENLUMI_REPLAY_LOG").expect("ENLUMI_REPLAY_LOG"));
    let profile_path =
        PathBuf::from(std::env::var_os("ENLUMI_REPLAY_PROFILE").expect("ENLUMI_REPLAY_PROFILE"));
    let full = std::env::var("ENLUMI_REPLAY_FULL").as_deref() == Ok("1");
    let use_gpu = std::env::var("ENLUMI_REPLAY_GPU").as_deref() == Ok("1");
    let started = Instant::now();
    eprintln!("Reading {}", path.display());
    let log = Arc::new(crate::log::LogStore::read(&path).unwrap());
    eprintln!(
        "Log {:.2}s, {} fields; read in {:?}",
        log.duration,
        log.catalogue.len(),
        started.elapsed()
    );
    let mut definition = crate::profile::load(&profile_path).unwrap();
    let mut recipe = definition
        .workspace_presets
        .iter()
        .find(|workspace| workspace.name == "PX4 Flight Review")
        .unwrap()
        .clone();
    if !full {
        recipe.views.retain(|view| view.name == "Attitude");
        recipe.active = 0;
        let mut needed = BTreeSet::new();
        references(&serde_json::to_value(&recipe).unwrap(), &mut needed);
        let geometry = recipe
            .views
            .iter()
            .flat_map(|view| &view.plots)
            .flat_map(|plot| plot.geometry.clone())
            .collect::<BTreeSet<_>>();
        definition
            .geometry
            .retain(|item| geometry.contains(&item.id));
        references(
            &serde_json::to_value(&definition.geometry).unwrap(),
            &mut needed,
        );
        loop {
            let before = needed.len();
            for filter in &definition.filters {
                if needed.contains(&filter.id) {
                    references(&serde_json::to_value(filter).unwrap(), &mut needed);
                }
            }
            if needed.len() == before {
                break;
            }
        }
        definition
            .filters
            .retain(|filter| needed.contains(&filter.id));
        definition.workspace_presets = vec![recipe.clone()];
        definition.views.clear();
        definition.view_presets.clear();
        definition.plot_presets.clear();
    }
    let samples = Rc::new(RefCell::new(None));
    let gpu = Rc::new(RefCell::new(None));
    let device = gpu.clone();
    let graphics = zgui_render_wgpu::SharedGraphics::new();
    let factory: zgui::runtime::RendererFactory = if use_gpu {
        Box::new(move |_, target| {
            let mut renderer = graphics.renderer_offscreen(
                target,
                zgui_render_wgpu::wgpu::TextureFormat::Bgra8Unorm,
                false,
            )?;
            eprintln!("GPU: {}", renderer.gpu().describe());
            *device.borrow_mut() = Some(renderer.gpu().clone());
            zgui_render_vector_vello::attach(&mut renderer, target.size);
            if std::env::var_os("ENLUMI_REPLAY_SCREENSHOTS").is_some() {
                Ok(Box::new(ReadbackRenderer(renderer)) as Box<dyn Renderer>)
            } else {
                Ok(Box::new(renderer) as Box<dyn Renderer>)
            }
        })
    } else {
        Box::new(|_, target| {
            let mut renderer = CaptureRenderer::new();
            renderer.configure(target);
            Ok(Box::new(renderer))
        })
    };
    let model = Rc::new(RefCell::new(None));
    let built = model.clone();
    let handler=app().with_size(1500.,850.).with_renderer(factory)
        .with_probe(Rc::new(ReplayProbe{samples:samples.clone(),gpu}))
        .with_stylesheet(format!("{}\n{}\n{}\n{}",plot::zgui_surface::stylesheet(),plot::zgui_time_series::stylesheet(),plot::zgui_workspace::stylesheet(),crate::theme::stylesheet()))
        .into_handler(move ||{let workbench=crate::state::Workbench::new();*built.borrow_mut()=Some(workbench);
            view!{AppTheme(palette=crate::theme::Palette::Forest){column(class="page workbench"){row(class="toolbar"){Fps()}row(class="body"){Browser(workbench=workbench) PlotWorkspace(workbench=workbench)}}}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1500.),
        DevicePx(850.),
    )));
    harness.settle(128);
    let workbench = model.borrow().unwrap();
    workbench.log.set(Some(log.clone()));
    let mut ids = ResourceIds::default();
    ids.reconcile(&definition, || workbench.id());
    eprintln!(
        "Preparing {} filters (full workspace: {full})",
        definition.filters.len()
    );
    let prepared = crate::profile::prepare::prepare_with_ids(
        definition.clone(),
        &log,
        profile_path.parent().unwrap(),
        ids,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let fields = prepared.fields.clone();
    let key = ProfileKey::new(profile_path, 1);
    let instance = AppliedProfile::from_prepared(key.clone(), definition, prepared);
    workbench.commit_instance(instance, fields, vec![]).unwrap();
    workbench.open_workspace_preset(&key, recipe).unwrap();
    let view = workbench.views.items.with_untracked(|views| {
        views
            .iter()
            .find(|view| view.name.get_untracked() == "Attitude")
            .unwrap()
            .id
    });
    workbench.switch_view(view);
    let deadline = Instant::now() + Duration::from_secs(180);
    while workbench
        .all_panes()
        .iter()
        .any(|pane| pane.loading.with_untracked(Option::is_some))
    {
        harness.settle(128);
        assert!(
            Instant::now() < deadline,
            "Plots did not finish: {}",
            workbench.status.get_untracked()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let panes = workbench.panes().get_untracked();
    assert_eq!(panes.len(), 3);
    harness.settle(128);
    let fitted = panes
        .iter()
        .map(|pane| pane.plot.ranges.get_untracked())
        .collect::<Vec<_>>();
    for pane in &panes {
        eprintln!(
            "plot {}: {:?}",
            pane.id,
            pane.lines.with_untracked(|lines| lines
                .iter()
                .map(|line| (line.name.clone(), line.data.samples().len()))
                .collect::<Vec<_>>())
        );
    }
    let output = std::env::var_os("ENLUMI_REPLAY_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/attitude-replay.csv"));
    let mut output = std::io::BufWriter::new(std::fs::File::create(output).unwrap());
    writeln!(
        output,
        "case,frame_ms,paint_submit_ms,gpu_wait_ms,draw_calls,uploaded_bytes,restyled"
    )
    .unwrap();
    let geometry = panes
        .iter()
        .map(|pane| pane.geometry.get_untracked())
        .collect::<Vec<_>>();
    let epoch = harness.now();
    for name in [
        "linked-full",
        "linked-zoom",
        "unlinked-zoom",
        "no-bands-zoom",
        "maximized-zoom",
    ] {
        for (index, pane) in panes.iter().enumerate() {
            pane.geometry.set(if name == "no-bands-zoom" {
                Default::default()
            } else {
                geometry[index].clone()
            });
            pane.links.update(|links| {
                links.x = name != "unlinked-zoom";
                links.pointer = true;
            });
            pane.plot.auto_fit.set(false);
            pane.plot
                .set_view(plot::Ranges::new(
                    if name == "linked-full" {
                        (fitted[index].x.min, fitted[index].x.max)
                    } else {
                        (log.duration * 0.3, log.duration * 0.4)
                    },
                    fitted[index].y,
                ))
                .unwrap();
        }
        workbench
            .maximized()
            .set((name == "maximized-zoom").then_some(panes[0].id));
        for _ in 0..45 {
            harness.advance(Duration::from_millis(17));
            harness.settle(32);
            std::thread::sleep(Duration::from_millis(3));
        }
        let bounds = panes[0]
            .menu_area
            .get_untracked()
            .unwrap()
            .window_bounds()
            .unwrap();
        let scale = panes[0].dock.node.scale();
        let x = (bounds.origin.x.0 + bounds.size.width.0 * 0.5) / scale;
        let y = (bounds.origin.y.0 + bounds.size.height.0 * 0.5) / scale;
        let expected_span = panes[0].plot.ranges.get_untracked().x.span();
        let event = replay_pointer(&harness, epoch, PointerAction::Pressed, x, y);
        harness.deliver_to_first(event);
        *samples.borrow_mut() = Some(Samples::default());
        zgui_profile::latency::note("replay.begin", name);
        for step in 0..180 {
            let start = Instant::now();
            let cpu = thread_cpu();
            let offset = ((step as f32 / 30.).sin()) * 90.;
            let event = replay_pointer(&harness, epoch, PointerAction::Moved, x + offset, y);
            harness.deliver_to_first(event);
            harness.advance(Duration::from_millis(17));
            harness.settle(32);
            let elapsed = start.elapsed();
            samples
                .borrow_mut()
                .as_mut()
                .unwrap()
                .steps
                .push(elapsed.as_secs_f64() * 1000.);
            if let Some(elapsed) = cpu
                .zip(thread_cpu())
                .and_then(|(start, end)| end.checked_sub(start))
            {
                samples
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .cpu_steps
                    .push(elapsed.as_secs_f64() * 1000.);
            }
            std::thread::sleep(Duration::from_millis(17).saturating_sub(elapsed));
        }
        let measured = samples.borrow_mut().take().unwrap();
        zgui_profile::latency::mark("replay.end");
        zgui_profile::latency::flush();
        assert!(
            (panes[0].plot.ranges.get_untracked().x.span() - expected_span).abs()
                <= expected_span * 1e-8,
            "A pan must preserve the configured zoom span: {name}"
        );
        eprintln!(
            "{name}: UI thread CPU p95 {} ms",
            if measured.cpu_steps.is_empty() {
                "unavailable".into()
            } else {
                format!("{:.2}", p95(measured.cpu_steps.iter().copied()))
            }
        );
        let event = replay_pointer(&harness, epoch, PointerAction::Released, x, y);
        harness.deliver_to_first(event);
        harness.settle(32);
        if let Some(directory) = std::env::var_os("ENLUMI_REPLAY_SCREENSHOTS")
            && use_gpu
        {
            let directory = PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            let renderer = harness.app().windows()[0]
                .renderer()
                .as_any()
                .and_then(|renderer| renderer.downcast_ref::<zgui_render_wgpu::WgpuRenderer>())
                .expect("GPU replay must expose image readback");
            {
                let pixels = renderer.read_composed();
                let size = pixels.size();
                let mut image = std::io::BufWriter::new(
                    std::fs::File::create(directory.join(format!("{name}.ppm"))).unwrap(),
                );
                write!(image, "P6\n{} {}\n255\n", size.width, size.height).unwrap();
                for y in 0..size.height {
                    for x in 0..size.width {
                        image.write_all(&pixels.rgba(x, y)[..3]).unwrap();
                    }
                }
            }
        }
        eprintln!(
            "{name}: {} frames, input step p95 {:.2}ms; frame p95 {:.2}ms; paint/submit p95 {:.2}ms; GPU completion wait p95 {:.2}ms; mean restyled {:.1}; uploads {:.2}MiB",
            measured.frames.len(),
            p95(measured.steps.iter().copied()),
            p95(measured.frames.iter().map(|frame| frame.0)),
            p95(measured.frames.iter().map(|frame| frame.1)),
            p95(measured.frames.iter().map(|frame| frame.2)),
            measured
                .frames
                .iter()
                .map(|frame| frame.5 as f64)
                .sum::<f64>()
                / measured.frames.len().max(1) as f64,
            measured.frames.iter().map(|frame| frame.4).sum::<u64>() as f64 / 1048576.
        );
        for (frame, paint, wait, calls, uploads, restyled) in measured.frames {
            writeln!(
                output,
                "{name},{frame:.3},{paint:.3},{wait:.3},{calls},{uploads},{restyled}"
            )
            .unwrap();
        }
        output.flush().unwrap();
    }
}

#[test]
#[ignore = "real offscreen GPU preview; writes target/xy-builder-preview.ppm"]
fn render_xy_builder_preview() {
    use crate::{
        state::Line,
        theme::AppThemeProps,
        trajectory_axes::{TrajectorySetupProps, assign},
        workspace::PlotWorkspaceProps,
    };
    let graphics = zgui_render_wgpu::SharedGraphics::new();
    let captured = Rc::new(RefCell::new(None));
    let saved = captured.clone();
    let handler=app().with_size(1240.,760.).with_renderer(Box::new(move |_,target| {
        let mut renderer=graphics.renderer_offscreen(target,zgui_render_wgpu::wgpu::TextureFormat::Bgra8Unorm,false)?;
        zgui_render_vector_vello::attach(&mut renderer,target.size);
        Ok(Box::new(ReadbackRenderer(renderer)) as Box<dyn Renderer>)
    })).with_stylesheet(format!("{}\n{}\n{}\n{}\n.xy-preview-builder {{ width:300px;min-width:300px;flex:none;border-left:1px solid var(--app-border); }}",
        plot::zgui_surface::stylesheet(),plot::zgui_time_series::stylesheet(),plot::zgui_workspace::stylesheet(),crate::theme::stylesheet()))
    .into_handler(move || {
        let workbench=crate::state::Workbench::new();let original=workbench.pane();
        let lines=[("east","East [m]"),("north","North [m]"),("target-east","Target East [m]"),("target-north","Target North [m]")]
            .into_iter().enumerate().map(|(axis,(source,name))| {
                let id=workbench.id();let mut style=original.plot.style(&id.to_string());style.color=crate::line_colors::PALETTE[0];
                original.plot.styles.update(|styles|{styles.insert(id.to_string(),style);});
                let data=plot::time_series::SeriesData::new((0..2048).map(|i| {
                    let angle=i as f64*std::f64::consts::TAU/2047.;let radius=if axis<2{3.}else{2.};
                    plot::PlotPoint{x:i as f64/100.,y:radius*if axis%2==0{angle.cos()}else{angle.sin()}}
                }).collect::<Vec<_>>()).unwrap();
                Line{id,source:source.into(),name:name.into(),revision:0,data}
            }).collect::<Vec<_>>();
        original.lines.set(lines.clone());workbench.new_trajectory(false);
        let draft=workbench.trajectory_builder().pane;draft.lines.set(lines);
        assign(draft,0,false,&["east".into(),"target-east".into()]).unwrap();
        assign(draft,0,true,&["north".into(),"target-north".into()]).unwrap();
        workbench.drop_trajectory_pair(0,original.id);let xy=workbench.pane();workbench.drop_trajectory_pair(1,xy.id);
        workbench.maximized().set(Some(xy.id));*saved.borrow_mut()=Some(xy);
        view! {AppTheme(palette=crate::theme::Palette::Forest){column(class="page workbench"){row(class="body"){
            column(class="plot-page"){PlotWorkspace(workbench=workbench)}
            column(class="trajectory-side-panel xy-preview-builder"){TrajectorySetup(workbench=workbench,pane=draft)}
        }}}}
    }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1240.),
        DevicePx(760.),
    )));
    harness.settle(128);
    let xy = captured.borrow().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while xy.trajectory_data.with_untracked(Option::is_none) {
        harness.settle(128);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    for _ in 0..12 {
        harness.advance(Duration::from_millis(17));
        harness.settle(64);
        std::thread::sleep(Duration::from_millis(3));
    }
    let renderer = harness.app().windows()[0]
        .renderer()
        .as_any()
        .unwrap()
        .downcast_ref::<zgui_render_wgpu::WgpuRenderer>()
        .unwrap();
    let pixels = renderer.read_composed();
    let size = pixels.size();
    let mut file =
        std::io::BufWriter::new(std::fs::File::create("target/xy-builder-preview.ppm").unwrap());
    write!(file, "P6\n{} {}\n255\n", size.width, size.height).unwrap();
    for y in 0..size.height {
        for x in 0..size.width {
            file.write_all(&pixels.rgba(x, y)[..3]).unwrap();
        }
    }
}
