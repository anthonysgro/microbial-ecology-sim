// COLD PATH: Terminal visualization module.
// Allocations, `anyhow`, and dynamic dispatch are permitted.

pub mod color;
pub mod glyph;
pub mod input;
pub mod renderer;
pub mod stats;

/// Which field layer is currently being visualized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMode {
    /// Chemical species by 0-based index.
    Chemical(usize),
    Heat,
}

impl OverlayMode {
    pub fn label(&self) -> String {
        match self {
            Self::Chemical(i) => format!("Chemical {i}"),
            Self::Heat => "Heat".into(),
        }
    }
}

/// Configuration for the terminal renderer.
pub struct RendererConfig {
    /// Milliseconds to sleep between frames.
    pub frame_delay_ms: u64,
    /// Initial overlay mode.
    pub initial_overlay: OverlayMode,
}

/// Result of polling for user input during the render loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    /// No key pressed or unrecognized key.
    None,
    /// Switch to a different overlay mode.
    SwitchOverlay(OverlayMode),
    /// User requested quit.
    Quit,
}
