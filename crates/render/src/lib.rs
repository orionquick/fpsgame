//! wgpu renderer: static scene + instanced textured batches, two-cascade sun shadows, sky, haze, tonemapping.
//! Draws to a window surface or off-screen.
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

pub use wgpu as wgpu_reexport;

pub mod cull;
pub mod gi;
pub mod prof;
pub mod textured;
pub use cull::CullWorld;
pub use textured::{compress_material, read_compressed, write_compressed, Batch, CompressedTex, CompressedTex3, Detail, Instance, Material, TexVertex};

pub const MSAA: u32 = 4;
/// Material slots in the bindless arrays.
pub const MAX_MATERIALS: u32 = 2048;
/// Temporal history format (output resolution).
const TAA_HIST: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
const SHADOW_FAR_RES: u32 = 4096;
const SHADOW_NEAR_RES: u32 = 4096;
/// Half-size of the near shadow cascade around the camera, metres.
const NEAR_HALF: f32 = 70.0;
/// Grid the culled path's near cascade centre snaps to (it is redrawn when the eye changes cell).
const NEAR_CELL: f32 = 8.0;

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// BC block compression available (textures upload compressed).
    pub bc: bool,
    /// Texture binding arrays, non-uniform indexing and indirect first-instance: the bindless culled path.
    pub bindless: bool,
}

/// Instance options: wgpu's per-draw indirect validation off (it adds GPU work for every indirect draw; the culled
/// path's arguments are written by our own shader). WGPU_* environment variables still override.
pub fn instance_descriptor() -> wgpu::InstanceDescriptor {
    let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
    d.flags.remove(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL);
    d.with_env()
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
        let bc = adapter.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let ts = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        let bl = wgpu::Features::TEXTURE_BINDING_ARRAY | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING | wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY | wgpu::Features::INDIRECT_FIRST_INSTANCE;
        let bindless = adapter.features().contains(bl) && std::env::var("FPS_NO_BINDLESS").is_err();
        let bl = if bindless { bl } else { wgpu::Features::empty() };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("device"),
            required_limits: adapter.limits(),
            required_features: ts | bl | if bc { wgpu::Features::TEXTURE_COMPRESSION_BC } else { wgpu::Features::empty() },
            ..Default::default()
        }))
        .expect("no GPU device");
        Self { instance, adapter, device, queue, bc, bindless }
    }
    pub fn headless() -> Self {
        Self::new(wgpu::Instance::new(instance_descriptor()), None)
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
    /// Reflection probe: box min (w: enabled), box max (w: max mip), capture point (w: radiance decode scale).
    probe_min: [f32; 4],
    probe_max: [f32; 4],
    probe_pos: [f32; 4],
    /// GI volume: origin (w: cell size), dims (w: enabled).
    gi_min: [f32; 4],
    gi_dim: [f32; 4],
    /// Screen-space reflections: the previous frame's view-projection and eye (w: SSR on).
    prev_vp: [[f32; 4]; 4],
    prev_eye: [f32; 4],
    /// Camera white balance (rgb multipliers before the tonemap).
    white: [f32; 4],
}

pub struct Camera {
    pub view_proj: Mat4,
    pub eye: Vec3,
    /// Where the near shadow cascade is centred (in front of the camera).
    pub focus: Vec3,
    /// Haze density per metre; 0 for top-down review renders.
    pub fog: f32,
    /// 1 / tan(half the vertical field of view); 0 for orthographic views.
    pub proj_y: f32,
}

impl Camera {
    /// Top-down orthographic view of an x/z rectangle (north up, east right).
    pub fn ortho(rect: [f32; 4], top: f32) -> Self {
        let (cx, cz) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
        let (hw, hh) = ((rect[2] - rect[0]) * 0.5, (rect[3] - rect[1]) * 0.5);
        let eye = Vec3::new(cx, top + 50.0, cz);
        let view = Mat4::look_at_rh(eye, Vec3::new(cx, 0.0, cz), Vec3::NEG_Z);
        Self { view_proj: Mat4::orthographic_rh(-hw, hw, -hh, hh, 1.0, top + 400.0) * view, eye, focus: Vec3::new(cx, 0.0, cz), fog: 0.0, proj_y: 0.0 }
    }
    /// Top-down orthographic view of `rect` cut at height `cut` (everything above it left out): interior plans.
    pub fn plan(rect: [f32; 4], cut: f32) -> Self {
        let (cx, cz) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
        let (hw, hh) = ((rect[2] - rect[0]) * 0.5, (rect[3] - rect[1]) * 0.5);
        let eye = Vec3::new(cx, cut, cz);
        let view = Mat4::look_at_rh(eye, Vec3::new(cx, cut - 1.0, cz), Vec3::NEG_Z);
        Self { view_proj: Mat4::orthographic_rh(-hw, hw, -hh, hh, 0.01, 60.0) * view, eye, focus: Vec3::new(cx, cut - 3.0, cz), fog: 0.0, proj_y: 0.0 }
    }
    /// First-person view. Yaw 0 = north (-z), 90 = east (+x); pitch up positive. Degrees.
    pub fn perspective(eye: Vec3, yaw_deg: f32, pitch_deg: f32, fov_y_deg: f32, aspect: f32) -> Self {
        let dir = look_dir(yaw_deg, pitch_deg);
        let view = Mat4::look_at_rh(eye, eye + dir, Vec3::Y);
        let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        Self { view_proj: Mat4::perspective_rh(fov_y_deg.to_radians(), aspect, 0.1, 5000.0) * view, eye, focus: eye + flat * (NEAR_HALF * 0.6), fog: haze_density(), proj_y: 1.0 / (fov_y_deg.to_radians() * 0.5).tan() }
    }
}

pub fn look_dir(yaw_deg: f32, pitch_deg: f32) -> Vec3 {
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    Vec3::new(y.sin() * p.cos(), p.sin(), -y.cos() * p.cos())
}

/// Sun from azimuth (degrees clockwise from north) and elevation (degrees above the horizon).
/// The sun: mid-afternoon from the west-southwest, low enough to stream deep into the old factory halls (as in
/// the reference photos). FPS_SUN="azimuth,elevation" in degrees overrides it.
pub fn default_sun() -> Vec3 {
    let v: Vec<f32> = std::env::var("FPS_SUN").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    if v.len() == 2 { sun_from(v[0], v[1]) } else { sun_from(245.0, 32.0) }
}

pub fn sun_from(azimuth_deg: f32, elevation_deg: f32) -> Vec3 {
    let (a, e) = (azimuth_deg.to_radians(), elevation_deg.to_radians());
    Vec3::new(a.sin() * e.cos(), e.sin(), -a.cos() * e.cos()).normalize()
}

/// Local light (lamps): position, range, colour (linear, pre-multiplied by intensity).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLight {
    pub pos: [f32; 3],
    pub radius: f32,
    pub color: [f32; 3],
    pub _pad: f32,
}

/// Lamp capacity, light-grid table size (a power of two) and grid list capacity.
pub const MAX_LIGHTS: usize = 8192;
const LIGHT_TABLE: usize = 1 << 16;
const LIGHT_IDX: usize = 1 << 20;
const LIGHT_CELL: f32 = 2.0;
/// Afternoon sunlight (linear, intensity folded in): warm, as in the reference photos.
pub const SUN_COL: [f32; 3] = [3.5, 3.0, 2.4];
/// Screen-space reflection history: display colour + linear eye distance.
const HIST_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const PROBE_RES: u32 = 256;
pub const PROBE_MIPS: u32 = 7;
/// Radiance is stored scaled down by this in the 8-bit probe and scaled back up when sampled.
const PROBE_ENCODE: f32 = 0.25;

