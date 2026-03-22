use super::ControlGui;
use crate::core::PresetCommand;

impl ControlGui {
    /// Build the Presets tab
    pub(super) fn build_presets_tab(&mut self, ui: &imgui::Ui) {
        ui.text("Quick Presets");
        ui.separator();

        // Quick preset slots (1-8)
        let button_size = [80.0, 60.0];
        let spacing = 8.0;
        let total_width = 4.0 * button_size[0] + 3.0 * spacing;
        let start_x = (ui.window_content_region_max()[0] - ui.window_content_region_min()[0] - total_width) / 2.0;

        ui.new_line();

        for row in 0..2 {
            let y_pos = ui.cursor_screen_pos()[1];

            for col in 0..4 {
                let slot = row * 4 + col + 1;
                let x_pos = start_x + col as f32 * (button_size[0] + spacing);

                ui.set_cursor_screen_pos([x_pos, y_pos]);

                let label = format!("{}", slot);
                let is_active = false; // TODO: Check if slot has preset

                let button_color = if is_active {
                    [0.2, 0.6, 1.0, 1.0]
                } else {
                    [0.3, 0.3, 0.3, 1.0]
                };

                let _style = ui.push_style_color(imgui::StyleColor::Button, button_color);
                if ui.button_with_size(&label, button_size) {
                    let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.preset_command = PresetCommand::ApplySlot(slot);
                }

                if ui.is_item_hovered() {
                    ui.tooltip_text(format!("Quick slot {} (Shift+F{})", slot, slot));
                }

                ui.same_line_with_spacing(0.0, spacing);
            }
            ui.new_line();
        }

        ui.separator();

        // Preset management buttons
        ui.text("Preset Management");

        if ui.button("Save New Preset") {
            // TODO: Open save dialog
            ui.open_popup("save_preset_popup");
        }
        ui.same_line();

        if ui.button("Refresh List") {
            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
            state.preset_command = PresetCommand::Refresh;
        }

        ui.same_line();

        if ui.button("Import") {
            // TODO: Import preset
        }

        ui.same_line();

        if ui.button("Export") {
            // TODO: Export preset
        }

        // Save preset popup
        let mut preset_name_buffer = String::with_capacity(256);
        if ui.modal_popup_config("save_preset_popup")
            .resizable(false)
            .always_auto_resize(true)
            .begin_popup()
            .is_some()
        {
            ui.text("Enter preset name:");

            ui.input_text("##preset_name", &mut preset_name_buffer)
                .build();

            if ui.button("Save") && !preset_name_buffer.is_empty() {
                let name = preset_name_buffer.clone();
                let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                state.preset_command = PresetCommand::Save { name };
                ui.close_current_popup();
            }

            ui.same_line();
            if ui.button("Cancel") {
                ui.close_current_popup();
            }
        }

        ui.separator();

        // Preset list
        ui.text("Available Presets");
        ui.text_disabled("(Click to load, Right-click for options)");

        ui.child_window("presets_list")
            .size([0.0, 200.0])
            .build(|| {
                // TODO: List actual presets from PresetBank
                ui.text_disabled("No presets loaded (PresetBank integration needed)");
            });
    }
}
