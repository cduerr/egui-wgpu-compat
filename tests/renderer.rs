#![cfg(feature = "gpu-tests")]

use std::{
    fmt::Write as _,
    sync::{Mutex, OnceLock, mpsc},
    time::Duration,
};

use egui_wgpu_compat::{Renderer, RendererOptions, ScreenDescriptor, wgpu};
use epaint::{
    ClippedPrimitive, Color32, ColorImage, ImageDelta, Mesh, Primitive, TextureId,
    emath::{Rect, pos2, vec2},
    textures::TextureOptions,
};

const SNAPSHOT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots/visual.rgba");
const EGUI_SNAPSHOT_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots");
const SNAPSHOT_TOLERANCE: u8 = 1;
const UV: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

static GPU: OnceLock<TestGpu> = OnceLock::new();
static TEST_LOCK: Mutex<()> = Mutex::new(());

struct TestGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl TestGpu {
    fn new() -> Self {
        let backends =
            std::env::var("EGUI_WGPU_TEST_BACKEND").map_or(wgpu::Backends::PRIMARY, |backend| {
                match backend.to_ascii_lowercase().as_str() {
                    "vulkan" => wgpu::Backends::VULKAN,
                    "dx12" => wgpu::Backends::DX12,
                    "metal" => wgpu::Backends::METAL,
                    other => panic!("unsupported EGUI_WGPU_TEST_BACKEND: {other}"),
                }
            });
        let force_fallback_adapter = std::env::var("EGUI_WGPU_TEST_FORCE_FALLBACK")
            .is_ok_and(|value| !value.is_empty() && value != "0");
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backends;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .expect("renderer tests require a compatible GPU adapter");
        let info = adapter.get_info();
        eprintln!(
            "renderer tests using {:?} {:?}: {}",
            info.backend, info.device_type, info.name
        );
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("egui-wgpu-compat test device"),
            ..Default::default()
        }))
        .expect("failed to create renderer test device");
        Self { device, queue }
    }
}

fn gpu() -> &'static TestGpu {
    GPU.get_or_init(TestGpu::new)
}

#[test]
fn visual_snapshot() {
    let _guard = TEST_LOCK.lock().expect("test lock poisoned");
    let gpu = gpu();
    let mut renderer = Renderer::new(
        &gpu.device,
        wgpu::TextureFormat::Rgba8Unorm,
        RendererOptions::PREDICTABLE,
    );

    let managed_id = TextureId::Managed(0);
    update_managed_texture(
        gpu,
        &mut renderer,
        managed_id,
        [2, 2],
        vec![Color32::RED, Color32::GREEN, Color32::BLUE, Color32::WHITE],
    );
    let white_id = TextureId::Managed(1);
    update_managed_texture(gpu, &mut renderer, white_id, [1, 1], vec![Color32::WHITE]);

    let (native_texture, native_view) = native_texture(
        gpu,
        [2, 2],
        &[
            255, 255, 0, 255, 0, 255, 255, 255, 255, 0, 255, 255, 0, 0, 0, 255,
        ],
    );
    let native_id =
        renderer.register_native_texture(&gpu.device, &native_view, wgpu::FilterMode::Nearest);

    let mut jobs = vec![
        textured_quad(
            managed_id,
            Rect::from_min_size(pos2(0.0, 0.0), vec2(16.0, 16.0)),
            Rect::EVERYTHING,
            Color32::WHITE,
        ),
        textured_quad(
            native_id,
            Rect::from_min_size(pos2(16.0, 0.0), vec2(16.0, 16.0)),
            Rect::EVERYTHING,
            Color32::WHITE,
        ),
        textured_quad(
            white_id,
            Rect::from_min_size(pos2(0.0, 16.0), vec2(32.0, 16.0)),
            Rect::EVERYTHING,
            Color32::BLUE,
        ),
    ];
    jobs.push(textured_quad(
        white_id,
        Rect::from_min_size(pos2(4.0, 12.0), vec2(16.0, 16.0)),
        Rect::from_min_max(pos2(8.0, 16.0), pos2(18.0, 26.0)),
        Color32::from_rgba_premultiplied(128, 0, 0, 128),
    ));

    let pixels = render(gpu, &mut renderer, &jobs, [32, 32], 1.0);
    drop(native_texture);

    if std::env::var_os("EGUI_WGPU_UPDATE_SNAPSHOT").is_some() {
        write_snapshot(&pixels, [32, 32]);
    }

    let expected = read_snapshot([32, 32]);
    let different = pixels
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .filter(|(actual, expected)| {
            actual
                .iter()
                .zip(expected.iter())
                .any(|(actual, expected)| actual.abs_diff(*expected) > SNAPSHOT_TOLERANCE)
        })
        .count();
    if different != 0 {
        let exact_differences = pixels
            .chunks_exact(4)
            .zip(expected.chunks_exact(4))
            .filter(|(actual, expected)| actual != expected)
            .count();
        panic!(
            "{different} pixels exceed the per-channel tolerance and {exact_differences} differ exactly from {SNAPSHOT_PATH}; regenerate only after visually reviewing the change"
        );
    }
}

