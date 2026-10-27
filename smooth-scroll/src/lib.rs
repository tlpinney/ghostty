//! A Ratatui scroll view for the experimental Ghostty smooth scroll region.
//!
//! The app declares its scroll view with `ESC _ smooth;top;bottom;pos;max ESC \`
//! after every draw. A patched Ghostty accumulates trackpad pixels over that
//! region, shifts it by the sub-row remainder, and sends one wheel event per
//! whole row. The app just scrolls one row per wheel event and redraws.
//! See SMOOTH_SCROLL.md at the repository root for the protocol.

use std::io::{self, Write};

use ratatui::buffer::Buffer;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, StatefulWidget, Widget};
use ratatui::{DefaultTerminal, Frame};
use unicode_width::UnicodeWidthStr;

/// Clears any declared region; send on exit.
pub const SMOOTH_OFF: &[u8] = b"\x1b_smooth;off\x1b\\";

/// Scroll position for [`SmoothScroll`].
#[derive(Default)]
pub struct SmoothScrollState {
    pub pos: usize,
    pub max: usize,
    /// Content rows visible at once (the region minus its overscan row).
    pub visible: usize,
    /// The region last rendered: (top row, overscan bottom row).
    region: Option<(u16, u16)>,
}

impl SmoothScrollState {
    pub fn scroll(&mut self, rows: isize) {
        self.pos = self.pos.saturating_add_signed(rows).min(self.max);
    }

    pub fn scroll_to(&mut self, pos: usize) {
        self.pos = pos.min(self.max);
    }

    /// The APC declaring the region, or clearing it when `enabled` is false.
    pub fn apc(&self, enabled: bool) -> String {
        match (enabled, self.region) {
            (true, Some((top, bottom))) => {
                format!("\x1b_smooth;{top};{bottom};{};{}\x1b\\", self.pos, self.max)
            }
            _ => "\x1b_smooth;off\x1b\\".to_string(),
        }
    }
}

/// A scroll view over pre-wrapped lines (one line per row). It draws one
/// extra line into its last row; Ghostty keeps that overscan row hidden and
/// reveals it as the region shifts.
pub struct SmoothScroll<'a> {
    pub lines: &'a [Line<'a>],
}

impl StatefulWidget for SmoothScroll<'_> {
    type State = SmoothScrollState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.height < 2 {
            state.region = None;
            return;
        }
        state.visible = area.height as usize - 1;
        state.max = self.lines.len().saturating_sub(state.visible);
        state.pos = state.pos.min(state.max);

        let end = (state.pos + area.height as usize).min(self.lines.len());
        Paragraph::new(self.lines[state.pos..end].to_vec()).render(area, buf);
        state.region = Some((area.y, area.y + area.height - 1));
    }
}

/// Draw a frame and declare the region in one synchronized update, so
/// Ghostty never shows new content with the old offset.
pub fn draw_smooth(
    terminal: &mut DefaultTerminal,
    state: &mut SmoothScrollState,
    smooth: bool,
    render: impl FnOnce(&mut Frame, &mut SmoothScrollState),
) -> io::Result<()> {
    execute!(io::stdout(), BeginSynchronizedUpdate)?;
    terminal.draw(|f| render(f, state))?;
    // The region comes from this draw, so declare it after rendering.
    let mut out = io::stdout();
    out.write_all(state.apc(smooth).as_bytes())?;
    execute!(out, EndSynchronizedUpdate)
}

/// Greedy word wrap of a styled line to `width` columns. Words longer than
/// the width are split. Leading indentation is repeated on continuation
/// rows so wrapped list items and quotes stay aligned.
pub fn wrap_line(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let indent_len = text.len() - text.trim_start().len();
    let indent = &text[..indent_len];
    let indent = if indent.width() * 2 < width { indent.to_string() } else { String::new() };

    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut cur: Vec<Span<'static>> = Vec::new();
    let mut cur_w = 0;
    let flush = |cur: &mut Vec<Span<'static>>, cur_w: &mut usize, rows: &mut Vec<Line<'static>>| {
        rows.push(Line::from(std::mem::take(cur)).style(line.style));
        *cur_w = 0;
    };

    for span in &line.spans {
        for word in span.content.split_inclusive(' ') {
            let mut word = word.to_string();
            let trimmed_w = word.trim_end().width();
            if cur_w > 0 && cur_w + trimmed_w > width {
                flush(&mut cur, &mut cur_w, &mut rows);
                if !indent.is_empty() {
                    cur.push(Span::raw(indent.clone()));
                    cur_w = indent.width();
                }
                word = word.trim_start().to_string();
            }
            // Hard-split words that can't fit on a row by themselves.
            while cur_w + word.trim_end().width() > width && !word.is_empty() {
                let room = width.saturating_sub(cur_w).max(1);
                let mut split = 0;
                let mut w = 0;
                for (i, c) in word.char_indices() {
                    let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                    if w + cw > room {
                        break;
                    }
                    w += cw;
                    split = i + c.len_utf8();
                }
                if split == 0 {
                    split = word.chars().next().map_or(0, char::len_utf8);
                }
                let rest = word.split_off(split);
                cur.push(Span::styled(word, span.style));
                flush(&mut cur, &mut cur_w, &mut rows);
                word = rest;
            }
            if !word.is_empty() {
                cur_w += word.width();
                cur.push(Span::styled(word, span.style));
            }
        }
    }
    if !cur.is_empty() || rows.is_empty() {
        rows.push(Line::from(cur).style(line.style));
    }
    rows
}
