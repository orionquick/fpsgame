//! wgpu renderer: static scene + instanced textured batches, two-cascade sun shadows, sky, haze, tonemapping.
//! Draws to a window surface or off-screen.
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

pub use wgpu as wgpu_reexport;

pub mod textured;
pub use textured::{Batch, Instance, Material, TexVertex};

pub const MSAA: u32 = 4;
pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_FAR_RES: u32 = 4096;
const SHADOW_NEAR_RES: u32 = 4096;
/// Half-size of the near shadow cascade around the camera, metres.
const NEAR_HALF: f32 = 70.0;

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn new(instance: wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Self {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("no GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("device"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .expect("no GPU device");
        Self { instance, adapter, device, queue }
    }
    pub fn headless() -> Self {
        Self::new(wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()), None)
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub col: [f32; 3],
}

pub struct Scene {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    n_idx: u32,
    pub min: Vec3,
    pub max: Vec3,
}

impl Scene {
    pub fn new(gpu: &Gpu, verts: &[Vertex], idx: &[u32]) -> Self {
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in verts {
            min = min.min(Vec3::from(v.pos));
            max = max.max(Vec3::from(v.pos));
        }
        let verts = if verts.is_empty() { &[Vertex { pos: [0.0; 3], nrm: [0.0, 1.0, 0.0], col: [0.0; 3] }; 3][..] } else { verts };
        let idx = if idx.is_empty() { &[0u32, 0, 0][..] } else { idx };
        let vbuf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("verts"), contents: bytemuck::cast_slice(verts), usage: wgpu::BufferUsages::VERTEX });
        let ibuf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("idx"), contents: bytemuck::cast_slice(idx), usage: wgpu::BufferUsages::INDEX });
        Self { vbuf, ibuf, n_idx: idx.len() as u32, min, max }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_far: [[f32; 4]; 4],
    light_near: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    sun_dir: [f32; 4],
    sun_col: [f32; 4],
    sky_zenith: [f32; 4],
    sky_horizon: [f32; 4],
    ground_col: [f32; 4],
    eye: [f32; 4],
    misc: [f32; 4],
}

pub struct Camera {
    pub view_proj: Mat4,
    pub eye: Vec3,
    /// Where the near shadow cascade is centred (in front of the camera).
    pub focus: Vec3,
    /// Haze density per metre; 0 for top-down review renders.
    pub fog: f32,
}

impl Camera {
    /// Top-down orthographic view of an x/z rectangle (north up, east right).
    pub fn ortho(rect: [f32; 4], top: f32) -> Self {
        let (cx, cz) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
        let (hw, hh) = ((rect[2] - rect[0]) * 0.5, (rect[3] - rect[1]) * 0.5);
        let eye = Vec3::new(cx, top + 50.0, cz);
        let view = Mat4::look_at_rh(eye, Vec3::new(cx, 0.0, cz), Vec3::NEG_Z);
        Self { view_proj: Mat4::orthographic_rh(-hw, hw, -hh, hh, 1.0, top + 400.0) * view, eye, focus: Vec3::new(cx, 0.0, cz), fog: 0.0 }
    }
    /// First-person view. Yaw 0 = north (-z), 90 = east (+x); pitch up positive. Degrees.
    pub fn perspective(eye: Vec3, yaw_deg: f32, pitch_deg: f32, fov_y_deg: f32, aspect: f32) -> Self {
        let dir = look_dir(yaw_deg, pitch_deg);
        let view = Mat4::look_at_rh(eye, eye + dir, Vec3::Y);
        let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        Self { view_proj: Mat4::perspective_rh(fov_y_deg.to_radians(), aspect, 0.1, 5000.0) * view, eye, focus: eye + flat * (NEAR_HALF * 0.6), fog: 0.0009 }
    }
}

pub fn look_dir(yaw_deg: f32, pitch_deg: f32) -> Vec3 {
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    Vec3::new(y.sin() * p.cos(), p.sin(), -y.cos() * p.cos())
}

