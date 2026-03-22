//! # Control GUI
//!
//! Main ImGui interface for controlling the application.

#![allow(deprecated)]

use crate::core::{AudioCommand, GuiTab, InputCommand, OutputCommand, SharedState, MidiCommand, OscCommand, PresetCommand, WebCommand, MotionParams, BlendMode, ResolutionPreset};
use crate::core::lfo::{LfoTarget, Waveform, beat_division_to_hz};
use crate::input::InputManager;
use std::sync::{Arc, Mutex};

/// Main control GUI
pub struct ControlGui {
    shared_state: Arc<Mutex<SharedState>>,

    // Device lists
    webcam_devices: Vec<String>,
    ndi_sources: Vec<String>,
    #[cfg(target_os = "macos")]
    syphon_servers: Vec<crate::input::SyphonServerInfo>,
    audio_devices: Vec<String>,

    // Selection state
    selected_webcam: usize,
    selected_ndi: usize,
    #[cfg(target_os = "macos")]
    selected_syphon: usize,
    selected_audio_device: usize,

    // NDI output name
    #[cfg(feature = "ndi")]
    ndi_output_name: String,

    // Syphon output name (macOS)
    #[cfg(target_os = "macos")]
    syphon_output_name: String,

    // Spout sender list and selection (Windows)
    #[cfg(target_os = "windows")]
    spout_senders: Vec<crate::input::SpoutSenderInfo>,
    #[cfg(target_os = "windows")]
    selected_spout: usize,
    #[cfg(target_os = "windows")]
    spout_output_name: String,

    // V4L2 device path (Linux)
    #[cfg(target_os = "linux")]
    v4l2_device_path: String,

    // Preview texture IDs
    pub input_preview_texture_id: Option<imgui::TextureId>,
    pub output_preview_texture_id: Option<imgui::TextureId>,

    // Preset save dialog buffer
    preset_name_buffer: String,
}

impl ControlGui {
    /// Create a new control GUI
    pub fn new(shared_state: Arc<Mutex<SharedState>>) -> anyhow::Result<Self> {
        let syphon_name = {
            let state = shared_state.lock().unwrap_or_else(|e| e.into_inner());
            #[cfg(target_os = "macos")]
            let syphon = state.syphon_output.server_name.clone();
            #[cfg(not(target_os = "macos"))]
            let syphon = String::new();
            syphon
        };
        #[cfg(feature = "ndi")]
        let ndi_name = {
            let state = shared_state.lock().unwrap_or_else(|e| e.into_inner());
            state.ndi_output.stream_name.clone()
        };

        Ok(Self {
            shared_state,
            webcam_devices: Vec::new(),
            ndi_sources: Vec::new(),
            #[cfg(target_os = "macos")]
            syphon_servers: Vec::new(),
            audio_devices: Vec::new(),
            selected_webcam: 0,
            selected_ndi: 0,
            #[cfg(target_os = "macos")]
            selected_syphon: 0,
            selected_audio_device: 0,
            #[cfg(feature = "ndi")]
            ndi_output_name: ndi_name,
            #[cfg(target_os = "macos")]
            syphon_output_name: syphon_name,
            #[cfg(target_os = "windows")]
            spout_senders: Vec::new(),
            #[cfg(target_os = "windows")]
            selected_spout: 0,
            #[cfg(target_os = "windows")]
            spout_output_name: "RustJay".to_string(),
            #[cfg(target_os = "linux")]
            v4l2_device_path: "/dev/video10".to_string(),
            input_preview_texture_id: None,
            output_preview_texture_id: None,
            preset_name_buffer: String::new(),
        })
    }

    /// Set input preview texture ID
    pub fn set_input_preview_texture(&mut self, texture_id: imgui::TextureId) {
        self.input_preview_texture_id = Some(texture_id);
    }

    /// Set output preview texture ID
    pub fn set_output_preview_texture(&mut self, texture_id: imgui::TextureId) {
        self.output_preview_texture_id = Some(texture_id);
    }

    /// Sync GUI device lists from the current InputManager state
    pub fn update_device_lists(&mut self, input_manager: &InputManager) {
        self.webcam_devices = input_manager.webcam_devices().to_vec();
        self.ndi_sources = input_manager.ndi_sources().to_vec();
        #[cfg(target_os = "macos")]
        {
            self.syphon_servers = input_manager.syphon_servers().to_vec();
        }
        #[cfg(target_os = "windows")]
        {
            self.spout_senders = input_manager.spout_senders().to_vec();
        }

        // Audio device list is fast - refresh synchronously
        self.audio_devices = crate::audio::list_audio_devices();
        log::info!("[GUI] Found {} audio device(s)", self.audio_devices.len());
        for device in &self.audio_devices {
            log::info!("  - {}", device);
        }

        if let Ok(mut state) = self.shared_state.lock() {
            state.audio.available_devices = self.audio_devices.clone();
        }
    }

