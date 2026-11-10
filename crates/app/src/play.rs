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
    targets: Targets,
    batches: Vec<render::Batch>,
    lights: Vec<render::PointLight>,
    probes: Vec<[Vec3; 3]>,
    /// Index of the probe currently baked.
    baked: Option<usize>,
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
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).unwrap();
        let gpu = Gpu::new(instance, Some(&surface));
        let size = window.inner_size();
        let mut config = surface.get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1)).expect("surface unsupported");
        let caps = surface.get_capabilities(&gpu.adapter);
        config.format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);
        let renderer = Renderer::new(&gpu, config.format);
        let mut lib = crate::library::Library::open(&crate::root());
        let tree_pl = lib.as_ref().map_or(vec![], |l| crate::dressing::trees(l, &self.city, self.editor_scene.as_ref().and_then(|s| s.bounds).unwrap_or_else(|| self.city.play_rect())));
        let base = crate::world::Base::new(&self.city, !tree_pl.is_empty());
        let scene = crate::world::scene(&gpu, &base, &self.city, self.editor_scene.as_ref());
        let mut batches = vec![];
        if let Some(lib) = lib.as_mut() {
            batches = crate::world::city_batches(&gpu, &renderer, lib, &base);
            batches.extend(crate::bespoke::all_batches(&gpu, &renderer, lib, &self.city));
            let mut pl = tree_pl;
            pl.extend(crate::buildings::build(lib, &self.city, self.editor_scene.as_ref().and_then(|s| s.bounds).unwrap_or_else(|| self.city.play_rect())).0);
            batches.extend(crate::library::prefab_batches(&gpu, &renderer, lib, &pl));
            if let Some(s) = self.editor_scene.as_ref() {
                let mut pl = crate::world::object_placements(lib, &self.city, s);
                let area = s.bounds.unwrap_or_else(|| self.city.play_rect());
                pl.extend(crate::dressing::dress(lib, &self.city, area));
                for (id, v, i) in crate::dressing::markings(lib, &self.city, area) {
                    let mat = lib.material(&gpu, &renderer, &id);
                    let yellow = lib.catalog.get(&id).is_some_and(|a| a.title.contains("Yellow"));
                    let tint = if yellow { [1.15, 1.05, 0.55] } else { [1.0, 1.0, 1.0] };
                    batches.extend(render::Batch::new(&gpu, &v, &i, &[render::Instance::new(glam::Mat4::IDENTITY, tint, 1.0)], mat));
                }
                    batches.extend(crate::library::prefab_batches(&gpu, &renderer, lib, &pl));
            }
        }
        let targets = Targets::new(&gpu, &renderer, config.width, config.height);
        let lights = crate::bespoke::all_lights(&self.city);
        let probes = crate::bespoke::all_probes(&self.city);
        self.gfx = Some(Gfx { window, surface, config, gpu, renderer, scene, targets, batches, lights, probes, baked: None });
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
                    g.targets = Targets::new(&g.gpu, &g.renderer, g.config.width, g.config.height);
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
                if inside != g.baked {
                    let refs: Vec<&render::Batch> = g.batches.iter().collect();
                    match inside {
                        Some(i) => {
                            let p = g.probes[i];
                            g.renderer.set_exposure(1.0);
                            g.renderer.set_lights(&g.gpu, &g.lights, p[0]);
                            g.renderer.bake_probe(&g.gpu, &g.scene, &refs, p[0], p[1], p[2]);
                            g.renderer.set_exposure(2.2);
                        }
                        None => {
                            g.renderer.clear_probe();
                            g.renderer.set_exposure(1.0);
                        }
                    }
                    g.baked = inside;
                }
                g.renderer.set_lights(&g.gpu, &g.lights, cam.eye);
                let mut enc = g.gpu.device.create_command_encoder(&Default::default());
                g.renderer.render(&g.gpu, &mut enc, &g.scene, &g.batches.iter().collect::<Vec<_>>(), &cam, &g.targets, &view);
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
