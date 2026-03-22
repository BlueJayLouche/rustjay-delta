// Motion Extraction Shader - Posy's RGB Delay Method
// 
// Creates motion trails by delaying RGB channels by different amounts:
// - Red:   Current frame (or recent)
// - Green: Delayed by N frames
// - Blue:  More delayed by M frames
//
// This creates colorful motion trails where moving objects leave 
// red->green->blue trails behind them.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
};

struct MotionParams {
    // Delays per channel in frames (0-16)
    // x = red delay, y = green delay, z = blue delay
    delays: vec3<f32>,
    // w = max history for normalization
    delays_w: f32,
    
    // Settings: x = intensity, y = blend_mode, z = grayscale_input, w = feedback
    settings: vec4<f32>,
    
    // Channel gains (can be negative for inversion)
    channel_gain: vec3<f32>,
    // w = unused
    channel_gain_w: f32,
    
    // Additional mixing options
    // x = input_mix, y = trail_fade, z = threshold, w = smoothing
    mix_options: vec4<f32>,
};

// History texture array - we bind up to 3 specific frames
@group(0) @binding(0)
var history_tex_0: texture_2d<f32>;  // For red channel
@group(0) @binding(1)
var history_tex_1: texture_2d<f32>;  // For green channel
@group(0) @binding(2)
var history_tex_2: texture_2d<f32>;  // For blue channel
@group(0) @binding(3)
var input_tex: texture_2d<f32>;      // Current input (for mixing)
@group(0) @binding(4)
var history_sampler: sampler;

// Motion parameters uniform
@group(1) @binding(0)
var<uniform> params: MotionParams;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) texcoord: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.texcoord = texcoord;
    return out;
}

/// Convert RGB to grayscale using luminance weights
fn rgb_to_luma(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
}

/// Apply blend mode between two colors
fn apply_blend(base: vec3<f32>, blend: vec3<f32>, mode: i32) -> vec3<f32> {
    switch mode {
        // 0 = Replace (just return blend)
        case 0: { return blend; }
        // 1 = Add
        case 1: { return min(base + blend, vec3<f32>(1.0)); }
        // 2 = Multiply
        case 2: { return base * blend; }
        // 3 = Screen
        case 3: { return 1.0 - (1.0 - base) * (1.0 - blend); }
        // 4 = Difference
        case 4: { return abs(base - blend); }
        // 5 = Overlay
        case 5: { 
            return mix(
                2.0 * base * blend,
                1.0 - 2.0 * (1.0 - base) * (1.0 - blend),
                step(vec3<f32>(0.5), base)
            );
        }
        // 6 = Lighten
        case 6: { return max(base, blend); }
        // 7 = Darken
        case 7: { return min(base, blend); }
        // Default = Replace
        default: { return blend; }
    }
}

/// Apply a real motion threshold.
/// Values below the threshold are suppressed; remaining range is renormalized.
fn apply_threshold(color: vec3<f32>, threshold: f32) -> vec3<f32> {
    if (threshold <= 0.0) {
        return color;
    }
    let t = clamp(threshold, 0.0, 0.99);
    return max(color - vec3<f32>(t), vec3<f32>(0.0)) / (1.0 - t);
}

/// Negative gains invert the signal instead of being clamped away.
fn apply_channel_gain(value: f32, gain: f32) -> f32 {
    if (gain >= 0.0) {
        return clamp(value * gain, 0.0, 1.0);
    }
    return clamp((1.0 - value) * -gain, 0.0, 1.0);
}

