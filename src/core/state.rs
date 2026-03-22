//! # Shared State
//!
//! Thread-safe state shared between the GUI, renderer, and input/output threads.

use serde::{Deserialize, Serialize};
use crate::core::lfo::LfoState;

// Command enums live in their home modules; re-export here for backward compatibility.
pub use crate::audio::AudioCommand;
pub use crate::input::InputCommand;
pub use crate::midi::MidiCommand;
pub use crate::osc::OscCommand;
pub use crate::output::OutputCommand;
pub use crate::presets::PresetCommand;
pub use crate::web::WebControlCommand as WebCommand;

/// Type of video input source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputType {
    None,
    Webcam,
    #[cfg(feature = "ndi")]
    Ndi,
    #[cfg(target_os = "macos")]
    Syphon,
    #[cfg(target_os = "windows")]
    Spout,
    #[cfg(target_os = "linux")]
    V4l2,
}

impl Default for InputType {
    fn default() -> Self {
        InputType::None
    }
}

impl InputType {
    /// Get display name for UI
    pub fn name(&self) -> &'static str {
        match self {
            InputType::None => "None",
            InputType::Webcam => "Webcam",
            #[cfg(feature = "ndi")]
            InputType::Ndi => "NDI",
            #[cfg(target_os = "macos")]
            InputType::Syphon => "Syphon",
            #[cfg(target_os = "windows")]
            InputType::Spout => "Spout",
            #[cfg(target_os = "linux")]
            InputType::V4l2 => "V4L2",
        }
    }
}

/// Current state of the video input
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// Type of active input
    pub input_type: InputType,
    /// Source name (NDI source, webcam device name, Syphon server)
    pub source_name: String,
    /// Whether input is active and receiving frames
    pub is_active: bool,
    /// Current resolution
    pub width: u32,
    pub height: u32,
    /// Frame rate (if known)
    pub fps: f32,
}

/// Blend modes for motion extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum BlendMode {
    Replace = 0,
    Add = 1,
    Multiply = 2,
    Screen = 3,
    Difference = 4,
    Overlay = 5,
    Lighten = 6,
    Darken = 7,
}

impl BlendMode {
    pub fn name(&self) -> &'static str {
        match self {
            BlendMode::Replace => "Replace",
            BlendMode::Add => "Add",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Difference => "Difference",
            BlendMode::Overlay => "Overlay",
            BlendMode::Lighten => "Lighten",
            BlendMode::Darken => "Darken",
        }
    }
    
    pub fn all() -> &'static [BlendMode] {
        &[
            BlendMode::Replace,
            BlendMode::Add,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Difference,
            BlendMode::Overlay,
            BlendMode::Lighten,
            BlendMode::Darken,
        ]
    }
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::Replace
    }
}

/// Motion extraction parameters (Posy's RGB delay method)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MotionParams {
    /// Enable/disable motion extraction
    pub enabled: bool,
    /// Delay per channel (in frames, 0-16)
    pub red_delay: u32,
    pub green_delay: u32,
    pub blue_delay: u32,
    /// Effect intensity (0-1)
    pub intensity: f32,
    /// Blend mode
    pub blend_mode: BlendMode,
    /// Convert input to grayscale first
    pub grayscale_input: bool,
    /// Individual channel gains (can be negative)
    pub red_gain: f32,
    pub green_gain: f32,
    pub blue_gain: f32,
    /// Input mix amount (how much original to preserve)
    pub input_mix: f32,
    /// Trail fade/gamma adjustment
    pub trail_fade: f32,
    /// Threshold for posterization (0 = off)
    pub threshold: f32,
    /// Smoothing amount (0 = off)
    pub smoothing: f32,
}

impl Default for MotionParams {
    fn default() -> Self {
        Self {
            enabled: true,
            red_delay: 0,      // Current frame
            green_delay: 2,    // 2 frames ago
            blue_delay: 4,     // 4 frames ago
            intensity: 1.0,
            blend_mode: BlendMode::Replace,
            grayscale_input: true,
            red_gain: 1.0,
            green_gain: 1.0,
            blue_gain: 1.0,
            input_mix: 0.0,
            trail_fade: 0.0,
            threshold: 0.0,
            smoothing: 0.0,
        }
    }
}

