//! Windowed first-person mode: walk the city with building collision.
use crate::world::Collision;
use city::City;
use glam::Vec3;
use render::wgpu_reexport as wgpu;
use render::{Camera, Gpu, Renderer, Scene, Targets};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const EYE: f32 = 1.7;
/// GI steps run after a new field lands (a few per frame); the light has settled by then.
const GI_SETTLE: u32 = 400;
const HEIGHT: f32 = 1.8;
const RADIUS: f32 = 0.35;
const WALK: f32 = 4.5;
const SPRINT: f32 = 8.0;
const FLY: f32 = 30.0;
const JUMP: f32 = 5.2;
const GRAVITY: f32 = 20.0;
const STEP: f32 = 0.45;

struct Player {
    pos: Vec3,
    vy: f32,
    yaw: f32,
    pitch: f32,
    grounded: bool,
    fly: bool,
}

struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    renderer: Renderer,
    scene: Scene,
    /// Render targets and the frame at the internal resolution (upscaled to the window).
    targets: Targets,
    frame: wgpu::TextureView,
    batches: Vec<render::Batch>,
    /// GPU instance registry for culling and level of detail (built from `batches`).
    world: render::CullWorld,
    lights: Vec<render::PointLight>,
    probes: Vec<[Vec3; 3]>,
    /// Index of the probe currently baked.
    baked: Option<usize>,
    /// GI voxelizer inputs and where the GI volume was last centred.
    gi_tris: Arc<Vec<crate::gi::Tri>>,
    gi_city: Arc<Vec<crate::gi::Prism>>,
    gi_at: Option<Vec3>,
    /// A GI re-voxelization running on a worker thread (the frame keeps the old field until it lands).
    gi_job: Option<(std::sync::mpsc::Receiver<crate::gi::Field>, Vec3)>,
    /// Propagation steps still to run after the last upload (the static field then stops changing).
    gi_left: u32,
    terrain: Arc<city::model::Terrain>,
}

struct App {
    city: City,
    editor_scene: Option<editor::Scene>,
    /// Playable rectangle from the scene; the player can't leave it.
    bounds: Option<[f32; 4]>,
    collision: Collision,
    gfx: Option<Gfx>,
    player: Player,
    keys: HashSet<KeyCode>,
    grabbed: bool,
    last: Instant,
    title_at: Instant,
    frames: u32,
}

/// Everything the game draws: the static scene, textured batches, lamps, reflection probes, GI inputs.
pub struct Content {
    pub scene: Scene,
    pub batches: Vec<render::Batch>,
    pub lights: Vec<render::PointLight>,
    pub probes: Vec<[Vec3; 3]>,
    pub gi_tris: Vec<crate::gi::Tri>,
    pub gi_city: Vec<crate::gi::Prism>,
}

