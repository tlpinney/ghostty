#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow"]
# ///
"""Pixel-smooth scrolling test for Ghostty via the kitty graphics protocol.

Renders a page of text into one tall image, uploads it once, then each frame
re-places it with a source-rect y offset in pixels. Ghostty composites it on
the GPU, so the scroll moves by pixels instead of whole rows.

Keys:
  j/k, arrows, wheel/trackpad   scroll (animated)
  space / b                     page down / up
  a                             toggle auto back-and-forth
  s                             toggle snap-to-row mode (normal terminal feel)
  + / -                         auto speed (px per frame)
  q                             quit

Usage: smooth.py [HZ]   (default 100; match your display's refresh rate)
"""

import base64
import fcntl
import io
import math
import os
import select
import struct
import sys
import termios
import time
import tty

from PIL import Image, ImageDraw, ImageFont

FONT_PATH = "/System/Library/Fonts/Menlo.ttc"
FG = (220, 223, 228)
BG = (24, 26, 31)
DIM = (110, 118, 129)
ACCENT = (97, 175, 239)
IMAGE_ID = 4242
PLACEMENT_ID = 1
MAX_IMAGE_H = 9000  # stay under common kitty-protocol size limits


def out(s: str) -> None:
    sys.stdout.write(s)


def winsize():
    buf = fcntl.ioctl(sys.stdout.fileno(), termios.TIOCGWINSZ, b"\0" * 8)
    rows, cols, xpx, ypx = struct.unpack("HHHH", buf)
    if not xpx or not ypx:
        sys.exit("terminal did not report pixel size (TIOCGWINSZ); run this in Ghostty")
    return rows, cols, xpx, ypx


def sample_lines(n: int) -> list[str]:
    words = ("render scroll pixel glyph metal shader atlas cell grid frame "
             "vsync easing momentum viewport offset ghostty zed gpui").split()
    lines = []
    for i in range(n):
        if i % 20 == 0:
            lines.append(f"── section {i // 20 + 1} " + "─" * 60)
            continue
        body = " ".join(words[(i * 7 + k * 3) % len(words)] for k in range(6 + i % 9))
        lines.append(f"{i:4d} │ {body}")
    return lines


