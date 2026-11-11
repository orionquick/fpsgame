//! Voxel flood-fill global illumination (GPU side).
//!
//! The app voxelizes the scene around the player into `GI_DIM` cells (albedo + solid flag) and injects light
//! (sky radiance in open-air cells, sun and lamp light reflected by surface cells); [`Gi::iterate`] then runs
//! Jacobi propagation steps in a compute shader, ping-ponging between two RGBA16F volumes. The main shaders
//! sample the result trilinearly as indirect diffuse light. Steps can be spread over frames: the field is kept,
//! so changes flood outward and settle (dirty updates without bookkeeping).

use crate::Gpu;
use glam::Vec3;

pub const GI_DIM: [u32; 3] = [96, 48, 96];

pub struct Gi {
    vox: wgpu::Texture,
    emit: wgpu::Texture,
    /// Ping-pong light volumes; the shaders read `light[0]` (an even number of steps always ends there).
    _light: [wgpu::Texture; 2],
    pub(crate) view: wgpu::TextureView,
    /// Direct sky light per cell and travel direction (same layout as the light volumes).
    _sky: wgpu::Texture,
    pub(crate) sky_view: wgpu::TextureView,
    step: wgpu::ComputePipeline,
    init: wgpu::ComputePipeline,
    skylight: wgpu::ComputePipeline,
    sky_bind: wgpu::BindGroup,
    /// binds[0]: light[0] -> light[1]; binds[1]: light[1] -> light[0]
    binds: [wgpu::BindGroup; 2],
    /// Globals gi_min (origin, cell size) and gi_dim (dims, enabled).
    pub(crate) params: std::cell::Cell<[[f32; 4]; 2]>,
}

/// `layers`: 1 for per-cell data, 6 for the light volumes (one stack of cells per travel direction).
fn volume(d: &wgpu::Device, label: &str, format: wgpu::TextureFormat, usage: wgpu::TextureUsages, layers: u32) -> wgpu::Texture {
    d.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: GI_DIM[0], height: GI_DIM[1], depth_or_array_layers: GI_DIM[2] * layers },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format,
        usage,
        view_formats: &[],
    })
}