/// Sun from azimuth (degrees clockwise from north) and elevation (degrees above the horizon).
pub fn sun_from(azimuth_deg: f32, elevation_deg: f32) -> Vec3 {
    let (a, e) = (azimuth_deg.to_radians(), elevation_deg.to_radians());
    Vec3::new(a.sin() * e.cos(), e.sin(), -a.cos() * e.cos()).normalize()
}

struct Cascade {
    view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    tex_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    tex_shadow_pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind: wgpu::BindGroup,
    far: Cascade,
    near: Cascade,
    pub(crate) mat_bgl: wgpu::BindGroupLayout,
    pub(crate) mat_sampler: wgpu::Sampler,
    /// Direction toward the sun (world space).
    pub sun_dir: Vec3,
    pub color_format: wgpu::TextureFormat,
}

const VERTEX_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];

fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &VERTEX_ATTRS }
}

fn depth_state(write: bool, compare: wgpu::CompareFunction, bias: wgpu::DepthBiasState) -> Option<wgpu::DepthStencilState> {
    Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(write), depth_compare: Some(compare), stencil: Default::default(), bias })
}

impl Renderer {
    pub fn new(gpu: &Gpu, color_format: wgpu::TextureFormat) -> Self {
        let d = &gpu.device;
        let common = include_str!("common.wgsl");
        let module = |label, src: &str| d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(format!("{common}\n{src}").into()) });
        let flat_mod = module("flat", include_str!("shader.wgsl"));
        let tex_mod = module("tex", include_str!("tex.wgsl"));
        let shadow_mod = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("shadow"), source: wgpu::ShaderSource::Wgsl(include_str!("shadow.wgsl").into()) });

        let depth_tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let uniform = |binding, vis| wgpu::BindGroupLayoutEntry { binding, visibility: vis, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None };
        let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                depth_tex(1),
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
                depth_tex(3),
            ],
        });
        let sbgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("shadow"), entries: &[uniform(0, wgpu::ShaderStages::VERTEX)] });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let mat_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &[tex_entry(0), tex_entry(1), tex_entry(2), wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None }],
        });
        let mat_sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("material"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });

        let cascade = |res: u32, label: &str| {
            let view = d
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: res, height: res, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: DEPTH,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            let uniform = d.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: 64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let bind = d.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout: &sbgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }] });
            Cascade { view, uniform, bind }
        };
        let far = cascade(SHADOW_FAR_RES, "shadow far");
        let near = cascade(SHADOW_NEAR_RES, "shadow near");
        let globals = d.create_buffer(&wgpu::BufferDescriptor { label: Some("globals"), size: std::mem::size_of::<Globals>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let cmp = d.create_sampler(&wgpu::SamplerDescriptor { label: Some("shadow"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, compare: Some(wgpu::CompareFunction::LessEqual), ..Default::default() });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&far.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&cmp) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&near.view) },
            ],
        });

        let layout = |label, groups: &[Option<&wgpu::BindGroupLayout>]| d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(label), bind_group_layouts: groups, immediate_size: 0 });
        let main_layout = layout("main", &[Some(&bgl)]);
        let tex_layout = layout("tex", &[Some(&bgl), Some(&mat_bgl)]);
        let shadow_layout = layout("shadow", &[Some(&sbgl)]);
        let tex_shadow_layout = layout("tex shadow", &[Some(&sbgl), Some(&mat_bgl)]);
        let prim = wgpu::PrimitiveState { cull_mode: None, ..Default::default() };
        let color = [Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let ms = wgpu::MultisampleState { count: MSAA, ..Default::default() };
        let shadow_bias = wgpu::DepthBiasState { constant: 3, slope_scale: 2.5, clamp: 0.0 };
        let pipe = |label, layout: &wgpu::PipelineLayout, vs: wgpu::VertexState, fs: Option<wgpu::FragmentState>, depth, ms: wgpu::MultisampleState| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor { label: Some(label), layout: Some(layout), vertex: vs, primitive: prim, depth_stencil: depth, multisample: ms, fragment: fs, multiview_mask: None, cache: None })
        };
        let tl = textured::layouts().map(Some);
        let pipeline = pipe(
            "main",
            &main_layout,
            wgpu::VertexState { module: &flat_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[Some(vertex_layout())] },
            Some(wgpu::FragmentState { module: &flat_mod, entry_point: Some("fs"), compilation_options: Default::default(), targets: &color }),
            depth_state(true, wgpu::CompareFunction::Less, Default::default()),
            ms,
        );
        let sky_pipeline = pipe(
            "sky",
            &main_layout,
            wgpu::VertexState { module: &flat_mod, entry_point: Some("vs_sky"), compilation_options: Default::default(), buffers: &[] },
            Some(wgpu::FragmentState { module: &flat_mod, entry_point: Some("fs_sky"), compilation_options: Default::default(), targets: &color }),
            depth_state(false, wgpu::CompareFunction::Always, Default::default()),
            ms,
        );
        let tex_pipeline = pipe(
            "tex",
            &tex_layout,
            wgpu::VertexState { module: &tex_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &tl },
            Some(wgpu::FragmentState { module: &tex_mod, entry_point: Some("fs"), compilation_options: Default::default(), targets: &color }),
            depth_state(true, wgpu::CompareFunction::Less, Default::default()),
            ms,
        );
        let shadow_pipeline = pipe(
            "shadow",
            &shadow_layout,
            wgpu::VertexState { module: &shadow_mod, entry_point: Some("vs_flat"), compilation_options: Default::default(), buffers: &[Some(vertex_layout())] },
            None,
            depth_state(true, wgpu::CompareFunction::Less, shadow_bias),
            Default::default(),
        );
        let tex_shadow_pipeline = pipe(
            "tex shadow",
            &tex_shadow_layout,
            wgpu::VertexState { module: &shadow_mod, entry_point: Some("vs_tex"), compilation_options: Default::default(), buffers: &tl },
            Some(wgpu::FragmentState { module: &shadow_mod, entry_point: Some("fs_tex"), compilation_options: Default::default(), targets: &[] }),
            depth_state(true, wgpu::CompareFunction::Less, shadow_bias),
            Default::default(),
        );
        Self { pipeline, sky_pipeline, tex_pipeline, shadow_pipeline, tex_shadow_pipeline, globals, bind, far, near, mat_bgl, mat_sampler, sun_dir: sun_from(205.0, 52.0), color_format }
    }

    fn light_far(&self, scene: &Scene) -> Mat4 {
        let c = (scene.min + scene.max) * 0.5;
        let r = (scene.max - scene.min).length() * 0.5;
        let up = if self.sun_dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
        let view = Mat4::look_at_rh(c + self.sun_dir * r * 2.0, c, up);
        Mat4::orthographic_rh(-r, r, -r, r, r * 0.5, r * 3.5) * view
    }

    /// Near cascade around `focus`, snapped to whole texels so shadows don't shimmer as the camera moves.
    fn light_near(&self, focus: Vec3) -> Mat4 {
        let up = if self.sun_dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
        let view = Mat4::look_at_rh(Vec3::ZERO, -self.sun_dir, up);
        let texel = 2.0 * NEAR_HALF / SHADOW_NEAR_RES as f32;
        let c = view.transform_point3(focus);
        let c = Vec3::new((c.x / texel).round() * texel, (c.y / texel).round() * texel, c.z);
        Mat4::orthographic_rh(c.x - NEAR_HALF, c.x + NEAR_HALF, c.y - NEAR_HALF, c.y + NEAR_HALF, -c.z - 600.0, -c.z + 600.0) * view
    }

    fn shadow_pass(&self, enc: &mut wgpu::CommandEncoder, cascade: &Cascade, scene: &Scene, batches: &[&Batch]) {
        let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &cascade.view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_pipeline(&self.shadow_pipeline);
        p.set_bind_group(0, &cascade.bind, &[]);
        p.set_vertex_buffer(0, scene.vbuf.slice(..));
        p.set_index_buffer(scene.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        p.draw_indexed(0..scene.n_idx, 0, 0..1);
        p.set_pipeline(&self.tex_shadow_pipeline);
        for b in batches {
            p.set_bind_group(1, &b.material.bind, &[]);
            p.set_vertex_buffer(0, b.vb.slice(..));
            p.set_vertex_buffer(1, b.inst.slice(..));
            p.set_index_buffer(b.ib.slice(..), wgpu::IndexFormat::Uint32);
            p.draw_indexed(0..b.n_idx, 0, 0..b.n_inst);
        }
    }

    /// Records the shadow + main passes. `msaa` is the multisampled colour target, `resolve` the final one.
    pub fn render(&self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, scene: &Scene, batches: &[&Batch], cam: &Camera, msaa: &wgpu::TextureView, resolve: &wgpu::TextureView, depth: &wgpu::TextureView) {
        let (lf, ln) = (self.light_far(scene), self.light_near(cam.focus));
        let v4 = |r: f32, g: f32, b: f32, w: f32| [r, g, b, w];
        let gl = Globals {
            view_proj: cam.view_proj.to_cols_array_2d(),
            light_far: lf.to_cols_array_2d(),
            light_near: ln.to_cols_array_2d(),
            inv_view_proj: cam.view_proj.inverse().to_cols_array_2d(),
            sun_dir: [self.sun_dir.x, self.sun_dir.y, self.sun_dir.z, 1.0],
            sun_col: v4(3.4, 3.1, 2.75, 0.0),
            sky_zenith: v4(0.16, 0.34, 0.78, 0.0),
            sky_horizon: v4(0.62, 0.74, 0.90, 0.0),
            ground_col: v4(0.42, 0.37, 0.30, 0.0),
            eye: [cam.eye.x, cam.eye.y, cam.eye.z, cam.fog],
            misc: v4(1.15, 0.0, 0.0, 0.0),
        };
        gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&gl));
        gpu.queue.write_buffer(&self.far.uniform, 0, bytemuck::cast_slice(&lf.to_cols_array()));
        gpu.queue.write_buffer(&self.near.uniform, 0, bytemuck::cast_slice(&ln.to_cols_array()));
        self.shadow_pass(enc, &self.far, scene, batches);
        self.shadow_pass(enc, &self.near, scene, batches);
        let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: msaa,
                depth_slice: None,
                resolve_target: Some(resolve),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_bind_group(0, &self.bind, &[]);
        p.set_pipeline(&self.sky_pipeline);
        p.draw(0..3, 0..1);
        p.set_pipeline(&self.pipeline);
        p.set_vertex_buffer(0, scene.vbuf.slice(..));
        p.set_index_buffer(scene.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        p.draw_indexed(0..scene.n_idx, 0, 0..1);
        p.set_pipeline(&self.tex_pipeline);
        for b in batches {
            p.set_bind_group(1, &b.material.bind, &[]);
            p.set_vertex_buffer(0, b.vb.slice(..));
            p.set_vertex_buffer(1, b.inst.slice(..));
            p.set_index_buffer(b.ib.slice(..), wgpu::IndexFormat::Uint32);
            p.draw_indexed(0..b.n_idx, 0, 0..b.n_inst);
        }
    }
}