/// Builds the game's content (shared by `play` and `bench`).
pub fn load(gpu: &Gpu, renderer: &Renderer, city: &City, editor_scene: Option<&editor::Scene>) -> Content {
    let mut lib = crate::library::Library::open(&crate::root());
    let area = editor_scene.and_then(|s| s.bounds).unwrap_or_else(|| city.play_rect());
    let tree_pl = lib.as_ref().map_or(vec![], |l| crate::dressing::trees(l, city, area));
    let base = crate::world::Base::new(city, !tree_pl.is_empty());
    let scene = crate::world::scene(gpu, &base, city, editor_scene);
    let mut batches = vec![];
    if let Some(lib) = lib.as_mut() {
        let report = |what: &str, b: &[render::Batch]| {
            if std::env::var("FPS_BENCH_REPORT").is_ok() {
                let t: u64 = b.iter().map(|b| b.tris()).sum();
                let mut v: Vec<(u64, u32, u64)> = b.iter().map(|b| (b.tris(), b.instances(), b.tris() / b.instances().max(1) as u64)).collect();
                v.sort_by(|a, b| b.0.cmp(&a.0));
                println!("{what}: {} batches {:.2}M tris; top (tris, inst, tris/inst) {:?}", b.len(), t as f64 / 1e6, &v[..v.len().min(6)]);
            }
        };
        batches = crate::world::city_batches(gpu, renderer, lib, &base);
        report("city", &batches);
        let bb = crate::bespoke::all_batches(gpu, renderer, lib, city);
        report("bespoke", &bb);
        batches.extend(bb);
        let tb = crate::library::prefab_batches(gpu, renderer, lib, &tree_pl);
        report("trees", &tb);
        batches.extend(tb);
        let pb = crate::library::prefab_batches(gpu, renderer, lib, &crate::buildings::build(lib, city, area).0);
        report("prefab buildings", &pb);
        batches.extend(pb);
        if let Some(s) = editor_scene {
            let mut pl = crate::world::object_placements(lib, city, s);
            pl.extend(crate::dressing::dress(lib, city, area));
            for (id, v, i) in crate::dressing::markings(lib, city, area) {
                let mat = lib.material(gpu, renderer, &id);
                let yellow = lib.catalog.get(&id).is_some_and(|a| a.title.contains("Yellow"));
                let tint = if yellow { [1.15, 1.05, 0.55] } else { [1.0, 1.0, 1.0] };
                batches.extend(render::Batch::new(gpu, &v, &i, &[render::Instance::new(glam::Mat4::IDENTITY, tint, 1.0)], mat));
            }
            let sb = crate::library::prefab_batches(gpu, renderer, lib, &pl);
            report("scene objects", &sb);
            batches.extend(sb);
        }
    }
    let lights = if crate::bespoke::undecorated() { vec![] } else { crate::bespoke::all_lights(city) };
    let probes = crate::bespoke::all_probes(city);
    let gi_tris = lib.as_ref().map_or(vec![], |l| crate::gi::tris(l, city));
    let gi_city = crate::gi::prisms(city);
    Content { scene, batches, lights, probes, gi_tris, gi_city }
}

/// Output resolution for a window: its logical (point) size, so a Retina window is drawn at a quarter of its
/// pixels and bilinearly scaled up to the screen.
fn output_size(window: &Window, w: u32, h: u32) -> (u32, u32) {
    let s = 1.0 / window.scale_factor().max(1.0);
    (((w as f64 * s).round() as u32).max(1), ((h as f64 * s).round() as u32).max(1))
}

/// Internal render scale of the output (FPS_RENDER_SCALE, default 0.66 per axis): temporal upscaling fills it in.
pub fn render_scale() -> f32 {
    std::env::var("FPS_RENDER_SCALE").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.66).clamp(0.25, 1.0)
}

