//! Retained map/height textures; orbiting uploads only view/tile uniforms.
use super::{
    camera::{Camera, WORLD},
    terrain::{MAX_RENDER_TILES, Orbit, Scene, ground_scale},
    tiles::{Key, Payload},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
};
use zgui::surface::{GpuShare, wgpu};

struct Image {
    version: u64,
    bytes: usize,
    used: u64,
    texture: wgpu::Texture,
}
pub struct Frame<'a> {
    pub output: &'a wgpu::TextureView,
    pub extent: [u32; 2],
    pub camera: Camera,
    pub orbit: Orbit,
    pub scene: &'a Scene,
    pub css: [f64; 2],
}
pub struct Renderer {
    pub uploads: u64,
    pub resident_bytes: usize,
    tick: u64,
    pipeline: wgpu::RenderPipeline,
    view: wgpu::Buffer,
    view_bind: wgpu::BindGroup,
    tiles: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    images: BTreeMap<Key, Image>,
    materials: BTreeMap<(Key, Key), wgpu::BindGroup>,
    stride: u64,
    fallback: wgpu::Texture,
    paths: super::flight3::Renderer,
    depth: Option<([u32; 2], wgpu::Texture)>,
}
impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let view = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.3d.view"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(64),
                },
                count: None,
            }],
        });
        let view_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &view_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view.as_entire_binding(),
            }],
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("map.3d.tile"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(64),
                    },
                    count: None,
                },
            ],
        });
        let stride = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(64);
        let tiles = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.3d.tiles"),
            size: MAX_RENDER_TILES as u64 * stride,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("map.terrain"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terrain.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&view_layout), Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map.terrain"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                // Reversed depth preserves distant precision with a nearby camera.
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
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
        let mesh = mesh_indices(48);
        let index_count = mesh.len() as u32;
        let bytes = mesh
            .iter()
            .flat_map(|index| index.to_ne_bytes())
            .collect::<Vec<_>>();
        let indices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map.terrain.indices"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&indices, 0, &bytes);
        let fallback = texture(
            device,
            queue,
            1,
            1,
            wgpu::TextureFormat::Rgba8Unorm,
            &[140, 154, 135, 255],
            4,
        );
        let paths = super::flight3::Renderer::new(device, &view_layout);
        Self {
            uploads: 0,
            resident_bytes: 0,
            tick: 0,
            pipeline,
            view,
            view_bind,
            tiles,
            indices,
            index_count,
            layout,
            sampler,
            images: BTreeMap::new(),
            materials: BTreeMap::new(),
            stride,
            fallback,
            paths,
            depth: None,
        }
    }
    fn remove_image(&mut self, key: Key) {
        if let Some(image) = self.images.remove(&key) {
            self.resident_bytes -= image.bytes;
        }
        self.materials
            .retain(|(raster, elevation), _| *raster != key && *elevation != key);
    }
    pub fn render(&mut self, gpu: &GpuShare, encoder: &mut wgpu::CommandEncoder, frame: Frame<'_>) {
        let Frame {
            output,
            extent,
            camera,
            orbit,
            scene,
            css,
        } = frame;
        let device = gpu.device();
        let queue = gpu.queue();
        self.tick += 1;
        let wanted = scene
            .tiles
            .iter()
            .take(MAX_RENDER_TILES)
            .flat_map(|tile| {
                [
                    Key::raster_from(tile.raster_id, tile.provider),
                    Key::elevation(tile.dem_id),
                ]
            })
            .collect::<BTreeSet<_>>();
        for tile in scene.tiles.iter().take(MAX_RENDER_TILES) {
            for (key, payload) in [
                (
                    Key::raster_from(tile.raster_id, tile.provider),
                    tile.raster.as_ref(),
                ),
                (Key::elevation(tile.dem_id), tile.elevation.as_ref()),
            ] {
                let Some(payload) = payload else {
                    continue;
                };
                if let Some(image) = self.images.get_mut(&key)
                    && image.version == payload.version()
                {
                    image.used = self.tick;
                    continue;
                }
                self.remove_image(key);
                let bytes = match payload.as_ref() {
                    Payload::Raster(r) => r.width as usize * r.height as usize * 4,
                    Payload::Elevation(e) => e.width as usize * e.height as usize * 4,
                };
                while self.resident_bytes + bytes > 384 * 1024 * 1024 {
                    let oldest = self
                        .images
                        .iter()
                        .filter(|(key, _)| !wanted.contains(key))
                        .min_by_key(|(_, image)| image.used)
                        .map(|(&key, _)| key);
                    let Some(oldest) = oldest else {
                        break;
                    };
                    self.remove_image(oldest);
                }
                let tex = match payload.as_ref() {
                    Payload::Raster(r) => texture(
                        device,
                        queue,
                        r.width,
                        r.height,
                        wgpu::TextureFormat::Rgba8Unorm,
                        &r.rgba,
                        4,
                    ),
                    Payload::Elevation(e) => texture(
                        device,
                        queue,
                        e.width,
                        e.height,
                        wgpu::TextureFormat::R32Float,
                        &floats(&e.heights),
                        4,
                    ),
                };
                self.images.insert(
                    key,
                    Image {
                        version: payload.version(),
                        bytes,
                        used: self.tick,
                        texture: tex,
                    },
                );
                self.uploads += 1;
                self.resident_bytes += bytes;
            }
        }
        queue.write_buffer(&self.view, 0, &floats(&orbit.matrix(camera, css)));
        if self.depth.as_ref().is_none_or(|(size, _)| *size != extent) {
            let depth = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("map.depth"),
                size: wgpu::Extent3d {
                    width: extent[0],
                    height: extent[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth = Some((extent, depth));
        }
        let mut binds = Vec::new();
        let scale = ground_scale(camera);
        let base = scene.center_height(camera);
        for tile in scene.tiles.iter().take(MAX_RENDER_TILES) {
            let dem = Key::elevation(tile.dem_id);
            let Some(elevation) = self.images.get(&dem) else {
                continue;
            };
            let imagery = self
                .images
                .get(&Key::raster_from(tile.raster_id, tile.provider))
                .map_or(&self.fallback, |i| &i.texture);
            let span = WORLD / f64::from(1_u32 << tile.id.z);
            let ratio = f64::from(1_u32 << (tile.id.z - dem.z));
            let raster_ratio = (1_u32 << (tile.id.z - tile.raster_id.z)) as f64;
            let values = [
                ((-WORLD / 2. + tile.id.x as f64 * span - camera.center[0]) * scale) as f32,
                ((camera.center[1] - WORLD / 2. + f64::from(tile.id.y) * span) * scale) as f32,
                (span * scale) as f32,
                base,
                (f64::from(tile.id.canonical_x()).rem_euclid(ratio) / ratio) as f32,
                (f64::from(tile.id.y).rem_euclid(ratio) / ratio) as f32,
                (1. / ratio) as f32,
                (WORLD / f64::from(1_u32 << dem.z) * scale / 512.) as f32,
                orbit.exaggeration,
                48.,
                (span * scale / 100.).max(10.) as f32,
                0.,
                (f64::from(tile.id.canonical_x()).rem_euclid(raster_ratio) / raster_ratio) as f32,
                (f64::from(tile.id.y).rem_euclid(raster_ratio) / raster_ratio) as f32,
                (1. / raster_ratio) as f32,
                0.,
            ];
            let offset = binds.len() as u64 * self.stride;
            queue.write_buffer(&self.tiles, offset, &floats(&values));
            let bind = self
                .materials
                .entry((Key::raster_from(tile.raster_id, tile.provider), dem))
                .or_insert_with(|| {
                    let raster_view = imagery.create_view(&Default::default());
                    let height_view = elevation.texture.create_view(&Default::default());
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("map.3d.material"),
                        layout: &self.layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&raster_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(&height_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &self.tiles,
                                    offset: 0,
                                    size: NonZeroU64::new(64),
                                }),
                            },
                        ],
                    })
                });
            binds.push(bind.clone());
        }
        let depth = self
            .depth
            .as_ref()
            .unwrap()
            .1
            .create_view(&Default::default());
        self.paths
            .prepare(gpu, scene, camera, orbit.exaggeration, css);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("map.terrain"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.16,
                        g: 0.22,
                        b: 0.29,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_bind_group(0, &self.view_bind, &[]);
        for (index, bind) in binds.iter().enumerate() {
            pass.set_bind_group(1, bind, &[(index as u64 * self.stride) as u32]);
            pass.draw_indexed(0..self.index_count, 0, 0..1);
        }
        self.paths.draw(&mut pass);
    }
}
fn mesh_indices(grid: u32) -> Vec<u32> {
    let mut indices = Vec::new();
    for y in 0..grid {
        for x in 0..grid {
            let a = y * (grid + 1) + x;
            let b = a + 1;
            let c = a + grid + 1;
            indices.extend([a, b, c, c, b, c + 1]);
        }
    }
    let surface = (grid + 1) * (grid + 1);
    for edge in 0..4 {
        let start = surface + edge * (grid + 1) * 2;
        for cell in 0..grid {
            let a = start + cell * 2;
            indices.extend([a, a + 2, a + 1, a + 1, a + 2, a + 3]);
        }
    }
    indices
}
fn floats(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|f| f.to_ne_bytes()).collect()
}
fn texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    bytes: &[u8],
    stride: u32,
) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("map.tile"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * stride),
            rows_per_image: Some(height),
        },
        size,
    );
    texture
}
