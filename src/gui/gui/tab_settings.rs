//! # Settings Tab
//!
//! Application settings including resolution, frame history, and UI scale.

use super::ControlGui;
use crate::core::ResolutionPreset;
use imgui::Ui;

impl ControlGui {
    /// Build the Settings tab
    pub(super) fn build_settings_tab(&mut self, ui: &Ui) {
        ui.text("Application Settings");
        ui.separator();

        let (mut ui_scale, mut resolution_preset, mut history_size) = {
            let state = self.shared_state.lock().unwrap();
            (state.ui_scale, state.resolution_preset, state.history_size)
        };

        // === RESOLUTION ===
        ui.text_colored([0.0, 1.0, 1.0, 1.0], "Output Resolution");
        
        let resolutions = [
            ResolutionPreset::P720,
            ResolutionPreset::P1080,
            ResolutionPreset::P1440,
            ResolutionPreset::P4K,
        ];
        let res_names: Vec<&str> = resolutions.iter().map(|r| r.name()).collect();
        let mut current_res_idx = resolutions.iter().position(|&r| r == resolution_preset).unwrap_or(0);
        
        if ui.combo_simple_string("##resolution", &mut current_res_idx, &res_names) {
            resolution_preset = resolutions[current_res_idx];
            let mut state = self.shared_state.lock().unwrap();
            state.set_resolution_preset(resolution_preset);
        }
        
        // Show current resolution
        let (w, h) = resolution_preset.dimensions();
        ui.text_disabled(format!("{}x{}", w, h));

        ui.spacing();
        ui.separator();
        ui.spacing();

        // === FRAME HISTORY ===
        ui.text_colored([0.0, 1.0, 1.0, 1.0], "Frame History");
        ui.text_disabled("More frames = longer trails, more memory");
        
        let mut history = history_size as i32;
        if ui.slider_config("History Size", 2, 16)
            .display_format("%d frames")
            .build(&mut history)
        {
            history_size = history as usize;
            let mut state = self.shared_state.lock().unwrap();
            state.history_size = history_size;
        }
        
        // Show recommended for current resolution
        let recommended = resolution_preset.recommended_history();
        if history_size != recommended {
            ui.text_disabled(format!("Recommended for {}: {} frames", resolution_preset.name(), recommended));
        }

        ui.spacing();
        ui.separator();
        ui.spacing();

        // === UI SCALE ===
        ui.text_colored([0.0, 1.0, 1.0, 1.0], "UI Scale");
        if ui.slider("Scale", 0.5, 2.0, &mut ui_scale) {
            let mut state = self.shared_state.lock().unwrap();
            state.ui_scale = ui_scale;
        }

        ui.spacing();
        ui.separator();
        ui.spacing();

        ui.text("Keyboard Shortcuts:");
        ui.bullet_text("Shift+F - Toggle Fullscreen");
        ui.bullet_text("Escape - Exit Application");

        ui.spacing();
        ui.separator();
        ui.spacing();

        ui.text("Performance:");
        ui.text_disabled("All textures use native BGRA format for optimal macOS performance.");
        ui.text_disabled("Motion extraction uses frame history for time-delayed RGB effects.");
    }
}
