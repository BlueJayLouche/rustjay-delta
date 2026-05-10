//! # Audio Routing System
//!
//! Routes audio FFT bands to motion extraction parameters.
//! Tailored for rustjay-delta motion extraction parameters.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::core::MotionParams;

/// FFT frequency bands (8-band spectrum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FftBand {
    SubBass = 0,    // 20-60 Hz
    Bass = 1,       // 60-120 Hz
    LowMid = 2,     // 120-250 Hz
    Mid = 3,        // 250-500 Hz
    HighMid = 4,    // 500-2000 Hz
    High = 5,       // 2000-4000 Hz
    VeryHigh = 6,   // 4000-8000 Hz
    Presence = 7,   // 8000-16000 Hz
}

impl FftBand {
    pub fn name(&self) -> &'static str {
        match self {
            FftBand::SubBass => "Sub Bass",
            FftBand::Bass => "Bass",
            FftBand::LowMid => "Low Mid",
            FftBand::Mid => "Mid",
            FftBand::HighMid => "High Mid",
            FftBand::High => "High",
            FftBand::VeryHigh => "Very High",
            FftBand::Presence => "Presence",
        }
    }
    
    pub fn short_name(&self) -> &'static str {
        match self {
            FftBand::SubBass => "Sub",
            FftBand::Bass => "Bass",
            FftBand::LowMid => "LoMid",
            FftBand::Mid => "Mid",
            FftBand::HighMid => "HiMid",
            FftBand::High => "High",
            FftBand::VeryHigh => "VHigh",
            FftBand::Presence => "Presence",
        }
    }
    
    pub fn all() -> &'static [FftBand] {
        &[
            FftBand::SubBass,
            FftBand::Bass,
            FftBand::LowMid,
            FftBand::Mid,
            FftBand::HighMid,
            FftBand::High,
            FftBand::VeryHigh,
            FftBand::Presence,
        ]
    }
    
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(FftBand::SubBass),
            1 => Some(FftBand::Bass),
            2 => Some(FftBand::LowMid),
            3 => Some(FftBand::Mid),
            4 => Some(FftBand::HighMid),
            5 => Some(FftBand::High),
            6 => Some(FftBand::VeryHigh),
            7 => Some(FftBand::Presence),
            _ => None,
        }
    }
}

/// Parameters that can be modulated by audio
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModulationTarget {
    RedDelay,
    GreenDelay,
    BlueDelay,
    Intensity,
    InputMix,
    TrailFade,
    Threshold,
    Smoothing,
    RedGain,
    GreenGain,
    BlueGain,
}

impl ModulationTarget {
    pub fn name(&self) -> &'static str {
        match self {
            ModulationTarget::RedDelay => "Red Delay",
            ModulationTarget::GreenDelay => "Green Delay",
            ModulationTarget::BlueDelay => "Blue Delay",
            ModulationTarget::Intensity => "Intensity",
            ModulationTarget::InputMix => "Input Mix",
            ModulationTarget::TrailFade => "Trail Fade",
            ModulationTarget::Threshold => "Threshold",
            ModulationTarget::Smoothing => "Smoothing",
            ModulationTarget::RedGain => "Red Gain",
            ModulationTarget::GreenGain => "Green Gain",
            ModulationTarget::BlueGain => "Blue Gain",
        }
    }
    
    pub fn all() -> &'static [ModulationTarget] {
        &[
            ModulationTarget::RedDelay,
            ModulationTarget::GreenDelay,
            ModulationTarget::BlueDelay,
            ModulationTarget::Intensity,
            ModulationTarget::InputMix,
            ModulationTarget::TrailFade,
            ModulationTarget::Threshold,
            ModulationTarget::Smoothing,
            ModulationTarget::RedGain,
            ModulationTarget::GreenGain,
            ModulationTarget::BlueGain,
        ]
    }
}

/// A single audio-to-parameter routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRoute {
    /// Unique ID for this route
    pub id: usize,
    /// Which FFT band to use
    pub band: FftBand,
    /// Which parameter to modulate
    pub target: ModulationTarget,
    /// Modulation depth (-1.0 to 1.0, can be bipolar)
    pub amount: f32,
    /// Attack smoothing (0.0 = instant, 1.0 = very slow)
    pub attack: f32,
    /// Release smoothing (0.0 = instant, 1.0 = very slow)
    pub release: f32,
    /// Whether this route is enabled
    pub enabled: bool,
    /// Current modulated value (runtime only, not serialized)
    #[serde(skip)]
    pub current_value: f32,
    /// Current smoothed FFT value (runtime only)
    #[serde(skip)]
    smoothed_fft: f32,
}