impl MotionParams {
    /// Reset to default values
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    
    /// Apply a named preset
    pub fn apply_preset(&mut self, preset: &str) {
        match preset {
            "classic_posy" => {
                self.red_delay = 0;
                self.green_delay = 2;
                self.blue_delay = 4;
                self.intensity = 1.0;
                self.grayscale_input = true;
                self.blend_mode = BlendMode::Replace;
            }
            "reverse" => {
                self.red_delay = 4;
                self.green_delay = 2;
                self.blue_delay = 0;
                self.intensity = 1.0;
                self.grayscale_input = true;
                self.blend_mode = BlendMode::Replace;
            }
            "subtle" => {
                self.red_delay = 0;
                self.green_delay = 1;
                self.blue_delay = 2;
                self.intensity = 0.5;
                self.grayscale_input = true;
                self.blend_mode = BlendMode::Add;
            }
            "extreme" => {
                self.red_delay = 0;
                self.green_delay = 4;
                self.blue_delay = 8;
                self.intensity = 1.0;
                self.grayscale_input = false;
                self.blend_mode = BlendMode::Add;
            }
            "rgb_trails" => {
                self.red_delay = 0;
                self.green_delay = 3;
                self.blue_delay = 6;
                self.intensity = 0.8;
                self.grayscale_input = false;
                self.blend_mode = BlendMode::Screen;
            }
            "strobe" => {
                self.red_delay = 0;
                self.green_delay = 0;
                self.blue_delay = 1;
                self.intensity = 1.0;
                self.grayscale_input = false;
                self.blend_mode = BlendMode::Difference;
            }
            _ => {}
        }
    }
    
    /// Get preset names
    pub fn preset_names() -> &'static [(&'static str, &'static str)] {
        &[
            ("classic_posy", "Classic Posy"),
            ("reverse", "Reverse"),
            ("subtle", "Subtle"),
            ("extreme", "Extreme"),
            ("rgb_trails", "RGB Trails"),
            ("strobe", "Strobe"),
        ]
    }
}

/// Audio analysis state
#[derive(Debug, Clone)]
pub struct AudioState {
    /// 8-band FFT values (normalized 0-1)
    pub fft: [f32; 8],
    /// Overall volume/energy (0-1)
    pub volume: f32,
    /// Beat detected this frame
    pub beat: bool,
    /// Estimated BPM
    pub bpm: f32,
    /// Beat phase (0-1)
    pub beat_phase: f32,
    /// Audio processing enabled
    pub enabled: bool,
    /// Amplitude multiplier
    pub amplitude: f32,
    /// Smoothing factor (0-1)
    pub smoothing: f32,
    /// Selected audio input device name
    pub selected_device: Option<String>,
    /// List of available audio devices
    pub available_devices: Vec<String>,
    /// Normalize FFT bands to maximum value
    pub normalize: bool,
    /// Apply +3dB per octave pink noise compensation
    pub pink_noise_shaping: bool,
    /// Tap tempo times (for BPM calculation)
    pub tap_times: Vec<f64>,
    /// Last tap time (for timeout detection)
    pub last_tap_time: f64,
    /// Tap tempo display message
    pub tap_tempo_info: String,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            fft: [0.0; 8],
            volume: 0.0,
            beat: false,
            bpm: 120.0,
            beat_phase: 0.0,
            enabled: true,
            amplitude: 1.0,
            smoothing: 0.5,
            selected_device: None,
            available_devices: Vec::new(),
            normalize: true,
            pink_noise_shaping: false,
            tap_times: Vec::new(),
            last_tap_time: 0.0,
            tap_tempo_info: "Tap to set tempo".to_string(),
        }
    }
}

/// NDI output state
#[derive(Debug, Clone, Default)]
pub struct NdiOutputState {
    /// Output stream name
    pub stream_name: String,
    /// Whether output is active
    pub is_active: bool,
    /// Include alpha channel
    pub include_alpha: bool,
}

/// Syphon output state (macOS only)
#[derive(Debug, Clone, Default)]
pub struct SyphonOutputState {
    /// Server name displayed to clients
    pub server_name: String,
    /// Whether output is enabled
    pub enabled: bool,
}

/// Resolution configuration
#[derive(Debug, Clone)]
pub struct ResolutionState {
    /// Internal processing resolution width
    pub internal_width: u32,
    /// Internal processing resolution height
    pub internal_height: u32,
    /// Input resolution width
    pub input_width: u32,
    /// Input resolution height
    pub input_height: u32,
}

impl Default for ResolutionState {
    fn default() -> Self {
        Self {
            internal_width: 1280,
            internal_height: 720,
            input_width: 1280,
            input_height: 720,
        }
    }
}

/// Performance metrics for output window
#[derive(Debug, Clone, Copy, Default)]
pub struct PerformanceMetrics {
    /// Output window FPS (frames per second)
    pub fps: f32,
    /// Frame time in milliseconds
    pub frame_time_ms: f32,
}

/// Resolution presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionPreset {
    P720,   // 1280x720
    P1080,  // 1920x1080
    P1440,  // 2560x1440
    P4K,    // 3840x2160
}

impl ResolutionPreset {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ResolutionPreset::P720 => (1280, 720),
            ResolutionPreset::P1080 => (1920, 1080),
            ResolutionPreset::P1440 => (2560, 1440),
            ResolutionPreset::P4K => (3840, 2160),
        }
    }
    
    pub fn name(&self) -> &'static str {
        match self {
            ResolutionPreset::P720 => "720p",
            ResolutionPreset::P1080 => "1080p",
            ResolutionPreset::P1440 => "1440p",
            ResolutionPreset::P4K => "4K",
        }
    }
    
    pub fn recommended_history(&self) -> usize {
        match self {
            ResolutionPreset::P720 => 8,
            ResolutionPreset::P1080 => 8,
            ResolutionPreset::P1440 => 7,
            ResolutionPreset::P4K => 6,
        }
    }
}