    /// Build the ImGui UI
    pub fn build_ui(&mut self, ui: &mut imgui::Ui) {
        let window_size = ui.io().display_size;

        // Main control window
        ui.window("RustJay Delta - Controls")
            .position([10.0, 10.0], imgui::Condition::FirstUseEver)
            .size([400.0, window_size[1] - 20.0], imgui::Condition::FirstUseEver)
            .movable(false)
            .collapsible(false)
            .resizable(true)
            .menu_bar(true)
            .build(|| {
                self.build_menu_bar(ui);
                self.build_tabs(ui);
            });

        // Preview windows
        let show_preview = {
            let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
            state.show_preview
        };

        if show_preview {
            let preview_pos = [420.0, 10.0];
            let preview_size = [
                (window_size[0] - preview_pos[0] - 10.0).max(200.0),
                (window_size[1] / 2.0 - 15.0).max(200.0),
            ];

            ui.window("Input Preview")
                .position(preview_pos, imgui::Condition::FirstUseEver)
                .size(preview_size, imgui::Condition::FirstUseEver)
                .build(|| {
                    self.build_input_preview(ui);
                });

            let output_preview_pos = [420.0, window_size[1] / 2.0 + 5.0];
            let output_preview_size = [
                (window_size[0] - output_preview_pos[0] - 10.0).max(200.0),
                (window_size[1] / 2.0 - 15.0).max(200.0),
            ];

            ui.window("Output Preview")
                .position(output_preview_pos, imgui::Condition::FirstUseEver)
                .size(output_preview_size, imgui::Condition::FirstUseEver)
                .build(|| {
                    self.build_output_preview(ui);
                });
        }

        // Audio routing window (conditional)
        let show_routing = {
            let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
            state.audio_routing.show_window
        };

        if show_routing {
            self.build_routing_window(ui);
        }

        // LFO window (conditional)
        let show_lfo = {
            let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
            state.lfo.show_window
        };

        if show_lfo {
            self.build_lfo_window(ui);
        }
    }

    /// Build the menu bar
    fn build_menu_bar(&mut self, ui: &imgui::Ui) {
        ui.menu_bar(|| {
            ui.menu("File", || {
                if ui.menu_item("Exit") {
                    // Signal exit - handled by app
                }
            });

            ui.menu("View", || {
                let show_preview = {
                    let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.show_preview
                };
                if ui.menu_item_config("Show Previews").selected(show_preview).build() {
                    let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.show_preview = !state.show_preview;
                }
            });

            ui.menu("Devices", || {
                if ui.menu_item("Refresh All") {
                    let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.input_command = InputCommand::RefreshDevices;
                }
            });
        });
    }

    /// Build the main tabs
    fn build_tabs(&mut self, ui: &imgui::Ui) {
        let tabs = [
            GuiTab::Input,
            GuiTab::Motion,
            GuiTab::Audio,
            GuiTab::Output,
            GuiTab::Presets,
            GuiTab::Midi,
            GuiTab::Osc,
            GuiTab::Web,
            GuiTab::Settings,
        ];

        let current_tab = {
            let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
            state.current_tab
        };

        if let Some(_tab_bar) = ui.tab_bar("##main_tabs") {
            for tab in &tabs {
                let is_selected = current_tab == *tab;
                if let Some(_tab_item) = ui.tab_item(tab.name()) {
                    if !is_selected {
                        let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                        state.current_tab = *tab;
                    }
                }
            }
        }

        ui.separator();

        // Build content for current tab
        match current_tab {
            GuiTab::Input => self.build_input_tab(ui),
            GuiTab::Motion => self.build_motion_tab(ui),
            GuiTab::Audio => self.build_audio_tab(ui),
            GuiTab::Output => self.build_output_tab(ui),
            GuiTab::Presets => self.build_presets_tab(ui),
            GuiTab::Midi => self.build_midi_tab(ui),
            GuiTab::Osc => self.build_osc_tab(ui),
            GuiTab::Web => self.build_web_tab(ui),
            GuiTab::Settings => self.build_settings_tab(ui),
        }
    }
}

/// Get local IP address helper
pub(super) fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return Some(addr.ip().to_string());
            }
        }
    }
    None
}

mod tab_input;
mod tab_motion;
mod tab_audio;
mod tab_output;
mod tab_presets;
mod tab_settings;
mod tab_control;
mod tab_lfo;
