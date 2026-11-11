//! Reuse GPU allocations and packing storage for retained 3D trace layers.
use zgui::surface::{GpuShare, wgpu};
#[derive(Default)]
pub struct Buffer {
    pub uploads: u64,
    pub uploaded_bytes: u64,
    buffer: Option<wgpu::Buffer>,
    bytes: Vec<u8>,
    count: u32,
}
impl Buffer {
    pub fn upload(&mut self, gpu: &GpuShare, vertices: &[[f32; 11]]) {
        self.count = (vertices.len() / 2) as u32;
        self.bytes.clear();
        self.bytes.reserve(self.count as usize * 72);
        for pair in vertices.chunks_exact(2) {
            for value in pair[0][..3]
                .iter()
                .chain(&pair[1][..3])
                .chain(&pair[0][3..8])
                .chain(&pair[0][8..11])
                .chain(&pair[1][3..7])
            {
                self.bytes.extend_from_slice(&value.to_ne_bytes());
            }
        }
        if self.bytes.is_empty() {
            return;
        }
        let size = self.bytes.len() as u64;
        if self
            .buffer
            .as_ref()
            .is_none_or(|buffer| buffer.size() < size)
        {
            self.buffer = Some(gpu.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("map.trace3.vertices"),
                size: size.next_power_of_two().max(256),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        gpu.queue()
            .write_buffer(self.buffer.as_ref().unwrap(), 0, &self.bytes);
        self.uploads += 1;
        self.uploaded_bytes += size;
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count > 0
            && let Some(buffer) = &self.buffer
        {
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..6, 0..self.count);
        }
    }
}