impl AudioRoute {
    /// Create a new audio route
    pub fn new(id: usize, band: FftBand, target: ModulationTarget) -> Self {
        Self {
            id,
            band,
            target,
            amount: 0.5,
            attack: 0.1,
            release: 0.3,
            enabled: true,
            current_value: 0.0,
            smoothed_fft: 0.0,
        }
    }
    
    /// Process this route with new FFT data
    /// 
    /// # Arguments
    /// * `fft_bands` - Array of 8 FFT band values (0.0 to 1.0)
    /// * `delta_time` - Time since last frame in seconds
    pub fn process(&mut self, fft_bands: &[f32; 8], delta_time: f32) {
        if !self.enabled {
            self.current_value = 0.0;
            self.smoothed_fft = self.smoothed_fft * 0.9; // Decay to 0
            return;
        }
        
        // Get current FFT value for our band
        let target_value = fft_bands[self.band as usize];
        
        // Apply attack/release smoothing
        let diff = target_value - self.smoothed_fft;
        let smoothing = if diff > 0.0 { self.attack } else { self.release };
        
        // Exponential smoothing
        let smoothing_factor = (-delta_time / smoothing.max(0.001)).exp();
        self.smoothed_fft = self.smoothed_fft * smoothing_factor + target_value * (1.0 - smoothing_factor);
        
        // Calculate modulation value
        self.current_value = self.smoothed_fft * self.amount;
    }
    
    /// Reset smoothed values
    pub fn reset(&mut self) {
        self.current_value = 0.0;
        self.smoothed_fft = 0.0;
    }
}

/// Manages all audio-to-parameter routings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingMatrix {
    routes: Vec<AudioRoute>,
    next_id: usize,
    max_routes: usize,
}

impl RoutingMatrix {
    /// Create a new routing matrix
    pub fn new(max_routes: usize) -> Self {
        Self {
            routes: Vec::new(),
            next_id: 0,
            max_routes,
        }
    }
    
    /// Create with default routes
    pub fn with_defaults() -> Self {
        let mut matrix = Self::new(8);
        
        // Add some default routes for delta
        matrix.add_route(FftBand::Bass, ModulationTarget::Intensity);
        matrix.add_route(FftBand::High, ModulationTarget::BlueDelay);
        
        matrix
    }
    
    /// Add a new route
    /// 
    /// Returns the ID of the new route, or None if at max capacity
    pub fn add_route(&mut self, band: FftBand, target: ModulationTarget) -> Option<usize> {
        if self.routes.len() >= self.max_routes {
            return None;
        }
        
        let id = self.next_id;
        self.next_id += 1;
        
        self.routes.push(AudioRoute::new(id, band, target));
        Some(id)
    }
    
    /// Remove a route by ID
    pub fn remove_route(&mut self, id: usize) {
        self.routes.retain(|r| r.id != id);
    }
    
    /// Remove a route by index
    pub fn remove_route_at(&mut self, index: usize) {
        if index < self.routes.len() {
            self.routes.remove(index);
        }
    }
    
    /// Get a route by ID
    pub fn get_route(&self, id: usize) -> Option<&AudioRoute> {
        self.routes.iter().find(|r| r.id == id)
    }
    
    /// Get a mutable route by ID
    pub fn get_route_mut(&mut self, id: usize) -> Option<&mut AudioRoute> {
        self.routes.iter_mut().find(|r| r.id == id)
    }
    
    /// Get all routes
    pub fn routes(&self) -> &[AudioRoute] {
        &self.routes
    }
    
    /// Get mutable access to all routes
    pub fn routes_mut(&mut self) -> &mut [AudioRoute] {
        &mut self.routes
    }
    
    /// Process all routes with new FFT data
    pub fn process(&mut self, fft_bands: &[f32; 8], delta_time: f32) {
        for route in &mut self.routes {
            route.process(fft_bands, delta_time);
        }
    }
    
    /// Get the modulation value for a specific target
    /// 
    /// If multiple routes target the same parameter, their values are summed
    /// and clamped to a reasonable range.
    pub fn get_modulation(&self, target: ModulationTarget) -> f32 {
        let total: f32 = self.routes
            .iter()
            .filter(|r| r.target == target && r.enabled)
            .map(|r| r.current_value)
            .sum();
        
        // Clamp to reasonable range
        total.clamp(-2.0, 2.0)
    }
    
