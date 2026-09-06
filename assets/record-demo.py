#!/usr/bin/env python3
# Re-record assets/demo.gif. Needs pyte and pillow:
#   python3 -m venv .venv && .venv/bin/pip install pyte pillow
#   cd assets && ../.venv/bin/python record-demo.py ../target/release/mdread demo.md demo.gif
import os, pty, sys, time, select, struct, fcntl, termios, pyte
from PIL import Image, ImageDraw, ImageFont

COLS, ROWS = 80, 24
BIN, DOC, OUT = sys.argv[1], sys.argv[2], sys.argv[3]

# ---- terminal ----
pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"; os.environ["COLORTERM"] = "truecolor"
    os.execv(BIN, [BIN, DOC])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
screen = pyte.Screen(COLS, ROWS); stream = pyte.ByteStream(screen)

def pump(t):
    end = time.time() + t
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.03)
        if r:
            try: stream.feed(os.read(fd, 65536))
            except OSError: return

# ---- rendering ----
FONT = ImageFont.truetype("/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf", 15)
BOLD = ImageFont.truetype("/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Bold.ttf", 15)
CW = int(FONT.getlength("M")); CH = 20
PAD = 12
W, H = COLS*CW + 2*PAD, ROWS*CH + 2*PAD
BG = (0x1e, 0x22, 0x2a)          # window background
DEFAULT_FG = (0xd8, 0xde, 0xe9)
ANSI = {"black":(0,0,0),"red":(205,49,49),"green":(13,188,121),"brown":(229,229,16),
        "blue":(36,114,200),"magenta":(188,63,188),"cyan":(17,168,205),"white":(229,229,229),
        "brightblack":(102,102,102),"brightred":(241,76,76),"brightgreen":(35,209,139),
        "brightyellow":(245,245,67),"brightblue":(59,142,234),"brightmagenta":(214,112,214),
        "brightcyan":(41,184,219),"brightwhite":(255,255,255)}
def color(c, default):
    if c == "default": return default
    if c in ANSI: return ANSI[c]
    try: return tuple(int(c[i:i+2], 16) for i in (0, 2, 4))
    except Exception: return default

frames, durations = [], []
def snap(hold_ms):
    img = Image.new("RGB", (W, H), BG); d = ImageDraw.Draw(img)
    for y in range(ROWS):
        line = screen.buffer[y]
        for x in range(COLS):
            ch = line[x]
            fg, bg = color(ch.fg, DEFAULT_FG), color(ch.bg, None)
            if ch.reverse: fg, bg = (bg or BG), fg
            px, py = PAD + x*CW, PAD + y*CH
            if bg: d.rectangle([px, py, px+CW-1, py+CH-1], fill=bg)
            if ch.data and ch.data != " ":
                f = BOLD if ch.bold else FONT
                d.text((px, py+1), ch.data, font=f, fill=fg)
                if ch.underscore: d.line([px, py+CH-3, px+CW-1, py+CH-3], fill=fg)
    frames.append(img); durations.append(hold_ms)

def key(s, settle=0.25, hold=700):
    os.write(fd, s.encode()); pump(settle); snap(hold)

# ---- script ----
pump(0.8); snap(1800)
for _ in range(3): key("\x1b[B", 0.12, 140)            # a few arrow-downs
key("\x1b[B", 0.12, 900)
key(" ", 0.25, 1400)                                    # page down
key("/", 0.25, 700)
for c in "search": key(c, 0.08, 110)
snap(600)
key("\r", 0.3, 1800)                                    # first hit + [k/n]
key("n", 0.25, 1300)
key("n", 0.25, 1300)
key("n", 0.25, 1300)
key("\x1b", 0.25, 1200)                                 # clear search
key("?", 0.3, 2200)                                     # help overlay
key("\x1b", 0.25, 1200)
key("g", 0.25, 1500)                                    # back to top
key("q", 0.2, 10)
frames.pop(); durations.pop()

frames[0].save(OUT, save_all=True, append_images=frames[1:], duration=durations, loop=0, optimize=True)
print("frames", len(frames), "size", os.path.getsize(OUT)//1024, "KB", W, "x", H)
