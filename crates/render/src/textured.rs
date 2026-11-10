//! Textured PBR meshes drawn with per-instance transforms (city surfaces and Megascans prefabs).
use crate::Gpu;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TexVertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub uv: [f32; 2],
    pub tan: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub model: [[f32; 4]; 4],
    /// rgb tint, a = uv scale.
    pub tint: [f32; 4],
    /// x: mode (0 lit, 3 one-sided glazing, 4 emissive with strength y); for lit surfaces y: indoor ambient factor (0 = outdoor), z: indoor lamp light.
    pub params: [f32; 4],
}

impl Instance {
    pub fn new(model: glam::Mat4, tint: [f32; 3], uv_scale: f32) -> Self {
        Self { model: model.to_cols_array_2d(), tint: [tint[0], tint[1], tint[2], uv_scale], params: [0.0; 4] }
    }
    pub fn with_params(mut self, params: [f32; 4]) -> Self {
        self.params = params;
        self
    }
}

pub(crate) const VERTEX_ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
pub(crate) const INSTANCE_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4];

pub(crate) fn layouts() -> [wgpu::VertexBufferLayout<'static>; 2] {
    [
        wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<TexVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &VERTEX_ATTRS },
        wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &INSTANCE_ATTRS },
    ]
}

pub struct Material {
    pub(crate) bind: wgpu::BindGroup,
}

/// Uploads one texture with a full CPU-generated mip chain.
fn upload(gpu: &Gpu, img: &image::RgbaImage, format: wgpu::TextureFormat) -> wgpu::TextureView {
    let (w, h) = img.dimensions();
    let levels = 32 - w.max(h).leading_zeros();
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("material"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut level = img.clone();
    for mip in 0..levels {
        let (lw, lh) = level.dimensions();
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: mip, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &level,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(lw * 4), rows_per_image: Some(lh) },
            wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
        );
        if lw > 1 || lh > 1 {
            level = image::imageops::resize(&level, (lw / 2).max(1), (lh / 2).max(1), image::imageops::FilterType::Triangle);
        }
    }
    tex.create_view(&Default::default())
}

impl Material {
    /// `base` is sRGB colour; `orm` is occlusion/roughness/metal; `normal` is tangent-space, OpenGL convention.
    pub fn new(gpu: &Gpu, r: &crate::Renderer, base: &image::RgbaImage, orm: &image::RgbaImage, normal: &image::RgbaImage) -> Self {
        let views = [upload(gpu, base, wgpu::TextureFormat::Rgba8UnormSrgb), upload(gpu, orm, wgpu::TextureFormat::Rgba8Unorm), upload(gpu, normal, wgpu::TextureFormat::Rgba8Unorm)];
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &r.mat_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&views[0]) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&views[1]) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&views[2]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&r.mat_sampler) },
            ],
        });
        Self { bind }
    }
    /// A plain material that shows the vertex/instance tint (for untextured surfaces).
    pub fn plain(gpu: &Gpu, r: &crate::Renderer, roughness: u8) -> Self {
        let px = |c: [u8; 4]| image::RgbaImage::from_pixel(4, 4, image::Rgba(c));
        Self::new(gpu, r, &px([255, 255, 255, 255]), &px([255, roughness, 0, 255]), &px([128, 128, 255, 255]))
    }
}

/// One mesh with one material, drawn once per instance.
pub struct Batch {
    pub(crate) vb: wgpu::Buffer,
    pub(crate) ib: wgpu::Buffer,
    pub(crate) n_idx: u32,
    pub(crate) inst: wgpu::Buffer,
    pub(crate) n_inst: u32,
    pub(crate) material: std::sync::Arc<Material>,
    /// Drawn after opaque geometry with blending, casts no shadow (glazing).
    pub transparent: bool,
}

impl Batch {
    pub fn transparent(mut self) -> Self {
        self.transparent = true;
        self
    }
    pub fn new(gpu: &Gpu, verts: &[TexVertex], idx: &[u32], instances: &[Instance], material: std::sync::Arc<Material>) -> Option<Self> {
        if verts.is_empty() || idx.is_empty() || instances.is_empty() {
            return None;
        }
        let mk = |c: &[u8], usage| gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("batch"), contents: c, usage });
        Some(Self {
            vb: mk(bytemuck::cast_slice(verts), wgpu::BufferUsages::VERTEX),
            ib: mk(bytemuck::cast_slice(idx), wgpu::BufferUsages::INDEX),
            n_idx: idx.len() as u32,
            inst: mk(bytemuck::cast_slice(instances), wgpu::BufferUsages::VERTEX),
            n_inst: instances.len() as u32,
            material,
            transparent: false,
        })
    }
}
