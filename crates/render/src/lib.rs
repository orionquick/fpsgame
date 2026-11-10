//! wgpu renderer: one static scene, a sun shadow map, and a forward pass. Draws to a window surface or off-screen.
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

pub use wgpu as wgpu_reexport;

pub const MSAA: u32 = 4;
pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_RES: u32 = 4096;

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
        let vbuf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("verts"), contents: bytemuck::cast_slice(verts), usage: wgpu::BufferUsages::VERTEX });
        let ibuf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("idx"), contents: bytemuck::cast_slice(idx), usage: wgpu::BufferUsages::INDEX });
        Self { vbuf, ibuf, n_idx: idx.len() as u32, min, max }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_vp: [[f32; 4]; 4],
    sun_dir: [f32; 4],
    sun_col: [f32; 4],
    sky_col: [f32; 4],
    ground_col: [f32; 4],
    eye: [f32; 4],
    fog_col: [f32; 4],
}

pub struct Camera {
    pub view_proj: Mat4,
    pub eye: Vec3,
    /// Exponential fog density per metre; 0 for top-down review renders.
    pub fog: f32,
}

impl Camera {
    /// Top-down orthographic view of an x/z rectangle (north up, east right).
    pub fn ortho(rect: [f32; 4], top: f32) -> Self {
        let (cx, cz) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
        let (hw, hh) = ((rect[2] - rect[0]) * 0.5, (rect[3] - rect[1]) * 0.5);
        let eye = Vec3::new(cx, top + 50.0, cz);
        let view = Mat4::look_at_rh(eye, Vec3::new(cx, 0.0, cz), Vec3::NEG_Z);
        Self { view_proj: Mat4::orthographic_rh(-hw, hw, -hh, hh, 1.0, top + 400.0) * view, eye, fog: 0.0 }
    }
    /// First-person view. Yaw 0 = north (-z), 90 = east (+x); pitch up positive. Degrees.
    pub fn perspective(eye: Vec3, yaw_deg: f32, pitch_deg: f32, fov_y_deg: f32, aspect: f32) -> Self {
        let dir = look_dir(yaw_deg, pitch_deg);
        let view = Mat4::look_at_rh(eye, eye + dir, Vec3::Y);
        Self { view_proj: Mat4::perspective_rh(fov_y_deg.to_radians(), aspect, 0.1, 5000.0) * view, eye, fog: 0.00035 }
    }
}

pub fn look_dir(yaw_deg: f32, pitch_deg: f32) -> Vec3 {
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    Vec3::new(y.sin() * p.cos(), p.sin(), -y.cos() * p.cos())
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind: wgpu::BindGroup,
    shadow_bind: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    /// Direction toward the sun (world space).
    pub sun_dir: Vec3,
    pub color_format: wgpu::TextureFormat,
}

const VERTEX_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];

fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &VERTEX_ATTRS }
}

/// Sun from azimuth (degrees clockwise from north) and elevation (degrees above the horizon).
pub fn sun_from(azimuth_deg: f32, elevation_deg: f32) -> Vec3 {
    let (a, e) = (azimuth_deg.to_radians(), elevation_deg.to_radians());
    Vec3::new(a.sin() * e.cos(), e.sin(), -a.cos() * e.cos()).normalize()
}

impl Renderer {
    pub fn new(gpu: &Gpu, color_format: wgpu::TextureFormat) -> Self {
        let d = &gpu.device;
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("shader"), source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
        let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
            ],
        });
        let globals = d.create_buffer(&wgpu::BufferDescriptor { label: Some("globals"), size: std::mem::size_of::<Globals>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let shadow = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d { width: SHADOW_RES, height: SHADOW_RES, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow.create_view(&Default::default());
        let samp = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&samp) },
            ],
        });
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("main"), bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let sbgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let shadow_bind = d.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("shadow globals"), layout: &sbgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }] });
        let shadow_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("shadow"), bind_group_layouts: &[Some(&sbgl)], immediate_size: 0 });
        let prim = wgpu::PrimitiveState { cull_mode: None, ..Default::default() };
        let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("main"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[Some(vertex_layout())] },
            primitive: prim,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: MSAA, ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let shadow_pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &[Some(vertex_layout())] },
            primitive: prim,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 4, slope_scale: 3.0, clamp: 0.0 },
            }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, shadow_pipeline, globals, bind, shadow_bind, shadow_view, sun_dir: sun_from(168.0, 68.0), color_format }
    }

    fn light_vp(&self, scene: &Scene) -> Mat4 {
        let c = (scene.min + scene.max) * 0.5;
        let r = (scene.max - scene.min).length() * 0.5;
        let up = if self.sun_dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
        let view = Mat4::look_at_rh(c + self.sun_dir * r * 2.0, c, up);
        Mat4::orthographic_rh(-r, r, -r, r, r * 0.5, r * 3.5) * view
    }

    /// Records the shadow + main passes. `msaa` is the multisampled colour target, `resolve` the final one.
    pub fn render(&self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, scene: &Scene, cam: &Camera, msaa: &wgpu::TextureView, resolve: &wgpu::TextureView, depth: &wgpu::TextureView) {
        let lin = |r: f32, g: f32, b: f32| [r, g, b, 1.0];
        let gl = Globals {
            view_proj: cam.view_proj.to_cols_array_2d(),
            light_vp: self.light_vp(scene).to_cols_array_2d(),
            sun_dir: [self.sun_dir.x, self.sun_dir.y, self.sun_dir.z, 0.0],
            sun_col: lin(1.7, 1.6, 1.45),
            sky_col: lin(0.30, 0.36, 0.46),
            ground_col: lin(0.14, 0.13, 0.12),
            eye: [cam.eye.x, cam.eye.y, cam.eye.z, cam.fog],
            fog_col: lin(0.62, 0.72, 0.86),
        };
        gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&gl));
        {
            let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            p.set_pipeline(&self.shadow_pipeline);
            p.set_bind_group(0, &self.shadow_bind, &[]);
            p.set_vertex_buffer(0, scene.vbuf.slice(..));
            p.set_index_buffer(scene.ibuf.slice(..), wgpu::IndexFormat::Uint32);
            p.draw_indexed(0..scene.n_idx, 0, 0..1);
        }
        let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: msaa,
                depth_slice: None,
                resolve_target: Some(resolve),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.55, g: 0.66, b: 0.82, a: 1.0 }), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_pipeline(&self.pipeline);
        p.set_bind_group(0, &self.bind, &[]);
        p.set_vertex_buffer(0, scene.vbuf.slice(..));
        p.set_index_buffer(scene.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        p.draw_indexed(0..scene.n_idx, 0, 0..1);
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
pub fn render_image(gpu: &Gpu, r: &Renderer, scene: &Scene, cam: &Camera, w: u32, h: u32) -> image::RgbaImage {
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
    r.render(gpu, &mut enc, scene, cam, &t.msaa, &out.create_view(&Default::default()), &t.depth);
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
