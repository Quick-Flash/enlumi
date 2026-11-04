//! An event-driven producer on zgui's device. No polling loop or UI-thread rendering.
use super::camera::Camera;
use super::{
    terrain::{Orbit, Scene},
    terrain_gpu,
};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU64, Ordering},
};
use zgui::surface::{GpuShare, SurfaceConfig, SurfaceEvent, SurfaceHandle, wgpu};

#[derive(Default)]
pub struct Stats {
    pub frames: AtomicU64,
    pub rebuilds: AtomicU64,
    pub uploads: AtomicU64,
    pub resident_bytes: AtomicU64,
    pub scene_revision: AtomicU64,
    pub stopped: AtomicU64,
}

#[derive(Clone)]
struct State {
    gpu: Option<GpuShare>,
    size: [u32; 2],
    dpi: f32,
    camera: Camera,
    scene: Option<Arc<Scene>>,
    orbit: Orbit,
    visible: bool,
    closed: bool,
    revision: u64,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

pub struct Producer {
    pub handle: SurfaceHandle,
    // Owns the endpoint whose weak reference the event callback uses.
    _events: Arc<SurfaceHandle>,
    shared: Arc<Shared>,
    capture: Capture,
    #[cfg(test)]
    pub stats: Arc<Stats>,
}

#[derive(Clone)]
pub struct Capture {
    frames: Arc<CaptureFrames>,
    shared: Arc<Shared>,
}
#[derive(Default)]
struct CaptureFrames {
    frame: Mutex<Option<CapturedFrame>>,
    ready: Condvar,
}

#[derive(Clone)]
struct CapturedFrame {
    gpu: GpuShare,
    texture: Arc<wgpu::Texture>,
    camera: Camera,
    orbit: Orbit,
    css: [f64; 2],
    providers: Vec<super::tiles::Provider>,
    revision: u64,
}
pub struct Snapshot {
    pub png: Vec<u8>,
    pub metres_per_pixel: Option<f64>,
    pub providers: Vec<super::tiles::Provider>,
}
impl Capture {
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.frames, &other.frames)
    }
    pub fn png(&self) -> Result<Snapshot, String> {
        let requested = {
            let state = self.shared.state.lock().unwrap();
            if state.closed || !state.visible || state.gpu.is_none() {
                return Err("3D map is not ready to capture".into());
            }
            state.revision
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut frame = self.frames.frame.lock().unwrap();
        while frame
            .as_ref()
            .is_none_or(|frame| frame.revision < requested)
        {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err("3D screenshot timed out waiting for current frame".into());
            }
            frame = self.frames.ready.wait_timeout(frame, remaining).unwrap().0;
        }
        let captured = frame.as_ref().unwrap().clone();
        drop(frame);
        let frame = captured;
        let width = frame.texture.width();
        let height = frame.texture.height();
        let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let device = frame.gpu.device();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.screenshot"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("map.screenshot"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &frame.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            frame.texture.size(),
        );
        let submission = frame.gpu.queue().submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(3)),
            })
            .map_err(|error| format!("3D screenshot GPU readback failed: {error}"))?;
        rx.recv_timeout(std::time::Duration::from_secs(1))
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        let mapped = slice.get_mapped_range();
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for pixels in mapped.chunks_exact(row as usize) {
            rgba.extend_from_slice(&pixels[..(width * 4) as usize]);
        }
        drop(mapped);
        buffer.unmap();
        let image =
            image::RgbaImage::from_raw(width, height, rgba).ok_or("Invalid 3D screenshot size")?;
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|error| error.to_string())?;
        let [w, h] = frame.css;
        let metres_per_pixel = frame
            .orbit
            .ground_at(frame.camera, [w, h], [w / 2. - 0.5, h / 2.])
            .zip(
                frame
                    .orbit
                    .ground_at(frame.camera, [w, h], [w / 2. + 0.5, h / 2.]),
            )
            .map(|(a, b)| {
                (b[0] - a[0]).hypot(b[1] - a[1]) * super::terrain::ground_scale(frame.camera)
            });
        Ok(Snapshot {
            png: png.into_inner(),
            metres_per_pixel,
            providers: frame.providers,
        })
    }
}
impl Producer {
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                gpu: None,
                size: [0, 0],
                dpi: 1.,
                camera: Camera::default(),
                scene: None,
                orbit: Orbit::default(),
                visible: true,
                closed: false,
                revision: 0,
            }),
            changed: Condvar::new(),
        });
        let capture = Capture {
            frames: Arc::new(CaptureFrames::default()),
            shared: shared.clone(),
        };
        let event_capture = capture.clone();
        let handle = SurfaceHandle::new(SurfaceConfig::default());
        let endpoint = Arc::new(handle.clone());
        let event_output = Arc::downgrade(&endpoint);
        let events = shared.clone();
        handle.set_events(move |event| {
            let mut state = events.state.lock().unwrap();
            if state.closed {
                return;
            }
            if matches!(
                &event,
                SurfaceEvent::Attached { .. }
                    | SurfaceEvent::Resized { .. }
                    | SurfaceEvent::DeviceLost { .. }
                    | SurfaceEvent::Detached
                    | SurfaceEvent::Visible(false)
            ) {
                event_capture.frames.frame.lock().unwrap().take();
            }
            match event {
                SurfaceEvent::Attached { gpu, size, scale } => {
                    state.gpu = Some(gpu);
                    state.size = [size.width, size.height];
                    state.dpi = scale;
                }
                SurfaceEvent::Resized { size, scale } => {
                    state.size = [size.width, size.height];
                    state.dpi = scale;
                }
                SurfaceEvent::Visible(visible) => state.visible = visible,
                SurfaceEvent::DeviceLost { gpu } => {
                    // The host can import pending output immediately after this
                    // callback. SurfaceHandle has no clear operation, so replace
                    // any old-device frame with a tiny valid new-device image.
                    // Pipeline/terrain rebuilding remains on the worker.
                    if let Some(output) = event_output.upgrade() {
                        output.present(recovery_image(&gpu));
                    }
                    state.gpu = Some(gpu);
                }
                SurfaceEvent::Detached => {
                    state.gpu = None;
                    state.visible = false;
                }
            }
            state.revision = state.revision.wrapping_add(1);
            events.changed.notify_one();
        });
        let stats = Arc::new(Stats::default());
        let worker_stats = stats.clone();
        let worker = shared.clone();
        let output = handle.clone();
        let worker_capture = capture.clone();
        std::thread::spawn(move || {
            produce(&worker, &output, &worker_stats, &worker_capture);
            worker_stats.stopped.store(1, Ordering::Release);
        });
        Self {
            handle,
            _events: endpoint,
            shared,
            capture,
            #[cfg(test)]
            stats,
        }
    }

    pub fn capture(&self) -> Capture {
        self.capture.clone()
    }
    pub fn camera(&self, camera: Camera) {
        let mut state = self.shared.state.lock().unwrap();
        if state.camera != camera {
            state.camera = camera;
            state.revision = state.revision.wrapping_add(1);
            self.shared.changed.notify_one();
        }
    }
    pub fn terrain(&self, camera: Camera, orbit: Orbit, scene: Arc<Scene>) {
        let mut state = self.shared.state.lock().unwrap();
        if state.camera != camera
            || state.orbit != orbit
            || state
                .scene
                .as_ref()
                .is_none_or(|old| !Arc::ptr_eq(old, &scene))
        {
            state.camera = camera;
            state.orbit = orbit;
            state.scene = Some(scene);
            state.revision = state.revision.wrapping_add(1);
            self.shared.changed.notify_one();
        }
    }
}

