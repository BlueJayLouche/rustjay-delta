use super::App;
use crate::core::InputType;

impl App {
    /// Update input and upload frames to GPU
    pub(super) fn update_input(&mut self) {
        if let Some(ref mut manager) = self.input_manager {
            // Detect NDI source loss and surface it in shared state
            if manager.input_type() == InputType::Ndi && manager.is_ndi_source_lost() {
                log::warn!("[NDI] Source lost — clearing active input state");
                let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                state.input.is_active = false;
                state.input.source_name = "Signal lost".to_string();
            }

            manager.update();

            // Handle Syphon texture (GPU copy path for delta frame history)
            #[cfg(target_os = "macos")]
            if manager.input_type() == InputType::Syphon {
                if manager.has_frame() {
                    let dims = manager.syphon_output_texture()
                        .map(|t| (t.width(), t.height()));

                    if let Some((width, height)) = dims {
                        if let Some(texture) = manager.syphon_output_texture() {
                            if let Some(ref mut engine) = self.output_engine {
                                // For delta, we need to copy to owned texture for frame history
                                // Instead of zero-copy set_external_texture, use update_from_texture
                                engine.input_texture.update_from_texture(texture);
                            }
                        }
                        manager.clear_syphon_frame();
                        let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                        state.input.width = width;
                        state.input.height = height;
                    }
                }
            } else {
                // CPU fallback path
                if let Some(frame_data) = manager.take_frame() {
                    let (width, height) = manager.resolution();

                    if let Some(ref mut engine) = self.output_engine {
                        engine.input_texture.update(&frame_data, width, height);
                    }

                    let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.input.width = width;
                    state.input.height = height;
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                if let Some(frame_data) = manager.take_frame() {
                    let (width, height) = manager.resolution();

                    if let Some(ref mut engine) = self.output_engine {
                        engine.input_texture.update(&frame_data, width, height);
                    }

                    let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.input.width = width;
                    state.input.height = height;
                }
            }
        }
    }

    /// Update audio analysis
    pub(super) fn update_audio(&mut self) {
        // Reconnect if the stream reported an error (e.g. device unplugged)
        if let Some(ref analyzer) = self.audio_analyzer {
            if analyzer.take_stream_error() {
                let device = {
                    let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.audio.selected_device.clone()
                };
                log::warn!("[Audio] Stream error detected — attempting reconnect (device: {:?})", device);
                drop(analyzer); // release immutable borrow before we need mutable
                if let Some(ref mut analyzer) = self.audio_analyzer {
                    if let Err(e) = analyzer.start_with_device(device.as_deref()) {
                        log::error!("[Audio] Reconnect failed: {}", e);
                    }
                }
            }
        }

        // Sync settings from shared state TO analyzer
        if let Some(ref analyzer) = self.audio_analyzer {
            let (amplitude, smoothing, normalize, pink_noise) = {
                let state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                (state.audio.amplitude, state.audio.smoothing, state.audio.normalize, state.audio.pink_noise_shaping)
            };

            analyzer.set_amplitude(amplitude);
            analyzer.set_smoothing(smoothing);
            analyzer.set_normalize(normalize);
            analyzer.set_pink_noise_shaping(pink_noise);
        }

        // Read analysis results FROM analyzer TO shared state
        if let Some(ref analyzer) = self.audio_analyzer {
            let fft = analyzer.get_fft();
            let volume = analyzer.get_volume();
            let beat = analyzer.is_beat();
            let phase = analyzer.get_beat_phase();

            let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
            if state.audio.enabled {
                state.audio.fft = fft;
                state.audio.volume = volume;
                state.audio.beat = beat;
                state.audio.beat_phase = phase;

                // Process audio routing (updates internal smoothed values)
                // Actual application of modulation happens in render step
                if state.audio_routing.enabled {
                    let delta_time = self.frame_delta_time;
                    state.audio_routing.matrix.process(&fft, delta_time);
                }
            }
        }
    }

    /// Update LFO phases (modulation applied in final composite step)
    pub(super) fn update_lfo(&mut self) {
        let delta_time = self.frame_delta_time;
        let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
        let bpm = state.audio.bpm;
        let beat_phase = state.audio.beat_phase;
        state.lfo.bank.update(bpm, delta_time, beat_phase);
    }

    /// Update MIDI - apply mapped values to state (only when changed)
    pub(super) fn update_midi(&mut self) {
        // Periodically check whether the connected MIDI device is still present
        if let Some(ref mut manager) = self.midi_manager {
            if let Some(false) = manager.check_device_available_if_needed() {
                let name = manager.state().lock()
                    .map(|s| s.selected_device.clone().unwrap_or_default())
                    .unwrap_or_default();
                log::warn!("[MIDI] Device '{}' no longer available — disconnecting", name);
                manager.disconnect();
                // Surface the disconnection in shared state so the GUI can show a warning
                let mut state = self.shared_state.lock().unwrap_or_else(|e| e.into_inner());
                state.midi_command = crate::core::MidiCommand::None;
            }
        }

        if let Some(ref manager) = self.midi_manager {
            // Apply dirty MIDI values directly — lock midi state, then shared
            // state in one pass. No intermediate HashMap allocation needed.
            let midi_state_arc = manager.state();
            let mut midi_state = midi_state_arc.lock().unwrap_or_else(|e| e.into_inner());

            let has_dirty = midi_state.mappings.iter().any(|m| m.is_dirty());

            if has_dirty {
                if let Ok(mut shared) = self.shared_state.lock() {
                    for mapping in &mut midi_state.mappings {
                        if mapping.is_dirty() {
                            let v = mapping.get_scaled_value();
                            match mapping.param_path.as_str() {
                                "motion/red_delay" => shared.motion_params.red_delay = v.clamp(0.0, 16.0) as u32,
                                "motion/green_delay" => shared.motion_params.green_delay = v.clamp(0.0, 16.0) as u32,
                                "motion/blue_delay" => shared.motion_params.blue_delay = v.clamp(0.0, 16.0) as u32,
                                "motion/intensity" => shared.motion_params.intensity = v.clamp(0.0, 1.0),
                                "audio/amplitude" => shared.audio.amplitude = v.clamp(0.0, 5.0),
                                "audio/smoothing" => shared.audio.smoothing = v.clamp(0.0, 1.0),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    /// Update OSC - apply received values to state (only when changed)
    pub(super) fn update_osc(&mut self) {
        if let Some(ref server) = self.osc_server {
            // Collect only dirty values
            let (red_delay, green_delay, blue_delay, motion_enabled, amplitude, smoothing) = {
                if let Ok(mut osc_state) = server.state().lock() {
                    (
                        osc_state.get_value_if_dirty("/motion/red_delay"),
                        osc_state.get_value_if_dirty("/motion/green_delay"),
                        osc_state.get_value_if_dirty("/motion/blue_delay"),
                        osc_state.get_value_if_dirty("/motion/enabled"),
                        osc_state.get_value_if_dirty("/audio/amplitude"),
                        osc_state.get_value_if_dirty("/audio/smoothing"),
                    )
                } else {
                    (None, None, None, None, None, None)
                }
            };

            // Apply to shared state only if there are changes
            if red_delay.is_some() || green_delay.is_some() || blue_delay.is_some() ||
               motion_enabled.is_some() || amplitude.is_some() || smoothing.is_some() {
                if let Ok(mut shared) = self.shared_state.lock() {
                    if let Some(v) = red_delay {
                        shared.motion_params.red_delay = v.clamp(0.0, 16.0) as u32;
                    }
                    if let Some(v) = green_delay {
                        shared.motion_params.green_delay = v.clamp(0.0, 16.0) as u32;
                    }
                    if let Some(v) = blue_delay {
                        shared.motion_params.blue_delay = v.clamp(0.0, 16.0) as u32;
                    }
                    if let Some(v) = motion_enabled {
                        shared.motion_enabled = v > 0.5;
                    }
                    if let Some(v) = amplitude {
                        shared.audio.amplitude = v.clamp(0.0, 5.0);
                    }
                    if let Some(v) = smoothing {
                        shared.audio.smoothing = v.clamp(0.0, 1.0);
                    }
                }
            }
        }
    }

    /// Update web server with current state
    pub(super) fn update_web(&mut self) {
        if let Some(ref mut server) = self.web_server {
            if !server.is_running() {
                return;
            }

            // Sync current parameter values to web server
            if let Ok(state) = self.shared_state.lock() {
                server.update_parameter("motion/red_delay", state.motion_params.red_delay as f32);
                server.update_parameter("motion/green_delay", state.motion_params.green_delay as f32);
                server.update_parameter("motion/blue_delay", state.motion_params.blue_delay as f32);
                server.update_parameter("motion/intensity", state.motion_params.intensity);
                server.update_parameter("motion/enabled", if state.motion_enabled { 1.0 } else { 0.0 });
                server.update_parameter("audio/amplitude", state.audio.amplitude);
                server.update_parameter("audio/smoothing", state.audio.smoothing);
                server.update_parameter("audio/enabled", if state.audio.enabled { 1.0 } else { 0.0 });
                server.update_parameter("audio/normalize", if state.audio.normalize { 1.0 } else { 0.0 });
                server.update_parameter("audio/pink_noise", if state.audio.pink_noise_shaping { 1.0 } else { 0.0 });
                server.update_parameter("output/fullscreen", if state.output_fullscreen { 1.0 } else { 0.0 });
            }
        }
    }

    /// Poll for background device discovery completion and update the GUI when done.
    pub(super) fn poll_device_discovery(&mut self) {
        let done = self.input_manager.as_mut().map_or(false, |m| m.poll_discovery());
        if done {
            if let (Some(ref manager), Some(ref mut gui)) =
                (self.input_manager.as_ref(), self.control_gui.as_mut())
            {
                gui.update_device_lists(manager);
            }
            self.shared_state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .input_discovering = false;
        }
    }

    /// Update preview textures for GUI
    pub(super) fn update_preview_textures(&mut self) {
        // Skip all GPU preview copies when previews are hidden — saves overhead
        let show_preview = self.shared_state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .show_preview;
        if !show_preview {
            return;
        }

        // When Syphon is active, `input_texture.texture` is None (zero-copy external path).
        // Fall back to `render_target` so the input preview still shows something.
        let input_uses_external = self.output_engine.as_ref()
            .map(|e| e.input_texture.has_external_texture())
            .unwrap_or(false);

        if let (Some(ref mut renderer), Some(ref gui)) =
            (self.imgui_renderer.as_mut(), self.control_gui.as_ref())
        {
            // Single encoder/submit for both preview copies.
            let mut encoder = renderer.device().create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("Preview Encoder") },
            );
            let mut any_work = false;

            // Update input preview
            {
                let input_src = if input_uses_external {
                    // Syphon zero-copy path: use render_target as a proxy
                    self.output_engine.as_ref().map(|e| &e.render_target.texture)
                } else {
                    self.output_engine
                        .as_ref()
                        .and_then(|e| e.input_texture.texture.as_ref().map(|t| &t.texture))
                };
                if let (Some(tex), Some(preview_id)) = (input_src, gui.input_preview_texture_id) {
                    renderer.update_preview_texture(preview_id, tex, &mut encoder);
                    any_work = true;
                }
            }

            // Update output preview
            {
                let output_src = self.output_engine.as_ref().map(|e| &e.render_target.texture);
                if let (Some(tex), Some(preview_id)) = (output_src, gui.output_preview_texture_id) {
                    renderer.update_preview_texture(preview_id, tex, &mut encoder);
                    any_work = true;
                }
            }

            if any_work {
                renderer.queue().submit(std::iter::once(encoder.finish()));
            }
        }
    }
}
