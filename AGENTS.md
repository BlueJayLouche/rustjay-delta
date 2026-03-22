# Agent Guidelines for RustJay Delta

This file provides context and guidelines for AI assistants working on the RustJay Delta project.

## Project Overview

RustJay Delta is a GPU-accelerated motion extraction VJ tool based on **Posy's RGB delay technique**. It creates colorful motion trails by delaying the R, G, and B channels by different amounts and differencing adjacent frames to extract motion energy.

## Architecture Principles

### 1. Separation of Concerns

The most important architectural pattern is the **separation of base values from modulation**:

- **Base Values**: Stored in `motion_params` - these are the "user set" values
- **Modulation**: Applied at render time as additive offsets
- **Final Values**: Computed each frame: `final = base + lfo_mod + audio_mod`

**NEVER** write modulated values back to base values - this causes feedback loops.

### 2. Frame Update Flow

```
┌─────────────────────────────────────────────────────────────┐
│                         Frame Update                         │
├─────────────────────────────────────────────────────────────┤
│  1. Process Commands (input/output/audio/MIDI/OSC/web)      │
│  2. Update Audio (FFT analysis, beat detection)             │
│  3. Update LFO Phases (only update phase accumulators)      │
│  4. Update MIDI/OSC (process incoming messages)             │
│  5. Render (composite base + modulations)                   │
└─────────────────────────────────────────────────────────────┘
```

### 3. Frame History

Delta uses a **ring buffer of GPU textures** for time-delayed effects:

- Up to 16 frames stored as BGRA8 textures
- GPU-to-GPU copies only (no CPU readback)
- History size auto-scales with resolution
- Three frames are sampled each render (one per RGB channel)
- Frame differencing extracts motion energy

### 4. State Management

- `SharedState` is the single source of truth, wrapped in `Arc<Mutex<>>`
- GUI reads from `motion_params` to display sliders
- GUI writes to `motion_params` when user interacts
- Modulation systems only update their internal state (phases, smoothed values)
- Renderer composites everything at the last moment

## Key Modules

### Core (`src/core/`)

- **state.rs**: SharedState definition - includes `MotionParams`, `BlendMode`, `ResolutionPreset`
- **lfo.rs**: LFO engine - waveform generation, phase accumulation
- **vertex.rs**: GPU vertex data structures

### Audio (`src/audio/`)

- **mod.rs**: Audio capture, FFT analysis, beat detection
- **routing.rs**: Audio→parameter routing matrix for motion parameters

### Engine (`src/engine/`)

- **renderer.rs**: Main wgpu renderer with motion extraction shader
- **frame_history.rs**: Ring buffer for storing past frame textures
- **texture.rs**: Texture management
- **shaders/motion_extraction.wgsl**: Motion extraction shader

### GUI (`src/gui/`)

- **gui.rs**: ImGui interface builder - simplified tabs for delta
- **gui/tab_motion.rs**: Motion extraction controls
- **gui/tab_audio.rs**: Audio tab with routing matrix
- **gui/tab_input.rs**: Input source selection
- **gui/tab_output.rs**: Output configuration
- **gui/tab_settings.rs**: Resolution, history size, UI scale

### Control Systems

- **midi/mod.rs**: MIDI input with learn system
- **osc/mod.rs**: OSC server (UDP port 9000)
- **web/mod.rs**: WebSocket server + embedded HTML interface
- **presets/mod.rs**: Save/load system for motion presets
- **config/mod.rs**: Settings persistence

## Delta-Specific Parameters

### MotionParams

- `red_delay`, `green_delay`, `blue_delay`: 0-16 frames
- `intensity`: 0.0-1.0 (blend between input and motion)
- `blend_mode`: Replace, Add, Multiply, Screen, Difference, Overlay, Lighten, Darken
- `grayscale_input`: Convert to luminance before RGB assignment
- `red_gain`, `green_gain`, `blue_gain`: -2.0 to 2.0 (negative inverts)
- `input_mix`: 0.0-1.0 (how much original input to preserve)
- `trail_fade`: 0.0-1.0 (gamma-like boost for trails)
- `threshold`: 0.0-1.0 (suppress low motion areas)
- `smoothing`: 0.0-1.0 (spatial smoothing)

### Audio Routing Targets

- RedDelay, GreenDelay, BlueDelay
- Intensity, InputMix, TrailFade
- Threshold, Smoothing
- RedGain, GreenGain, BlueGain

## Important Constants

### Frame History
- MAX_HISTORY: 16 frames
- Default for 720p/1080p: 8 frames
- Default for 1440p: 7 frames
- Default for 4K: 6 frames

### Audio
- FFT bands: 8 bands from 20Hz to 16kHz
- Attack/release: 0.0-1.0 (0 = instant, 1 = very slow)
- Modulation clamp: -2.0 to 2.0 (summed across all routes)

## Common Pitfalls

### 1. Variable Shadowing in GUI

```rust
// WRONG - division_idx shadowed
let division_idx = bank.division;
if ui.combo(..., &mut division_idx) && division_idx != division_idx { ... }

// RIGHT - compare against original
let current_division = bank.division;
let mut division_idx = current_division;
if ui.combo(..., &mut division_idx) && division_idx != current_division { ... }
```

### 2. Writing Modulated Values to Base

```rust
// WRONG - creates feedback loop
state.motion_params.intensity = base_intensity + modulation;
state.audio_routing.base_intensity = state.motion_params.intensity; // NO!

// RIGHT - keep them separate
// (GUI writes to base, render reads base + applies modulation)
```

### 3. Borrow Issues with Mutex

```rust
// WRONG - holding lock while doing UI
let state = self.shared_state.lock().unwrap();
if ui.button("Click") { state.value = 1; } // Can't mutably borrow

// RIGHT - get values first, then update
let value = { let state = self.shared_state.lock().unwrap(); state.value };
if ui.button("Click") { 
    let mut state = self.shared_state.lock().unwrap();
    state.value = 1;
}
```

## Testing Checklist

When adding new features, verify:

- [ ] GUI displays correct base values (not modulated)
- [ ] GUI updates base values correctly
- [ ] Modulation works when enabled
- [ ] Values return to base when modulation disabled
- [ ] No drift/compounding over time
- [ ] Presets save/load correctly
- [ ] Web interface reflects changes
- [ ] MIDI/OSC can control the parameter

## Build Notes

- Always build with `--release` for testing (debug is very slow)
- On macOS, the build script handles framework linking automatically
- The web UI is embedded at compile time from `src/web/ui.html`

## Code Style

- Use `log::info!`, `log::warn!`, `log::error!` for logging (not println)
- Prefer `f32` over `f64` for GPU compatibility
- Use glam types (Vec3, Vec4) for shader-bound data
- Keep modulation logic in the render step, not in update loops
