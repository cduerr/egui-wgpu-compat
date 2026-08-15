# egui-wgpu-compat

[![Crates.io](https://img.shields.io/crates/v/egui-wgpu-compat.svg)](https://crates.io/crates/egui-wgpu-compat)
[![Documentation](https://docs.rs/egui-wgpu-compat/badge.svg)](https://docs.rs/egui-wgpu-compat)
[![CI](https://github.com/cduerr/egui-wgpu-compat/actions/workflows/ci.yml/badge.svg)](https://github.com/cduerr/egui-wgpu-compat/actions/workflows/ci.yml)

The official egui-wgpu 0.33.3 renderer, adapted to wgpu 30. This is a small
compatibility crate for applications that need to upgrade wgpu independently
from egui.

> Compatibility matrix: **egui/epaint 0.33.x → wgpu 30.x**

This crate is not an alternative backend and is not based on the old
`egui_wgpu_backend` repository. It deliberately tracks the official renderer.

## Usage

```toml
[dependencies]
egui = "0.33.3"
wgpu = "30"
egui_wgpu = { package = "egui-wgpu-compat", version = "0.1" }
```

The alias lets existing renderer imports such as `egui_wgpu::Renderer` and
`egui_wgpu::renderer::ScreenDescriptor` keep working.

The following renderer APIs retain their egui-wgpu 0.33.3 signatures:

- `Renderer::new`
- `Renderer::update_texture` and `Renderer::free_texture`
- `Renderer::update_buffers`
- `Renderer::render` into a caller-owned render pass
- `Renderer::register_native_texture`
- `Renderer::update_egui_texture_from_wgpu_texture`
- the sampler-options variants of the native-texture methods

See `examples/native_texture.rs` for ordinary managed textures and a registered
native wgpu texture.

## Scope

The caller owns and configures the wgpu device, queue, surface, output texture,
window, and event loop. Output format is selected when constructing `Renderer`.
SDR formats are the intended output.

`Rgba16Float` is accepted, but the inherited egui 0.33 shader treats it as a
gamma-encoded framebuffer. It therefore produces gamma-encoded compatibility
output, not linear HDR/scRGB output. HDR applications should render egui to an
SDR intermediate and composite it into the HDR pipeline with an explicit color
conversion and SDR-white mapping.

This crate performs no HDR tone mapping and imposes no surface policy.

The official crate's setup, capture, and optional winit painter modules are
outside this crate's non-owning renderer scope. Renderer paint callbacks remain
available.

Default features use wgpu's native defaults, including Vulkan on Linux, DX12 on
Windows, and Metal on macOS. Disable default features if the application needs
to select wgpu backends itself.

## Provenance and licensing

The renderer and shader are derived from
[egui-wgpu 0.33.3](https://github.com/emilk/egui/tree/0.33.3/crates/egui-wgpu).
The wgpu API adaptations follow the official egui migrations to
[wgpu 28](https://github.com/emilk/egui/commit/41b8f5f4e773543451a99e0c82af2426ecc19c76),
[wgpu 29](https://github.com/emilk/egui/commit/a59e803f2567ad12940280e3c50ad22187c10ae6),
and [wgpu 30](https://github.com/emilk/egui/commit/3fcadda5ba186c9a0f8c1546e2a3fefae6b1e863).

Like egui, this project is available under either the MIT or Apache-2.0
license. See NOTICE for attribution.

Contributions are welcome; see [CONTRIBUTING.md](CONTRIBUTING.md).
