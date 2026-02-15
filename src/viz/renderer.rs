// COLD PATH: Terminal renderer implementation.
// Allocations, `anyhow`, and dynamic dispatch are permitted.

use std::io::{self, Stdout, Write};

use anyhow::Context;
use crossterm::{
    cursor,
    event::{self, Event},
    style::{self, Stylize},
    terminal, ExecutableCommand, QueueableCommand,
};

use crate::grid::Grid;
use crate::viz::color::{chemical_color, heat_color, moisture_bg_color};
use crate::viz::glyph::value_to_glyph;
use crate::viz::input::map_key_event;
use crate::viz::stats::{compute_stats, format_stats_bar};
use crate::viz::{InputAction, OverlayMode, RendererConfig};

/// Number of terminal rows reserved for the stats bar below the grid.
const STATS_LINES: u16 = 1;

/// Normalize a raw field buffer into `out`, dividing each element by the buffer maximum.
///
/// Returns the max value found in `raw`.
///
/// - When max is near zero (< 1e-9), all outputs are 0.0 (Req 1.3).
/// - When all values are identical and non-zero, all outputs are 1.0 (Req 9.3).
/// - Otherwise, outputs are in `[0.0, 1.0]` (Req 1.2).
pub fn normalize_field(raw: &[f32], out: &mut Vec<f32>) -> f32 {
    let max_val = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    // Guard: empty slice or near-zero max → all zeros.
    let max_val = if raw.is_empty() { 0.0 } else { max_val };
    let divisor = if max_val.abs() < 1e-9 { 1.0 } else { max_val };

    out.clear();
    out.reserve(raw.len());
    for &v in raw {
        out.push(if max_val.abs() < 1e-9 { 0.0 } else { v / divisor });
    }
    max_val
}

/// COLD-path terminal renderer.
///
/// Borrows `&Grid` immutably each frame. Owns the terminal handle,
/// overlay state, and a pre-allocated normalization buffer.
pub struct Renderer {
    config: RendererConfig,
    overlay: OverlayMode,
    terminal_width: u16,
    terminal_height: u16,
    stdout: Stdout,
    /// Pre-allocated buffer reused each frame to avoid per-frame allocation.
    norm_buffer: Vec<f32>,
}

impl Renderer {
    /// Enter alternate screen, enable raw mode, hide cursor, query terminal size.
    ///
    /// Requirements: 4.3 (enter alt screen, hide cursor), 8.3 (query terminal size).
    pub fn init(config: RendererConfig) -> anyhow::Result<Self> {
        let (tw, th) = terminal::size().unwrap_or((80, 24));

        let mut stdout = io::stdout();
        stdout
            .execute(terminal::EnterAlternateScreen)
            .context("enter alternate screen")?;
        terminal::enable_raw_mode().context("enable raw mode")?;
        stdout
            .execute(cursor::Hide)
            .context("hide cursor")?;

        let overlay = config.initial_overlay;

        Ok(Self {
            config,
            overlay,
            terminal_width: tw,
            terminal_height: th,
            stdout,
            norm_buffer: Vec::new(),
        })
    }

    /// Restore terminal: leave alternate screen, show cursor, disable raw mode.
    ///
    /// Requirements: 4.4 (restore original screen, show cursor, reset attributes).
    pub fn shutdown(&mut self) -> anyhow::Result<()> {
        self.stdout
            .execute(cursor::Show)
            .context("show cursor")?;
        self.stdout
            .execute(terminal::LeaveAlternateScreen)
            .context("leave alternate screen")?;
        terminal::disable_raw_mode().context("disable raw mode")?;
        Ok(())
    }

    /// Switch the active overlay mode.
    pub fn set_overlay(&mut self, mode: OverlayMode) {
        self.overlay = mode;
    }

    /// Compute the clipped render dimensions for the current grid and terminal.
    ///
    /// Requirements: 8.1 (any grid size), 8.2 (clip to terminal viewport).
    fn render_dimensions(&self, grid: &Grid) -> (u16, u16) {
        let render_width = grid.width().min(u32::from(self.terminal_width)) as u16;
        let render_height = grid
            .height()
            .min(u32::from(self.terminal_height.saturating_sub(STATS_LINES)))
            as u16;
        (render_width, render_height)
    }

