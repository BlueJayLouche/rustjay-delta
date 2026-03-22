use super::ControlGui;
use crate::core::lfo::{Lfo, LfoTarget, Waveform, beat_division_to_hz};

impl ControlGui {
    /// Build the LFO control window
    pub(super) fn build_lfo_window(&mut self, ui: &imgui::Ui) {
        let mut show_window = {
            let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
            state.lfo.show_window
        };
        let mut should_close = false;

        ui.window("LFO Control")
            .size([520.0, 480.0], imgui::Condition::FirstUseEver)
            .opened(&mut show_window)
            .build(|| {
                ui.text("Low Frequency Oscillator Modulation");
                ui.text_disabled("Each LFO can modulate motion parameters");
                ui.separator();

                // Get BPM for display
                let bpm = {
                    let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.audio.bpm
                };
                ui.text(&format!("Tempo: {:.1} BPM", bpm));
                ui.spacing();

                let waveforms = ["Sine", "Triangle", "Ramp Up", "Ramp Down", "Square"];
                let targets = ["None", "Red Delay", "Green Delay", "Blue Delay", "Intensity", "Input Mix", "Trail Fade", "Threshold", "Smoothing"];
                let divisions = ["1/16", "1/8", "1/4", "1/2", "1", "2", "4", "8"];

                // Iterate through each LFO bank
                for i in 0..3 {
                    let mut needs_update = false;

                    // Get current values (store originals for change detection)
                    let (enabled, mut rate, mut amplitude, mut waveform_idx,
                         tempo_sync, current_division, mut phase_offset, mut target_idx) = {
                        let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                        let bank = &state.lfo.bank.lfos[i];
                        let wf_idx = bank.waveform as usize;
                        let tgt_idx = match bank.target {
                            LfoTarget::None => 0,
                            LfoTarget::RedDelay => 1,
                            LfoTarget::GreenDelay => 2,
                            LfoTarget::BlueDelay => 3,
                            LfoTarget::Intensity => 4,
                            LfoTarget::InputMix => 5,
                            LfoTarget::TrailFade => 6,
                            LfoTarget::Threshold => 7,
                            LfoTarget::Smoothing => 8,
                        };
                        (bank.enabled, bank.rate, bank.amplitude, wf_idx,
                         bank.tempo_sync, bank.division, bank.phase_offset, tgt_idx)
                    };
                    // Local mutable copy for UI
                    let mut division_idx = current_division;

                    let header_color = if enabled {
                        [0.2, 0.8, 0.2, 1.0]
                    } else {
                        [0.5, 0.5, 0.5, 1.0]
                    };

                    let _id_token = ui.push_id(format!("lfo_{}", i));

                    if ui.collapsing_header(
                        &format!("LFO {} - {}", i + 1, if enabled { "ON" } else { "OFF" }),
                        imgui::TreeNodeFlags::DEFAULT_OPEN
                    ) {
                        // Enable/disable checkbox
                        let mut enabled_mut = enabled;
                        if ui.checkbox("Enabled", &mut enabled_mut) && enabled_mut != enabled {
                            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                            state.lfo.bank.lfos[i].enabled = enabled_mut;
                        }

                        ui.separator();

                        // Rate control
                        if tempo_sync {
                            // Beat division dropdown
                            let _width = ui.push_item_width(100.0);
                            if ui.combo_simple_string(
                                "Beat Division",
                                &mut division_idx,
                                &divisions
                            ) && division_idx != current_division {
                                let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                                state.lfo.bank.lfos[i].division = division_idx;
                            }
                        } else {
                            // Free rate slider
                            let _width = ui.push_item_width(200.0);
                            if ui.slider("Rate (Hz)", 0.01, 10.0, &mut rate) {
                                let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                                state.lfo.bank.lfos[i].rate = rate;
                            }
                        }

                        // Tempo sync toggle
                        let mut sync = tempo_sync;
                        if ui.checkbox("Tempo Sync", &mut sync) && sync != tempo_sync {
                            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                            state.lfo.bank.lfos[i].tempo_sync = sync;
                        }

                        ui.separator();

                        // Target dropdown
                        let _width = ui.push_item_width(150.0);
                        if ui.combo_simple_string("Target", &mut target_idx, &targets) {
                            let new_target = match target_idx {
                                0 => LfoTarget::None,
                                1 => LfoTarget::RedDelay,
                                2 => LfoTarget::GreenDelay,
                                3 => LfoTarget::BlueDelay,
                                4 => LfoTarget::Intensity,
                                5 => LfoTarget::InputMix,
                                6 => LfoTarget::TrailFade,
                                7 => LfoTarget::Threshold,
                                8 => LfoTarget::Smoothing,
                                _ => LfoTarget::None,
                            };
                            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                            state.lfo.bank.lfos[i].target = new_target;
                        }

                        // Waveform dropdown
                        let _width = ui.push_item_width(100.0);
                        if ui.combo_simple_string("Waveform", &mut waveform_idx, &waveforms) {
                            let new_waveform = match waveform_idx {
                                0 => Waveform::Sine,
                                1 => Waveform::Triangle,
                                2 => Waveform::Ramp,
                                3 => Waveform::Saw,
                                4 => Waveform::Square,
                                _ => Waveform::Sine,
                            };
                            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                            state.lfo.bank.lfos[i].waveform = new_waveform;
                        }

                        ui.separator();

                        // Amplitude slider
                        let _width = ui.push_item_width(200.0);
                        if ui.slider("Amplitude", 0.0, 1.0, &mut amplitude) {
                            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                            state.lfo.bank.lfos[i].amplitude = amplitude;
                        }

                        // Phase offset slider
                        let _width = ui.push_item_width(200.0);
                        if ui.slider("Phase Offset", 0.0, 360.0, &mut phase_offset) {
                            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                            state.lfo.bank.lfos[i].phase_offset = phase_offset;
                        }

                        ui.separator();

                        // Visualization
                        ui.text("Waveform Preview:");
                        let draw_list = ui.get_window_draw_list();
                        let canvas_pos = ui.cursor_screen_pos();
                        let canvas_size = [200.0f32, 60.0f32];

                        // Background
                        draw_list.add_rect(
                            canvas_pos,
                            [canvas_pos[0] + canvas_size[0], canvas_pos[1] + canvas_size[1]],
                            [0.1, 0.1, 0.1, 1.0],
                        ).filled(true).build();

                        // Draw waveform
                        let waveform = match waveform_idx {
                            0 => Waveform::Sine,
                            1 => Waveform::Triangle,
                            2 => Waveform::Ramp,
                            3 => Waveform::Saw,
                            4 => Waveform::Square,
                            _ => Waveform::Sine,
                        };

                        let mut prev_x = canvas_pos[0];
                        let mut prev_y = canvas_pos[1] + canvas_size[1] / 2.0;

                        for x in 0..100 {
                            let phase = x as f32 / 100.0;
                            let value = Lfo::calculate_value(phase, waveform);
                            let pixel_x = canvas_pos[0] + phase * canvas_size[0];
                            let pixel_y = canvas_pos[1] + canvas_size[1] / 2.0 - value * canvas_size[1] / 2.5;

                            if x > 0 {
                                draw_list.add_line(
                                    [prev_x, prev_y],
                                    [pixel_x, pixel_y],
                                    [0.0, 1.0, 0.0, 1.0],
                                ).build();
                            }

                            prev_x = pixel_x;
                            prev_y = pixel_y;
                        }

                        ui.dummy(canvas_size);
                    }
                }
            });

        // Update window visibility state if window was closed
        if !show_window {
            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
            state.lfo.show_window = false;
        }
    }
}