/// MSAA colour + depth targets for a given size.
pub struct Targets {
    pub msaa: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    pub w: u32,
    pub h: u32,
}

impl Targets {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat, w: u32, h: u32) -> Self {
        let mk = |format, label| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: MSAA,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        Self { msaa: mk(format, "msaa"), depth: mk(DEPTH, "depth"), w, h }
    }
}

/// Renders one frame off-screen and reads it back as an RGBA image.
pub fn render_image(gpu: &Gpu, r: &Renderer, scene: &Scene, batches: &[&Batch], cam: &Camera, w: u32, h: u32) -> image::RgbaImage {
    let format = r.color_format;
    let t = Targets::new(gpu, format, w, h);
    let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("out"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let row = (w * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    r.render(gpu, &mut enc, scene, batches, cam, &t.msaa, &out.create_view(&Default::default()), &t.depth);
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &out, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map failed"));
    gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).expect("poll failed");
    let data = buf.slice(..).get_mapped_range().expect("mapped range");
    let mut img = image::RgbaImage::new(w, h);
    for y in 0..h {
        let src = &data[(y * row) as usize..(y * row + w * 4) as usize];
        img.as_mut()[(y * w * 4) as usize..((y + 1) * w * 4) as usize].copy_from_slice(src);
    }
    img
}