impl Default for ResolutionPreset {
    fn default() -> Self {
        ResolutionPreset::P720
    }
}

/// Shared state accessible from multiple threads
#[derive(Debug)]
pub struct SharedState {
    // Output window settings
    pub output_fullscreen: bool,
    pub output_width: u32,
    pub output_height: u32,

    // Input state
    pub input: InputState,
    pub input_command: InputCommand,

    // Motion extraction
    pub motion_params: MotionParams,
    pub motion_enabled: bool,

    // Audio analysis
    pub audio: AudioState,
    pub audio_command: AudioCommand,
    pub audio_routing: crate::audio::routing::AudioRoutingState,
    
    // LFO modulation
    pub lfo: LfoState,

    // NDI Output
    pub ndi_output: NdiOutputState,
    pub output_command: OutputCommand,

    // Syphon Output (macOS)
    #[cfg(target_os = "macos")]
    pub syphon_output: SyphonOutputState,

    // Resolution settings
    pub resolution: ResolutionState,
    pub resolution_preset: ResolutionPreset,
    pub history_size: usize,

    // Performance metrics (output FPS)
    pub performance: PerformanceMetrics,

    // UI state
    pub show_preview: bool,
    pub ui_scale: f32,

    // Current GUI tab
    pub current_tab: GuiTab,
    
    // MIDI commands
    pub midi_command: MidiCommand,
    
    // OSC commands
    pub osc_command: OscCommand,
    
    // OSC state (for GUI display)
    pub osc_enabled: bool,
    pub osc_port: u16,
    
    // Preset commands
    pub preset_command: PresetCommand,
    
    // Settings save request flag
    pub save_settings_requested: bool,

    // Background device discovery in progress
    pub input_discovering: bool,
    
    // Web server
    pub web_command: WebCommand,
    pub web_enabled: bool,
    pub web_port: u16,
}

/// GUI tab selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuiTab {
    #[default]
    Input,
    Motion,
    Audio,
    Output,
    Presets,
    Midi,
    Osc,
    Web,
    Settings,
}

impl GuiTab {
    /// Get display name for UI
    pub fn name(&self) -> &'static str {
        match self {
            GuiTab::Input => "Input",
            GuiTab::Motion => "Motion",
            GuiTab::Audio => "Audio",
            GuiTab::Output => "Output",
            GuiTab::Presets => "Presets",
            GuiTab::Midi => "MIDI",
            GuiTab::Osc => "OSC",
            GuiTab::Web => "Web",
            GuiTab::Settings => "Settings",
        }
    }
}

impl SharedState {
    /// Create new shared state with default values
    pub fn new() -> Self {
        Self {
            output_fullscreen: false,
            output_width: 1280,
            output_height: 720,

            input: InputState::default(),
            input_command: InputCommand::None,

            motion_params: MotionParams::default(),
            motion_enabled: true,

            audio: AudioState {
                enabled: true,
                amplitude: 1.0,
                smoothing: 0.5,
                normalize: true,
                pink_noise_shaping: false,
                ..Default::default()
            },
            audio_command: AudioCommand::None,
            audio_routing: crate::audio::routing::AudioRoutingState::new(),

            ndi_output: NdiOutputState {
                stream_name: "RustJay Output".to_string(),
                is_active: false,
                include_alpha: false,
            },
            output_command: OutputCommand::None,

            #[cfg(target_os = "macos")]
            syphon_output: SyphonOutputState {
                server_name: "RustJay".to_string(),
                enabled: false,
            },

            resolution: ResolutionState::default(),
            resolution_preset: ResolutionPreset::default(),
            history_size: ResolutionPreset::default().recommended_history(),
            performance: PerformanceMetrics::default(),

            show_preview: true,
            ui_scale: 1.0,

            current_tab: GuiTab::Input,
            
            midi_command: MidiCommand::None,
            osc_command: OscCommand::None,
            osc_enabled: false,
            osc_port: 9000,
            preset_command: PresetCommand::None,
            save_settings_requested: false,
            
            web_command: WebCommand::None,
            web_enabled: false,
            web_port: 8080,

            lfo: LfoState::new(),

            input_discovering: false,
        }
    }

    /// Toggle fullscreen state
    pub fn toggle_fullscreen(&mut self) {
        self.output_fullscreen = !self.output_fullscreen;
    }

    /// Set output resolution
    pub fn set_output_resolution(&mut self, width: u32, height: u32) {
        self.output_width = width;
        self.output_height = height;
    }
    
    /// Set resolution from preset
    pub fn set_resolution_preset(&mut self, preset: ResolutionPreset) {
        self.resolution_preset = preset;
        let (width, height) = preset.dimensions();
        self.output_width = width;
        self.output_height = height;
        self.history_size = preset.recommended_history();
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}
