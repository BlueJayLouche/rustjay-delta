# RustJay Delta

A GPU-accelerated motion extraction VJ tool based on **Posy's RGB delay technique**. Feed it any video source and it produces vivid, colourful motion trails by assigning temporally-delayed copies of the input to the R, G, and B channels independently.

## How It Works

RustJay Delta keeps a ring buffer of up to 16 past frames entirely on the GPU. Each frame, the shader samples three points in that history — one for red, one for green, one for blue — and differences adjacent pairs to extract motion energy:

```
Input frame  ──► GPU ring buffer (up to 16 frames)
                        │
              ┌─────────┼─────────┐
              ▼         ▼         ▼
         frame[R]   frame[G]   frame[B]   ← per-channel delays
              │         │         │
              └────── diff ───────┘
                        │
                 motion signal
                        │
              blend modes, gains, threshold, smoothing
                        │
                     output
```

Static areas produce near-zero motion signal and remain dark or neutral. Moving objects leave trails coloured by the delay spread — with the default 0/2/4 frame offsets, motion trails from red to green to blue.

## Features

### Motion Extraction

- **Per-channel frame delays** — Red, Green, Blue each independently 0–16 frames back
- **Grayscale input mode** — Convert to luminance before channel assignment for cleaner trails
- **Per-channel gain** — Negative values invert the channel signal
- **Intensity** — Blend between extracted motion and original input
- **Blend modes** — Replace, Add, Multiply, Screen, Difference, Overlay, Lighten, Darken
- **Threshold** — Suppress slow-moving areas; remaining range is renormalised
- **Trail fade** — Gamma-like boost that lifts darker trail areas
- **Spatial smoothing** — 5-tap neighbourhood sampling to soften noisy edges

### Built-in Presets

| Preset | Red | Green | Blue | Notes |
|---|---|---|---|---|
| Classic Posy | 0 | 2 | 4 | Grayscale input, Replace blend |
| Reverse | 4 | 2 | 0 | Trails run B→G→R |
| Subtle | 0 | 1 | 2 | Half intensity, Add blend |
| Extreme | 0 | 4 | 8 | Colour input, Add blend |
| RGB Trails | 0 | 3 | 6 | Colour input, Screen blend |
| Strobe | 0 | 0 | 1 | One-frame diff, Difference blend |

### GPU Frame Buffer

- Ring buffer of up to 16 frames stored as BGRA8 GPU textures
- GPU-to-GPU copies only — no CPU readback
- History size auto-scales with resolution to manage VRAM:
  - 720p / 1080p: 8 frames
  - 1440p: 7 frames
  - 4K: 6 frames

### Modulation

- **LFO** — 3 banks with Sine, Triangle, Ramp, Saw, Square waveforms; tempo sync
- **Audio reactivity** — 8-band FFT routed to any parameter; beat detection
- **MIDI** — CC mapping with learn mode
- **OSC** — UDP server (port 9000)
- **Web remote** — WebSocket interface (port 8080)

### Inputs / Outputs

| | Sources |
|---|---|
| **In** | Webcam, NDI, Syphon (macOS), Spout (Windows), V4L2 (Linux) |
| **Out** | NDI, Syphon (macOS), Spout (Windows), V4L2 (Linux), screen |

## Building

### Requirements

- Rust 1.75+
- macOS 11+ or Linux (Vulkan GPU)
- Xcode Command Line Tools (macOS)

### Clone and Build

```bash
cd rustjay-delta
cargo build --release
```

### NDI Support (Optional)

Download and install the NDI SDK from [ndi.video](https://ndi.video). On macOS the build system finds it automatically in `/usr/local/lib` or `/Library/NDI SDK for Apple/lib/macOS`.

### Syphon Support (macOS Only)

Syphon is enabled automatically on macOS. The build system finds the framework at `../syphon-rs/syphon-lib/Syphon.framework`.

**Requirements:** The `syphon-rs` repo must be present as a sibling directory:

```
developer/rust/
├── syphon-rs/          ← must exist
└── rustjay-delta/
```

If your layout differs, set `SYPHON_FRAMEWORK_DIR` before building:

```bash
SYPHON_FRAMEWORK_DIR=/path/to/syphon-rs/syphon-lib cargo build --release
```

## Running

```bash
cargo run --release
```

## Controls

### Keyboard Shortcuts

| Key | Action |
|---|---|
| `Esc` | Exit |
| `Space` | Toggle output fullscreen |
| `F` | Toggle fullscreen |

### GUI Tabs

#### Input Tab
- Select video source (Webcam, NDI, Syphon)
- Refresh device lists
- Input status and resolution display

#### Motion Tab
- Enable/disable motion extraction
- Per-channel delay sliders (Red 0–16, Green 0–16, Blue 0–16)
- Per-channel gain (−2.0 to 2.0; negative inverts)
- Intensity, blend mode dropdown
- Grayscale input toggle
- Input mix, trail fade, threshold, smoothing
- Preset buttons

#### Audio Tab
- Device selection
- Amplitude, smoothing, normalization
- 8-band FFT display and routing matrix
- Beat detection, tap tempo

#### Output Tab
- NDI output name and toggle
- Syphon output name and toggle (macOS)
- Resolution preset (720p / 1080p / 1440p / 4K)
- Fullscreen toggle

#### Settings Tab
- MIDI, OSC, Web remote controls
- UI scale

## Configuration

Settings are auto-saved to `~/.config/rustjay/settings.json`. You can also use `config.toml` in the project root:

```toml
[video]
internal_width = 1280
internal_height = 720
surface_format = "Bgra8Unorm"
vsync = true

[audio]
sample_rate = 48000
buffer_size = 1024
fft_size = 2048
```

## Technical Details

### Frame History Memory

At 1080p with 8 frames of history: 8 × 1920 × 1080 × 4 bytes ≈ **66 MB VRAM**. History size is automatically reduced at higher resolutions.

### Color Format

Native `Bgra8Unorm` throughout — matches macOS surface format and Syphon's native format, eliminating conversion overhead.

### Audio Processing

- `realfft` 3.4 with lock-free `AtomicU32` sharing — no mutex on the real-time audio thread
- 8-band FFT (20 Hz – 16 kHz), optional pink noise shaping
- Beat detection with energy history

## Troubleshooting

### "Library not loaded: Syphon.framework"

1. Verify the framework exists: `ls ../syphon-rs/syphon-lib/Syphon.framework`
2. Rebuild after setting the path: `SYPHON_FRAMEWORK_DIR=/path/to/syphon-rs/syphon-lib cargo build --release`

### NDI source not found

- Install the NDI Runtime from [ndi.video](https://ndi.video)
- Ensure source and receiver are on the same subnet
- Check firewall — NDI uses ports 5960–5969

### Performance

- Use `cargo run --release` — debug builds are significantly slower
- Lower the resolution preset in the Output tab to reduce VRAM usage
- Disable smoothing if CPU-bound

## License

MIT License — see LICENSE for details

## Credits

Motion extraction technique by [Posy](https://www.youtube.com/@Posy).

Built with:
- [wgpu](https://github.com/gfx-rs/wgpu) — GPU rendering
- [imgui-rs](https://github.com/imgui-rs/imgui-rs) — immediate mode GUI
- [grafton-ndi](https://crates.io/crates/grafton-ndi) — NDI support
- [nokhwa](https://github.com/l1npengtul/nokhwa) — webcam capture
- [realfft](https://github.com/HEnquist/realfft) — FFT analysis
- [syphon-rs](https://github.com/BlueJayLouche/syphon-rs) — Syphon integration (macOS)
