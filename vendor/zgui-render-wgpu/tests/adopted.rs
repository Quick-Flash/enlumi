//! Drawing on a device that a host opened.
//!
//! A host, for example a game engine, opens the one device of the process with its own features
//! and limits and sets its own device-lost callback. What is asserted here is that the adopted
//! device keeps all of that, that a frame lands in a texture the host created, and that these
//! graphics never open a device of their own.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use zgui_bits::DamageSet;
use zgui_color::Color;
use zgui_geom::{Device, Scale, Size};
use zgui_render::{RenderTarget, Renderer};
use zgui_render_wgpu::renderer::readback;
use zgui_render_wgpu::{Gpu, SharedGraphics, wgpu};
use zgui_scene::{Quad, Scene};

use support::{SIDE, device_lock, rect};

/// The format of the texture the host supplies: unencoded, red first.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A device as a host opens it, and whether the host's own callback saw it die.
struct Host {
    /// The instance.
    instance: wgpu::Instance,
    /// The adapter.
    adapter: wgpu::Adapter,
    /// The device, with the host's callback on it.
    device: wgpu::Device,
    /// The queue.
    queue: wgpu::Queue,
    /// Set by the host's device-lost callback.
    lost: Arc<AtomicBool>,
    /// A feature the host asked for and this crate never asks for, if the adapter has one.
    extra: wgpu::Features,
}

/// Opens a device the way a host does, or `None` when this machine has none.
fn open_host() -> Option<Host> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: zgui_render_wgpu::gpu::adapter::requested_backends(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = match futures::executor::block_on(
        instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
    ) {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!("skipped: no usable graphics device ({error})");
            return None;
        }
    };
    // A feature a host wants and this crate does not: it has to survive the adoption.
    let extra = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("test.host"),
            required_features: extra | (adapter.features() & wgpu::Features::DUAL_SOURCE_BLENDING),
            ..Default::default()
        }))
        .ok()?;
    let lost = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&lost);
    device.set_device_lost_callback(move |_, _| seen.store(true, Ordering::SeqCst));
    Some(Host {
        instance,
        adapter,
        device,
        queue,
        lost,
        extra,
    })
}

/// Adopts the host's device.
fn adopt(host: &Host) -> Arc<Gpu> {
    Gpu::from_existing(
        host.instance.clone(),
        host.adapter.clone(),
        host.device.clone(),
        host.queue.clone(),
    )
}

/// A translucent target the size of every other test's.
fn target() -> RenderTarget {
    RenderTarget::new(Size::new(SIDE, SIDE), Scale::new(1.0)).translucent()
}

/// A texture the host creates for the interface to present into.
fn texture(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test.host.ui"),
        size: wgpu::Extent3d {
            width: SIDE as u32,
            height: SIDE as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// A half-transparent red quad over the left half of the target, and nothing on the right.
fn half_red_left() -> Scene {
    let mut scene = Scene::new();
    scene.begin_frame(Size::new(SIDE, SIDE));
    let red = scene
        .paints
        .add(zgui_scene::Paint::Solid(Color::srgb_u8(255, 0, 0, 128)));
    scene.push_quad(Quad::filled(
        rect(0.0, 0.0, (SIDE / 2) as f32, SIDE as f32),
        red,
    ));
    scene.finish(&DamageSet::full());
    scene
}

#[test]
fn an_adopted_device_keeps_the_hosts_features_and_reports_its_own_capabilities() {
    let _device = device_lock();
    let Some(host) = open_host() else { return };
    let gpu = adopt(&host);
    assert!(gpu.is_adopted());
    assert!(
        gpu.device().features().contains(host.extra),
        "the host's features are the device's features"
    );
    assert_eq!(
        gpu.capabilities().subpixel_text,
        host.device
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING),
        "capabilities are read off the adopted device"
    );
    assert!(gpu.capabilities().max_texture_size > 0);
    assert!(gpu.vulkan_extensions().is_empty());
}

#[test]
fn a_frame_lands_in_the_hosts_texture_premultiplied() {
    let _device = device_lock();
    let Some(host) = open_host() else { return };
    let graphics = SharedGraphics::with_gpu(adopt(&host));
    assert!(graphics.is_adopted());
    let gpu = graphics
        .open_gpu()
        .expect("the adopted device is the primary");
    assert!(
        gpu.device() == &host.device,
        "open_gpu answers the host's device"
    );

    let ui = texture(&host.device);
    let mut renderer = graphics
        .renderer_supplied(target(), vec![ui.clone()])
        .expect("a host texture on the adopted device is accepted");
    assert!(renderer.present_into(0));
    let outcome = renderer.draw(&half_red_left(), &DamageSet::full());
    assert!(outcome.stats().is_some(), "{outcome:?}");

    let pixels = readback::read(
        renderer.gpu(),
        &ui,
        FORMAT,
        Size::<i32, Device>::new(SIDE, SIDE),
    );
    let inside = pixels.rgba(SIDE / 4, SIDE / 2);
    let outside = pixels.rgba(SIDE * 3 / 4, SIDE / 2);
    let near = |a: u8, b: u8| a.abs_diff(b) <= 1;
    assert!(
        near(inside[0], 128) && inside[1] == 0 && inside[2] == 0 && near(inside[3], 128),
        "half-transparent red is premultiplied in the host's texture: {inside:?}"
    );
    assert_eq!(
        outside,
        [0, 0, 0, 0],
        "undrawn pixels stay fully transparent"
    );
}

#[test]
fn the_hosts_device_lost_callback_is_not_replaced() {
    let _device = device_lock();
    let Some(host) = open_host() else { return };
    let gpu = adopt(&host);
    host.device.destroy();
    let _ = host.device.poll(wgpu::PollType::wait_indefinitely());
    assert!(
        host.lost.load(Ordering::SeqCst),
        "the host's callback still runs when the device dies"
    );
    assert!(
        !gpu.loss().is_lost(),
        "nothing reports the loss here until the host does"
    );
    (gpu.loss().observer())(wgpu::DeviceLostReason::Destroyed, "reported by the host");
    assert!(gpu.loss().is_lost());
}

#[test]
fn adopted_graphics_never_open_a_device_of_their_own() {
    let _device = device_lock();
    let Some(host) = open_host() else { return };
    let gpu = adopt(&host);
    let graphics = SharedGraphics::with_gpu(Arc::clone(&gpu));
    gpu.loss()
        .report(wgpu::DeviceLostReason::Unknown, "reported by the host");

    let refused = graphics.open_gpu();
    assert!(
        refused.is_err(),
        "a lost adopted device is not replaced by a device opened here"
    );
    assert!(
        graphics
            .renderer_offscreen(target(), FORMAT, false)
            .is_err()
    );

    // The host recovers and gives the new device. A second host device stands in for it.
    let Some(recovered) = open_host() else {
        return;
    };
    let replacement = adopt(&recovered);
    graphics.adopt(Arc::clone(&replacement));
    let reopened = graphics
        .open_gpu()
        .expect("the adopted replacement is usable");
    assert!(Arc::ptr_eq(&reopened, &replacement));
}

#[test]
fn adopting_the_primary_again_keeps_it() {
    let _device = device_lock();
    let Some(host) = open_host() else { return };
    let gpu = adopt(&host);
    let graphics = SharedGraphics::with_gpu(Arc::clone(&gpu));
    graphics.adopt(Arc::clone(&gpu));
    let current = graphics.gpu().expect("the primary is set at construction");
    assert!(Arc::ptr_eq(&current, &gpu));
}