struct Cascade {
    view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    tex_pipeline: wgpu::RenderPipeline,
    glass_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    tex_shadow_pipeline: wgpu::RenderPipeline,
    ao_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    pub(crate) post_bgl: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    lights: wgpu::Buffer,
    light_grid: wgpu::Buffer,
    light_idx: wgpu::Buffer,
    n_lights: std::cell::Cell<u32>,
    /// Identity of the uploaded light set (count and a hash of positions).
    lights_key: std::cell::Cell<u64>,
    probe_tex: wgpu::Texture,
    probe_down: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    probe_down_bgl: wgpu::BindGroupLayout,
    probe_down_samp: wgpu::Sampler,
    /// Current probe state (see Globals) and the radiance encode scale while baking (0 = normal output).
    probe: std::cell::Cell<[[f32; 4]; 3]>,
    encode: std::cell::Cell<f32>,
    exposure: std::cell::Cell<f32>,
    white: std::cell::Cell<[f32; 3]>,
    bind: wgpu::BindGroup,
    far: Cascade,
    near: Cascade,
    pub(crate) mat_bgl: wgpu::BindGroupLayout,
    pub(crate) mat_sampler: wgpu::Sampler,
    /// Direction toward the sun (world space).
    pub sun_dir: Vec3,
    pub color_format: wgpu::TextureFormat,
    /// Voxel flood-fill GI (indirect diffuse light).
    pub gi: gi::Gi,
    hist_bgl: wgpu::BindGroupLayout,
    // culled path (see cull.rs)
    pub(crate) inst_bgl: wgpu::BindGroupLayout,
    pub(crate) mat_arr_bgl: wgpu::BindGroupLayout,
    /// The culled path draws from one vertex/index buffer with all materials bound at once.
    pub(crate) bindless: bool,
    pub(crate) cull_bgl: wgpu::BindGroupLayout,
    pub(crate) cull_pipeline: wgpu::ComputePipeline,
    pub(crate) hiz_init_bgl: wgpu::BindGroupLayout,
    pub(crate) hiz_init_pipeline: wgpu::ComputePipeline,
    pub(crate) hiz_down_bgl: wgpu::BindGroupLayout,
    pub(crate) hiz_down_pipeline: wgpu::ComputePipeline,
    tex_ids_pipeline: wgpu::RenderPipeline,
    glass_ids_pipeline: wgpu::RenderPipeline,
    /// Depth prepass (no colour) and the shading pipelines that follow it (depth equal, no writes).
    flat_pre_pipeline: wgpu::RenderPipeline,
    tex_ids_pre_pipeline: wgpu::RenderPipeline,
    flat_shade_pipeline: wgpu::RenderPipeline,
    sky_late_pipeline: wgpu::RenderPipeline,
    /// The world, sun and centre the near shadow map was last drawn for by the culled path.
    near_key: std::cell::Cell<(u64, [f32; 3], [i32; 3])>,
    tex_shadow_ids_pipeline: wgpu::RenderPipeline,
    /// The world and sun the far shadow map was last drawn for by the culled path (it is static).
    far_key: std::cell::Cell<(u64, [f32; 3])>,
    /// GPU pass timing (FPS_GPU_TIMING).
    pub prof: Option<prof::Profiler>,
    /// MSAA samples of the main pass (1: the game, with temporal anti-aliasing).
    pub samples: u32,
    tsr_pre_bgl: wgpu::BindGroupLayout,
    tsr_res_bgl: wgpu::BindGroupLayout,
    tsr_present_bgl: wgpu::BindGroupLayout,
    tsr_pre_pipeline: wgpu::RenderPipeline,
    tsr_res_pipeline: wgpu::RenderPipeline,
    tsr_present_pipeline: wgpu::RenderPipeline,
}

const ID_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![4 => Uint32];
const VERTEX_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];

fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &VERTEX_ATTRS }
}

fn depth_state(write: bool, compare: wgpu::CompareFunction, bias: wgpu::DepthBiasState) -> Option<wgpu::DepthStencilState> {
    Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(write), depth_compare: Some(compare), stencil: Default::default(), bias })
}

impl Renderer {
    /// With 4x MSAA (review shots and photo comparisons).
    pub fn new(gpu: &Gpu, color_format: wgpu::TextureFormat) -> Self {
        Self::with_samples(gpu, color_format, MSAA)
    }

