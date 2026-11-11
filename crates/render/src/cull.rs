//! GPU-driven culling and level of detail. Every instance of every batch goes into one storage buffer; each frame a
//! compute pass tests each instance against a view (frustum, projected size, last frame's depth pyramid) and appends
//! visible ids to per-(batch, level) lists that feed indirect draws. The vertex shader fetches its instance by id.
use crate::textured::Batch;
use crate::Gpu;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BatchInfo {
    lmin: [f32; 3],
    n_lods: u32,
    lmax: [f32; 3],
    draw0: u32,
    flags: u32,
    pad: [u32; 3],
    /// Error per level in model units.
    err: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ViewU {
    pub planes: [[f32; 4]; 6],
    pub prev_vp: [[f32; 4]; 4],
    pub eye: [f32; 4],
    pub lod: [f32; 4],
    pub hiz: [f32; 4],
    pub misc: [u32; 4],
    pub lod2: [f32; 4],
}

/// One culled view's outputs: its uniform, the indirect args (5 u32 per draw) and the visible-id lists.
pub(crate) struct ViewBufs {
    pub uniform: wgpu::Buffer,
    pub args: wgpu::Buffer,
    pub ids: wgpu::Buffer,
}

/// The instance registry for a fixed set of batches (build once after loading, pass with the same batches).
pub struct CullWorld {
    pub(crate) insts: wgpu::Buffer,
    inst_batch: wgpu::Buffer,
    batch_info: wgpu::Buffer,
    draw_base: wgpu::Buffer,
    pub(crate) n_inst: u32,
    pub(crate) n_batches: usize,
    /// Per batch: first draw index and level count. Draw d's ids start at `bases[d]`.
    pub(crate) first_draw: Vec<(u32, u32)>,
    pub(crate) bases: Vec<u32>,
    args_template: Vec<u32>,
    /// Main, near cascade, far cascade.
    pub(crate) views: [ViewBufs; 3],
    pub(crate) inst_bind: wgpu::BindGroup,
    /// Bindless: the shared vertex/index buffers and the material table; draws [0, n_opaque_draws) are opaque.
    mega: Option<(wgpu::Buffer, wgpu::Buffer)>,
    mat_bind: Option<wgpu::BindGroup>,
    n_opaque_draws: u32,
    n_draws: u32,
    /// Identifies this world for the cached far shadow map.
    pub(crate) id: u64,
}

pub(crate) const MAIN: usize = 0;
pub(crate) const NEAR: usize = 1;
pub(crate) const FAR: usize = 2;

impl CullWorld {
    pub fn new(gpu: &Gpu, r: &crate::Renderer, batches: &[&Batch]) -> Self {
        let d = &gpu.device;
        let n_inst: u32 = batches.iter().map(|b| b.n_inst).sum();
        let mut first_draw = vec![];
        let mut bases = vec![];
        let mut info = vec![];
        let mut inst_batch = Vec::with_capacity(n_inst as usize);
        let mut args = vec![];
        let mut next = 0u32;
        let bindless = r.bindless;
        // bindless: every batch's vertices and indices in one buffer pair, every material in one table
        let (mut vb_off, mut ib_off) = (vec![0u32; batches.len()], vec![0u32; batches.len()]);
        let (mut nv, mut ni) = (0u64, 0u64);
        let mut mats: Vec<std::sync::Arc<crate::Material>> = vec![];
        let mut mat_idx = vec![0u32; batches.len()];
        for (bi, b) in batches.iter().enumerate() {
            vb_off[bi] = nv as u32;
            ib_off[bi] = ni as u32;
            nv += b.n_verts as u64;
            ni += b.ib.size() / 4;
            let k = match mats.iter().position(|m| std::sync::Arc::ptr_eq(m, &b.material)) {
                Some(k) => k,
                None => {
                    mats.push(b.material.clone());
                    mats.len() - 1
                }
            };
            mat_idx[bi] = (k as u32).min(crate::MAX_MATERIALS - 1);
        }
        if mats.len() > crate::MAX_MATERIALS as usize {
            eprintln!("cull: {} materials, {} slots", mats.len(), crate::MAX_MATERIALS);
        }
        // draw slots: opaque batches first, then glazing, so each is one contiguous run of indirect draws
        first_draw = vec![(0u32, 0u32); batches.len()];
        let order: Vec<usize> = (0..batches.len()).filter(|&i| !batches[i].transparent).chain((0..batches.len()).filter(|&i| batches[i].transparent)).collect();
        let mut n_opaque_draws = 0u32;
        let mut draw0_of = vec![0u32; batches.len()];
        for &bi in &order {
            let b = batches[bi];
            let draw0 = bases.len() as u32;
            draw0_of[bi] = draw0;
            first_draw[bi] = (draw0, b.lods.len() as u32);
            for l in &b.lods {
                bases.push(next);
                // index_count, instance_count, first_index, base_vertex, first_instance
                if bindless {
                    args.extend([l[1], 0, l[0] + ib_off[bi], vb_off[bi], next]);
                } else {
                    args.extend([l[1], 0, l[0], 0, 0]);
                }
                next += b.n_inst;
            }
            if !b.transparent {
                n_opaque_draws = bases.len() as u32;
            }
        }
        let mut inst_mat = Vec::with_capacity(n_inst as usize);
        for (bi, b) in batches.iter().enumerate() {
            let mut err = [0.0f32; 4];
            for (k, e) in b.lod_err.iter().take(4).enumerate() {
                err[k] = *e;
            }
            info.push(BatchInfo { lmin: b.local[0].into(), n_lods: b.lods.len().min(4) as u32, lmax: b.local[1].into(), draw0: draw0_of[bi], flags: b.transparent as u32, pad: [0; 3], err });
            inst_batch.extend(std::iter::repeat_n(bi as u32, b.n_inst as usize));
            inst_mat.extend(std::iter::repeat_n(mat_idx[bi], b.n_inst as usize));
        }
        let n_draws = bases.len() as u32;
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let insts = d.create_buffer(&wgpu::BufferDescriptor { label: Some("cull insts"), size: (n_inst.max(1) as u64) * std::mem::size_of::<crate::Instance>() as u64, usage: storage, mapped_at_creation: false });
        let mut enc = d.create_command_encoder(&Default::default());
        let mut off = 0u64;
        let stride = std::mem::size_of::<crate::Instance>() as u64;
        for b in batches {
            enc.copy_buffer_to_buffer(&b.inst, 0, &insts, off, b.n_inst as u64 * stride);
            off += b.n_inst as u64 * stride;
        }
        let mega = bindless.then(|| {
            let vsz = std::mem::size_of::<crate::TexVertex>() as u64;
            let vbuf = d.create_buffer(&wgpu::BufferDescriptor { label: Some("mega vertices"), size: (nv * vsz).max(4), usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let ibuf = d.create_buffer(&wgpu::BufferDescriptor { label: Some("mega indices"), size: (ni * 4).max(4), usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            for (bi, b) in batches.iter().enumerate() {
                enc.copy_buffer_to_buffer(&b.vb, 0, &vbuf, vb_off[bi] as u64 * vsz, b.n_verts as u64 * vsz);
                enc.copy_buffer_to_buffer(&b.ib, 0, &ibuf, ib_off[bi] as u64 * 4, b.ib.size());
            }
            (vbuf, ibuf)
        });
        gpu.queue.submit([enc.finish()]);
        let mat_bind = bindless.then(|| {
            let mats = &mats[..mats.len().min(crate::MAX_MATERIALS as usize)];
            let v = |k: usize| mats.iter().map(|m| &m.views[k]).collect::<Vec<_>>();
            let (b0, b1, b2) = (v(0), v(1), v(2));
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("materials"),
                layout: &r.mat_arr_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureViewArray(&b0) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureViewArray(&b1) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureViewArray(&b2) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&r.mat_sampler) },
                ],
            })
        });
        let init = |label, c: &[u8], usage| d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: c, usage });
        let pad = |v: &[u32]| if v.is_empty() { vec![0u32] } else { v.to_vec() };
        let inst_batch = init("cull inst batch", bytemuck::cast_slice(&pad(&inst_batch)), storage);
        let batch_info = init("cull batches", bytemuck::cast_slice(&if info.is_empty() { vec![BatchInfo::zeroed()] } else { info }), storage);
        let draw_base = init("cull draw base", bytemuck::cast_slice(&pad(&bases)), storage);
        let view = |label| ViewBufs {
            uniform: d.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: std::mem::size_of::<ViewU>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
            args: init(label, bytemuck::cast_slice(&pad(&args)), wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC),
            ids: d.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: next.max(1) as u64 * 4, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX, mapped_at_creation: false }),
        };
        let inst_mat = init("cull inst material", bytemuck::cast_slice(&pad(&inst_mat)), storage);
        let inst_bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("instances"),
            layout: &r.inst_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: insts.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: inst_mat.as_entire_binding() }],
        });
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            insts,
            inst_batch,
            batch_info,
            draw_base,
            n_inst,
            n_batches: batches.len(),
            first_draw,
            bases,
            args_template: pad(&args),
            views: [view("cull main"), view("cull near"), view("cull far")],
            inst_bind,
            mega,
            mat_bind,
            n_opaque_draws,
            n_draws,
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Debug readback of the last frame's visibility: per view (main, near, far) the visible instances and
    /// triangles, and the heaviest draws as (batch, level, instances, triangles).
    pub fn stats(&self, gpu: &Gpu) -> Vec<(u64, u64, Vec<(usize, u32, u32, u64)>)> {
        let n = self.args_template.len() as u64 * 4;
        let mut out = vec![];
        for v in &self.views {
            let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("stats"), size: n, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            enc.copy_buffer_to_buffer(&v.args, 0, &buf, 0, n);
            gpu.queue.submit([enc.finish()]);
            buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
            gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).expect("poll");
            let data = buf.slice(..).get_mapped_range().expect("range");
            let a: &[u32] = bytemuck::cast_slice(&data);
            let (mut inst, mut tris) = (0u64, 0u64);
            let mut top = vec![];
            for (bi, &(d0, nl)) in self.first_draw.iter().enumerate() {
                for l in 0..nl {
                    let d = (d0 + l) as usize;
                    let (ic, n) = (a[d * 5], a[d * 5 + 1]);
                    let t = ic as u64 / 3 * n as u64;
                    inst += n as u64;
                    tris += t;
                    top.push((bi, l, n, t));
                }
            }
            top.sort_by(|a, b| b.3.cmp(&a.3));
            top.truncate(8);
            out.push((inst, tris, top));
        }
        out
    }

    /// Resets a view's counts and records its cull dispatch.
    pub(crate) fn cull(&self, gpu: &Gpu, r: &crate::Renderer, enc: &mut wgpu::CommandEncoder, v: usize, u: &ViewU, hiz: &wgpu::TextureView) {
        let vb = &self.views[v];
        gpu.queue.write_buffer(&vb.args, 0, bytemuck::cast_slice(&self.args_template));
        let groups = self.n_inst.div_ceil(64);
        let (gx, gy) = (groups.min(65535), groups.div_ceil(65535).max(1));
        let mut u = *u;
        u.misc[1] = self.n_inst;
        u.misc[2] = gx * 64;
        gpu.queue.write_buffer(&vb.uniform, 0, bytemuck::bytes_of(&u));
        if self.n_inst == 0 {
            return;
        }
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cull"),
            layout: &r.cull_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.insts.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.inst_batch.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.batch_info.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.draw_base.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: vb.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: vb.args.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: vb.ids.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(hiz) },
            ],
        });
        let mut p = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("cull"), timestamp_writes: r.prof.as_ref().and_then(|p| p.compute("cull")) });
        p.set_pipeline(&r.cull_pipeline);
        p.set_bind_group(0, &bind, &[]);
        p.dispatch_workgroups(gx, gy, 1);
    }

    /// Records the indirect draws of the batches whose world bounds meet `planes` (opaque or glazing only).
    pub(crate) fn draw<'a>(&'a self, p: &mut wgpu::RenderPass<'a>, batches: &[&'a Batch], v: usize, planes: &[[f32; 4]; 6], glass: bool) {
        let vb = &self.views[v];
        if let (Some((vbuf, ibuf)), Some(mb)) = (&self.mega, &self.mat_bind) {
            // everything in one run: no state changes between draws (first_instance offsets the id lists)
            p.set_bind_group(1, mb, &[]);
            p.set_vertex_buffer(0, vbuf.slice(..));
            p.set_vertex_buffer(1, vb.ids.slice(..));
            p.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
            let (a, b) = if glass { (self.n_opaque_draws, self.n_draws) } else { (0, self.n_opaque_draws) };
            if b > a {
                p.multi_draw_indexed_indirect(&vb.args, a as u64 * 20, b - a);
            }
            let _ = (batches, planes);
            return;
        }
        for (bi, b) in batches.iter().enumerate() {
            if b.transparent != glass || !box_visible(planes, &b.world) {
                continue;
            }
            p.set_bind_group(1, &b.material.bind, &[]);
            p.set_vertex_buffer(0, b.vb.slice(..));
            p.set_index_buffer(b.ib.slice(..), wgpu::IndexFormat::Uint32);
            let (d0, n) = self.first_draw[bi];
            for d in d0..d0 + n {
                let base = self.bases[d as usize] as u64 * 4;
                p.set_vertex_buffer(1, vb.ids.slice(base..));
                p.draw_indexed_indirect(&vb.args, d as u64 * 20);
            }
        }
    }
}

