//! Stub renderer module for migration period.
//!
//! This provides placeholder types while migrating to Bevy rendering.
//! All rendering operations are no-ops; Bevy handles actual rendering.

use marble_core::GameState;
use web_sys::HtmlCanvasElement;

use crate::camera::CameraState;
// Re-export gizmo types for compatibility
pub use crate::components::editor::gizmo::{CircleInstance, LineInstance, RectInstance};

/// Stub renderer - all operations are no-ops.
/// Bevy handles actual rendering via its own canvas and game loop.
pub struct WgpuRenderer {
    width: u32,
    height: u32,
}

impl WgpuRenderer {
    /// Creates a stub renderer. Always succeeds.
    pub async fn new(_canvas: HtmlCanvasElement) -> Result<Self, String> {
        tracing::info!("WgpuRenderer stub created - Bevy handles actual rendering");
        Ok(Self {
            width: 800,
            height: 600,
        })
    }

    /// Resize - no-op in stub.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Get width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Render - no-op in stub.
    pub fn render(&mut self, _game_state: &GameState, _camera: &CameraState) {
        // No-op: Bevy handles rendering
    }

    /// Render with overlay - no-op in stub.
    pub fn render_with_overlay(
        &mut self,
        _game_state: &GameState,
        _camera: &CameraState,
        _overlay_circles: &[CircleInstance],
        _overlay_lines: &[LineInstance],
        _overlay_rects: &[RectInstance],
    ) {
        // No-op: Bevy handles rendering
    }
}
