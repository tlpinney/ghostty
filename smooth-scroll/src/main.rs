//! Ratatui demo for the experimental Ghostty smooth scroll region.
//!
//! Keys: j/k or arrows scroll, s toggles the smooth declaration, q quits.
//! See SMOOTH_SCROLL.md at the repository root for the protocol.

use std::io::{self, Write};
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::{DefaultTerminal, Frame};
use smooth_demo::{draw_smooth, SmoothScroll, SmoothScrollState, SMOOTH_OFF};

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
        draw_smooth(terminal, &mut state, smooth, |f, s| draw(f, &lines, s, smooth))?;

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
    out.write_all(SMOOTH_OFF)?;
    execute!(out, DisableMouseCapture)?;
    ratatui::restore();
    result
}
