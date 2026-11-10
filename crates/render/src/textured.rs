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
    /// Edge wear: position on its flat face in metres (u, v) and the face's size (w, h); all zero for meshes
    /// without face edges (the shader then finds edges from curvature).
    pub edge: [f32; 4],
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

pub(crate) const VERTEX_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4, 10 => Float32x4];
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

/// A material's three compressed maps.
pub type CompressedTex3 = [CompressedTex; 3];

/// One block-compressed texture with its mip chain (down to 4x4).
pub struct CompressedTex {
    /// 0 BC1 sRGB, 1 BC3 sRGB, 2 BC1 linear, 3 BC5 linear (RG)
    pub kind: u8,
    pub w: u32,
    pub h: u32,
    pub mips: Vec<Vec<u8>>,
}

impl CompressedTex {
    fn format(&self) -> wgpu::TextureFormat {
        match self.kind {
            0 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
            1 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
            2 => wgpu::TextureFormat::Bc1RgbaUnorm,
            _ => wgpu::TextureFormat::Bc5RgUnorm,
        }
    }
    fn compress(img: &image::RgbaImage, kind: u8) -> Self {
        let fmt = match kind {
            1 => texpresso::Format::Bc3,
            3 => texpresso::Format::Bc5,
            _ => texpresso::Format::Bc1,
        };
        let params = texpresso::Params { algorithm: texpresso::Algorithm::RangeFit, ..Default::default() };
        let (w, h) = img.dimensions();
        let mut level = img.clone();
        let mut mips = vec![];
        loop {
            let (lw, lh) = level.dimensions();
            let mut out = vec![0u8; fmt.compressed_size(lw as usize, lh as usize)];
            fmt.compress(level.as_raw(), lw as usize, lh as usize, params, &mut out);
            mips.push(out);
            if lw <= 4 || lh <= 4 {
                break;
            }
            level = image::imageops::resize(&level, lw / 2, lh / 2, image::imageops::FilterType::Triangle);
        }
        Self { kind, w, h, mips }
    }
}

/// Compresses a material's three maps: base as BC1 (BC3 if it has cut-out alpha), ORM as BC1, normal as BC5.
pub fn compress_material(base: &image::RgbaImage, orm: &image::RgbaImage, normal: &image::RgbaImage) -> [CompressedTex; 3] {
    let alpha = base.pixels().any(|p| p[3] < 250);
    [CompressedTex::compress(base, if alpha { 1 } else { 0 }), CompressedTex::compress(orm, 2), CompressedTex::compress(normal, 3)]
}

/// Serialises compressed maps (for the on-disk texture cache).
pub fn write_compressed(t: &[CompressedTex; 3]) -> Vec<u8> {
    let mut b = b"BCT1".to_vec();
    for x in t {
        b.push(x.kind);
        b.extend(x.w.to_le_bytes());
        b.extend(x.h.to_le_bytes());
        b.extend((x.mips.len() as u32).to_le_bytes());
        for m in &x.mips {
            b.extend((m.len() as u32).to_le_bytes());
            b.extend(m);
        }
    }
    b
}

pub fn read_compressed(b: &[u8]) -> Option<[CompressedTex; 3]> {
    if b.len() < 4 || &b[..4] != b"BCT1" {
        return None;
    }
    let mut o = 4usize;
    let mut u32_at = |o: &mut usize| -> Option<u32> {
        let v = u32::from_le_bytes(b.get(*o..*o + 4)?.try_into().ok()?);
        *o += 4;
        Some(v)
    };
    let mut out = vec![];
    for _ in 0..3 {
        let kind = *b.get(o)?;
        o += 1;
        let w = u32_at(&mut o)?;
        let h = u32_at(&mut o)?;
        let n = u32_at(&mut o)?;
        let mut mips = vec![];
        for _ in 0..n {
            let l = u32_at(&mut o)? as usize;
            mips.push(b.get(o..o + l)?.to_vec());
            o += l;
        }
        out.push(CompressedTex { kind, w, h, mips });
    }
    out.try_into().ok()
}

fn upload_compressed(gpu: &Gpu, t: &CompressedTex) -> wgpu::TextureView {
    let format = t.format();
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("material bc"),
        size: wgpu::Extent3d { width: t.w, height: t.h, depth_or_array_layers: 1 },
        mip_level_count: t.mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let block = if t.kind == 0 || t.kind == 2 { 8 } else { 16 };
    for (mip, data) in t.mips.iter().enumerate() {
        let (lw, lh) = ((t.w >> mip).max(4), (t.h >> mip).max(4));
        let bw = lw.div_ceil(4);
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: mip as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bw * block), rows_per_image: Some(lh.div_ceil(4)) },
            wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
        );
    }
    tex.create_view(&Default::default())
}

impl Material {
    /// A material from block-compressed maps (8x / 4x smaller on the GPU than RGBA8).
    pub fn from_compressed(gpu: &Gpu, r: &crate::Renderer, t: &[CompressedTex; 3]) -> Self {
        let views = [upload_compressed(gpu, &t[0]), upload_compressed(gpu, &t[1]), upload_compressed(gpu, &t[2])];
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
