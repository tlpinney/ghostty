//! A Markdown reader using the experimental Ghostty smooth scroll region.
//!
//! Usage: smooth-md [FILE.md]   (default: README.md)
//!
//! Keys: j/k or arrows line, d/u half page, space/b page, g/G top/bottom,
//! ]/[ next/previous heading, r reload, s toggle smooth, q quit.

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::{DefaultTerminal, Frame};
use smooth_demo::{draw_smooth, wrap_line, SmoothScroll, SmoothScrollState, SMOOTH_OFF};

/// Readable line length; wider terminals get centered text.
const MAX_WIDTH: u16 = 100;

struct Doc {
    path: PathBuf,
    source: String,
    /// Wrapped rows for `width`, and which rows start a heading.
    width: u16,
    rows: Vec<Line<'static>>,
    headings: Vec<usize>,
}

impl Doc {
    fn load(path: PathBuf) -> io::Result<Self> {
        let source = std::fs::read_to_string(&path)?;
        Ok(Self { path, source, width: 0, rows: Vec::new(), headings: Vec::new() })
    }

    fn reload(&mut self) -> io::Result<()> {
        self.source = std::fs::read_to_string(&self.path)?;
        self.width = 0;
        Ok(())
    }

    /// Re-wrap the document if the text width changed.
    fn layout(&mut self, width: u16) {
        if width == self.width {
            return;
        }
        self.width = width;
        self.rows.clear();
        self.headings.clear();
        let text = tui_markdown::from_str(&self.source);
        for line in &text.lines {
            let is_heading = line.spans.first().is_some_and(|s| s.content.starts_with('#'));
            if is_heading {
                self.headings.push(self.rows.len());
            }
            self.rows.extend(wrap_line(line, width as usize));
        }
    }

    /// The heading text of the section containing row `pos`.
    fn section(&self, pos: usize) -> String {
        let Some(&row) = self.headings.iter().rev().find(|&&h| h <= pos) else {
            return String::new();
        };
        let text: String = self.rows[row].spans.iter().map(|s| s.content.as_ref()).collect();
        text.trim_start_matches('#').trim().to_string()
    }
}

/// The centered text column inside `area`.
fn text_area(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(MAX_WIDTH);
    let x = area.x + (area.width - width) / 2;
    Rect { x, width, ..area }
}

fn draw(frame: &mut Frame, doc: &mut Doc, state: &mut SmoothScrollState, smooth: bool) {
    let [header, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let column = text_area(body);
    doc.layout(column.width);
    frame.render_stateful_widget(SmoothScroll::new(&doc.rows).text_area(column), body, state);

    let name = doc.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let section = doc.section(state.pos);
    frame.render_widget(
        Line::from(format!(" {name}  ›  {section}")).bold().reversed(),
        header,
    );

    let percent = if state.max == 0 { 100 } else { state.pos * 100 / state.max };
    frame.render_widget(
        Line::from(format!(
            " {} │ {:3}% │ line {}/{} │ j/k d/u space/b g/G ]/[ heading  r reload  s smooth  q quit",
            if smooth { "SMOOTH" } else { "rows" },
            percent,
            state.pos + 1,
            doc.rows.len(),
        ))
        .reversed(),
        status,
    );
}

fn run(terminal: &mut DefaultTerminal, mut doc: Doc) -> io::Result<()> {
    let mut state = SmoothScrollState::default();
    let mut smooth = true;

    loop {
        draw_smooth(terminal, std::slice::from_mut(&mut state), smooth, |f, s| {
            draw(f, &mut doc, &mut s[0], smooth)
        })?;

        let half = (state.visible / 2).max(1) as isize;
        let page = state.visible.saturating_sub(1).max(1) as isize;

        // Block for the first event, then drain any queued ones before redrawing.
        let mut ev = Some(event::read()?);
        while let Some(e) = ev.take() {
            match e {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => state.scroll(1),
                    KeyCode::Char('k') | KeyCode::Up => state.scroll(-1),
                    KeyCode::Char('d') => state.scroll(half),
                    KeyCode::Char('u') => state.scroll(-half),
                    KeyCode::Char(' ') | KeyCode::PageDown => state.scroll(page),
                    KeyCode::Char('b') | KeyCode::PageUp => state.scroll(-page),
                    KeyCode::Char('g') | KeyCode::Home => state.scroll_to(0),
                    KeyCode::Char('G') | KeyCode::End => state.scroll_to(usize::MAX),
                    KeyCode::Char(']') => {
                        if let Some(&h) = doc.headings.iter().find(|&&h| h > state.pos) {
                            state.scroll_to(h);
                        }
                    }
                    KeyCode::Char('[') => {
                        if let Some(&h) = doc.headings.iter().rev().find(|&&h| h < state.pos) {
                            state.scroll_to(h);
                        }
                    }
                    KeyCode::Char('r') => doc.reload()?,
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
    let path = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "README.md".into()));
    let doc = Doc::load(path.clone())
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;

    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal, doc);
    let mut out = io::stdout();
    out.write_all(SMOOTH_OFF)?;
    execute!(out, DisableMouseCapture)?;
    ratatui::restore();
    result
}
