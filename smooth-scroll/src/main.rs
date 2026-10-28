//! Ratatui demo for the experimental Ghostty smooth scroll region.
//!
//! Usage: smooth-demo [--split]
//!
//! `--split` shows two scroll views side by side (regions 0 and 1), each
//! scrolled independently by the trackpad, like panes in a multiplexer.
//!
//! Keys: j/k or arrows scroll, tab switches the keyboard pane (split),
//! s toggles the smooth declaration, q quits.
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

fn sample_lines(seed: usize) -> Vec<Line<'static>> {
    let words = "render scroll pixel glyph metal shader atlas cell grid frame \
                 vsync easing momentum viewport offset ghostty ratatui region";
    let words: Vec<&str> = words.split_whitespace().collect();
    let accent = if seed == 0 { Color::Cyan } else { Color::Magenta };
    (0..500)
        .map(|i| {
            if i % 20 == 0 {
                return Line::from(format!("── section {} {}", i / 20 + 1, "─".repeat(60)))
                    .fg(accent);
            }
            let body: Vec<&str> = (0..6 + (i + seed) % 9)
                .map(|k| words[(i * 7 + k * 3 + seed * 5) % words.len()])
                .collect();
            Line::from(vec![
                Span::styled(format!("{i:4} │ "), Style::new().dark_gray()),
                Span::raw(body.join(" ")),
            ])
        })
        .collect()
}

fn draw(frame: &mut Frame, panes: &[Vec<Line>], states: &mut [SmoothScrollState], focus: usize, smooth: bool) {
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

    // Side-by-side panes with a one-column gutter between them.
    let mut constraints = Vec::new();
    for i in 0..panes.len() {
        if i > 0 {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Fill(1));
    }
    let areas = Layout::horizontal(constraints).split(body);
    for (i, (lines, state)) in panes.iter().zip(states.iter_mut()).enumerate() {
        if i > 0 {
            frame.render_widget(
                ratatui::widgets::Paragraph::new(vec![Line::from("│").dark_gray(); body.height as usize]),
                areas[i * 2 - 1],
            );
        }
        frame.render_stateful_widget(SmoothScroll::new(lines), areas[i * 2], state);
    }

    let positions: Vec<String> = states.iter().map(|s| format!("{}/{}", s.pos, s.max)).collect();
    let keys = if panes.len() > 1 { "tab pane  j/k" } else { "j/k" };
    frame.render_widget(
        Line::from(format!(
            " {} │ pos {} │ pane {} │ {keys} scroll  s toggle smooth  q quit",
            if smooth { "SMOOTH" } else { "rows" },
            positions.join("  "),
            focus + 1,
        ))
        .reversed(),
        status,
    );
}

fn run(terminal: &mut DefaultTerminal, split: bool) -> io::Result<()> {
    let n = if split { 2 } else { 1 };
    let panes: Vec<Vec<Line>> = (0..n).map(sample_lines).collect();
    let mut states: Vec<SmoothScrollState> = (0..n as u32).map(SmoothScrollState::with_id).collect();
    let mut focus = 0;
    let mut smooth = true;

    loop {
        draw_smooth(terminal, &mut states, smooth, |f, s| draw(f, &panes, s, focus, smooth))?;

        // Block for the first event, then drain any queued ones before redrawing.
        let mut ev = Some(event::read()?);
        while let Some(e) = ev.take() {
            match e {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => states[focus].scroll(1),
                    KeyCode::Char('k') | KeyCode::Up => states[focus].scroll(-1),
                    KeyCode::Char(' ') | KeyCode::PageDown => states[focus].scroll(20),
                    KeyCode::Char('b') | KeyCode::PageUp => states[focus].scroll(-20),
                    KeyCode::Tab => focus = (focus + 1) % n,
                    KeyCode::Char('s') => smooth = !smooth,
                    _ => {}
                },
                Event::Mouse(m) => {
                    // Wheel events scroll whichever pane is under the pointer.
                    let target = states.iter().position(|s| s.contains(m.column, m.row));
                    match (m.kind, target) {
                        (MouseEventKind::ScrollDown, Some(i)) => states[i].scroll(1),
                        (MouseEventKind::ScrollUp, Some(i)) => states[i].scroll(-1),
                        _ => {}
                    }
                }
                _ => {}
            }
            if event::poll(Duration::ZERO)? {
                ev = Some(event::read()?);
            }
        }
    }
}

fn main() -> io::Result<()> {
    let split = std::env::args().any(|a| a == "--split");
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal, split);
    let mut out = io::stdout();
    out.write_all(SMOOTH_OFF)?;
    execute!(out, DisableMouseCapture)?;
    ratatui::restore();
    result
}
