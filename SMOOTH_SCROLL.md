# Experimental: pixel smooth scrolling

This fork of [Ghostty](https://github.com/ghostty-org/ghostty) adds
Zed-style pixel smooth scrolling, both for the shell scrollback and for
TUI apps that opt in through a small escape sequence. It is an
experiment, not an upstream proposal, and it is not affiliated with the
Ghostty project.

All changes live on the `smooth-scroll` branch.

## AI disclosure

Following Ghostty's [AI usage policy](AI_POLICY.md): everything in this
fork's changes (the Zig and Metal patch, the escape sequence design, the
Ratatui demo, the Python prototype, and this document) was written by
[Claude Code](https://claude.com/claude-code) using Claude Opus 5.5. A
human directed the work, chose the design, read the full diff, and tested
it by hand on macOS. No AI-generated media is included. This fork has not
been submitted upstream.

## Why a terminal patch is needed

Terminals work in whole rows. Ghostty's viewport is a row index, the
renderer places rows at fixed positions, and apps can only address cells.
Trackpads report pixels, but Ghostty accumulates them and moves a whole
row at a time (and upstream discards the sub-row remainder). An app cannot
fake smoothness either: anything it animates runs on its own clock, not
Ghostty's frame clock, so frames get uneven steps and visibly stutter.
The offset has to live inside the renderer.

## 1. Scrollback

Precision (trackpad) scrolling keeps the sub-row remainder and the
renderer shifts the whole grid by that many pixels, including macOS
momentum. Nothing to enable.

## 2. App-declared smooth scroll regions

An app declares a region of viewport rows that it scrolls one row per
wheel event:

```
ESC _ smooth ; top ; bottom ; pos ; max ESC \     declare / update
ESC _ smooth ; off ESC \                          clear
```

- `top`, `bottom`: 0-based viewport rows, inclusive. `bottom` is an
  **overscan row**: the app fills it with the next line of content and
  Ghostty keeps it hidden until the region shifts up into it.
- `pos`: the app's current scroll position (its first visible content line).
- `max`: the largest `pos` the app allows.

While a region is declared and mouse reporting is on, precision scrolls
over it are accumulated by Ghostty. Ghostty renders the region shifted up
by the sub-row remainder and sends the app one ordinary SGR wheel event
(button 64/65) per whole row. The app scrolls one row per wheel event and
redraws as usual.

The app re-sends the declaration after every redraw, ideally inside the
same synchronized update (mode 2026) as the cells. When `pos` moves in the
direction of outstanding wheel requests, Ghostty treats it as an
acknowledgement and subtracts those rows from its offset, so the view
never jumps back while the app catches up. Moves the app makes on its own
(keyboard, jumps) don't disturb the accumulated offset.

The region is cleared on `smooth;off` and when the app switches between
the primary and alternate screen. Terminals that don't know the sequence
ignore the APC, so apps degrade to normal row scrolling.

## Demo

`smooth-scroll/` contains a Ratatui app with a `SmoothScroll` widget, a
Ghostty config matching Zed's defaults for side-by-side comparison, and
the original kitty-graphics prototype (`prototype/smooth.py`) that runs in
an unpatched terminal.

```sh
zig build -Doptimize=ReleaseFast          # builds zig-out/Ghostty.app
(cd smooth-scroll && cargo build --release)

G=zig-out/Ghostty.app/Contents/MacOS/ghostty
$G --config-file=smooth-scroll/zed-one-dark.conf \
   -e smooth-scroll/target/release/smooth-demo
```

Use the trackpad over the text. `s` toggles the region declaration so you
can compare with normal row scrolling; `q` quits.

`smooth-md` is a Markdown reader built on the same widget:

```sh
$G --config-file=smooth-scroll/zed-one-dark.conf \
   -e smooth-scroll/target/release/smooth-md README.md
```

Keys: `j`/`k` line, `d`/`u` half page, `space`/`b` page, `g`/`G` top and
bottom, `]`/`[` next and previous heading, `r` reload, `s` toggle smooth,
`q` quit. The reusable `SmoothScroll` widget and word wrapper live in
`smooth-scroll/src/lib.rs`. The Zed config expects
the [Lilex](https://github.com/mishamyrt/Lilex) font.

## Known limitations

- macOS (Metal) only; the OpenGL renderer is not updated.
- Only precision (trackpad) input is smoothed; mouse wheels and keyboard
  scrolling still move whole rows.
- Regions always show one blank row (the overscan row).
- In the scrollback, a strip up to one row tall at the top or bottom edge
  can be blank mid-scroll, since rows outside the viewport aren't drawn.
- Mouse selection hit-testing ignores the pixel offset, so clicks can land
  one row off while the view sits between rows.
- Images (kitty graphics) do not move with the offset.
- No tests yet.
