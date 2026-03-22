//! # Frame History Buffer
//!
//! Ring buffer for storing past frame textures, enabling time-based effects
//! like Posy's RGB delay motion extraction technique.
//!
//! Uses GPU-to-GPU copies for efficiency, storing frames as BGRA8 textures.

use crate::engine::texture::Texture;
use std::sync::Arc;

/// Ring buffer storing frame history for time-delayed effects
pub struct FrameHistory {
    /// GPU textures for frame storage (ring buffer)
    frames: Vec<Texture>,
    /// Current write index (points to oldest frame)
    write_index: usize,
    /// Maximum number of frames to store
    max_history: usize,
    /// Current frame dimensions
    width: u32,
    height: u32,
    /// Device reference for creating new textures
    device: Arc<wgpu::Device>,
    /// Whether history has been initialized
    initialized: bool,
}

impl FrameHistory {
    /// Maximum supported history size
    pub const MAX_HISTORY: usize = 16;
    /// Default history size for 720p/1080p
    pub const DEFAULT_HISTORY: usize = 8;
    /// Reduced history size for 4K to save memory
    pub const REDUCED_HISTORY_4K: usize = 6;
    
    /// Create new frame history with specified size
    /// 
    /// # Arguments
    /// * `device` - wgpu device for texture creation
    /// * `queue` - wgpu queue (kept for API consistency, not used directly)
    /// * `max_history` - Number of frames to store (1-16)
    pub fn new(
        device: Arc<wgpu::Device>,
        _queue: Arc<wgpu::Queue>,
        max_history: usize,
    ) -> Self {
        let max_history = max_history.clamp(1, Self::MAX_HISTORY);
        
        Self {
            frames: Vec::with_capacity(max_history),
            write_index: 0,
            max_history,
            width: 0,
            height: 0,
            device,
            initialized: false,
        }
    }
    
    /// Initialize or resize the frame history
    /// 
    /// Creates frame textures at the specified resolution.
    /// If already initialized at a different size, clears and recreates.
    fn ensure_size(&mut self, width: u32, height: u32) {
        if self.initialized && self.width == width && self.height == height {
            return;
        }
        
        log::info!(
            "FrameHistory: initializing {} frames at {}x{}",
            self.max_history,
            width,
            height
        );
        
        // Clear existing frames
        self.frames.clear();
        self.write_index = 0;
        
        // Create new frame textures
        for i in 0..self.max_history {
            let tex = Texture::create_render_target(
                &self.device,
                width,
                height,
                &format!("Frame History {}", i),
            );
            self.frames.push(tex);
        }
        
        self.width = width;
        self.height = height;
        self.initialized = true;
    }
    
    /// Push a new frame into the history ring buffer
    /// 
    /// Copies the source texture to the current write position and advances.
    /// This overwrites the oldest frame.
    pub fn push_frame(
        &mut self,
        source: &wgpu::Texture,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let width = source.width();
        let height = source.height();
        
        // Initialize if needed (or resize if dimensions changed)
        self.ensure_size(width, height);
        
        // Get the frame to write to
        let dest_frame = &self.frames[self.write_index];
        
        // Copy source texture to history frame
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &dest_frame.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        
        // Advance write index (wraps around)
        self.write_index = (self.write_index + 1) % self.max_history;
    }
    
    /// Get a frame from N frames ago
    /// 
    /// # Arguments
    /// * `frames_ago` - Number of frames to go back (0 = most recent written)
    /// 
    /// # Returns
    /// Reference to the texture from N frames ago, or None if not available
    pub fn get_frame(&self, frames_ago: usize) -> Option<&Texture> {
        if !self.initialized || frames_ago >= self.max_history {
            return None;
        }
        
        // Calculate index: write_index - 1 - frames_ago (with wrap)
        // write_index points to next write position, so -1 is most recent
        let index = if frames_ago < self.write_index {
            self.write_index - 1 - frames_ago
        } else {
            self.max_history - 1 - (frames_ago - self.write_index)
        };
        
        self.frames.get(index)
    }
    
    /// Get frame at a specific history index (0 = newest, max-1 = oldest)
    /// 
    /// This is useful for shaders that need specific delay offsets
    pub fn get_frame_by_delay(&self, delay_frames: usize) -> Option<&Texture> {
        self.get_frame(delay_frames)
    }
    
    /// Get texture view from N frames ago
    pub fn get_view(&self, frames_ago: usize) -> Option<&wgpu::TextureView> {
        self.get_frame(frames_ago).map(|t| &t.view)
    }
    
