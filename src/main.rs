//! # RustJay Delta
//!
//! A GPU-accelerated motion extraction VJ tool based on Posy's RGB delay technique.
//!
//! ## Features
//! - Single video input with hot-swappable sources (Webcam, NDI, Syphon)
//! - BGRA format throughout for native macOS performance
//! - HSB color manipulation (Hue, Saturation, Brightness)
//! - Real-time audio analysis with FFT and beat detection
//! - NDI and Syphon output
//! - Dual-window architecture (Control + Fullscreen Output)
//! - ImGui-based control interface
//!
//! ## Architecture
//! The application uses a dual-window architecture:
//! - **Control Window**: ImGui-based interface for adjusting settings
//! - **Output Window**: Fullscreen-capable display with hidden cursor
//!
//! ## Keyboard Shortcuts
//! - `Shift+F`: Toggle fullscreen on output window
//! - `Escape`: Exit application

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use env_logger;
use log::info;
use std::sync::{Arc, Mutex};

mod app;
mod audio;
mod config;
mod core;
mod engine;
mod gui;
mod input;
mod midi;
mod osc;
mod output;
mod presets;
#[cfg(target_os = "linux")]
mod v4l2_devices;
mod web;

use core::SharedState;

/// Application entry point
fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_module("wgpu_hal::metal", log::LevelFilter::Warn)
        .filter_module("naga", log::LevelFilter::Warn)
        .filter_module("wgpu_core", log::LevelFilter::Warn)
        .filter_module("winit", log::LevelFilter::Warn)
        .filter_module("tracing::span", log::LevelFilter::Warn)
        .init();

    info!("Starting RustJay Delta v{}", env!("CARGO_PKG_VERSION"));

    // Create shared state
    let shared_state = Arc::new(Mutex::new(SharedState::new()));

    // Run the application
    app::run_app(shared_state)?;

    Ok(())
}