fn frame_view(gpu: &Gpu, format: wgpu::TextureFormat, w: u32, h: u32) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("frame"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

pub fn run(city: City, scene: Option<editor::Scene>, spawn: (f32, f32), yaw: f32) -> Result<(), Box<dyn std::error::Error>> {
    let collision = Collision::new(&city, scene.as_ref());
    let (spawn, yaw) = scene.as_ref().and_then(|s| s.spawns.first()).map_or((spawn, yaw), |sp| ((sp.x as f32, sp.z as f32), sp.yaw as f32));
    let y = city.terrain.at(spawn.0 as f64, spawn.1 as f64) as f32;
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let bounds = scene.as_ref().and_then(|s| s.bounds).map(|b| [b[0] as f32, b[1] as f32, b[2] as f32, b[3] as f32]);
    let mut app = App {
        city,
        bounds,
        editor_scene: scene,
        collision,
        gfx: None,
        player: Player { pos: Vec3::new(spawn.0, y, spawn.1), vy: 0.0, yaw, pitch: 0.0, grounded: true, fly: false },
        keys: HashSet::new(),
        grabbed: false,
        last: Instant::now(),
        title_at: Instant::now(),
        frames: 0,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

impl App {
    fn grab(&mut self, on: bool) {
        let Some(g) = &self.gfx else { return };
        let ok = if on { g.window.set_cursor_grab(CursorGrabMode::Locked).or_else(|_| g.window.set_cursor_grab(CursorGrabMode::Confined)).is_ok() } else { g.window.set_cursor_grab(CursorGrabMode::None).is_ok() };
        g.window.set_cursor_visible(!on);
        self.grabbed = on && ok;
    }

    fn step(&mut self, dt: f32) {
        let p = &mut self.player;
        let k = |c: KeyCode| self.keys.contains(&c) as i32 as f32;
        let fwd = render::look_dir(p.yaw, if p.fly { p.pitch } else { 0.0 });
        let right = Vec3::new(-fwd.z, 0.0, fwd.x).normalize_or_zero();
        let mut wish = fwd * (k(KeyCode::KeyW) - k(KeyCode::KeyS)) + right * (k(KeyCode::KeyD) - k(KeyCode::KeyA));
        if !p.fly {
            wish.y = 0.0;
        }
        let wish = wish.normalize_or_zero();
        if p.fly {
            let speed = FLY * if self.keys.contains(&KeyCode::ShiftLeft) { 3.0 } else { 1.0 };
            p.pos += (wish + Vec3::Y * (k(KeyCode::Space) - k(KeyCode::ControlLeft))) * speed * dt;
            return;
        }
        let speed = if self.keys.contains(&KeyCode::ShiftLeft) { SPRINT } else { WALK };
        let mut xz = [p.pos.x + wish.x * speed * dt, p.pos.z + wish.z * speed * dt];
        self.collision.push_out(&mut xz, p.pos.y + STEP, HEIGHT - STEP, RADIUS);
        if let Some(b) = self.bounds {
            xz = [xz[0].clamp(b[0] + RADIUS, b[2] - RADIUS), xz[1].clamp(b[1] + RADIUS, b[3] - RADIUS)];
        }
        p.pos.x = xz[0];
        p.pos.z = xz[1];
        if p.grounded && self.keys.contains(&KeyCode::Space) {
            p.vy = JUMP;
            p.grounded = false;
        }
        p.vy -= GRAVITY * dt;
        p.pos.y += p.vy * dt;
        let ground = self.city.terrain.at(p.pos.x as f64, p.pos.z as f64) as f32;
        let floor = self.collision.roof_under(xz, p.pos.y, STEP).map_or(ground, |r| r.max(ground));
        if p.pos.y <= floor {
            p.pos.y = floor;
            p.vy = 0.0;
            p.grounded = true;
        } else if p.pos.y > floor + 0.05 {
            p.grounded = false;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let window = Arc::new(el.create_window(Window::default_attributes().with_title("San Diego FPS").with_inner_size(winit::dpi::LogicalSize::new(1440, 900))).unwrap());
        let instance = wgpu::Instance::new(render::instance_descriptor());
        let surface = instance.create_surface(window.clone()).unwrap();
        let gpu = Gpu::new(instance, Some(&surface));
        let size = window.inner_size();
        let mut config = surface.get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1)).expect("surface unsupported");
        let caps = surface.get_capabilities(&gpu.adapter);
        config.format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        // FPS_NO_VSYNC: present as fast as frames are ready (the title then shows the real frame rate)
        config.present_mode = if std::env::var("FPS_NO_VSYNC").is_ok() { wgpu::PresentMode::AutoNoVsync } else { wgpu::PresentMode::AutoVsync };
        surface.configure(&gpu.device, &config);
        // one sample per pixel: temporal anti-aliasing and upscaling instead of MSAA
        let renderer = Renderer::with_samples(&gpu, config.format, 1);
        let c = load(&gpu, &renderer, &self.city, self.editor_scene.as_ref());
        let (ow, oh) = output_size(&window, config.width, config.height);
        let targets = Targets::with_upscale(&gpu, &renderer, ow, oh, render_scale());
        let frame = frame_view(&gpu, config.format, ow, oh);
        let Content { scene, batches, lights, probes, gi_tris, gi_city } = c;
        let world = render::CullWorld::new(&gpu, &renderer, &batches.iter().collect::<Vec<_>>());
        self.gfx = Some(Gfx { window, surface, config, gpu, renderer, scene, targets, frame, batches, world, lights, probes, baked: None, gi_tris: Arc::new(gi_tris), gi_city: Arc::new(gi_city), gi_at: None, gi_job: None, gi_left: 0, terrain: Arc::new(self.city.terrain.clone()) });
        self.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                if let Some(g) = &mut self.gfx {
                    g.config.width = s.width.max(1);
                    g.config.height = s.height.max(1);
                    g.surface.configure(&g.gpu.device, &g.config);
                    let (ow, oh) = output_size(&g.window, g.config.width, g.config.height);
                    g.targets = Targets::with_upscale(&g.gpu, &g.renderer, ow, oh, render_scale());
                    g.frame = frame_view(&g.gpu, g.config.format, ow, oh);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => self.grab(true),
            WindowEvent::Focused(false) => self.grab(false),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                if event.state == ElementState::Pressed {
                    match code {
                        KeyCode::Escape if self.grabbed => self.grab(false),
                        KeyCode::Escape => el.exit(),
                        KeyCode::KeyF if !event.repeat => {
                            self.player.fly = !self.player.fly;
                            self.player.vy = 0.0;
                        }
                        _ => {}
                    }
                    self.keys.insert(code);
                } else {
                    self.keys.remove(&code);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last).as_secs_f32().min(0.05);
                self.last = now;
                self.step(dt);
                let Some(g) = &mut self.gfx else { return };
                let frame = match g.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    _ => {
                        g.surface.configure(&g.gpu.device, &g.config);
                        return;
                    }
                };
                let view = frame.texture.create_view(&Default::default());
                let p = &self.player;
                let cam = Camera::perspective(p.pos + Vec3::Y * EYE, p.yaw, p.pitch, 70.0, g.config.width as f32 / g.config.height as f32);
                // re-bake the reflection probe when the player moves into another room
                let inside = g.probes.iter().position(|p| cam.eye.cmpge(p[1]).all() && cam.eye.cmple(p[2]).all());
                // flood-fill GI: re-voxelize around the player on entering a room or after moving 12 m; the light
                // keeps converging a few steps every frame (the previous field is the starting point)
                let moved = g.gi_at.is_none_or(|a| a.distance(cam.eye) > 12.0);
                if !g.gi_tris.is_empty() && std::env::var("FPS_NO_GI").is_err() {
                    let want = moved || (inside != g.baked && inside.is_some());
                    if g.gi_at.is_none() {
                        // the first field before the first frame
                        let t = g.terrain.clone();
                        let f = crate::gi::build(&g.gi_tris, &g.gi_city, |x, z| t.at(x as f64, z as f64) as f32, &g.lights, g.renderer.sun_dir, cam.eye);
                        g.renderer.gi.upload(&g.gpu, f.origin, crate::gi::CELL, &f.vox, &f.emit, true);
                        g.renderer.gi_iterate(&g.gpu, 160);
                        g.gi_at = Some(cam.eye);
                    } else if want && g.gi_job.is_none() {
                        let (tx, rx) = std::sync::mpsc::channel();
                        let (tris, city, terrain, lights, sun, eye) = (g.gi_tris.clone(), g.gi_city.clone(), g.terrain.clone(), g.lights.clone(), g.renderer.sun_dir, cam.eye);
                        std::thread::spawn(move || {
                            let _ = tx.send(crate::gi::build(&tris, &city, |x, z| terrain.at(x as f64, z as f64) as f32, &lights, sun, eye));
                        });
                        g.gi_job = Some((rx, eye));
                    }
                    if let Some((rx, eye)) = &g.gi_job {
                        if let Ok(f) = rx.try_recv() {
                            g.renderer.gi.upload(&g.gpu, f.origin, crate::gi::CELL, &f.vox, &f.emit, false);
                            g.renderer.gi_iterate(&g.gpu, 48);
                            g.gi_left = GI_SETTLE;
                            g.gi_at = Some(*eye);
                            g.gi_job = None;
                            // reflections re-bake with the new light
                            g.baked = None;
                        }
                    }
                }
                if inside != g.baked {
                    let refs: Vec<&render::Batch> = g.batches.iter().collect();
                    match inside {
                        Some(i) => {
                            let p = g.probes[i];
                            g.renderer.set_exposure(1.0);
                            g.renderer.set_lights(&g.gpu, &g.lights, p[0]);
                            g.renderer.bake_probe_culled(&g.gpu, &g.scene, &g.world, &refs, p[0], p[1], p[2]);
                            g.renderer.set_exposure(if g.renderer.gi.enabled() { crate::INTERIOR_EXPOSURE } else { 2.2 });
                            g.renderer.set_white_balance(if g.lights.is_empty() { crate::INTERIOR_WHITE } else { crate::LAMPLIT_WHITE });
                        }
                        None => {
                            g.renderer.clear_probe();
                            g.renderer.set_exposure(1.0);
                            g.renderer.set_white_balance([1.0; 3]);
                        }
                    }
                    g.baked = inside;
                }
                g.renderer.set_lights(&g.gpu, &g.lights, cam.eye);
                let mut enc = g.gpu.device.create_command_encoder(&Default::default());
                if g.renderer.gi.enabled() && g.gi_left > 0 {
                    g.renderer.gi_steps(&mut enc, 4);
                    g.gi_left = g.gi_left.saturating_sub(4);
                }
                let refs: Vec<&render::Batch> = g.batches.iter().collect();
                if std::env::var("FPS_NO_CULL").is_ok() {
                    g.renderer.render(&g.gpu, &mut enc, &g.scene, &refs, &cam, &g.targets, &g.frame);
                } else {
                    g.renderer.render_culled(&g.gpu, &mut enc, &g.scene, &g.world, &refs, &cam, &g.targets, &g.frame);
                }
                g.renderer.blit(&g.gpu, &mut enc, &g.frame, &view);
                g.gpu.queue.submit([enc.finish()]);
                g.gpu.queue.present(frame);
                self.frames += 1;
                if now.duration_since(self.title_at).as_secs_f32() > 0.5 {
                    let fps = self.frames as f32 / now.duration_since(self.title_at).as_secs_f32();
                    g.window.set_title(&format!(
                        "San Diego FPS  |  x {:.0}  z {:.0}  y {:.1}  yaw {:.0}  |  {:.0} fps  |  {}  |  click: mouse look, WASD, Shift sprint, Space jump, F fly, Esc",
                        p.pos.x, p.pos.z, p.pos.y, p.yaw.rem_euclid(360.0), fps, if p.fly { "FLY" } else { "WALK" }
                    ));
                    self.title_at = now;
                    self.frames = 0;
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let (DeviceEvent::MouseMotion { delta }, true) = (event, self.grabbed) {
            self.player.yaw += delta.0 as f32 * 0.12;
            self.player.pitch = (self.player.pitch - delta.1 as f32 * 0.12).clamp(-89.0, 89.0);
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(g) = &self.gfx {
            g.window.request_redraw();
        }
    }
}

/// Headless frame timing over the game's own content: `views` are (eye, yaw, pitch); prints ms per frame.
pub fn bench(city: &City, editor_scene: Option<&editor::Scene>, views: &[(Vec3, f32, f32)], w: u32, h: u32, frames: u32) {
    let gpu = Gpu::headless();
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    // like the game: one sample + temporal upscaling (FPS_BENCH_MSAA: the old 4x MSAA at full resolution)
    let msaa = std::env::var("FPS_BENCH_MSAA").is_ok();
    let renderer = if msaa { Renderer::new(&gpu, format) } else { Renderer::with_samples(&gpu, format, 1) };
    let t0 = Instant::now();
    let c = load(&gpu, &renderer, city, editor_scene);
    let tris: u64 = c.batches.iter().map(|b| b.tris()).sum();
    let inst: u64 = c.batches.iter().map(|b| b.instances() as u64).sum();
    println!("loaded in {:.1}s: {} batches, {} instances, {:.2}M textured tris per pass, {} lights", t0.elapsed().as_secs_f32(), c.batches.len(), inst, tris as f64 / 1e6, c.lights.len());
    let targets = if msaa { Targets::new(&gpu, &renderer, w, h) } else { Targets::with_upscale(&gpu, &renderer, w, h, render_scale()) };
    let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench out"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = out.create_view(&Default::default());
    let refs: Vec<&render::Batch> = c.batches.iter().collect();
    let world = render::CullWorld::new(&gpu, &renderer, &refs);
    let culled = std::env::var("FPS_NO_CULL").is_err();
    let wait = || {
        gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).expect("poll");
    };
    for (vi, &(eye, yaw, pitch)) in views.iter().enumerate() {
        let cam = Camera::perspective(eye, yaw, pitch, 70.0, w as f32 / h as f32);
        let inside = c.probes.iter().position(|p| cam.eye.cmpge(p[1]).all() && cam.eye.cmple(p[2]).all());
        let t_gi = Instant::now();
        if !c.gi_tris.is_empty() && std::env::var("FPS_NO_GI").is_err() {
            let terrain = |x: f32, z: f32| city.terrain.at(x as f64, z as f64) as f32;
            let f = crate::gi::build(&c.gi_tris, &c.gi_city, terrain, &c.lights, renderer.sun_dir, cam.eye);
            renderer.gi.upload(&gpu, f.origin, crate::gi::CELL, &f.vox, &f.emit, true);
            renderer.gi_iterate(&gpu, 160);
        }
        wait();
        let gi_ms = t_gi.elapsed().as_secs_f32() * 1000.0;
        let t_probe = Instant::now();
        if let Some(i) = inside {
            let p = c.probes[i];
            renderer.set_lights(&gpu, &c.lights, p[0]);
            if culled {
                renderer.bake_probe_culled(&gpu, &c.scene, &world, &refs, p[0], p[1], p[2]);
            } else {
                renderer.bake_probe(&gpu, &c.scene, &refs, p[0], p[1], p[2]);
            }
            wait();
            println!("   probe bake {:.0} ms", t_probe.elapsed().as_secs_f32() * 1000.0);
        } else {
            renderer.clear_probe();
        }
        let frame = || {
            if let Some(p) = &renderer.prof {
                p.reset();
            }
            renderer.set_lights(&gpu, if std::env::var("FPS_BENCH_NO_LIGHTS").is_ok() { &[] } else { &c.lights }, cam.eye);
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            // steady state: the field has settled and stops iterating (FPS_BENCH_GIITER times the settling frames)
            if renderer.gi.enabled() && std::env::var("FPS_BENCH_GIITER").is_ok() {
                renderer.gi_steps(&mut enc, 4);
            }
            if culled {
                renderer.render_culled(&gpu, &mut enc, &c.scene, &world, &refs, &cam, &targets, &view);
            } else {
                renderer.render(&gpu, &mut enc, &c.scene, &refs, &cam, &targets, &view);
            }
            if let Some(p) = &renderer.prof {
                p.resolve(&mut enc);
            }
            gpu.queue.submit([enc.finish()]);
        };
        println!("   gi rebuild {gi_ms:.0} ms");
        for _ in 0..3 {
            frame();
        }
        wait();
        let t = Instant::now();
        let mut worst = 0.0f32;
        let mut cpu = 0.0f32;
        for _ in 0..frames {
            let f0 = Instant::now();
            frame();
            cpu += f0.elapsed().as_secs_f32();
            wait();
            worst = worst.max(f0.elapsed().as_secs_f32());
        }
        println!("   cpu (encode + submit) {:.2} ms/frame", cpu * 1000.0 / frames as f32);
        if let Some(p) = &renderer.prof {
            frame();
            let t = p.read(&gpu);
            let total: f32 = t.iter().map(|x| x.1).sum();
            println!("   gpu {total:.2} ms: {}", t.iter().map(|(l, ms)| format!("{l} {ms:.2}")).collect::<Vec<_>>().join(", "));
        }
        let ms = t.elapsed().as_secs_f32() * 1000.0 / frames as f32;
        if culled && std::env::var("FPS_BENCH_REPORT").is_ok() {
            for (name, (inst, tris, top)) in ["main", "near", "far"].iter().zip(world.stats(&gpu)) {
                let top: Vec<String> = top.iter().filter(|t| t.3 > 0).map(|(b, l, n, t)| format!("\n      {} lod{l}/{:?} x{n} {:.1}M", refs[*b].label, refs[*b].lod_sizes().iter().map(|i| i / 3).collect::<Vec<_>>(), *t as f64 / 1e6)).collect();
                println!("   {name}: {inst} instances, {:.2}M tris; heaviest {}", tris as f64 / 1e6, top.join(", "));
            }
        }
        let grab = || {
            let row = (w * 4).div_ceil(256) * 256;
            let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("png"), size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: &out, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
            gpu.queue.submit([enc.finish()]);
            buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
            wait();
            let data = buf.slice(..).get_mapped_range().expect("range");
            let mut img = image::RgbaImage::new(w, h);
            for y in 0..h {
                img.as_mut()[(y * w * 4) as usize..((y + 1) * w * 4) as usize].copy_from_slice(&data[(y * row) as usize..(y * row + w * 4) as usize]);
            }
            img
        };
        if let Ok(tag) = std::env::var("FPS_BENCH_PNG") {
            let _ = grab().save(crate::root().join(format!("out/bench_{tag}_{vi}.png")));
        }
        // still-camera flicker (Ruinbound's metric): per-pixel temporal sigma of luma over 32 frames, 8-bit levels
        if std::env::var("FPS_BENCH_FLICKER").is_ok() {
            let n = 32;
            let (mut s1, mut s2) = (vec![0f32; (w * h) as usize], vec![0f32; (w * h) as usize]);
            for _ in 0..n {
                frame();
                let img = grab();
                for (k, p) in img.pixels().enumerate() {
                    let l = 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
                    s1[k] += l;
                    s2[k] += l * l;
                }
            }
            let sig: Vec<f32> = s1.iter().zip(&s2).map(|(a, b)| (b / n as f32 - (a / n as f32).powi(2)).max(0.0).sqrt()).collect();
            let mean = sig.iter().sum::<f32>() / sig.len() as f32;
            let over = sig.iter().filter(|&&x| x > 3.0).count() as f32 / sig.len() as f32 * 100.0;
            println!("   flicker: mean sigma {mean:.2} levels, {over:.1}% of pixels over 3 levels");
            let heat = image::GrayImage::from_fn(w, h, |x, y| image::Luma([(sig[(y * w + x) as usize] * 25.0).min(255.0) as u8]));
            let _ = heat.save(crate::root().join(format!("out/flicker_{vi}.png")));
        }
        println!("view ({:.0},{:.1},{:.0}) yaw {:.0}{}: {:.2} ms/frame ({:.0} fps), worst {:.1} ms", eye.x, eye.y, eye.z, yaw, if inside.is_some() { " [indoor]" } else { "" }, ms, 1000.0 / ms, worst * 1000.0);
    }
}