    /// Draw one frame: select field buffer, normalize, compute stats,
    /// write glyph+color per cell, then the stats bar.
    ///
    /// Requirements: 4.1 (cursor to origin), 4.2 (overwrite in place),
    /// 5.1 (one frame per tick), 7.2 (stats from same snapshot), 7.3 (stats below grid).
    pub fn render_frame(&mut self, grid: &Grid, tick: u64) -> anyhow::Result<()> {
        // Select the raw field buffer based on current overlay.
        let raw_buffer: &[f32] = match self.overlay {
            OverlayMode::Chemical(species) => grid
                .read_chemical(species)
                .context("read chemical buffer")?,
            OverlayMode::Heat => grid.read_heat(),
            OverlayMode::Moisture => grid.read_moisture(),
        };

        // Normalize into the pre-allocated buffer.
        normalize_field(raw_buffer, &mut self.norm_buffer);

        // Compute stats from the raw buffer for meaningful physical values.
        let gw = grid.width() as usize;
        let gh = grid.height() as usize;
        let center_index = (gh / 2) * gw + (gw / 2);
        let stats = compute_stats(raw_buffer, center_index);

        let (render_w, render_h) = self.render_dimensions(grid);

        // Move cursor to top-left origin (Req 4.1).
        self.stdout.queue(cursor::MoveTo(0, 0))?;

        // Iterate over clipped rows/cols, writing glyph+color per cell.
        for y in 0..render_h {
            self.stdout.queue(cursor::MoveTo(0, y))?;
            for x in 0..render_w {
                let idx = (y as usize) * gw + (x as usize);
                let norm_val = self.norm_buffer[idx];
                let glyph = value_to_glyph(norm_val);

                match self.overlay {
                    OverlayMode::Heat => {
                        let fg = heat_color(norm_val);
                        self.stdout
                            .queue(style::PrintStyledContent(
                                style::style(glyph).with(fg),
                            ))?;
                    }
                    OverlayMode::Moisture => {
                        let bg = moisture_bg_color(norm_val);
                        let fg = crossterm::style::Color::White;
                        self.stdout
                            .queue(style::PrintStyledContent(
                                style::style(glyph).with(fg).on(bg),
                            ))?;
                    }
                    OverlayMode::Chemical(_) => {
                        let fg = chemical_color(norm_val);
                        self.stdout
                            .queue(style::PrintStyledContent(
                                style::style(glyph).with(fg),
                            ))?;
                    }
                }
            }
            // Clear remainder of line to avoid stale characters from wider previous frames.
            self.stdout
                .queue(terminal::Clear(terminal::ClearType::UntilNewLine))?;
        }

        // Stats bar below the grid (Req 7.3).
        let bar = format_stats_bar(tick, &self.overlay, &stats);
        self.stdout.queue(cursor::MoveTo(0, render_h))?;
        self.stdout.queue(style::Print(bar))?;
        self.stdout
            .queue(terminal::Clear(terminal::ClearType::UntilNewLine))?;

        self.stdout.flush().context("flush frame")?;
        Ok(())
    }

    /// Poll for a single key event with zero timeout (non-blocking).
    ///
    /// Delegates to `map_key_event()` for key-to-action translation.
    ///
    /// Requirements: 5.3 (quit on q/Esc), 6.1–6.4 (overlay switching).
    pub fn poll_input(&mut self, num_chemicals: usize) -> anyhow::Result<InputAction> {
        if event::poll(std::time::Duration::ZERO).context("poll input")? {
            if let Event::Key(key_event) = event::read().context("read key event")? {
                return Ok(map_key_event(key_event, num_chemicals));
            }
        }
        Ok(InputAction::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_zero_buffer_normalizes_to_zero() {
        let raw = vec![0.0; 5];
        let mut out = Vec::new();
        let max = normalize_field(&raw, &mut out);
        assert!(max.abs() < 1e-9);
        assert!(out.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn identical_nonzero_normalizes_to_one() {
        let raw = vec![3.5, 3.5, 3.5, 3.5];
        let mut out = Vec::new();
        let max = normalize_field(&raw, &mut out);
        assert!((max - 3.5).abs() < f32::EPSILON);
        assert!(out.iter().all(|&v| (v - 1.0).abs() < f32::EPSILON));
    }

    #[test]
    fn basic_normalization() {
        let raw = vec![0.0, 5.0, 10.0];
        let mut out = Vec::new();
        let max = normalize_field(&raw, &mut out);
        assert!((max - 10.0).abs() < f32::EPSILON);
        assert!((out[0] - 0.0).abs() < f32::EPSILON);
        assert!((out[1] - 0.5).abs() < f32::EPSILON);
        assert!((out[2] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_buffer() {
        let raw: Vec<f32> = vec![];
        let mut out = Vec::new();
        let max = normalize_field(&raw, &mut out);
        assert!(max.abs() < f32::EPSILON);
        assert!(out.is_empty());
    }

    #[test]
    fn reuses_output_buffer() {
        let raw = vec![2.0, 4.0];
        let mut out = vec![99.0; 10]; // pre-filled with junk
        normalize_field(&raw, &mut out);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.5).abs() < f32::EPSILON);
        assert!((out[1] - 1.0).abs() < f32::EPSILON);
    }
}