/// Extract motion by differencing adjacent points in time before assigning RGB.
/// This removes static background content instead of just tinting it.
fn extract_motion(
    current_sample: vec4<f32>,
    sample_0: vec4<f32>,
    sample_1: vec4<f32>,
    sample_2: vec4<f32>,
) -> vec3<f32> {
    var motion: vec3<f32>;

    if (params.settings.z > 0.5) {
        let current_luma = rgb_to_luma(current_sample.rgb);
        let l0 = rgb_to_luma(sample_0.rgb);
        let l1 = rgb_to_luma(sample_1.rgb);
        let l2 = rgb_to_luma(sample_2.rgb);

        motion = vec3<f32>(
            abs(l0 - l1),
            abs(l1 - l2),
            abs(current_luma - l2),
        );
    } else {
        motion = vec3<f32>(
            abs(sample_0.r - sample_1.r),
            abs(sample_1.g - sample_2.g),
            abs(current_sample.b - sample_2.b),
        );
    }

    motion.r = apply_channel_gain(motion.r, params.channel_gain.r);
    motion.g = apply_channel_gain(motion.g, params.channel_gain.g);
    motion.b = apply_channel_gain(motion.b, params.channel_gain.b);

    return motion;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample from three different history frames
    let sample_0 = textureSample(history_tex_0, history_sampler, in.texcoord);
    let sample_1 = textureSample(history_tex_1, history_sampler, in.texcoord);
    let sample_2 = textureSample(history_tex_2, history_sampler, in.texcoord);
    let input_sample = textureSample(input_tex, history_sampler, in.texcoord);
    
    // Extract actual motion energy instead of displaying raw historical frames.
    var motion = extract_motion(input_sample, sample_0, sample_1, sample_2);
    
    // Apply threshold if set
    motion = apply_threshold(motion, params.mix_options.z);
    
    // Apply smoothing if set by smoothing the extracted motion signal.
    if (params.mix_options.w > 0.0) {
        let offset = params.mix_options.w * 0.01;
        let t0 = in.texcoord + vec2<f32>(offset, 0.0);
        let t1 = in.texcoord - vec2<f32>(offset, 0.0);
        let t2 = in.texcoord + vec2<f32>(0.0, offset);
        let t3 = in.texcoord - vec2<f32>(0.0, offset);

        let smoothed = (
            extract_motion(
                textureSample(input_tex, history_sampler, t0),
                textureSample(history_tex_0, history_sampler, t0),
                textureSample(history_tex_1, history_sampler, t0),
                textureSample(history_tex_2, history_sampler, t0),
            ) +
            extract_motion(
                textureSample(input_tex, history_sampler, t1),
                textureSample(history_tex_0, history_sampler, t1),
                textureSample(history_tex_1, history_sampler, t1),
                textureSample(history_tex_2, history_sampler, t1),
            ) +
            extract_motion(
                textureSample(input_tex, history_sampler, t2),
                textureSample(history_tex_0, history_sampler, t2),
                textureSample(history_tex_1, history_sampler, t2),
                textureSample(history_tex_2, history_sampler, t2),
            ) +
            extract_motion(
                textureSample(input_tex, history_sampler, t3),
                textureSample(history_tex_0, history_sampler, t3),
                textureSample(history_tex_1, history_sampler, t3),
                textureSample(history_tex_2, history_sampler, t3),
            )
        ) * 0.25;
        motion = mix(motion, smoothed, params.mix_options.w);
    }
    
    // Apply blend mode with input
    let blend_mode = i32(params.settings.y);
    let base = input_sample.rgb;
    let blended = apply_blend(base, motion, blend_mode);
    
    // Apply intensity mixing
    let intensity = params.settings.x;
    let input_mix = params.mix_options.x;
    
    // Mix between input and blended result
    var output = mix(base * input_mix, blended, intensity);
    
    // Apply trail fade (gamma-like adjustment)
    let trail_fade = params.mix_options.y;
    if (trail_fade > 0.0) {
        // Boost darker areas to make trails more visible
        output = pow(output, vec3<f32>(1.0 - trail_fade * 0.5));
    }
    
    return vec4<f32>(output, 1.0);
}

// Alternative fragment shader that uses a texture array (if supported)
// This version would be more flexible for variable delays
@fragment
fn fs_main_array(in: VertexOutput) -> @location(0) vec4<f32> {
    // Note: This requires texture_2d_array support
    // For now, we use the 3-texture approach above which works everywhere
    
    // Sample from the three bound textures
    let sample_0 = textureSample(history_tex_0, history_sampler, in.texcoord);
    let sample_1 = textureSample(history_tex_1, history_sampler, in.texcoord);
    let sample_2 = textureSample(history_tex_2, history_sampler, in.texcoord);
    
    // Extract luminance for each channel
    let r = rgb_to_luma(sample_0.rgb);
    let g = rgb_to_luma(sample_1.rgb);
    let b = rgb_to_luma(sample_2.rgb);
    
    return vec4<f32>(r, g, b, 1.0);
}