    /// `samples` 1 for the game (temporal anti-aliasing instead of MSAA), 4 otherwise.
    pub fn with_samples(gpu: &Gpu, color_format: wgpu::TextureFormat, samples: u32) -> Self {
        // shaders that read the main depth see a multisampled or a plain depth texture
        let depth_ty = if samples > 1 { "texture_depth_multisampled_2d" } else { "texture_depth_2d" };
        let d = &gpu.device;
        let common = include_str!("common.wgsl");
        let module = |label, src: &str| d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(format!("{common}\n{src}").into()) });
        let flat_mod = module("flat", include_str!("shader.wgsl"));
        let tex_mod = module("tex", include_str!("tex.wgsl"));
        let post_mod = module("post", &include_str!("post.wgsl").replace("texture_depth_multisampled_2d", depth_ty));
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
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 9, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 10, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::Cube, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 6, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D3, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D3, multisampled: false },
                    count: None,
                },
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
        let post_tex = |binding, sample_type, multisampled| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled }, count: None };
        let post_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post"),
            entries: &[
                post_tex(0, wgpu::TextureSampleType::Depth, samples > 1),
                post_tex(1, wgpu::TextureSampleType::Float { filterable: false }, false),
                post_tex(2, wgpu::TextureSampleType::Float { filterable: false }, false),
            ],
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
        let sbuf = |label, size: usize| d.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: size as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let lights = sbuf("lights", MAX_LIGHTS * std::mem::size_of::<PointLight>());
        let light_grid = sbuf("light grid", LIGHT_TABLE * 8);
        let light_idx = sbuf("light idx", LIGHT_IDX * 4);
        let probe_tex = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("probe"),
            size: wgpu::Extent3d { width: PROBE_RES, height: PROBE_RES, depth_or_array_layers: 6 },
            mip_level_count: PROBE_MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let probe_view = probe_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::Cube), ..Default::default() });
        let probe_samp = d.create_sampler(&wgpu::SamplerDescriptor { label: Some("probe"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, mipmap_filter: wgpu::MipmapFilterMode::Linear, ..Default::default() });
        let gi = gi::Gi::new(d);
        let cmp = d.create_sampler(&wgpu::SamplerDescriptor { label: Some("shadow"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, compare: Some(wgpu::CompareFunction::LessEqual), ..Default::default() });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&far.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&cmp) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&near.view) },
                wgpu::BindGroupEntry { binding: 4, resource: lights.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&probe_view) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&probe_samp) },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&gi.view) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&gi.sky_view) },
                wgpu::BindGroupEntry { binding: 9, resource: light_grid.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 10, resource: light_idx.as_entire_binding() },
            ],
        });

        let layout = |label, groups: &[Option<&wgpu::BindGroupLayout>]| d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(label), bind_group_layouts: groups, immediate_size: 0 });
        let main_layout = layout("main", &[Some(&bgl)]);
        let hist_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("history"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                count: None,
            }],
        });
        let tex_layout = layout("tex", &[Some(&bgl), Some(&mat_bgl), Some(&hist_bgl)]);
        let shadow_layout = layout("shadow", &[Some(&sbgl)]);
        let tex_shadow_layout = layout("tex shadow", &[Some(&sbgl), Some(&mat_bgl)]);
        let prim = wgpu::PrimitiveState { cull_mode: None, ..Default::default() };
        let color = [Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let ms = wgpu::MultisampleState { count: samples, ..Default::default() };
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
        let glass_color = [Some(wgpu::ColorTargetState { format: color_format, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })];
        let glass_pipeline = pipe(
            "glass",
            &tex_layout,
            wgpu::VertexState { module: &tex_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &tl },
            Some(wgpu::FragmentState { module: &tex_mod, entry_point: Some("fs_glass"), compilation_options: Default::default(), targets: &glass_color }),
            depth_state(false, wgpu::CompareFunction::Less, Default::default()),
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
        // ---- culled path ----
        let storage = |binding, vis, read_only| wgpu::BindGroupLayoutEntry { binding, visibility: vis, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None }, count: None };
        let inst_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("instances"), entries: &[storage(0, wgpu::ShaderStages::VERTEX, true), storage(1, wgpu::ShaderStages::VERTEX, true)] });
        // bindless: every material's three maps in binding arrays, indexed per instance
        let bindless = gpu.bindless;
        let arr = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: std::num::NonZeroU32::new(MAX_MATERIALS),
        };
        let mat_arr_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("materials"),
            entries: &[arr(0), arr(1), arr(2), wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None }],
        });
        let ids_mat_bgl = if bindless { &mat_arr_bgl } else { &mat_bgl };
        let bindless_src = |src: &str| {
            let mut s = src.to_string();
            for (k, n) in ["t_base", "t_orm", "t_nrm"].iter().enumerate() {
                s = s.replace(&format!("@group(1) @binding({k}) var {n}: texture_2d<f32>;"), &format!("@group(1) @binding({k}) var {n}: binding_array<texture_2d<f32>>;"));
                s = s.replace(&format!("({n}, "), &format!("({n}[i.mat], "));
            }
            s
        };
        // the enable directive leads the module (before common.wgsl)
        let tex_ids_mod = if bindless {
            d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("tex bindless"), source: wgpu::ShaderSource::Wgsl(format!("enable wgpu_binding_array;\n{common}\n{}", bindless_src(include_str!("tex.wgsl"))).into()) })
        } else {
            module("tex ids", include_str!("tex.wgsl"))
        };
        let shadow_ids_src = if bindless { format!("enable wgpu_binding_array;\n{}", bindless_src(include_str!("shadow.wgsl"))) } else { include_str!("shadow.wgsl").to_string() };
        let shadow_ids_mod = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("shadow ids"), source: wgpu::ShaderSource::Wgsl(shadow_ids_src.into()) });
        let ids_layout = [Some(textured::layouts()[0].clone()), Some(wgpu::VertexBufferLayout { array_stride: 4, step_mode: wgpu::VertexStepMode::Instance, attributes: &ID_ATTRS })];
        let tex_ids_layout = layout("tex ids", &[Some(&bgl), Some(ids_mat_bgl), Some(&hist_bgl), Some(&inst_bgl)]);
        let tex_shadow_ids_layout = layout("tex shadow ids", &[Some(&sbgl), Some(ids_mat_bgl), Some(&inst_bgl)]);
        let tex_ids_pipeline = pipe(
            "tex ids",
            &tex_ids_layout,
            wgpu::VertexState { module: &tex_ids_mod, entry_point: Some("vs_ids"), compilation_options: Default::default(), buffers: &ids_layout },
            Some(wgpu::FragmentState { module: &tex_ids_mod, entry_point: Some("fs"), compilation_options: Default::default(), targets: &color }),
            depth_state(false, wgpu::CompareFunction::LessEqual, Default::default()),
            ms,
        );
        let no_color = [Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::empty() })];
        let tex_ids_pre_pipeline = pipe(
            "tex ids prepass",
            &tex_ids_layout,
            wgpu::VertexState { module: &tex_ids_mod, entry_point: Some("vs_ids_pre"), compilation_options: Default::default(), buffers: &ids_layout },
            Some(wgpu::FragmentState { module: &tex_ids_mod, entry_point: Some("fs_pre"), compilation_options: Default::default(), targets: &no_color }),
            depth_state(true, wgpu::CompareFunction::Less, Default::default()),
            ms,
        );
        let flat_pre_pipeline = pipe(
            "flat prepass",
            &main_layout,
            wgpu::VertexState { module: &flat_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[Some(vertex_layout())] },
            Some(wgpu::FragmentState { module: &flat_mod, entry_point: Some("fs_pre"), compilation_options: Default::default(), targets: &no_color }),
            depth_state(true, wgpu::CompareFunction::Less, Default::default()),
            ms,
        );
        let flat_shade_pipeline = pipe(
            "flat shade",
            &main_layout,
            wgpu::VertexState { module: &flat_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[Some(vertex_layout())] },
            Some(wgpu::FragmentState { module: &flat_mod, entry_point: Some("fs"), compilation_options: Default::default(), targets: &color }),
            depth_state(false, wgpu::CompareFunction::LessEqual, Default::default()),
            ms,
        );
        // the sky last, only where nothing was drawn (its depth is 1, the clear value)
        let sky_late_pipeline = pipe(
            "sky late",
            &main_layout,
            wgpu::VertexState { module: &flat_mod, entry_point: Some("vs_sky"), compilation_options: Default::default(), buffers: &[] },
            Some(wgpu::FragmentState { module: &flat_mod, entry_point: Some("fs_sky"), compilation_options: Default::default(), targets: &color }),
            depth_state(false, wgpu::CompareFunction::LessEqual, Default::default()),
            ms,
        );
        let glass_ids_pipeline = pipe(
            "glass ids",
            &tex_ids_layout,
            wgpu::VertexState { module: &tex_ids_mod, entry_point: Some("vs_ids"), compilation_options: Default::default(), buffers: &ids_layout },
            Some(wgpu::FragmentState { module: &tex_ids_mod, entry_point: Some("fs_glass"), compilation_options: Default::default(), targets: &glass_color }),
            depth_state(false, wgpu::CompareFunction::Less, Default::default()),
            ms,
        );
        let tex_shadow_ids_pipeline = pipe(
            "tex shadow ids",
            &tex_shadow_ids_layout,
            wgpu::VertexState { module: &shadow_ids_mod, entry_point: Some("vs_tex_ids"), compilation_options: Default::default(), buffers: &ids_layout },
            Some(wgpu::FragmentState { module: &shadow_ids_mod, entry_point: Some("fs_tex"), compilation_options: Default::default(), targets: &[] }),
            depth_state(true, wgpu::CompareFunction::Less, shadow_bias),
            Default::default(),
        );
        let cull_src = format!("const NSAMP: i32 = {samples};\n{}", include_str!("cull.wgsl").replace("texture_depth_multisampled_2d", depth_ty));
        let cull_mod = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("cull"), source: wgpu::ShaderSource::Wgsl(cull_src.into()) });
        let cs = wgpu::ShaderStages::COMPUTE;
        let unfilt = |binding| wgpu::BindGroupLayoutEntry { binding, visibility: cs, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None };
        let r32_store = |binding| wgpu::BindGroupLayoutEntry { binding, visibility: cs, ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::R32Float, view_dimension: wgpu::TextureViewDimension::D2 }, count: None };
        let cull_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cull"),
            entries: &[storage(0, cs, true), storage(1, cs, true), storage(2, cs, true), storage(3, cs, true), uniform(4, cs), storage(5, cs, false), storage(6, cs, false), unfilt(7)],
        });
        let hiz_init_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hiz init"),
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: cs, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: samples > 1 }, count: None }, r32_store(1)],
        });
        // temporal super resolution (tsr.wgsl, after Ruinbound's)
        let ftex = |binding| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None };
        let fu = uniform(0, wgpu::ShaderStages::FRAGMENT);
        let tsr_pre_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tsr pre"),
            entries: &[fu, ftex(1), wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None }],
        });
        let tsr_res_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tsr resolve"),
            entries: &[fu, ftex(1), ftex(3), ftex(4), ftex(5), wgpu::BindGroupLayoutEntry { binding: 6, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None }],
        });
        let tsr_present_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("tsr present"), entries: &[fu, ftex(7)] });
        let tsr_mod = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("tsr"), source: wgpu::ShaderSource::Wgsl(include_str!("tsr.wgsl").into()) });
        let tsr_pipe = |label, bgl: &wgpu::BindGroupLayout, entry, targets: &[Option<wgpu::ColorTargetState>]| {
            pipe(
                label,
                &layout(label, &[Some(bgl)]),
                wgpu::VertexState { module: &tsr_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                Some(wgpu::FragmentState { module: &tsr_mod, entry_point: Some(entry), compilation_options: Default::default(), targets }),
                None,
                Default::default(),
            )
        };
        let hist_t = Some(wgpu::ColorTargetState { format: TAA_HIST, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let tsr_pre_pipeline = tsr_pipe("tsr pre", &tsr_pre_bgl, "fs_pre", &[hist_t.clone(), hist_t.clone()]);
        let tsr_res_pipeline = tsr_pipe("tsr resolve", &tsr_res_bgl, "fs_resolve", &[hist_t.clone()]);
        let tsr_present_pipeline = tsr_pipe("tsr present", &tsr_present_bgl, "fs_present", &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })]);
        let hiz_down_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("hiz down"), entries: &[unfilt(2), r32_store(3)] });
        let compute = |label, bgl: &wgpu::BindGroupLayout, entry| {
            d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(label), layout: Some(&layout(label, &[Some(bgl)])), module: &cull_mod, entry_point: Some(entry), compilation_options: Default::default(), cache: None })
        };
        let cull_pipeline = compute("cull", &cull_bgl, "cull");
        let hiz_init_pipeline = compute("hiz init", &hiz_init_bgl, "hiz_init");
        let hiz_down_pipeline = compute("hiz down", &hiz_down_bgl, "hiz_down");

        let probe_down_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("probe down"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let probe_down_samp = d.create_sampler(&wgpu::SamplerDescriptor { label: Some("probe down"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, address_mode_u: wgpu::AddressMode::ClampToEdge, address_mode_v: wgpu::AddressMode::ClampToEdge, ..Default::default() });
        let down_mod = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("probe down"), source: wgpu::ShaderSource::Wgsl(include_str!("probe_down.wgsl").into()) });
        let probe_down = pipe(
            "probe down",
            &layout("probe down", &[Some(&probe_down_bgl)]),
            wgpu::VertexState { module: &down_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            Some(wgpu::FragmentState { module: &down_mod, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            None,
            Default::default(),
        );
        let blit_pipeline = pipe(
            "blit",
            &layout("blit", &[Some(&probe_down_bgl)]),
            wgpu::VertexState { module: &down_mod, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            Some(wgpu::FragmentState { module: &down_mod, entry_point: Some("fs_blit"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            None,
            Default::default(),
        );
        let post_layout = layout("post", &[Some(&bgl), Some(&post_bgl)]);
        let full = wgpu::VertexState { module: &post_mod, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] };
        let ao_pipeline = pipe(
            "ao",
            &post_layout,
            full.clone(),
            Some(wgpu::FragmentState { module: &post_mod, entry_point: Some("fs_ao"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: AO_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            None,
            Default::default(),
        );
        let composite_pipeline = pipe(
            "composite",
            &post_layout,
            full,
            Some(wgpu::FragmentState {
                module: &post_mod,
                entry_point: Some("fs_composite"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL }), Some(wgpu::ColorTargetState { format: HIST_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            None,
            Default::default(),
        );
        Self { mat_arr_bgl, bindless, samples, tsr_pre_bgl, tsr_res_bgl, tsr_present_bgl, tsr_pre_pipeline, tsr_res_pipeline, tsr_present_pipeline, inst_bgl, cull_bgl, cull_pipeline, hiz_init_bgl, hiz_init_pipeline, hiz_down_bgl, hiz_down_pipeline, tex_ids_pipeline, glass_ids_pipeline, flat_pre_pipeline, tex_ids_pre_pipeline, flat_shade_pipeline, sky_late_pipeline, near_key: std::cell::Cell::new((0, [0.0; 3], [0; 3])), tex_shadow_ids_pipeline, far_key: std::cell::Cell::new((0, [0.0; 3])), prof: prof::Profiler::new(gpu), hist_bgl, gi, probe_tex, probe_down, blit_pipeline, probe_down_bgl, probe_down_samp, probe: std::cell::Cell::new([[0.0; 4]; 3]), encode: std::cell::Cell::new(0.0), exposure: std::cell::Cell::new(1.0), white: std::cell::Cell::new([1.0; 3]), lights, light_grid, light_idx, lights_key: std::cell::Cell::new(u64::MAX), n_lights: std::cell::Cell::new(0), ao_pipeline, composite_pipeline, post_bgl, pipeline, sky_pipeline, tex_pipeline, glass_pipeline, shadow_pipeline, tex_shadow_pipeline, globals, bind, far, near, mat_bgl, mat_sampler, sun_dir: default_sun(), color_format }
    }

    /// Renders a reflection probe at `pos` for the box `min`..`max` (world): six 90-degree faces, then a blurred mip
    /// chain for rough reflections and diffuse light. Shading inside the box uses it until the next bake.
    pub fn bake_probe(&self, gpu: &Gpu, scene: &Scene, batches: &[&Batch], pos: Vec3, min: Vec3, max: Vec3) {
        self.bake_faces(gpu, pos, min, max, &|enc, cam, t, out| self.render(gpu, enc, scene, batches, cam, t, out));
    }

    /// `bake_probe` through the culled path (the probe's low resolution picks coarse levels of detail).
    pub fn bake_probe_culled(&self, gpu: &Gpu, scene: &Scene, world: &CullWorld, batches: &[&Batch], pos: Vec3, min: Vec3, max: Vec3) {
        self.bake_faces(gpu, pos, min, max, &|enc, cam, t, out| {
            // the faces don't share a view: no occlusion test against the previous face's depth
            t.prev_cull.set(None);
            self.render_culled(gpu, enc, scene, world, batches, cam, t, out)
        });
    }

    /// Bilinear copy of `src` onto `dst` (the internal-resolution frame onto the window).
    pub fn blit(&self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, src: &wgpu::TextureView, dst: &wgpu::TextureView) {
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout: &self.probe_down_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.probe_down_samp) }],
        });
        let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: dst, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: self.prof.as_ref().and_then(|p| p.render("blit")),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_pipeline(&self.blit_pipeline);
        p.set_bind_group(0, &bind, &[]);
        p.draw(0..3, 0..1);
    }

    fn bake_faces(&self, gpu: &Gpu, pos: Vec3, min: Vec3, max: Vec3, face: &dyn Fn(&mut wgpu::CommandEncoder, &Camera, &Targets, &wgpu::TextureView)) {
        self.probe.set([[0.0; 4]; 3]);
        self.encode.set(PROBE_ENCODE);
        let t = Targets::new(gpu, self, PROBE_RES, PROBE_RES);
        let tmp = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("probe face"),
            size: wgpu::Extent3d { width: PROBE_RES, height: PROBE_RES, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let tmp_view = tmp.create_view(&Default::default());
        // WebGPU cube face order and orientation (+X -X +Y -Y +Z -Z), rendered with a mirrored x so the images
        // land the way cube sampling reads them
        let faces = [(Vec3::X, Vec3::Y), (Vec3::NEG_X, Vec3::Y), (Vec3::Y, Vec3::NEG_Z), (Vec3::NEG_Y, Vec3::Z), (Vec3::Z, Vec3::Y), (Vec3::NEG_Z, Vec3::Y)];
        let proj = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)) * Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.05, 400.0);
        for (i, (fwd, up)) in faces.iter().enumerate() {
            let view = Mat4::look_at_rh(pos, pos + *fwd, *up);
            let cam = Camera { view_proj: proj * view, eye: pos, focus: pos, fog: haze_density(), proj_y: 1.0 };
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            face(&mut enc, &cam, &t, &tmp_view);
            enc.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo { texture: &tmp, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyTextureInfo { texture: &self.probe_tex, mip_level: 0, origin: wgpu::Origin3d { x: 0, y: 0, z: i as u32 }, aspect: wgpu::TextureAspect::All },
                wgpu::Extent3d { width: PROBE_RES, height: PROBE_RES, depth_or_array_layers: 1 },
            );
            gpu.queue.submit([enc.finish()]);
        }
        // blurred mips: each level filters the one above
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        for mip in 1..PROBE_MIPS {
            for face in 0..6u32 {
                let src = self.probe_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_mip_level: mip - 1, mip_level_count: Some(1), base_array_layer: face, array_layer_count: Some(1), ..Default::default() });
                let dst = self.probe_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_mip_level: mip, mip_level_count: Some(1), base_array_layer: face, array_layer_count: Some(1), ..Default::default() });
                let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("probe down"),
                    layout: &self.probe_down_bgl,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&src) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.probe_down_samp) }],
                });
                let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("probe down"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &dst, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: self.prof.as_ref().and_then(|p| p.render("probe down")),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                p.set_pipeline(&self.probe_down);
                p.set_bind_group(0, &bind, &[]);
                p.draw(0..3, 0..1);
            }
        }
        gpu.queue.submit([enc.finish()]);
        self.encode.set(0.0);
        self.probe.set([[min.x, min.y, min.z, 1.0], [max.x, max.y, max.z, (PROBE_MIPS - 1) as f32], [pos.x, pos.y, pos.z, 1.0 / PROBE_ENCODE]]);
    }

    /// Runs `n` GI propagation steps now (shots: thousands at once; play: a few per frame).
    /// `n` GI propagation steps recorded into a frame's encoder (timed as "gi").
    pub fn gi_steps(&self, enc: &mut wgpu::CommandEncoder, n: u32) {
        self.gi.iterate_ts(enc, n, self.prof.as_ref().and_then(|p| p.compute("gi")));
    }

    pub fn gi_iterate(&self, gpu: &Gpu, n: u32) {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        self.gi.iterate(&mut enc, n);
        gpu.queue.submit([enc.finish()]);
    }

    /// Camera white balance: interiors lit by blue skylight through windows are warmed, as a camera would.
    pub fn set_white_balance(&self, rgb: [f32; 3]) {
        self.white.set(rgb);
    }

    /// Camera exposure (1 = outdoor daylight; interiors are exposed brighter, as a camera would).
    pub fn set_exposure(&self, e: f32) {
        self.exposure.set(e);
    }

    /// Turns the reflection probe off (outdoor shots).
    pub fn clear_probe(&self) {
        self.probe.set([[0.0; 4]; 3]);
    }

    /// Sets the local lights. They are static: all of them go up once with a hashed world grid of the lamps
    /// reaching each 2 m cell (each pixel then only visits its cell's lamps); later calls with the same set are free.
    pub fn set_lights(&self, gpu: &Gpu, lights: &[PointLight], _eye: Vec3) {
        let mut key = lights.len() as u64 ^ 0x9e37_79b9_7f4a_7c15;
        for l in lights {
            for v in [l.pos[0], l.pos[1], l.pos[2], l.radius] {
                key = (key ^ v.to_bits() as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
        if key == self.lights_key.get() {
            return;
        }
        self.lights_key.set(key);
        let lights = &lights[..lights.len().min(MAX_LIGHTS)];
        let hash = |c: [i32; 3]| ((c[0] as u32).wrapping_mul(73856093) ^ (c[1] as u32).wrapping_mul(19349663) ^ (c[2] as u32).wrapping_mul(83492791)) as usize & (LIGHT_TABLE - 1);
        let mut lists: Vec<Vec<u32>> = vec![vec![]; LIGHT_TABLE];
        for (i, l) in lights.iter().enumerate() {
            let p = Vec3::from(l.pos);
            let lo = ((p - l.radius) / LIGHT_CELL).floor().as_ivec3();
            let hi = ((p + l.radius) / LIGHT_CELL).floor().as_ivec3();
            for x in lo.x..=hi.x {
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        // cells the sphere reaches (nearest point of the cell within the radius)
                        let cmin = Vec3::new(x as f32, y as f32, z as f32) * LIGHT_CELL;
                        if p.clamp(cmin, cmin + LIGHT_CELL).distance_squared(p) <= l.radius * l.radius {
                            let h = hash([x, y, z]);
                            if lists[h].last() != Some(&(i as u32)) {
                                lists[h].push(i as u32);
                            }
                        }
                    }
                }
            }
        }
        let mut grid = vec![[0u32; 2]; LIGHT_TABLE];
        let mut idx: Vec<u32> = vec![];
        for (h, l) in lists.iter().enumerate() {
            let n = l.len().min(LIGHT_IDX.saturating_sub(idx.len()));
            grid[h] = [idx.len() as u32, n as u32];
            idx.extend_from_slice(&l[..n]);
        }
        if idx.is_empty() {
            idx.push(0);
        }
        self.n_lights.set(lights.len() as u32);
        if !lights.is_empty() {
            gpu.queue.write_buffer(&self.lights, 0, bytemuck::cast_slice(lights));
        }
        gpu.queue.write_buffer(&self.light_grid, 0, bytemuck::cast_slice(&grid));
        gpu.queue.write_buffer(&self.light_idx, 0, bytemuck::cast_slice(&idx));
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
            timestamp_writes: self.prof.as_ref().and_then(|p| p.render("shadow")),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_pipeline(&self.shadow_pipeline);
        p.set_bind_group(0, &cascade.bind, &[]);
        p.set_vertex_buffer(0, scene.vbuf.slice(..));
        p.set_index_buffer(scene.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        p.draw_indexed(0..scene.n_idx, 0, 0..1);
        p.set_pipeline(&self.tex_shadow_pipeline);
        for b in batches.iter().filter(|b| !b.transparent) {
            p.set_bind_group(1, &b.material.bind, &[]);
            p.set_vertex_buffer(0, b.vb.slice(..));
            p.set_vertex_buffer(1, b.inst.slice(..));
            p.set_index_buffer(b.ib.slice(..), wgpu::IndexFormat::Uint32);
            p.draw_indexed(0..b.n_idx, 0, 0..b.n_inst);
        }
    }

    /// Writes the frame's globals and cascade matrices; returns the far and near light view-projections.
    fn write_globals(&self, gpu: &Gpu, scene: &Scene, cam: &Camera, t: &Targets, near_focus: Vec3) -> (Mat4, Mat4) {
        let (lf, ln) = (self.light_far(scene), self.light_near(near_focus));
        let v4 = |r: f32, g: f32, b: f32, w: f32| [r, g, b, w];
        let gl = Globals {
            view_proj: cam.view_proj.to_cols_array_2d(),
            light_far: lf.to_cols_array_2d(),
            light_near: ln.to_cols_array_2d(),
            inv_view_proj: cam.view_proj.inverse().to_cols_array_2d(),
            sun_dir: [self.sun_dir.x, self.sun_dir.y, self.sun_dir.z, self.exposure.get()],
            sun_col: v4(SUN_COL[0], SUN_COL[1], SUN_COL[2], 0.0),
            sky_zenith: v4(0.16, 0.34, 0.78, 0.0),
            sky_horizon: v4(0.62, 0.74, 0.90, 0.0),
            ground_col: v4(0.42, 0.37, 0.30, 0.0),
            eye: [cam.eye.x, cam.eye.y, cam.eye.z, cam.fog],
            misc: v4(1.15, self.n_lights.get() as f32, if std::env::var("FPS_PROBE_DEBUG").is_ok() && self.encode.get() == 0.0 { 1.0 } else { 0.0 }, self.encode.get()),
            probe_min: self.probe.get()[0],
            probe_max: self.probe.get()[1],
            probe_pos: self.probe.get()[2],
            gi_min: self.gi.params.get()[0],
            gi_dim: self.gi.params.get()[1],
            prev_vp: t.prev.get().0.to_cols_array_2d(),
            prev_eye: {
                let (_, e, valid) = t.prev.get();
                let on = valid && self.encode.get() == 0.0 && std::env::var("FPS_NO_SSR").is_err();
                [e.x, e.y, e.z, if on { 1.0 } else { 0.0 }]
            },
            white: {
                let w = self.white.get();
                // w: 1 with temporal super resolution (0 without): AO, GI, shadows and reflections take fewer samples on
                // fixed patterns (patterns that change every frame read as flicker: TSR's clip box rejects the history
                // over whole flat regions)
                [w[0], w[1], w[2], t.taa.as_ref().map_or(0.0, |_| 1.0)]
            },
        };
        gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&gl));
        gpu.queue.write_buffer(&self.far.uniform, 0, bytemuck::cast_slice(&lf.to_cols_array()));
        gpu.queue.write_buffer(&self.near.uniform, 0, bytemuck::cast_slice(&ln.to_cols_array()));
        (lf, ln)
    }

    /// Ambient occlusion from the depth, composite onto the output, and keep this frame as reflection history.
    fn post(&self, enc: &mut wgpu::CommandEncoder, cam: &Camera, t: &Targets, out: &wgpu::TextureView) {
        for (pipe, view, group) in [(&self.ao_pipeline, &t.ao, &t.post_ao), (&self.composite_pipeline, out, &t.post)] {
            let att = |view| Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } });
            let composite = std::ptr::eq(pipe, &self.composite_pipeline);
            let atts = [att(view), att(&t.hist_out)];
            let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post"),
                color_attachments: if composite { &atts[..] } else { &atts[..1] },
                depth_stencil_attachment: None,
                timestamp_writes: self.prof.as_ref().and_then(|p| p.render("post")),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            p.set_pipeline(pipe);
            p.set_bind_group(0, &self.bind, &[]);
            p.set_bind_group(1, group, &[]);
            p.draw(0..3, 0..1);
        }
        // this frame's composite becomes the next frame's reflection history
        enc.copy_texture_to_texture(t.hist_out_tex.as_image_copy(), t.hist_tex.as_image_copy(), wgpu::Extent3d { width: t.w, height: t.h, depth_or_array_layers: 1 });
        t.prev.set((cam.view_proj, cam.eye, true));
    }

    /// Records the shadow + main passes. `msaa` is the multisampled colour target, `resolve` the final one.
    pub fn render(&self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, scene: &Scene, batches: &[&Batch], cam: &Camera, t: &Targets, out: &wgpu::TextureView) {
        let depth = &t.depth;
        self.write_globals(gpu, scene, cam, t, cam.focus);
        self.near_key.set((0, [0.0; 3], [0; 3]));
        if std::env::var("FPS_BENCH_NO_SHADOW").is_err() {
            self.shadow_pass(enc, &self.far, scene, batches);
            self.shadow_pass(enc, &self.near, scene, batches);
        }
        let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(t.main_color())],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: self.prof.as_ref().and_then(|p| p.render("main")),
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
        for (pipe, glass) in [(&self.tex_pipeline, false), (&self.glass_pipeline, true)] {
            p.set_pipeline(pipe);
            p.set_bind_group(2, &t.hist_bind, &[]);
            for b in batches.iter().filter(|b| b.transparent == glass) {
                p.set_bind_group(1, &b.material.bind, &[]);
                p.set_vertex_buffer(0, b.vb.slice(..));
                p.set_vertex_buffer(1, b.inst.slice(..));
                p.set_index_buffer(b.ib.slice(..), wgpu::IndexFormat::Uint32);
                p.draw_indexed(0..b.n_idx, 0, 0..b.n_inst);
            }
        }
        drop(p);
        self.post(enc, cam, t, out);
        // the plain path redraws the far cascade itself
        self.far_key.set((0, [0.0; 3]));
    }

    /// The culled frame: GPU visibility and level of detail per instance (frustum, projected size, last frame's
    /// depth pyramid), indirect draws, and the far shadow map kept while the world and sun stay the same.
    /// `batches` must be the ones `world` was built from.
    pub fn render_culled(&self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, scene: &Scene, world: &CullWorld, batches: &[&Batch], cam: &Camera, t: &Targets, out: &wgpu::TextureView) {
        assert_eq!(world.n_batches, batches.len(), "render_culled: batches differ from the CullWorld's");
        // temporal super resolution: a Halton(2,3) sub-pixel jitter, 8 x the upscale area ratio phases (FSR2's rule)
        let unjit = cam.view_proj;
        let jit_cam;
        let (cam, jit_px) = match &t.taa {
            Some(a) => {
                let phases = ((8.0 * (a.out_w * a.out_h) as f32 / (t.w * t.h) as f32).ceil() as u32).clamp(8, 128);
                let i = a.frame.get() % phases + 1;
                // texel i samples the unjittered point i + 0.5 - jit (pixels, y down)
                let jp = [halton(i, 2) - 0.5, halton(i, 3) - 0.5];
                let (jx, jy) = (jp[0] * 2.0 / t.w as f32, -jp[1] * 2.0 / t.h as f32);
                jit_cam = Camera { view_proj: Mat4::from_translation(Vec3::new(jx, jy, 0.0)) * cam.view_proj, eye: cam.eye, focus: cam.focus, fog: cam.fog, proj_y: cam.proj_y };
                (&jit_cam, jp)
            }
            None => (cam, [0.0; 2]),
        };
        // the near cascade is centred on the eye snapped to an 8 m grid and kept until the eye leaves the cell
        // (the world is static), so it is only redrawn about once a second at a walk
        let cell = (cam.eye / NEAR_CELL).round();
        let near_focus = cell * NEAR_CELL;
        let (lf, ln) = self.write_globals(gpu, scene, cam, t, near_focus);
        let q = lod_quality();
        let px_scale = t.h as f32 * 0.5 * cam.proj_y;
        let hiz_on = t.prev_cull.get().filter(|_| std::env::var("FPS_NO_OCCLUSION").is_err());
        let main_planes = cull::planes(&cam.view_proj);
        let view = |planes: [[f32; 4]; 6], scale: f32, ortho: bool, min_px: f32, shadow: bool, hiz: Option<Mat4>| cull::ViewU {
            planes,
            prev_vp: hiz.unwrap_or(Mat4::IDENTITY).to_cols_array_2d(),
            eye: [cam.eye.x, cam.eye.y, cam.eye.z, scale],
            lod: [ortho as u32 as f32, min_px, 1.0 / q, 0.0],
            hiz: [hiz.is_some() as u32 as f32, t.hiz.size[0] as f32, t.hiz.size[1] as f32, t.hiz.mips as f32],
            misc: [shadow as u32, 0, 0, 0],
            lod2: [0.0; 4],
        };
        let near_planes = cull::planes(&ln);
        let far_planes = cull::planes(&lf);
        let sun = self.sun_dir.to_array();
        let far_needed = self.far_key.get() != (world.id, sun) || std::env::var("FPS_BENCH_NO_FARCACHE").is_ok();
        let skip = std::env::var("FPS_BENCH_SKIP").unwrap_or_default();
        let skip = |w: &str| skip.split(',').any(|x| x == w);
        if !skip("cull") {
            world.cull(gpu, self, enc, cull::MAIN, &view(main_planes, px_scale, false, 0.6, false, hiz_on), &t.hiz.full);
        }
        let near_key = (world.id, sun, cell.as_ivec3().to_array());
        let near_needed = self.near_key.get() != near_key || std::env::var("FPS_BENCH_NO_FARCACHE").is_ok();
        if near_needed {
            world.cull(gpu, self, enc, cull::NEAR, &view(near_planes, px_scale, false, 0.6, true, None), &t.hiz.full);
        }
        if far_needed {
            // texel-sized detail: the far map is view independent, so it can be kept
            let texel = 2.0 * (scene.max - scene.min).length() * 0.5 / SHADOW_FAR_RES as f32;
            world.cull(gpu, self, enc, cull::FAR, &view(far_planes, 1.0 / texel, true, 0.5, true, None), &t.hiz.full);
        }
        let shadows = std::env::var("FPS_BENCH_NO_SHADOW").is_err();
        for (v, cascade, planes) in [(cull::NEAR, &self.near, &near_planes), (cull::FAR, &self.far, &far_planes)] {
            if !shadows || (v == cull::FAR && !far_needed) || (v == cull::NEAR && !near_needed) {
                continue;
            }
            let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow culled"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &cascade.view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: self.prof.as_ref().and_then(|p| p.render("shadow culled")),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            p.set_pipeline(&self.shadow_pipeline);
            p.set_bind_group(0, &cascade.bind, &[]);
            p.set_vertex_buffer(0, scene.vbuf.slice(..));
            p.set_index_buffer(scene.ibuf.slice(..), wgpu::IndexFormat::Uint32);
            p.draw_indexed(0..scene.n_idx, 0, 0..1);
            p.set_pipeline(&self.tex_shadow_ids_pipeline);
            p.set_bind_group(2, &world.inst_bind, &[]);
            world.draw(&mut p, batches, v, planes, false);
        }
        if shadows && far_needed {
            self.far_key.set((world.id, sun));
        }
        if shadows && near_needed {
            self.near_key.set(near_key);
        }
        let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main culled"),
            color_attachments: &[Some(t.main_color())],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &t.depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: self.prof.as_ref().and_then(|p| p.render("main culled")),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_bind_group(0, &self.bind, &[]);
        // depth prepass: every opaque surface, depth only
        p.set_pipeline(&self.flat_pre_pipeline);
        p.set_vertex_buffer(0, scene.vbuf.slice(..));
        p.set_index_buffer(scene.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        p.draw_indexed(0..scene.n_idx, 0, 0..1);
        p.set_pipeline(&self.tex_ids_pre_pipeline);
        p.set_bind_group(2, &t.hist_bind, &[]);
        p.set_bind_group(3, &world.inst_bind, &[]);
        if !skip("pre") {
            world.draw(&mut p, batches, cull::MAIN, &main_planes, false);
        }
        // shading: only the nearest surface passes the depth test
        p.set_pipeline(&self.flat_shade_pipeline);
        p.set_vertex_buffer(0, scene.vbuf.slice(..));
        p.set_index_buffer(scene.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        p.draw_indexed(0..scene.n_idx, 0, 0..1);
        p.set_pipeline(if skip("shadefs") { &self.tex_ids_pre_pipeline } else { &self.tex_ids_pipeline });
        if !skip("shade") {
            world.draw(&mut p, batches, cull::MAIN, &main_planes, false);
        }
        p.set_pipeline(&self.sky_late_pipeline);
        p.draw(0..3, 0..1);
        p.set_pipeline(&self.glass_ids_pipeline);
        p.set_bind_group(2, &t.hist_bind, &[]);
        p.set_bind_group(3, &world.inst_bind, &[]);
        world.draw(&mut p, batches, cull::MAIN, &main_planes, true);
        drop(p);
        if !skip("post") {
            self.post(enc, cam, t, t.taa.as_ref().map_or(out, |a| &a.color));
        }
        if let Some(a) = &t.taa {
            let f = a.frame.get();
            let k = |name: &str, d: f32| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
            let u = TsrU {
                inv_vp: cam.view_proj.inverse().to_cols_array_2d(),
                cur_vp: unjit.to_cols_array_2d(),
                prev_vp: a.prev.get().unwrap_or(unjit).to_cols_array_2d(),
                rsz: [t.w as f32, t.h as f32, 1.0 / t.w as f32, 1.0 / t.h as f32],
                osz: [a.out_w as f32, a.out_h as f32, 1.0 / a.out_w as f32, 1.0 / a.out_h as f32],
                jit: [jit_px[0], jit_px[1], a.prev.get().is_none() as u32 as f32, 0.0],
                // Ruinbound's tuning (clip 1.5 sigma still / 0.75 moving at 2 render px/frame, history 4 moving), with a
                // longer still history (24: a static world, no particles; measured half the still flicker of 12)
                k: [k("FPS_TSR_GAMMA", 1.5), k("FPS_TSR_GMOVE", 0.75), k("FPS_TSR_GSPEED", 2.0), k("FPS_TSR_ACC", 24.0)],
                k2: [k("FPS_TSR_ACCMOVE", 4.0), k("FPS_TSR_KN", 2.5), k("FPS_TSR_KW", 2.0), k("FPS_TSR_SHARP", 0.27 * 1.5)],
            };
            gpu.queue.write_buffer(&a.uniform, 0, bytemuck::bytes_of(&u));
            let cur = (f % 2) as usize;
            let att = |view| Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } });
            for (label, pipe, bind, targets) in [
                ("tsr pre", &self.tsr_pre_pipeline, &a.pre_bind, vec![att(&a.s0), att(&a.s1)]),
                ("tsr resolve", &self.tsr_res_pipeline, &a.res_binds[cur], vec![att(&a.hist[1 - cur])]),
                ("tsr present", &self.tsr_present_pipeline, &a.present_binds[cur], vec![att(out)]),
            ] {
                let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some(label), color_attachments: &targets, depth_stencil_attachment: None, timestamp_writes: self.prof.as_ref().and_then(|p| p.render(label)), occlusion_query_set: None, multiview_mask: None });
                p.set_pipeline(pipe);
                p.set_bind_group(0, bind, &[]);
                p.draw(0..3, 0..1);
            }
            a.prev.set(Some(unjit));
            a.frame.set(f + 1);
        }
        // the depth pyramid for next frame's occlusion test
        if !skip("hiz") {
            t.hiz.build(self, enc);
        }
        t.prev_cull.set(Some(cam.view_proj));
    }
}