def render_page(lines, width_px, cell_w, cell_h):
    # Pick a font size whose line height fits the terminal cell height.
    size = int(cell_h * 0.78)
    font = ImageFont.truetype(FONT_PATH, size)
    img = Image.new("RGB", (width_px, cell_h * len(lines)), BG)
    d = ImageDraw.Draw(img)
    for i, line in enumerate(lines):
        y = i * cell_h + (cell_h - size) // 2
        color = ACCENT if line.startswith("──") else FG
        num, sep, rest = line.partition("│")
        if sep:
            d.text((cell_w // 2, y), num + sep, font=font, fill=DIM)
            d.text((cell_w // 2 + font.getlength(num + sep), y), rest, font=font, fill=color)
        else:
            d.text((cell_w // 2, y), line, font=font, fill=color)
    return img


def transmit(img) -> None:
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    data = base64.standard_b64encode(buf.getvalue()).decode()
    chunks = [data[i:i + 4096] for i in range(0, len(data), 4096)]
    for n, chunk in enumerate(chunks):
        more = 1 if n < len(chunks) - 1 else 0
        if n == 0:
            out(f"\x1b_Ga=t,f=100,i={IMAGE_ID},q=2,m={more};{chunk}\x1b\\")
        else:
            out(f"\x1b_Gm={more};{chunk}\x1b\\")
    sys.stdout.flush()


def place(y_px: int, w_px: int, h_px: int, cols: int, rows: int) -> None:
    # Cursor to top-left, re-place same image+placement id (replaces old one),
    # C=1 so the cursor does not move.
    out(f"\x1b[H\x1b_Ga=p,i={IMAGE_ID},p={PLACEMENT_ID},x=0,y={y_px},"
        f"w={w_px},h={h_px},c={cols},r={rows},C=1,q=2\x1b\\")


def main() -> None:
    rows, cols, xpx, ypx = winsize()
    cell_w, cell_h = xpx // cols, ypx // rows
    view_rows = rows - 1  # last row is the status line
    view_h = view_rows * cell_h
    view_w = cols * cell_w

    n_lines = min(400, MAX_IMAGE_H // cell_h)
    img = render_page(sample_lines(n_lines), view_w, cell_w, cell_h)
    max_scroll = max(0, img.height - view_h)

    fd = sys.stdin.fileno()
    old = termios.tcgetattr(fd)
    tty.setraw(fd)
    # alt screen, hide cursor, SGR mouse with wheel reporting
    out("\x1b[?1049h\x1b[?25l\x1b[?1000h\x1b[?1006h\x1b[2J")
    transmit(img)

    hz = float(sys.argv[1]) if len(sys.argv) > 1 else 100.0
    frame_dt = 1.0 / hz

    pos = 0.0        # current pixel offset (float)
    target = 0.0     # where easing is heading
    auto = True
    snap = False
    step = 2         # auto mode: constant whole pixels per frame
    direction = 1
    t0 = time.monotonic()
    next_frame = t0
    last_write = t0
    frames, fps, fps_t = 0, 0.0, t0
    jit_max, jitter = 0.0, 0.0
    buf = b""
    log_path = os.environ.get("SMOOTH_LOG")
    log = open(log_path, "w") if log_path else None

    try:
        while True:
            # Fixed-rate loop: wait until the next frame deadline, reading
            # input while we wait, so every frame moves the same amount.
            while True:
                wait = next_frame - time.monotonic()
                if wait <= 0:
                    break
                r, _, _ = select.select([fd], [], [], wait)
                if r:
                    buf += os.read(fd, 1024)
            next_frame += frame_dt
            if time.monotonic() - next_frame > frame_dt:
                next_frame = time.monotonic() + frame_dt  # fell behind; resync
            while buf:
                if buf.startswith(b"\x1b[<"):
                    end = next((i for i, c in enumerate(buf) if c in b"Mm"), -1)
                    if end < 0:
                        break
                    btn = int(buf[3:end].split(b";")[0])
                    buf = buf[end + 1:]
                    if btn in (64, 65):
                        auto = False
                        target += (-1 if btn == 64 else 1) * cell_h * 1.5
                    continue
                if buf.startswith(b"\x1b["):
                    if len(buf) < 3:
                        break
                    k, buf = buf[2:3], buf[3:]
                    key = {b"A": b"k", b"B": b"j", b"5": b"b", b"6": b" "}.get(k)
                    if k in (b"5", b"6") and buf.startswith(b"~"):
                        buf = buf[1:]
                else:
                    key, buf = buf[:1], buf[1:]
                if key in (b"q", b"\x03"):
                    return
                if key == b"j":
                    auto = False; target += cell_h * 3
                elif key == b"k":
                    auto = False; target -= cell_h * 3
                elif key == b" ":
                    auto = False; target += view_h * 0.9
                elif key == b"b":
                    auto = False; target -= view_h * 0.9
                elif key == b"a":
                    auto = not auto; pos = float(round(pos))
                elif key == b"s":
                    snap = not snap
                elif key == b"+":
                    step = min(step + 1, 16)
                elif key == b"-":
                    step = max(step - 1, 1)

            now = time.monotonic()

            if auto:
                # constant-velocity ping-pong: identical integer step every frame
                pos += direction * step
                if pos >= max_scroll or pos <= 0:
                    direction = -direction
                    pos = min(max(pos, 0.0), float(max_scroll))
                target = pos
            else:
                target = min(max(target, 0.0), float(max_scroll))
                # easing on the fixed frame step, not wall-clock dt
                pos += (target - pos) * (1 - math.exp(-frame_dt * 18))
                if abs(target - pos) < 0.5:
                    pos = target

            y = int(round(pos))
            if snap:
                y = (y // cell_h) * cell_h

            status = (f" {'AUTO ' + str(step) + 'px/f' if auto else 'manual'} │ "
                      f"{'SNAP rows' if snap else 'SMOOTH px'} │ y={y:5d}/{max_scroll} │ "
                      f"{fps:4.0f}/{hz:.0f} fps │ jitter {jitter:4.1f}ms │ "
                      f"a s j/k wheel space/b +/- q")
            # synchronized update so the placement + status land in one frame
            out("\x1b[?2026h")
            place(y, view_w, view_h, cols, view_rows)
            out(f"\x1b[{rows};1H\x1b[0;7m{status[:cols].ljust(cols)}\x1b[0m")
            out("\x1b[?2026l")
            sys.stdout.flush()

            wrote = time.monotonic()
            jit_max = max(jit_max, abs((wrote - last_write) - frame_dt) * 1000)
            last_write = wrote
            frames += 1
            if now - fps_t >= 0.5:
                fps = frames / (now - fps_t)
                jitter, jit_max = jit_max, 0.0
                if log:
                    log.write(f"{now - t0:7.2f}s fps={fps:5.1f} jitter_ms={jitter:5.2f} "
                              f"auto={auto} snap={snap} step={step} y={y}\n")
                    log.flush()
                frames, fps_t = 0, now
    finally:
        out(f"\x1b_Ga=d,d=I,i={IMAGE_ID},q=2\x1b\\")
        out("\x1b[?1006l\x1b[?1000l\x1b[?25h\x1b[?1049l")
        sys.stdout.flush()
        termios.tcsetattr(fd, termios.TCSADRAIN, old)


if __name__ == "__main__":
    main()
