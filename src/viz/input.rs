// COLD PATH: Input handling for terminal visualization.

use crossterm::event::{KeyCode, KeyEvent};

use super::{InputAction, OverlayMode};

/// Map a `crossterm` key event to an [`InputAction`].
///
/// - Digit keys `'1'`–`'9'` switch to the corresponding chemical overlay
///   (1-indexed input → 0-based `Chemical` index), provided the index is
///   within `num_chemicals`. Out-of-range digits are ignored.
/// - `'h'` → Heat overlay.
/// - `'m'` → Moisture overlay.
/// - `'q'` / `Esc` → Quit.
/// - Everything else → `None`.
pub fn map_key_event(event: KeyEvent, num_chemicals: usize) -> InputAction {
    match event.code {
        KeyCode::Char('q') | KeyCode::Esc => InputAction::Quit,
        KeyCode::Char('h') => InputAction::SwitchOverlay(OverlayMode::Heat),
        KeyCode::Char('m') => InputAction::SwitchOverlay(OverlayMode::Moisture),
        KeyCode::Char(c @ '1'..='9') => {
            let index = (c as usize) - ('1' as usize);
            if index < num_chemicals {
                InputAction::SwitchOverlay(OverlayMode::Chemical(index))
            } else {
                InputAction::None
            }
        }
        _ => InputAction::None,
    }
}