use bytemuck::Zeroable;

/// Frustum planes (inward, normalised) from a view-projection with clip z in 0..1.
pub(crate) fn planes(vp: &Mat4) -> [[f32; 4]; 6] {
    let r = |i| vp.row(i);
    let ps = [r(3) + r(0), r(3) - r(0), r(3) + r(1), r(3) - r(1), r(2), r(3) - r(2)];
    ps.map(|p| {
        let l = p.truncate().length().max(1e-12);
        (p / l).to_array()
    })
}

pub(crate) fn box_visible(planes: &[[f32; 4]; 6], b: &[Vec3; 2]) -> bool {
    let c = (b[0] + b[1]) * 0.5;
    let e = (b[1] - b[0]) * 0.5;
    planes.iter().all(|p| {
        let n = Vec3::new(p[0], p[1], p[2]);
        n.dot(c) + p[3] + n.abs().dot(e) >= 0.0
    })
}

/// The depth pyramid of a target size: max depth per texel, rebuilt from the main pass each frame.
pub(crate) struct Hiz {
    pub full: wgpu::TextureView,
    init: wgpu::BindGroup,
    downs: Vec<(wgpu::BindGroup, [u32; 2])>,
    pub mips: u32,
    pub size: [u32; 2],
}

impl Hiz {
    pub fn new(gpu: &Gpu, r: &crate::Renderer, depth: &wgpu::TextureView, w: u32, h: u32) -> Self {
        let d = &gpu.device;
        let mips = 32 - w.max(h).leading_zeros();
        let tex = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("hiz"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let mip = |m: u32| tex.create_view(&wgpu::TextureViewDescriptor { base_mip_level: m, mip_level_count: Some(1), ..Default::default() });
        let init = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hiz init"),
            layout: &r.hiz_init_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&mip(0)) }],
        });
        let downs = (1..mips)
            .map(|m| {
                let g = d.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("hiz down"),
                    layout: &r.hiz_down_bgl,
                    entries: &[wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&mip(m - 1)) }, wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&mip(m)) }],
                });
                (g, [(w >> m).max(1), (h >> m).max(1)])
            })
            .collect();
        Self { full: tex.create_view(&Default::default()), init, downs, mips, size: [w, h] }
    }

    pub fn build(&self, r: &crate::Renderer, enc: &mut wgpu::CommandEncoder) {
        let mut p = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("hiz"), timestamp_writes: r.prof.as_ref().and_then(|p| p.compute("hiz")) });
        p.set_pipeline(&r.hiz_init_pipeline);
        p.set_bind_group(0, &self.init, &[]);
        p.dispatch_workgroups(self.size[0].div_ceil(8), self.size[1].div_ceil(8), 1);
        p.set_pipeline(&r.hiz_down_pipeline);
        for (g, s) in &self.downs {
            p.set_bind_group(0, g, &[]);
            p.dispatch_workgroups(s[0].div_ceil(8), s[1].div_ceil(8), 1);
        }
    }
}
