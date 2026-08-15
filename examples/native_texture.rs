use egui_wgpu_compat::{
    Renderer, RendererOptions, ScreenDescriptor,
    wgpu::{self, TextureFormat},
};
use epaint::{
    ClippedPrimitive, Color32, ColorImage, ImageDelta, Mesh, Primitive, TextureId,
    emath::{Rect, pos2, vec2},
    textures::TextureOptions,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pollster::block_on(run())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // The application owns wgpu. A windowed application would use its existing
    // device, queue, surface, and output view instead.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("egui-wgpu-compat example device"),
            ..Default::default()
        })
        .await?;

    let format = TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&device, format, RendererOptions::default());

    // Floating-point output is also a renderer choice. Tone mapping remains
    // the caller's responsibility.
    let _float_renderer = Renderer::new(
        &device,
        TextureFormat::Rgba16Float,
        RendererOptions {
            dithering: false,
            ..Default::default()
        },
    );

    // An ordinary egui-managed texture.
    let managed_id = TextureId::Managed(0);
    let managed_image = ColorImage::filled([2, 2], Color32::LIGHT_BLUE);
    renderer.update_texture(
        &device,
        &queue,
        managed_id,
        &ImageDelta::full(managed_image, TextureOptions::LINEAR),
    );

    // A caller-owned native wgpu texture registered with egui.
    let native_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("example native texture"),
        size: wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let native_view = native_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let native_id =
        renderer.register_native_texture(&device, &native_view, wgpu::FilterMode::Linear);

    // Paint one quad with each texture into a caller-owned output.
    let paint_jobs = [
        textured_quad(
            managed_id,
            Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 32.0)),
        ),
        textured_quad(
            native_id,
            Rect::from_min_size(pos2(32.0, 0.0), vec2(32.0, 32.0)),
        ),
    ];
    let screen = ScreenDescriptor {
        size_in_pixels: [64, 32],
        pixels_per_point: 1.0,
    };
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("example output"),
        size: wgpu::Extent3d {
            width: 64,
            height: 32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("egui-wgpu-compat example encoder"),
    });

    let callback_commands =
        renderer.update_buffers(&device, &queue, &mut encoder, &paint_jobs, &screen);
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui-wgpu-compat example pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
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
        renderer.render(&mut pass, &paint_jobs, &screen);
    }
    queue.submit(callback_commands.into_iter().chain([encoder.finish()]));

    renderer.free_texture(&managed_id);
    renderer.free_texture(&native_id);
    Ok(())
}

fn textured_quad(texture_id: TextureId, rect: Rect) -> ClippedPrimitive {
    let mut mesh = Mesh::with_texture(texture_id);
    mesh.add_rect_with_uv(
        rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    ClippedPrimitive {
        clip_rect: rect,
        primitive: Primitive::Mesh(mesh),
    }
}