#[test]
fn egui_context_snapshots() {
    let _guard = TEST_LOCK.lock().expect("test lock poisoned");

    for (name, pixels_per_point) in [("egui_ui_1x.png", 1.0), ("egui_ui_1_5x.png", 1.5)] {
        let size_in_points = vec2(240.0, 128.0);
        let size_in_pixels = [
            (size_in_points.x * pixels_per_point) as u32,
            (size_in_points.y * pixels_per_point) as u32,
        ];
        let pixels = render_egui_ui(size_in_points, size_in_pixels, pixels_per_point);
        assert_png_snapshot(name, &pixels, size_in_pixels);
    }
}

fn render_egui_ui(
    size_in_points: egui::Vec2,
    size_in_pixels: [u32; 2],
    pixels_per_point: f32,
) -> Vec<u8> {
    let gpu = gpu();
    let mut renderer = Renderer::new(
        &gpu.device,
        wgpu::TextureFormat::Rgba8Unorm,
        RendererOptions::PREDICTABLE,
    );
    let context = egui::Context::default();
    context.set_pixels_per_point(pixels_per_point);
    let image = egui::ColorImage::new(
        [2, 2],
        vec![Color32::RED, Color32::GREEN, Color32::BLUE, Color32::WHITE],
    );
    let texture = context.load_texture("integration image", image, egui::TextureOptions::NEAREST);
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size_in_points)),
        ..Default::default()
    };
    let output = context.run(input, |context| {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_rgb(24, 28, 36))
                    .inner_margin(8.0),
            )
            .show(context, |ui| {
                ui.heading("egui + wgpu 30");
                ui.label("Font atlas, textures, clipping, and tessellation");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.image((texture.id(), vec2(42.0, 42.0)));
                    egui::ScrollArea::vertical()
                        .max_width(78.0)
                        .max_height(42.0)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label("Visible row");
                                ui.colored_label(Color32::LIGHT_BLUE, "Tinted text");
                                ui.label("Clipped row");
                                ui.label("Outside the viewport");
                            });
                        });
                    ui.add(egui::Button::new("Button").fill(Color32::from_rgb(70, 90, 120)));
                });
            });
    });

    for (id, delta) in &output.textures_delta.set {
        renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
    }
    let primitives = context.tessellate(output.shapes, pixels_per_point);
    let pixels = render(
        gpu,
        &mut renderer,
        &primitives,
        size_in_pixels,
        pixels_per_point,
    );
    for id in &output.textures_delta.free {
        renderer.free_texture(id);
    }
    pixels
}

#[test]
fn texture_lifecycle_updates_pixels_and_releases_bindings() {
    let _guard = TEST_LOCK.lock().expect("test lock poisoned");
    let gpu = gpu();
    let mut renderer = Renderer::new(
        &gpu.device,
        wgpu::TextureFormat::Rgba8Unorm,
        RendererOptions::PREDICTABLE,
    );
    let managed_id = TextureId::Managed(7);
    update_managed_texture(gpu, &mut renderer, managed_id, [1, 1], vec![Color32::RED]);
    assert_pixel(
        &render_single_texture(gpu, &mut renderer, managed_id),
        [255, 0, 0, 255],
    );

    renderer.update_texture(
        &gpu.device,
        &gpu.queue,
        managed_id,
        &ImageDelta::partial(
            [0, 0],
            ColorImage::filled([1, 1], Color32::GREEN),
            TextureOptions::NEAREST,
        ),
    );
    assert_pixel(
        &render_single_texture(gpu, &mut renderer, managed_id),
        [0, 255, 0, 255],
    );

    let (blue_texture, blue_view) = native_texture(gpu, [1, 1], &[0, 0, 255, 255]);
    let native_id =
        renderer.register_native_texture(&gpu.device, &blue_view, wgpu::FilterMode::Nearest);
    assert_pixel(
        &render_single_texture(gpu, &mut renderer, native_id),
        [0, 0, 255, 255],
    );

    let (yellow_texture, yellow_view) = native_texture(gpu, [1, 1], &[255, 255, 0, 255]);
    renderer.update_egui_texture_from_wgpu_texture(
        &gpu.device,
        &yellow_view,
        wgpu::FilterMode::Nearest,
        native_id,
    );
    assert_pixel(
        &render_single_texture(gpu, &mut renderer, native_id),
        [255, 255, 0, 255],
    );

    renderer.free_texture(&managed_id);
    renderer.free_texture(&native_id);
    assert!(renderer.texture(&managed_id).is_none());
    assert!(renderer.texture(&native_id).is_none());
    drop((blue_texture, yellow_texture));
}