/// Aerial haze per metre (FPS_HAZE): 0.0005 loses about a tenth of the contrast at 200 m, as in clear-day photos
/// across the ballpark (0.0009 before, which greyed the far stands).
fn haze_density() -> f32 {
    std::env::var("FPS_HAZE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0005)
}

/// Level-of-detail quality (FPS_LOD, default 0.5: a level may be off by up to 2 px; 1 = 1 px, higher = finer).
fn lod_quality() -> f32 {
    std::env::var("FPS_LOD").ok().and_then(|v| v.parse().ok()).unwrap_or(0.5)
}

/// Temporal super resolution state: the render-resolution composite and clip-box stats, ping-pong output-resolution
/// histories (rgb + confidence), and the resolved frame for the present pass.
struct Taa {
    color: wgpu::TextureView,
    /// stats, history[i]: bind groups per history parity (resolve reads hist[i], writes hist[1 - i]; present reads hist[1 - i])
    pre_bind: wgpu::BindGroup,
    res_binds: [wgpu::BindGroup; 2],
    present_binds: [wgpu::BindGroup; 2],
    s0: wgpu::TextureView,
    s1: wgpu::TextureView,
    hist: [wgpu::TextureView; 2],
    uniform: wgpu::Buffer,
    frame: std::cell::Cell<u32>,
    /// Previous frame's unjittered view-projection.
    prev: std::cell::Cell<Option<Mat4>>,
    out_w: u32,
    out_h: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TsrU {
    inv_vp: [[f32; 4]; 4],
    cur_vp: [[f32; 4]; 4],
    prev_vp: [[f32; 4]; 4],
    rsz: [f32; 4],
    osz: [f32; 4],
    jit: [f32; 4],
    k: [f32; 4],
    k2: [f32; 4],
}

fn halton(mut i: u32, b: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while i > 0 {
        f /= b as f32;
        r += f * (i % b) as f32;
        i /= b;
    }
    r
}

/// MSAA colour + depth targets for a given size.
pub struct Targets {
    pub msaa: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    /// Resolved main pass, before ambient occlusion.
    pub scene: wgpu::TextureView,
    pub ao: wgpu::TextureView,
    post: wgpu::BindGroup,
    /// Same as `post` without the AO texture (the AO pass writes it).
    post_ao: wgpu::BindGroup,
    /// Reflection history read by the main pass (last frame) and written by the composite (this frame).
    hist_tex: wgpu::Texture,
    hist_out_tex: wgpu::Texture,
    hist_out: wgpu::TextureView,
    hist_bind: wgpu::BindGroup,
    /// Previous frame's view-projection and eye, and whether the history holds a frame yet.
    prev: std::cell::Cell<(Mat4, Vec3, bool)>,
    /// Depth pyramid of the last culled frame, and that frame's view-projection.
    hiz: cull::Hiz,
    prev_cull: std::cell::Cell<Option<Mat4>>,
    /// Multisampled main pass (else it renders straight into `scene`).
    ms: bool,
    /// Temporal upscaling state (`with_upscale`).
    taa: Option<Taa>,
    pub w: u32,
    pub h: u32,
}

impl Targets {
    /// The main pass colour attachment: multisampled and resolved into `scene`, or `scene` itself.
    fn main_color(&self) -> wgpu::RenderPassColorAttachment<'_> {
        let (view, resolve_target, store) = if self.ms { (&self.msaa, Some(&self.scene), wgpu::StoreOp::Discard) } else { (&self.scene, None, wgpu::StoreOp::Store) };
        wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store } }
    }
}

