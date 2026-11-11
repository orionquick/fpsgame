//! GPU pass timing with timestamp queries (FPS_GPU_TIMING=1; needs the adapter's TIMESTAMP_QUERY feature).
use crate::Gpu;
use std::cell::RefCell;

const N: u32 = 64;

pub struct Profiler {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    labels: RefCell<Vec<&'static str>>,
    period: f32,
}

impl Profiler {
    pub fn new(gpu: &Gpu) -> Option<Self> {
        if std::env::var("FPS_GPU_TIMING").is_err() || !gpu.device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let d = &gpu.device;
        Some(Self {
            set: d.create_query_set(&wgpu::QuerySetDescriptor { label: Some("timing"), ty: wgpu::QueryType::Timestamp, count: N }),
            resolve: d.create_buffer(&wgpu::BufferDescriptor { label: Some("timing resolve"), size: N as u64 * 8, usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false }),
            read: d.create_buffer(&wgpu::BufferDescriptor { label: Some("timing read"), size: N as u64 * 8, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false }),
            labels: RefCell::new(vec![]),
            period: gpu.queue.get_timestamp_period(),
        })
    }
    /// Starts a frame's timings.
    pub fn reset(&self) {
        self.labels.borrow_mut().clear();
    }
    fn slot(&self, label: &'static str) -> Option<u32> {
        let mut l = self.labels.borrow_mut();
        let i = l.len() as u32;
        if i * 2 + 1 >= N {
            return None;
        }
        l.push(label);
        Some(i * 2)
    }
    pub fn render(&self, label: &'static str) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.slot(label).map(|i| wgpu::RenderPassTimestampWrites { query_set: &self.set, beginning_of_pass_write_index: Some(i), end_of_pass_write_index: Some(i + 1) })
    }
    pub fn compute(&self, label: &'static str) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        self.slot(label).map(|i| wgpu::ComputePassTimestampWrites { query_set: &self.set, beginning_of_pass_write_index: Some(i), end_of_pass_write_index: Some(i + 1) })
    }
    /// Records the resolve at the end of a frame's encoder.
    pub fn resolve(&self, enc: &mut wgpu::CommandEncoder) {
        let n = self.labels.borrow().len() as u32 * 2;
        if n > 0 {
            enc.resolve_query_set(&self.set, 0..n, &self.resolve, 0);
            enc.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, n as u64 * 8);
        }
    }
    /// Reads the last resolved frame (after submit): per pass label, milliseconds (summed by label). Clears.
    pub fn read(&self, gpu: &Gpu) -> Vec<(&'static str, f32)> {
        let labels = std::mem::take(&mut *self.labels.borrow_mut());
        if labels.is_empty() {
            return vec![];
        }
        self.read.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).expect("poll");
        let mut out: Vec<(&'static str, f32)> = vec![];
        {
            let data = self.read.slice(..).get_mapped_range().expect("range");
            let t: &[u64] = bytemuck::cast_slice(&data);
            for (i, l) in labels.iter().enumerate() {
                let ms = t[i * 2 + 1].saturating_sub(t[i * 2]) as f32 * self.period / 1e6;
                match out.iter_mut().find(|(k, _)| k == l) {
                    Some(e) => e.1 += ms,
                    None => out.push((l, ms)),
                }
            }
        }
        self.read.unmap();
        out
    }
}
