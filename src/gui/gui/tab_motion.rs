//! # Motion Tab
//!
//! Motion extraction controls using Posy's RGB delay method.

use super::ControlGui;
use crate::core::{BlendMode, MotionParams};
use imgui::Ui;

impl ControlGui {
    /// Build the Motion tab
    pub(super) fn build_motion_tab(&mut self, ui: &Ui) {
        let (mut params, mut enabled) = {
            let state = self.shared_state.lock().unwrap();
            (state.motion_params, state.motion_enabled)
        };

        ui.text("Motion Extraction (Posy RGB Delay)");
        ui.separator();

        // Enable/disable
        if ui.checkbox("Enable Motion Extraction", &mut enabled) {
            let mut state = self.shared_state.lock().unwrap();
            state.motion_enabled = enabled;
        }

        ui.spacing();

        if enabled {
            // === PRESETS ===
            ui.text_colored([1.0, 0.8, 0.0, 1.0], "Presets");
            
            let presets = MotionParams::preset_names();
            for (preset_id, preset_name) in presets {
                if ui.button(preset_name) {
                    params.apply_preset(preset_id);
                    let mut state = self.shared_state.lock().unwrap();
                    state.motion_params = params;
                }
                ui.same_line();
            }
            ui.new_line();
            ui.spacing();
            ui.separator();
            ui.spacing();

            // === CHANNEL DELAYS ===
            ui.text_colored([0.0, 1.0, 1.0, 1.0], "Channel Delays (frames)");
            ui.text_disabled("Delay each RGB channel by N frames");
            
            // Red delay
            ui.text("Red Delay");
            let mut red_delay = params.red_delay as i32;
            if ui.slider_config("##red_delay", 0, 16)
                .display_format("%d frames")
                .build(&mut red_delay)
            {
                params.red_delay = red_delay as u32;
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.red_delay = params.red_delay;
            }

            // Green delay
            ui.text("Green Delay");
            let mut green_delay = params.green_delay as i32;
            if ui.slider_config("##green_delay", 0, 16)
                .display_format("%d frames")
                .build(&mut green_delay)
            {
                params.green_delay = green_delay as u32;
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.green_delay = params.green_delay;
            }

            // Blue delay
            ui.text("Blue Delay");
            let mut blue_delay = params.blue_delay as i32;
            if ui.slider_config("##blue_delay", 0, 16)
                .display_format("%d frames")
                .build(&mut blue_delay)
            {
                params.blue_delay = blue_delay as u32;
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.blue_delay = params.blue_delay;
            }

            ui.spacing();
            ui.separator();
            ui.spacing();

            // === INTENSITY & BLEND ===
            ui.text_colored([0.0, 1.0, 1.0, 1.0], "Mixing");
            
            // Intensity
            ui.text("Effect Intensity");
            if ui.slider_config("##intensity", 0.0, 1.0)
                .display_format("%.2f")
                .build(&mut params.intensity)
            {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.intensity = params.intensity;
            }

            // Input mix
            ui.text("Original Input Mix");
            if ui.slider_config("##input_mix", 0.0, 1.0)
                .display_format("%.2f")
                .build(&mut params.input_mix)
            {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.input_mix = params.input_mix;
            }

            // Blend mode
            ui.text("Blend Mode");
            let blend_modes = BlendMode::all();
            let current_mode = params.blend_mode;
            let mode_names: Vec<&str> = blend_modes.iter().map(|m| m.name()).collect();
            let mut current_idx = blend_modes.iter().position(|&m| m == current_mode).unwrap_or(0);
            
            if ui.combo_simple_string("##blend_mode", &mut current_idx, &mode_names) {
                params.blend_mode = blend_modes[current_idx];
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.blend_mode = params.blend_mode;
            }

            ui.spacing();
            ui.separator();
            ui.spacing();

            // === CHANNEL GAINS ===
            ui.text_colored([0.0, 1.0, 1.0, 1.0], "Channel Gains");
            ui.text_disabled("Adjust individual channel brightness (-2 to 2)");
            
            // Red gain
            ui.text("Red Gain");
            if ui.slider_config("##red_gain", -2.0, 2.0)
                .display_format("%.2f")
                .build(&mut params.red_gain)
            {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.red_gain = params.red_gain;
            }

            // Green gain
            ui.text("Green Gain");
            if ui.slider_config("##green_gain", -2.0, 2.0)
                .display_format("%.2f")
                .build(&mut params.green_gain)
            {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.green_gain = params.green_gain;
            }

            // Blue gain
            ui.text("Blue Gain");
            if ui.slider_config("##blue_gain", -2.0, 2.0)
                .display_format("%.2f")
                .build(&mut params.blue_gain)
            {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.blue_gain = params.blue_gain;
            }

            ui.spacing();
            ui.separator();
            ui.spacing();

            // === OPTIONS ===
            ui.text_colored([0.0, 1.0, 1.0, 1.0], "Options");
            
            // Grayscale input
            if ui.checkbox("Convert to Grayscale First", &mut params.grayscale_input) {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.grayscale_input = params.grayscale_input;
            }
            ui.text_disabled("(Classic Posy method - more distinct trails)");

            ui.spacing();

            // Trail fade
            ui.text("Trail Fade / Gamma");
            if ui.slider_config("##trail_fade", 0.0, 1.0)
                .display_format("%.2f")
                .build(&mut params.trail_fade)
            {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.trail_fade = params.trail_fade;
            }

            // Threshold
            ui.text("Motion Threshold");
            if ui.slider_config("##threshold", 0.0, 1.0)
                .display_format("%.2f")
                .build(&mut params.threshold)
            {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.threshold = params.threshold;
            }
            ui.text_disabled("(0 = off, higher = more background suppression)");

            // Smoothing
            ui.text("Smoothing");
            if ui.slider_config("##smoothing", 0.0, 1.0)
                .display_format("%.2f")
                .build(&mut params.smoothing)
            {
                let mut state = self.shared_state.lock().unwrap();
                state.motion_params.smoothing = params.smoothing;
            }
        }
    }
}