    /// Get sampler from N frames ago (all use same sampler settings)
    pub fn get_sampler(&self, frames_ago: usize) -> Option<&wgpu::Sampler> {
        self.get_frame(frames_ago).map(|t| &t.sampler)
    }
    
    /// Get the most recent frame (0 frames ago)
    pub fn current(&self) -> Option<&Texture> {
        self.get_frame(0)
    }
    
    /// Clear all history frames to black
    pub fn clear(&self, queue: &wgpu::Queue) {
        for frame in &self.frames {
            frame.clear_to_black(queue);
        }
    }
    
    /// Resize history to new dimensions (clears all history)
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            log::info!("FrameHistory: resizing from {}x{} to {}x{}", self.width, self.height, width, height);
            self.initialized = false;
            self.ensure_size(width, height);
        }
    }
    
    /// Get current resolution
    pub fn resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    
    /// Get maximum history size
    pub fn max_history(&self) -> usize {
        self.max_history
    }
    
    /// Check if history is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    
    /// Calculate memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        // BGRA8 = 4 bytes per pixel
        let pixels = (self.width * self.height) as usize;
        let bytes_per_frame = pixels * 4;
        bytes_per_frame * self.frames.len()
    }
    
    /// Get human-readable memory usage string
    pub fn memory_usage_string(&self) -> String {
        let bytes = self.memory_usage();
        if bytes >= 1024 * 1024 * 1024 {
            format!("{:.1} GB", bytes as f32 / (1024.0 * 1024.0 * 1024.0))
        } else if bytes >= 1024 * 1024 {
            format!("{:.1} MB", bytes as f32 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} KB", bytes as f32 / 1024.0)
        }
    }
    
    /// Adjust history size (clears existing history)
    /// 
    /// Use this when changing between resolution presets
    pub fn set_history_size(&mut self, max_history: usize) {
        let new_size = max_history.clamp(1, Self::MAX_HISTORY);
        if new_size != self.max_history {
            log::info!("FrameHistory: changing size from {} to {}", self.max_history, new_size);
            self.max_history = new_size;
            self.frames.clear();
            self.write_index = 0;
            self.initialized = false;
            
            // Re-initialize if we have dimensions
            if self.width > 0 && self.height > 0 {
                self.ensure_size(self.width, self.height);
            }
        }
    }
    
    /// Get recommended history size for a given resolution
    /// 
    /// Balances memory usage vs effect quality
    pub fn recommended_history_for_resolution(width: u32, height: u32) -> usize {
        let pixels = width * height;
        if pixels >= 3840 * 2160 {
            // 4K: use reduced history
            Self::REDUCED_HISTORY_4K
        } else if pixels >= 2560 * 1440 {
            // 1440p: slightly reduced
            7
        } else {
            // 720p/1080p: full history
            Self::DEFAULT_HISTORY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_frame_indexing() {
        // This test would need a wgpu device to run properly
        // Just testing the math logic here
        
        // Simulate ring buffer with 8 frames, write_index at 3
        let max_history = 8;
        let write_index = 3;
        
        // Most recent frame (0 ago) should be at index 2
        let frames_ago = 0;
        let index = if frames_ago < write_index {
            write_index - 1 - frames_ago
        } else {
            max_history - 1 - (frames_ago - write_index)
        };
        assert_eq!(index, 2);
        
        // Frame 2 ago should be at index 0
        let frames_ago = 2;
        let index = if frames_ago < write_index {
            write_index - 1 - frames_ago
        } else {
            max_history - 1 - (frames_ago - write_index)
        };
        assert_eq!(index, 0);
        
        // Frame 3 ago should wrap to index 7
        let frames_ago = 3;
        let index = if frames_ago < write_index {
            write_index - 1 - frames_ago
        } else {
            max_history - 1 - (frames_ago - write_index)
        };
        assert_eq!(index, 7);
    }
    
    #[test]
    fn test_memory_calculation() {
        // 1920x1080 @ 8 frames
        let pixels = 1920u32 * 1080u32;
        let bytes_per_frame = pixels as usize * 4;
        let total = bytes_per_frame * 8;
        
        // Should be around 66 MB
        assert_eq!(total, 1920 * 1080 * 4 * 8);
        assert!(total > 60 * 1024 * 1024);
        assert!(total < 70 * 1024 * 1024);
    }
}
