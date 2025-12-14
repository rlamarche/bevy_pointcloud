use bevy_ecs::prelude::*;
use bevy_render::Extract;
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn reset_render_asset_bytes_per_frame(
    mut bpf_limiter: ResMut<RenderOctreeNodesBytesPerFrameLimiter>,
) {
    bpf_limiter.reset();
}

pub fn extract_render_asset_bytes_per_frame(
    bpf: Extract<Res<RenderOctreeNodesBytesPerFrame>>,
    mut bpf_limiter: ResMut<RenderOctreeNodesBytesPerFrameLimiter>,
) {
    bpf_limiter.max_bytes = bpf.max_bytes;
}

/// A resource that defines the amount of data allowed to be transferred from CPU to GPU
/// each frame, preventing choppy frames at the cost of waiting longer for GPU assets
/// to become available.
#[derive(Resource, Default)]
pub struct RenderOctreeNodesBytesPerFrame {
    pub max_bytes: Option<usize>,
}

impl RenderOctreeNodesBytesPerFrame {
    /// `max_bytes`: the number of bytes to write per frame.
    ///
    /// This is a soft limit: only full assets are written currently, uploading stops
    /// after the first asset that exceeds the limit.
    ///
    /// To participate, assets should implement [`RenderAsset::byte_len`]. If the default
    /// is not overridden, the assets are assumed to be small enough to upload without restriction.
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes: Some(max_bytes),
        }
    }
}

/// A render-world resource that facilitates limiting the data transferred from CPU to GPU
/// each frame, preventing choppy frames at the cost of waiting longer for GPU assets
/// to become available.
#[derive(Resource, Default)]
pub struct RenderOctreeNodesBytesPerFrameLimiter {
    /// Populated by [`RenderOctreeNodesBytesPerFrame`] during extraction.
    pub max_bytes: Option<usize>,
    /// Bytes written this frame.
    pub bytes_written: AtomicUsize,
}

impl RenderOctreeNodesBytesPerFrameLimiter {
    /// Reset the available bytes. Called once per frame during extraction by [`crate::RenderPlugin`].
    pub fn reset(&mut self) {
        if self.max_bytes.is_none() {
            return;
        }
        self.bytes_written.store(0, Ordering::Relaxed);
    }

    /// Check how many bytes are available for writing.
    pub fn available_bytes(&self, required_bytes: usize) -> usize {
        if let Some(max_bytes) = self.max_bytes {
            let total_bytes = self
                .bytes_written
                .fetch_add(required_bytes, Ordering::Relaxed);

            // The bytes available is the inverse of the amount we overshot max_bytes
            if total_bytes >= max_bytes {
                required_bytes.saturating_sub(total_bytes - max_bytes)
            } else {
                required_bytes
            }
        } else {
            required_bytes
        }
    }

    /// Decreases the available bytes for the current frame.
    pub(crate) fn write_bytes(&self, bytes: usize) {
        if self.max_bytes.is_some() && bytes > 0 {
            self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
        }
    }

    /// Returns `true` if there are no remaining bytes available for writing this frame.
    pub(crate) fn exhausted(&self) -> bool {
        if let Some(max_bytes) = self.max_bytes {
            let bytes_written = self.bytes_written.load(Ordering::Relaxed);
            bytes_written >= max_bytes
        } else {
            false
        }
    }
}