impl Gi {
    pub(crate) fn new(d: &wgpu::Device) -> Self {
        use wgpu::TextureUsages as U;
        let vox = volume(d, "gi vox", wgpu::TextureFormat::Rgba8Unorm, U::TEXTURE_BINDING | U::COPY_DST, 1);
        let emit = volume(d, "gi emit", wgpu::TextureFormat::Rgba32Float, U::TEXTURE_BINDING | U::COPY_DST, 1);
        let light = [0, 1].map(|i| volume(d, &format!("gi light {i}"), wgpu::TextureFormat::Rgba16Float, U::TEXTURE_BINDING | U::STORAGE_BINDING, 6));
        let sky = volume(d, "gi sky", wgpu::TextureFormat::Rgba16Float, U::TEXTURE_BINDING | U::STORAGE_BINDING, 6);
        let tex = |binding, filterable| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable }, view_dimension: wgpu::TextureViewDimension::D3, multisampled: false },
            count: None,
        };
        let bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gi"),
            entries: &[
                tex(0, true),
                tex(1, false),
                tex(2, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba16Float, view_dimension: wgpu::TextureViewDimension::D3 },
                    count: None,
                },
                tex(4, true),
            ],
        });
        let views = light.each_ref().map(|t| t.create_view(&Default::default()));
        let (vv, ev) = (vox.create_view(&Default::default()), emit.create_view(&Default::default()));
        let sky_view = sky.create_view(&Default::default());
        let group = |src: &wgpu::TextureView, dst: &wgpu::TextureView, sky: &wgpu::TextureView| {
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gi"),
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&vv) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&ev) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(src) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(dst) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(sky) },
                ],
            })
        };
        let binds = [group(&views[0], &views[1], &sky_view), group(&views[1], &views[0], &sky_view)];
        // the sky pass writes the sky volume, so it reads a light volume in that slot instead
        let sky_bind = group(&views[0], &sky_view, &views[1]);
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("gi"), source: wgpu::ShaderSource::Wgsl(include_str!("gi.wgsl").into()) });
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("gi"), bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let pipe = |entry: &str| d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(entry), layout: Some(&layout), module: &module, entry_point: Some(entry), compilation_options: Default::default(), cache: None });
        let (step, init, skylight) = (pipe("propagate"), pipe("init"), pipe("skylight"));
        let view = light[0].create_view(&Default::default());
        Self { vox, emit, _light: light, view, _sky: sky, sky_view, step, init, skylight, sky_bind, binds, params: std::cell::Cell::new([[0.0; 4]; 2]) }
    }

    /// Uploads a freshly voxelized field. `reset` restarts the light from the injected values (else the previous
    /// field is kept as the starting point, which converges much faster after small changes).
    pub fn upload(&self, gpu: &Gpu, origin: Vec3, cell: f32, vox: &[[u8; 4]], emit: &[[f32; 4]], reset: bool) {
        let n = (GI_DIM[0] * GI_DIM[1] * GI_DIM[2]) as usize;
        assert!(vox.len() == n && emit.len() == n, "gi field must be GI_DIM cells");
        let size = wgpu::Extent3d { width: GI_DIM[0], height: GI_DIM[1], depth_or_array_layers: GI_DIM[2] };
        let write = |t: &wgpu::Texture, data: &[u8], texel: u32| {
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: t, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(GI_DIM[0] * texel), rows_per_image: Some(GI_DIM[1]) },
                size,
            )
        };
        write(&self.vox, bytemuck::cast_slice(vox), 4);
        write(&self.emit, bytemuck::cast_slice(emit), 16);
        self.params.set([[origin.x, origin.y, origin.z, cell], [GI_DIM[0] as f32, GI_DIM[1] as f32, GI_DIM[2] as f32, if std::env::var("FPS_GI_DEBUG").is_ok() { 2.0 } else { 1.0 }]]);
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut p = enc.begin_compute_pass(&Default::default());
            // direct sky light through the new voxels
            p.set_pipeline(&self.skylight);
            p.set_bind_group(0, &self.sky_bind, &[]);
            p.dispatch_workgroups(GI_DIM[0] / 4, GI_DIM[1] / 4, GI_DIM[2] * 6 / 4);
            if reset {
                // init writes the dst of each bind group: both bounce volumes start dark
                p.set_pipeline(&self.init);
                for b in &self.binds {
                    p.set_bind_group(0, b, &[]);
                    p.dispatch_workgroups(GI_DIM[0] / 4, GI_DIM[1] / 4, GI_DIM[2] * 6 / 4);
                }
            }
        }
        gpu.queue.submit([enc.finish()]);
    }

    /// Records `n` propagation steps (rounded up to even, so the result lands in the volume the shaders read).
    pub fn iterate(&self, enc: &mut wgpu::CommandEncoder, n: u32) {
        self.iterate_ts(enc, n, None);
    }

    pub fn iterate_ts(&self, enc: &mut wgpu::CommandEncoder, n: u32, ts: Option<wgpu::ComputePassTimestampWrites<'_>>) {
        let mut p = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("gi"), timestamp_writes: ts });
        p.set_pipeline(&self.step);
        for i in 0..n.div_ceil(2) * 2 {
            p.set_bind_group(0, &self.binds[(i % 2) as usize], &[]);
            p.dispatch_workgroups(GI_DIM[0] / 4, GI_DIM[1] / 4, GI_DIM[2] * 6 / 4);
        }
    }

    pub fn disable(&self) {
        let mut p = self.params.get();
        p[1][3] = 0.0;
        self.params.set(p);
    }

    pub fn enabled(&self) -> bool {
        self.params.get()[1][3] > 0.5
    }
}
