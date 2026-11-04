use std::time::Duration;
use zgui::render::*;
// The pinned backend exposes mutable downcasting but not shared downcasting.
// Forward every renderer operation and add the read-only seam for image validation.
pub(super) struct ReadbackRenderer(pub zgui_render_wgpu::WgpuRenderer);
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