#[test]
fn growing_buffers_preserves_subsequent_small_frames() {
    let _guard = TEST_LOCK.lock().expect("test lock poisoned");
    let gpu = gpu();
    let mut renderer = Renderer::new(
        &gpu.device,
        wgpu::TextureFormat::Rgba8Unorm,
        RendererOptions::PREDICTABLE,
    );
    let texture_id = TextureId::default();
    update_managed_texture(gpu, &mut renderer, texture_id, [1, 1], vec![Color32::WHITE]);

    let mut mesh = Mesh::default();
    for index in 0..600 {
        let x = (index % 64) as f32;
        let y = (index / 64) as f32;
        mesh.add_rect_with_uv(
            Rect::from_min_size(pos2(x, y), vec2(1.0, 1.0)),
            UV,
            Color32::WHITE,
        );
    }
    assert!(mesh.vertices.len() > 1024);
    assert!(mesh.indices.len() > 3072);
    let large = [ClippedPrimitive {
        clip_rect: Rect::EVERYTHING,
        primitive: Primitive::Mesh(mesh),
    }];
    let pixels = render(gpu, &mut renderer, &large, [64, 16], 1.0);
    let opaque_pixels = pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[3] == 255)
        .count();
    assert_eq!(opaque_pixels, 600);

    let small = [textured_quad(
        texture_id,
        Rect::from_min_size(pos2(0.0, 0.0), vec2(2.0, 2.0)),
        Rect::EVERYTHING,
        Color32::GREEN,
    )];
    let pixels = render(gpu, &mut renderer, &small, [2, 2], 1.0);
    assert!(
        pixels
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 255, 0, 255])
    );
}

#[test]
fn backend_smoke() {
    let _guard = TEST_LOCK.lock().expect("test lock poisoned");
    let gpu = gpu();
    let mut renderer = Renderer::new(
        &gpu.device,
        wgpu::TextureFormat::Rgba8Unorm,
        RendererOptions::PREDICTABLE,
    );
    let texture_id = TextureId::Managed(42);
    update_managed_texture(
        gpu,
        &mut renderer,
        texture_id,
        [1, 1],
        vec![Color32::LIGHT_BLUE],
    );
    let pixels = render_single_texture(gpu, &mut renderer, texture_id);
    assert_pixel(&pixels, Color32::LIGHT_BLUE.to_array());
}

fn update_managed_texture(
    gpu: &TestGpu,
    renderer: &mut Renderer,
    id: TextureId,
    size: [usize; 2],
    pixels: Vec<Color32>,
) {
    renderer.update_texture(
        &gpu.device,
        &gpu.queue,
        id,
        &ImageDelta::full(ColorImage::new(size, pixels), TextureOptions::NEAREST),
    );
}

fn native_texture(
    gpu: &TestGpu,
    size: [u32; 2],
    pixels: &[u8],
) -> (wgpu::Texture, wgpu::TextureView) {
    let extent = wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    };
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("renderer test native texture"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        texture.as_image_copy(),
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * size[0]),
            rows_per_image: Some(size[1]),
        },
        extent,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn textured_quad(
    texture_id: TextureId,
    rect: Rect,
    clip_rect: Rect,
    color: Color32,
) -> ClippedPrimitive {
    let mut mesh = Mesh::with_texture(texture_id);
    mesh.add_rect_with_uv(rect, UV, color);
    ClippedPrimitive {
        clip_rect,
        primitive: Primitive::Mesh(mesh),
    }
}

fn render_single_texture(gpu: &TestGpu, renderer: &mut Renderer, id: TextureId) -> Vec<u8> {
    let jobs = [textured_quad(
        id,
        Rect::from_min_size(pos2(0.0, 0.0), vec2(4.0, 4.0)),
        Rect::EVERYTHING,
        Color32::WHITE,
    )];
    render(gpu, renderer, &jobs, [4, 4], 1.0)
}

