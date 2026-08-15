//! The official egui-wgpu 0.33 renderer, adapted to wgpu 30.
//!
//! This crate only provides low-level rendering. It does not own or configure a
//! device, queue, surface, window, or event loop.

#![doc = include_str!("../README.md")]

pub use wgpu;

/// Low-level painting of egui on wgpu.
pub mod renderer;

pub use renderer::*;