impl Targets {
    pub fn new(gpu: &Gpu, r: &Renderer, w: u32, h: u32) -> Self {
        Self::new_ao(gpu, r, w, h, 1)
    }

    /// `ao_div`: ambient occlusion at 1/ao_div of the resolution (2 with temporal accumulation).
    fn new_ao(gpu: &Gpu, r: &Renderer, w: u32, h: u32, ao_div: u32) -> Self {
        let mk = |format, label, samples, usage| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let rt = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let rtt = rt | wgpu::TextureUsages::TEXTURE_BINDING;
        let msaa = mk(r.color_format, "msaa", r.samples, rt);
        let depth = mk(DEPTH, "depth", r.samples, rtt);
        let scene = mk(r.color_format, "scene", 1, rtt);
        let ao = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("ao"),
                size: wgpu::Extent3d { width: w.div_ceil(ao_div), height: h.div_ceil(ao_div), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: AO_FORMAT,
                usage: rtt,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let bind = |ao: &wgpu::TextureView| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post"),
                layout: &r.post_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&depth) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&scene) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(ao) },
                ],
            })
        };
        let dummy = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("ao dummy"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: AO_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let post = bind(&ao);
        let post_ao = bind(&dummy);
        let hist_mk = |label, usage| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HIST_FORMAT,
                usage,
                view_formats: &[],
            })
        };
        let hist_tex = hist_mk("history", wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        let hist_out_tex = hist_mk("history out", wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let hist_out = hist_out_tex.create_view(&Default::default());
        let hist_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("history"),
            layout: &r.hist_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&hist_tex.create_view(&Default::default())) }],
        });
        let hiz = cull::Hiz::new(gpu, r, &depth, w, h);
        Self { msaa, ms: r.samples > 1, depth, scene, ao, post, post_ao, hist_tex, hist_out_tex, hist_out, hist_bind, prev: std::cell::Cell::new((Mat4::IDENTITY, Vec3::ZERO, false)), hiz, prev_cull: std::cell::Cell::new(None), taa: None, w, h }
    }

    /// Targets rendering at `scale` of an output size, temporally upscaled to it (needs a 1-sample renderer).
    pub fn with_upscale(gpu: &Gpu, r: &Renderer, out_w: u32, out_h: u32, scale: f32) -> Self {
        assert_eq!(r.samples, 1, "temporal upscaling replaces MSAA");
        let (w, h) = (((out_w as f32 * scale).round() as u32).max(1), ((out_h as f32 * scale).round() as u32).max(1));
        let mut t = Self::new_ao(gpu, r, w, h, 2);
        let tex = |label, w, h, format, usage| {
            gpu.device.create_texture(&wgpu::TextureDescriptor { label: Some(label), size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] }).create_view(&Default::default())
        };
        let use_rt = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let color = tex("tsr colour", w, h, r.color_format, use_rt);
        let s0 = tex("tsr stats 0", w, h, TAA_HIST, use_rt);
        let s1 = tex("tsr stats 1", w, h, TAA_HIST, use_rt);
        let hist = [tex("tsr history a", out_w, out_h, TAA_HIST, use_rt), tex("tsr history b", out_w, out_h, TAA_HIST, use_rt)];
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("tsr"), size: std::mem::size_of::<TsrU>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        fn tv(b: u32, v: &wgpu::TextureView) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding: b, resource: wgpu::BindingResource::TextureView(v) }
        }
        let ub = wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() };
        let bg = |label, layout, entries: &[wgpu::BindGroupEntry]| gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout, entries });
        let pre_bind = bg("tsr pre", &r.tsr_pre_bgl, &[ub.clone(), tv(1, &color), tv(2, &t.depth)]);
        let res = |i: usize| bg("tsr resolve", &r.tsr_res_bgl, &[ub.clone(), tv(1, &color), tv(3, &s0), tv(4, &s1), tv(5, &hist[i]), wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&r.probe_down_samp) }]);
        let present = |i: usize| bg("tsr present", &r.tsr_present_bgl, &[ub.clone(), tv(7, &hist[1 - i])]);
        let (res_binds, present_binds) = ([res(0), res(1)], [present(0), present(1)]);
        t.taa = Some(Taa { color, pre_bind, res_binds, present_binds, s0, s1, hist, uniform, frame: std::cell::Cell::new(0), prev: std::cell::Cell::new(None), out_w, out_h });
        t
    }
}