fn render(
    gpu: &TestGpu,
    renderer: &mut Renderer,
    jobs: &[ClippedPrimitive],
    size: [u32; 2],
    pixels_per_point: f32,
) -> Vec<u8> {
    let extent = wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    };
    let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("renderer test output"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let screen = ScreenDescriptor {
        size_in_pixels: size,
        pixels_per_point,
    };
    let unpadded_bytes_per_row = 4 * size[0];
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("renderer test readback"),
        size: u64::from(padded_bytes_per_row) * u64::from(size[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("renderer test encoder"),
        });
    let callback_commands =
        renderer.update_buffers(&gpu.device, &gpu.queue, &mut encoder, jobs, &screen);
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("renderer test pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            })
            .forget_lifetime();
        renderer.render(&mut pass, jobs, &screen);
    }
    encoder.copy_texture_to_buffer(
        output.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(size[1]),
            },
        },
        extent,
    );
    let submission = gpu
        .queue
        .submit(callback_commands.into_iter().chain([encoder.finish()]));
    let slice = readback.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).expect("map receiver dropped");
    });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(WAIT_TIMEOUT),
        })
        .expect("GPU readback timed out");
    receiver
        .recv_timeout(WAIT_TIMEOUT)
        .expect("GPU map callback timed out")
        .expect("GPU readback mapping failed");

    let mapped = slice
        .get_mapped_range()
        .expect("failed to access mapped readback buffer");
    let pixels = mapped
        .chunks_exact(padded_bytes_per_row as usize)
        .flat_map(|row| row[..unpadded_bytes_per_row as usize].iter().copied())
        .collect();
    drop(mapped);
    readback.unmap();
    pixels
}

fn assert_pixel(pixels: &[u8], expected: [u8; 4]) {
    assert_eq!(&pixels[4 * 5..4 * 6], expected);
}

fn read_snapshot(size: [u32; 2]) -> Vec<u8> {
    let contents = std::fs::read_to_string(SNAPSHOT_PATH).expect("failed to read visual snapshot");
    let mut words = contents.split_whitespace();
    assert_eq!(words.next(), Some(size[0].to_string().as_str()));
    assert_eq!(words.next(), Some(size[1].to_string().as_str()));
    let pixels: Vec<u8> = words
        .flat_map(|pixel| {
            let value = u32::from_str_radix(pixel, 16).expect("invalid snapshot pixel");
            value.to_be_bytes()
        })
        .collect();
    assert_eq!(pixels.len(), (size[0] * size[1] * 4) as usize);
    pixels
}

fn write_snapshot(pixels: &[u8], size: [u32; 2]) {
    let mut output = format!("{} {}\n", size[0], size[1]);
    for row in pixels.chunks_exact(4 * size[0] as usize) {
        for (index, pixel) in row.chunks_exact(4).enumerate() {
            if index != 0 {
                output.push(' ');
            }
            write!(
                output,
                "{:02x}{:02x}{:02x}{:02x}",
                pixel[0], pixel[1], pixel[2], pixel[3]
            )
            .expect("writing to a String cannot fail");
        }
        output.push('\n');
    }
    std::fs::write(SNAPSHOT_PATH, output).expect("failed to update visual snapshot");
}

fn assert_png_snapshot(name: &str, pixels: &[u8], size: [u32; 2]) {
    let path = std::path::Path::new(EGUI_SNAPSHOT_DIRECTORY).join(name);
    if std::env::var_os("EGUI_WGPU_UPDATE_SNAPSHOT").is_some() {
        image::save_buffer(&path, pixels, size[0], size[1], image::ColorType::Rgba8)
            .expect("failed to update egui snapshot");
    }

    let expected = image::open(&path)
        .expect("failed to read egui snapshot")
        .into_rgba8();
    assert_eq!(expected.dimensions(), (size[0], size[1]));
    let different = pixels
        .chunks_exact(4)
        .zip(expected.as_raw().chunks_exact(4))
        .filter(|(actual, expected)| {
            actual
                .iter()
                .zip(expected.iter())
                .any(|(actual, expected)| actual.abs_diff(*expected) > SNAPSHOT_TOLERANCE)
        })
        .count();
    assert_eq!(
        different,
        0,
        "{different} pixels differ from {} beyond the per-channel tolerance; regenerate only after visually reviewing the change",
        path.display()
    );
}