    /// Get all modulations as a map
    pub fn get_all_modulations(&self) -> HashMap<ModulationTarget, f32> {
        let mut map = HashMap::new();
        for target in ModulationTarget::all() {
            map.insert(*target, self.get_modulation(*target));
        }
        map
    }
    
    /// Apply modulations to motion parameters
    /// 
    /// Returns a new MotionParams with all audio modulations applied
    pub fn apply_to_params(&self, base: &MotionParams) -> MotionParams {
        let mut result = *base;
        
        // Apply delay modulations (convert -1..1 to offset, clamp to valid range)
        let red_offset = self.get_modulation(ModulationTarget::RedDelay) * 4.0;
        result.red_delay = ((base.red_delay as f32 + red_offset).round() as u32).clamp(0, 16);
        
        let green_offset = self.get_modulation(ModulationTarget::GreenDelay) * 4.0;
        result.green_delay = ((base.green_delay as f32 + green_offset).round() as u32).clamp(0, 16);
        
        let blue_offset = self.get_modulation(ModulationTarget::BlueDelay) * 4.0;
        result.blue_delay = ((base.blue_delay as f32 + blue_offset).round() as u32).clamp(0, 16);
        
        // Apply intensity modulation
        let intensity_mod = self.get_modulation(ModulationTarget::Intensity);
        result.intensity = (base.intensity + intensity_mod).clamp(0.0, 1.0);
        
        // Apply input mix modulation
        let mix_mod = self.get_modulation(ModulationTarget::InputMix);
        result.input_mix = (base.input_mix + mix_mod).clamp(0.0, 1.0);
        
        // Apply trail fade modulation
        let fade_mod = self.get_modulation(ModulationTarget::TrailFade);
        result.trail_fade = (base.trail_fade + fade_mod).clamp(0.0, 1.0);
        
        // Apply threshold modulation
        let thresh_mod = self.get_modulation(ModulationTarget::Threshold);
        result.threshold = (base.threshold + thresh_mod).clamp(0.0, 1.0);
        
        // Apply smoothing modulation
        let smooth_mod = self.get_modulation(ModulationTarget::Smoothing);
        result.smoothing = (base.smoothing + smooth_mod).clamp(0.0, 1.0);
        
        // Apply gain modulations
        let red_gain_mod = self.get_modulation(ModulationTarget::RedGain);
        result.red_gain = (base.red_gain + red_gain_mod).clamp(-2.0, 2.0);
        
        let green_gain_mod = self.get_modulation(ModulationTarget::GreenGain);
        result.green_gain = (base.green_gain + green_gain_mod).clamp(-2.0, 2.0);
        
        let blue_gain_mod = self.get_modulation(ModulationTarget::BlueGain);
        result.blue_gain = (base.blue_gain + blue_gain_mod).clamp(-2.0, 2.0);
        
        result
    }
    
    /// Clear all routes
    pub fn clear(&mut self) {
        self.routes.clear();
    }
    
    /// Get number of routes
    pub fn len(&self) -> usize {
        self.routes.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
    
    /// Get max routes
    pub fn max_routes(&self) -> usize {
        self.max_routes
    }
    
    /// Check if can add more routes
    pub fn can_add_route(&self) -> bool {
        self.routes.len() < self.max_routes
    }
    
    /// Reset all smoothed values
    pub fn reset(&mut self) {
        for route in &mut self.routes {
            route.reset();
        }
    }

    /// Fix next_id after deserialization (serde skips it, defaults to 0)
    pub fn fix_next_id(&mut self) {
        self.next_id = self.routes.iter().map(|r| r.id).max().map_or(0, |id| id + 1);
    }
}

impl Default for RoutingMatrix {
    fn default() -> Self {
        Self::new(8)
    }
}

/// Audio routing state for the app
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRoutingState {
    /// The routing matrix
    pub matrix: RoutingMatrix,
    /// Whether audio routing is enabled
    pub enabled: bool,
    /// Show routing window
    #[serde(skip)]
    pub show_window: bool,
    /// Selected band for new route
    #[serde(skip)]
    pub selected_band: usize,
    /// Selected target for new route
    #[serde(skip)]
    pub selected_target: usize,
}

impl Default for AudioRoutingState {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRoutingState {
    pub fn new() -> Self {
        Self {
            matrix: RoutingMatrix::with_defaults(),
            enabled: false, // Disabled by default
            show_window: false,
            selected_band: 1, // Bass
            selected_target: 3, // Intensity
        }
    }
}
