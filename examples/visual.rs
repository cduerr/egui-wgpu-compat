use std::{sync::Arc, time::Instant};

use egui::{Color32, ColorImage, TextureHandle, TextureOptions, vec2};
use egui_wgpu_compat::{Renderer, RendererOptions, ScreenDescriptor, wgpu};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut App::default())?;
    Ok(())
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            self.state = Some(
                pollster::block_on(State::new(event_loop))
                    .expect("failed to initialize the visual example"),
            );
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if window_id != state.window.id() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size),
            WindowEvent::RedrawRequested => {
                state.render();
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    context: egui::Context,
    managed_texture: TextureHandle,
    native_texture_id: egui::TextureId,
    _native_texture: wgpu::Texture,
    started: Instant,
}

impl State {
    async fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("egui-wgpu-compat — wgpu 30 renderer")
                    .with_inner_size(LogicalSize::new(900, 520)),
            )?,
        );
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("visual example device"),
                ..Default::default()
            })
            .await?;
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface has no supported configuration")?;
        surface.configure(&device, &config);

        let mut renderer = Renderer::new(&device, config.format, RendererOptions::default());
        let context = egui::Context::default();
        let managed_texture = context.load_texture(
            "egui-managed gradient",
            checkerboard([96, 96], Color32::from_rgb(80, 170, 255)),
            TextureOptions::LINEAR,
        );
        let (native_texture, native_view) = native_gradient(&device, &queue);
        let native_texture_id =
            renderer.register_native_texture(&device, &native_view, wgpu::FilterMode::Linear);

        let info = adapter.get_info();
        println!("Rendering with {:?} on {}", info.backend, info.name);

        Ok(Self {
            window,
            surface,
            config,
            device,
            queue,
            renderer,
            context,
            managed_texture,
            native_texture_id,
            _native_texture: native_texture,
            started: Instant::now(),
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width != 0 && size.height != 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&mut self) {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let pixels_per_point = self.window.scale_factor() as f32;
        self.context.set_pixels_per_point(pixels_per_point);
        let screen_size = vec2(
            self.config.width as f32 / pixels_per_point,
            self.config.height as f32 / pixels_per_point,
        );
        let elapsed = self.started.elapsed().as_secs_f32();
        let managed_id = self.managed_texture.id();
        let native_id = self.native_texture_id;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen_size)),
            time: Some(f64::from(elapsed)),
            ..Default::default()
        };
        let output = self.context.run(input, |context| {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(17, 21, 29))
                        .inner_margin(28.0),
                )
                .show(context, |ui| {
                    ui.heading(egui::RichText::new("egui 0.33.3 -> wgpu 30").size(32.0));
                    ui.label(
                        egui::RichText::new("Rendered live by egui-wgpu-compat")
                            .size(18.0)
                            .color(Color32::LIGHT_BLUE),
                    );
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        texture_card(ui, "Ordinary egui texture", managed_id);
                        ui.add_space(18.0);
                        texture_card(ui, "Registered native texture", native_id);
                    });
                    ui.add_space(18.0);
                    let progress = 0.5 + 0.5 * elapsed.sin();
                    ui.add(
                        egui::ProgressBar::new(progress)
                            .animate(true)
                            .text("live render loop"),
                    );
                    ui.add_space(8.0);
                    clipping_demo(ui);
                    ui.add_space(8.0);
                    alpha_demo(ui);
                });
        });

        for (id, delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }
        let primitives = self.context.tessellate(output.shapes, pixels_per_point);
        let screen = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("visual example encoder"),
            });
        let callback_commands = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &primitives,
            &screen,
        );
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("visual example pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &primitives, &screen);
        }
        self.queue
            .submit(callback_commands.into_iter().chain([encoder.finish()]));
        self.queue.present(surface_texture);
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}

fn texture_card(ui: &mut egui::Ui, label: &str, texture_id: egui::TextureId) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(label).strong());
        ui.image((texture_id, vec2(180.0, 180.0)));
    });
}

fn clipping_demo(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Scissor clipping").strong());
    let (rect, _) = ui.allocate_exact_size(vec2(430.0, 46.0), egui::Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 3.0, Color32::from_rgb(29, 38, 51));
    for index in 0..12 {
        let x = rect.left() + index as f32 * 48.0;
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), vec2(30.0, rect.height())),
            0.0,
            if index % 2 == 0 {
                Color32::from_rgb(65, 170, 245)
            } else {
                Color32::from_rgb(245, 105, 125)
            },
        );
    }
    painter.circle_filled(
        egui::pos2(rect.right(), rect.center().y),
        31.0,
        Color32::YELLOW,
    );
    ui.painter().rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(rect.right(), rect.top()),
            vec2(2.0, rect.height()),
        ),
        0.0,
        Color32::WHITE,
    );
    ui.label("The yellow circle is deliberately cut exactly at the white edge.");
}

fn alpha_demo(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Premultiplied-alpha blending").strong());
    let (rect, _) = ui.allocate_exact_size(vec2(430.0, 62.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    const CELL: f32 = 12.0;
    for y in 0..6 {
        for x in 0..36 {
            let min = rect.min + vec2(x as f32 * CELL, y as f32 * CELL);
            painter.rect_filled(
                egui::Rect::from_min_size(min, vec2(CELL, CELL)),
                0.0,
                if (x + y) % 2 == 0 {
                    Color32::from_gray(55)
                } else {
                    Color32::from_gray(100)
                },
            );
        }
    }
    painter.rect_filled(
        egui::Rect::from_min_size(rect.min + vec2(34.0, 7.0), vec2(190.0, 48.0)),
        8.0,
        Color32::from_rgba_unmultiplied(255, 45, 65, 128),
    );
    painter.rect_filled(
        egui::Rect::from_min_size(rect.min + vec2(164.0, 7.0), vec2(190.0, 48.0)),
        8.0,
        Color32::from_rgba_unmultiplied(45, 120, 255, 128),
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "50% red + 50% blue overlap",
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );
}

fn checkerboard(size: [usize; 2], color: Color32) -> ColorImage {
    let mut image = ColorImage::filled(size, Color32::from_rgb(28, 36, 48));
    for y in 0..size[1] {
        for x in 0..size[0] {
            if (x / 12 + y / 12) % 2 == 0 {
                image[(x, y)] = color;
            }
        }
    }
    image
}

fn native_gradient(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    const SIZE: u32 = 96;
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            pixels.extend_from_slice(&[
                (255 * x / (SIZE - 1)) as u8,
                (255 * y / (SIZE - 1)) as u8,
                210,
                255,
            ]);
        }
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("visual example native texture"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * SIZE),
            rows_per_image: Some(SIZE),
        },
        texture.size(),
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
