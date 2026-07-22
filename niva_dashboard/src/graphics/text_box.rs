#![allow(dead_code)]
use std::collections::VecDeque;

use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;

/// Reusable scrolling monospace text box, similar to a terminal — always shows the most
/// recent output, bottom-anchored. Manual backward scrolling is not supported: the ring
/// buffer only needs to hold enough logical lines to keep the visible area full.
pub struct TextBoxRenderer {
    lines: VecDeque<String>,
    max_lines: usize,
}

impl TextBoxRenderer {
    pub fn new(max_lines: usize) -> Self {
        TextBoxRenderer {
            lines: VecDeque::with_capacity(max_lines),
            max_lines,
        }
    }

    /// Append a logical line, evicting the oldest one once at capacity.
    pub fn push_line(&mut self, line: impl Into<String>) {
        if self.lines.len() >= self.max_lines {
            self.lines.pop_front();
        }
        self.lines.push_back(line.into());
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Renders the box at (x, y) with the given size. Long logical lines are hard-wrapped
    /// (character grid, like a terminal — no word wrapping); only the trailing rows that
    /// fit the box height are drawn.
    pub fn render(
        &self,
        context: &mut GraphicsContext,
        ui_style: &UIStyle,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let font = ui_style.get_string(TEXT_MONOSPACE_FONT, TERMINAL_FONT_PATH);
        let font_size = ui_style.get_integer(TEXT_MONOSPACE_FONT_SIZE, 16);
        let text_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (1.0, 1.0, 1.0));
        let bg_enabled = ui_style.get_bool(TERMINAL_BACKGROUND_ENABLED, true);
        let bg_color = ui_style.get_color(TERMINAL_BACKGROUND_COLOR, (0.0, 0.0, 0.0));
        let border_enabled = ui_style.get_bool(TERMINAL_BORDER_ENABLED, true);
        let border_color = ui_style.get_color(TERMINAL_BORDER_COLOR, (1.0, 1.0, 1.0));
        let border_width = ui_style.get_float(TERMINAL_BORDER_WIDTH, 2.0);
        let padding = ui_style.get_float(TERMINAL_PADDING, 8.0);

        if bg_enabled {
            context.fill_rect(x, y, width, height, bg_color)?;
        }

        let inner_x = x + padding;
        let inner_y = y + padding;
        let inner_width = (width - 2.0 * padding).max(0.0);
        let inner_height = (height - 2.0 * padding).max(0.0);

        // Monospace font: any character's advance is the column width.
        let char_width = context.calculate_text_width_with_font("0", 1.0, &font, font_size)?;
        let chars_per_line = if char_width > 0.0 {
            ((inner_width / char_width).floor() as usize).max(1)
        } else {
            1
        };

        let line_height = context.get_line_height_with_font(1.0, &font, font_size)?;
        let visible_rows = if line_height > 0.0 {
            (inner_height / line_height).floor() as usize
        } else {
            0
        };

        if visible_rows > 0 {
            let mut wrapped: Vec<String> = Vec::new();
            for line in &self.lines {
                wrap_line(line, chars_per_line, &mut wrapped);
            }
            let start = wrapped.len().saturating_sub(visible_rows);
            let visible = &wrapped[start..];

            // Bottom-anchor: pin the last row to the bottom of the box instead of the top,
            // so a partially-filled box still reads like a terminal (output hugs the bottom).
            let top_y = inner_y + (inner_height - visible.len() as f32 * line_height).max(0.0);
            for (i, row) in visible.iter().enumerate() {
                let row_y = top_y + i as f32 * line_height;
                context.render_text_with_font(row, inner_x, row_y, 1.0, text_color, &font, font_size)?;
            }
        }

        if border_enabled {
            context.stroke_rect(x, y, width, height, border_color, border_width)?;
        }

        Ok(())
    }
}

/// Hard-wraps a single logical line into fixed-width visual rows (character grid, no word
/// wrapping — matches how a terminal wraps output).
fn wrap_line(line: &str, chars_per_line: usize, out: &mut Vec<String>) {
    if line.is_empty() {
        out.push(String::new());
        return;
    }
    let chars: Vec<char> = line.chars().collect();
    for chunk in chars.chunks(chars_per_line.max(1)) {
        out.push(chunk.iter().collect());
    }
}
