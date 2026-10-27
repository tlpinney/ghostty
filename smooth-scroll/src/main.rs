//! Ratatui demo for the experimental Ghostty smooth scroll region.
//!
//! The app declares its scroll view with `ESC _ smooth;top;bottom;pos;max ESC \`
//! after every draw. A patched Ghostty accumulates trackpad pixels over that
//! region, shifts it by the sub-row remainder, and sends one wheel event per
//! whole row. The app just scrolls one row per wheel event and redraws.
//!
//! Keys: j/k or arrows scroll, s toggles the smooth declaration, q quits.
//! See SMOOTH_SCROLL.md at the repository root for the protocol.

use std::io::{self, Write};
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, StatefulWidget, Widget};
use ratatui::{DefaultTerminal, Frame};

/// Scroll position for [`SmoothScroll`].
#[derive(Default)]
struct SmoothScrollState {
    pos: usize,
    max: usize,
    /// The region last rendered: (top row, overscan bottom row).
    region: Option<(u16, u16)>,
}

impl SmoothScrollState {
    fn scroll(&mut self, rows: isize) {
        self.pos = self.pos.saturating_add_signed(rows).min(self.max);
    }

    /// The APC declaring the region, or clearing it when `enabled` is false.
    fn apc(&self, enabled: bool) -> String {
        match (enabled, self.region) {
            (true, Some((top, bottom))) => {
                format!("\x1b_smooth;{top};{bottom};{};{}\x1b\\", self.pos, self.max)
            }
            _ => "\x1b_smooth;off\x1b\\".to_string(),
        }
    }
}

/// A scroll view that draws one extra line into its last row. Ghostty
/// keeps that overscan row hidden and reveals it as the region shifts.
struct SmoothScroll<'a> {
    lines: &'a [Line<'a>],
}

impl StatefulWidget for SmoothScroll<'_> {
    type State = SmoothScrollState;

    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer, state: &mut Self::State) {
        if area.height < 2 {
            state.region = None;
            return;
        }
        let visible = area.height as usize - 1;
        state.max = self.lines.len().saturating_sub(visible);
        state.pos = state.pos.min(state.max);

        let end = (state.pos + area.height as usize).min(self.lines.len());
        Paragraph::new(self.lines[state.pos..end].to_vec()).render(area, buf);
        state.region = Some((area.y, area.y + area.height - 1));
    }
}

fn sample_lines() -> Vec<Line<'static>> {
    let words = "render scroll pixel glyph metal shader atlas cell grid frame \
                 vsync easing momentum viewport offset ghostty ratatui region";
    let words: Vec<&str> = words.split_whitespace().collect();
    (0..500)
        .map(|i| {
            if i % 20 == 0 {
                return Line::from(format!("── section {} {}", i / 20 + 1, "─".repeat(60)))
                    .fg(Color::Cyan);
            }
            let body: Vec<&str> = (0..6 + i % 9).map(|k| words[(i * 7 + k * 3) % words.len()]).collect();
            Line::from(vec![
                Span::styled(format!("{i:4} │ "), Style::new().dark_gray()),
                Span::raw(body.join(" ")),
            ])
        })
        .collect()
}

fn draw(frame: &mut Frame, lines: &[Line], state: &mut SmoothScrollState, smooth: bool) {
    let [header, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    frame.render_widget(
        Line::from(" Ratatui smooth scroll demo (trackpad over the text)").bold().reversed(),
        header,
    );
    frame.render_stateful_widget(SmoothScroll { lines }, body, state);
    frame.render_widget(
        Line::from(format!(
            " {} │ pos {}/{} │ j/k scroll  s toggle smooth  q quit",
            if smooth { "SMOOTH" } else { "rows" },
            state.pos,
            state.max
        ))
        .reversed(),
        status,
    );
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let lines = sample_lines();
    let mut state = SmoothScrollState::default();
    let mut smooth = true;

    loop {
        // Cells and the region declaration land atomically, so Ghostty
        // never shows new content with the old offset.
        execute!(io::stdout(), BeginSynchronizedUpdate)?;
        terminal.draw(|f| draw(f, &lines, &mut state, smooth))?;
        let mut out = io::stdout();
        out.write_all(state.apc(smooth).as_bytes())?;
        execute!(out, EndSynchronizedUpdate)?;

        // Block for the first event, then drain any queued ones before redrawing.
        let mut ev = Some(event::read()?);
        while let Some(e) = ev.take() {
            match e {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => state.scroll(1),
                    KeyCode::Char('k') | KeyCode::Up => state.scroll(-1),
                    KeyCode::Char(' ') | KeyCode::PageDown => state.scroll(20),
                    KeyCode::Char('b') | KeyCode::PageUp => state.scroll(-20),
                    KeyCode::Char('s') => smooth = !smooth,
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollDown => state.scroll(1),
                    MouseEventKind::ScrollUp => state.scroll(-1),
                    _ => {}
                },
                _ => {}
            }
            if event::poll(Duration::ZERO)? {
                ev = Some(event::read()?);
            }
        }
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal);
    let mut out = io::stdout();
    out.write_all(b"\x1b_smooth;off\x1b\\")?;
    execute!(out, DisableMouseCapture)?;
    ratatui::restore();
    result
}
