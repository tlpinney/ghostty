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

An app declares rectangles of viewport cells that it scrolls one row per
wheel event:

```
ESC _ smooth ; id ; top ; bottom ; left ; right ; pos ; max ESC \   declare / update
ESC _ smooth ; top ; bottom ; pos ; max ESC \                       legacy: id 0, full width
ESC _ smooth ; off ESC \                                            clear all regions
ESC _ smooth ; off ; id ESC \                                       clear one region
```

- `id`: identifies the region, so an app (or a multiplexer, one per pane)
  can declare several side by side. Up to 8 regions are active at once.
- `top`, `bottom`, `left`, `right`: 0-based viewport cells, inclusive.
  `bottom` is an **overscan row**: the app fills it with the next line of
  content and Ghostty keeps it hidden until the region shifts up into it.
  Cells outside the rectangle are never shifted.
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

### Apps using it

- [pixelscroll.nvim](https://github.com/tlpinney/pixelscroll.nvim): pixel
  scrolling for Neovim (and LazyVim) windows. Turn it on with
  `SMOOTH_SCROLL=1`, directly in Ghostty Smooth or inside the herdr fork.

## Demo

`smooth-scroll/` contains a Ratatui app with a `SmoothScroll` widget, a
Ghostty config matching Zed's defaults for side-by-side comparison, and
the original kitty-graphics prototype (`prototype/smooth.py`) that runs in
an unpatched terminal.

```sh
zig build -Doptimize=ReleaseFast
(cd macos && xcodebuild -target Ghostty -configuration ReleaseLocal)
smooth-scroll/install.sh                  # builds/Ghostty-<sha>.app + builds/current
(cd smooth-scroll && cargo build --release)

G=builds/current/Contents/MacOS/ghostty
$G --config-file=smooth-scroll/zed-one-dark.conf \
   -e smooth-scroll/target/release/smooth-demo
```

`install.sh` copies each build into its own bundle, named "Ghostty Smooth"
with its own bundle id (`com.tlpinney.ghostty-smooth`) and a recolored icon
(`make-icon.py`), so it sits beside the official Ghostty and can be opened
normally (`open builds/current`, Spotlight, or the Dock). macOS treats it as
a separate app, so permissions such as Screen Recording are granted to it
separately; `~/.config/ghostty/config` still applies. Don't rebuild into an
app bundle that a running Ghostty uses: macOS kills the running process
(Code Signature Invalid) when it pages in code that no longer matches.
The `xcodebuild` step is run directly because `zig build` can report it as
failed on Xcode 27 even when it succeeds.

Use the trackpad over the text. `s` toggles the region declaration so you
can compare with normal row scrolling; `q` quits.

`smooth-demo --split` shows two independently scrolled regions side by
side, the layout a multiplexer such as herdr would declare for its panes.

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

## Testing inside herdr

The terminal multiplexer [herdr](https://github.com/herdrdev/herdr) needs
its own patch to relay regions from panes to this Ghostty: see
[tlpinney/herdr](https://github.com/tlpinney/herdr), branch
`smooth-scroll` (based on herdr v0.9.5, with its own `SMOOTH_SCROLL.md`).
Without it, apps inside herdr fall back to row scrolling.

To try it next to an installed herdr without touching its session, build
with debug assertions on. Herdr then uses its separate `herdr-dev` config
and server:

```sh
cd herdr   # tlpinney/herdr, branch smooth-scroll
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true CARGO_TARGET_DIR=target/devopt \
  ZIG=/path/to/zig-0.16.0 cargo build --release
```

Launch it in this Ghostty from a clean environment. Started from inside
another herdr pane, the inherited `HERDR_*` variables make it refuse to
start ("nested herdr is disabled") or talk to the other server:

```sh
env -u HERDR_ENV -u HERDR_PANE_ID -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH \
    -u HERDR_TAB_ID -u HERDR_WORKSPACE_ID -u HERDR_BIN_PATH \
  $G --config-file=smooth-scroll/zed-one-dark.conf \
     -e /path/to/herdr/target/devopt/release/herdr
```

Split a pane and run `smooth-md` (or `smooth-demo --split`) in each half;
each pane should scroll smoothly on its own while herdr's sidebar, tab bar,
and the neighbouring pane stay still. Through herdr, fast flicks can pause
briefly at row boundaries: each row costs a round trip through herdr, which
renders at about 60 fps, and regions have only one overscan row.

## Known limitations

- macOS (Metal) only; the OpenGL renderer is not updated.
- Only precision (trackpad) input is smoothed; mouse wheels and keyboard
  scrolling still move whole rows.
- Regions always show one blank row (the overscan row).
- In the scrollback, a strip up to one row tall at the top or bottom edge
  can be blank mid-scroll, since rows outside the viewport aren't drawn.
- Images (kitty graphics) do not move with the offset.
- Only the region parsing, acknowledgement, and table logic have unit
  tests (`zig build test -Dtest-filter=smooth`); rendering is tested by hand.