/// Renders one frame off-screen and reads it back as an RGBA image.
/// Scene luminance key of a rendered image: the log-average luminance before exposure, from the display image
/// through the inverse tonemap (ignoring the darkest and brightest 3% of pixels: black gaps, lamps, windows).
pub fn scene_key(img: &image::RgbaImage, exposure: f32, white: [f32; 3]) -> f32 {
    let lin = |c: u8| {
        let v = c as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let untone = |a: f32| {
        let y = a.clamp(0.0, 0.98);
        let (qa, qb, qc) = (2.43 * y - 2.51, 0.59 * y - 0.03, 0.14 * y);
        (-qb - (qb * qb - 4.0 * qa * qc).max(0.0).sqrt()) / (2.0 * qa)
    };
    let wl = 0.2126 * white[0] + 0.7152 * white[1] + 0.0722 * white[2];
    let mut l: Vec<f32> = img
        .pixels()
        .step_by(7)
        .map(|p| {
            let d = 0.2126 * lin(p[0]) + 0.7152 * lin(p[1]) + 0.0722 * lin(p[2]);
            (untone(d) / (exposure * wl)).max(1e-5).ln()
        })
        .collect();
    l.sort_by(f32::total_cmp);
    let (a, b) = (l.len() * 3 / 100, l.len() * 97 / 100);
    (l[a..b].iter().sum::<f32>() / (b - a).max(1) as f32).exp()
}

/// `render_image` with the exposure metered like a camera: a first pass is measured and the exposure set so the
/// scene's log-average luminance lands on `key` (display mid-grey ~0.18 before the tonemap); clamped to `range`.
pub fn render_image_metered(gpu: &Gpu, r: &Renderer, scene: &Scene, batches: &[&Batch], cam: &Camera, w: u32, h: u32, key: f32, range: (f32, f32)) -> (image::RgbaImage, f32) {
    let probe = render_once(gpu, r, scene, batches, cam, w / 2, h / 2, false);
    let k = scene_key(&probe, r.exposure.get(), r.white.get());
    let e = (key / k).clamp(range.0, range.1);
    r.set_exposure(e);
    (render_image(gpu, r, scene, batches, cam, w, h), e)
}

pub fn render_image(gpu: &Gpu, r: &Renderer, scene: &Scene, batches: &[&Batch], cam: &Camera, w: u32, h: u32) -> image::RgbaImage {
    render_once(gpu, r, scene, batches, cam, w, h, std::env::var("FPS_NO_SSR").is_err())
}

fn render_once(gpu: &Gpu, r: &Renderer, scene: &Scene, batches: &[&Batch], cam: &Camera, w: u32, h: u32, ssr: bool) -> image::RgbaImage {
    let format = r.color_format;
    let t = Targets::new(gpu, r, w, h);
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
    let scope_oom = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let scope_val = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    // a first pass fills the reflection history for the second
    if ssr {
        r.render(gpu, &mut enc, scene, batches, cam, &t, &out.create_view(&Default::default()));
    }
    r.render(gpu, &mut enc, scene, batches, cam, &t, &out.create_view(&Default::default()));
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &out, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([enc.finish()]);
    for (name, scope) in [("validation", scope_val), ("out of memory", scope_oom)] {
        if let Some(e) = pollster::block_on(scope.pop()) {
            eprintln!("render_image: GPU {name} error: {e}");
        }
    }
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