fn recovery_image(gpu: &GpuShare) -> Arc<wgpu::Texture> {
    let size = wgpu::Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let texture = Arc::new(gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("map.recovery"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }));
    gpu.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[18, 24, 30, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        size,
    );
    texture
}

impl Drop for Producer {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().unwrap();
        state.closed = true;
        self.shared.changed.notify_one();
    }
}

fn produce(shared: &Shared, handle: &SurfaceHandle, stats: &Stats, capture: &Capture) {
    let mut revision = None;
    let mut pipeline = None::<(u64, Renderer)>;
    let mut failed_generation = None;
    loop {
        let snapshot = {
            let mut state = shared.state.lock().unwrap();
            while !state.closed
                && (state.gpu.is_none()
                    || state
                        .gpu
                        .as_ref()
                        .is_some_and(|gpu| Some(gpu.generation()) == failed_generation)
                    || !state.visible
                    || state.size.contains(&0)
                    || revision == Some(state.revision))
            {
                state = shared.changed.wait(state).unwrap();
            }
            if state.closed {
                return;
            }
            state.clone()
        };
        revision = Some(snapshot.revision);
        let gpu = snapshot.gpu.as_ref().unwrap();
        if pipeline.as_ref().is_none_or(|(generation, renderer)| {
            *generation != gpu.generation()
                || matches!(renderer, Renderer::Terrain(_)) != snapshot.scene.is_some()
        }) {
            pipeline = Some((
                gpu.generation(),
                if snapshot.scene.is_some() {
                    Renderer::Terrain(Box::new(terrain_gpu::Renderer::new(
                        gpu.device(),
                        gpu.queue(),
                    )))
                } else {
                    Renderer::Checker(Pipeline::new(gpu.device()))
                },
            ));
            stats.rebuilds.fetch_add(1, Ordering::Release);
        }
        let renderer = &mut pipeline.as_mut().unwrap().1;
        let device = gpu.device();
        let limit = f64::from(device.limits().max_texture_dimension_2d);
        let shrink = (limit / f64::from(snapshot.size[0].max(snapshot.size[1])))
            .min((4_194_304. / (f64::from(snapshot.size[0]) * f64::from(snapshot.size[1]))).sqrt())
            .min(1.);
        let extent = snapshot
            .size
            .map(|v| (f64::from(v) * shrink).floor().max(1.) as u32);
        let texture = Arc::new(device.create_texture(&wgpu::TextureDescriptor {
            label: Some("map.preview"),
            size: wgpu::Extent3d {
                width: extent[0],
                height: extent[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        }));
        let css = snapshot
            .size
            .map(|v| f64::from(v) / f64::from(snapshot.dpi));
        let view = texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("map.preview"),
        });
        match renderer {
            Renderer::Terrain(renderer) => renderer.render(
                gpu,
                &mut encoder,
                terrain_gpu::Frame {
                    output: &view,
                    extent,
                    camera: snapshot.camera,
                    orbit: snapshot.orbit,
                    scene: snapshot.scene.as_ref().unwrap(),
                    css,
                },
            ),
            Renderer::Checker(renderer) => {
                let values = snapshot.camera.uniforms(css, snapshot.dpi * shrink as f32);
                let bytes = values
                    .into_iter()
                    .flat_map(f32::to_ne_bytes)
                    .collect::<Vec<_>>();
                gpu.queue().write_buffer(&renderer.uniforms, 0, &bytes);
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("map.preview"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&renderer.pipeline);
                pass.set_bind_group(0, &renderer.bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        if let Renderer::Terrain(renderer) = renderer {
            stats.uploads.store(renderer.uploads, Ordering::Release);
            stats
                .resident_bytes
                .store(renderer.resident_bytes as u64, Ordering::Release);
        }
        let submission = gpu.queue().submit([encoder.finish()]);
        // Bound in-flight work; waiting is confined to this producer. Each changed
        // frame has a fresh texture, so the UI can safely retain the previous one.
        if let Err(error) = device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(1)),
        }) {
            eprintln!("Map preview GPU submission failed: {error}");
            // Do not keep submitting to a stalled device. A DeviceLost event with
            // a replacement generation wakes us and rebuilds the pipeline.
            failed_generation = Some(gpu.generation());
            pipeline = None;
            continue;
        }
        let state = shared.state.lock().unwrap();
        // Present completed work while a newer camera revision is queued, so
        // continuous orbit/pan input cannot starve the display. A changed
        // device, viewport or visibility still invalidates the completed frame.
        if !state.closed
            && state.visible
            && state.size == snapshot.size
            && state.dpi == snapshot.dpi
            && state
                .gpu
                .as_ref()
                .is_some_and(|current| current.generation() == gpu.generation())
        {
            let providers = snapshot
                .scene
                .as_ref()
                .map(|scene| {
                    scene
                        .tiles
                        .iter()
                        .map(|tile| tile.provider)
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .collect()
                })
                .unwrap_or_default();
            *capture.frames.frame.lock().unwrap() = Some(CapturedFrame {
                gpu: gpu.clone(),
                texture: texture.clone(),
                camera: snapshot.camera,
                orbit: snapshot.orbit,
                css,
                providers,
                revision: snapshot.revision,
            });
            capture.frames.ready.notify_all();
            handle.present(texture);
            stats.scene_revision.store(
                snapshot.scene.as_ref().map_or(0, |s| s.revision),
                Ordering::Release,
            );
            stats.frames.fetch_add(1, Ordering::Release);
        }
    }
}

enum Renderer {
    Checker(Pipeline),
    Terrain(Box<terrain_gpu::Renderer>),
}

struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

impl Pipeline {
    fn new(device: &wgpu::Device) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.view"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("map.view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("map.view"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("map.preview"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("map.checker"),
            source: wgpu::ShaderSource::Wgsl(include_str!("checker.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map.preview"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            uniforms,
            bind,
        }
    }
}
