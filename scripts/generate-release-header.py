#!/usr/bin/env python3
"""Generate the deterministic 5:2 X release header for llmff v1.2.

Matches the style and 1600x640 (5:2) geometry of the blog X headers.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/assets/llmff-v1.2-header.png"
WIDTH = 1600
HEIGHT = 640
SCALE = 2

BG = "#f8fafc"
INK = "#17202a"
MUTED = "#5d6b7a"
LINE = "#b8c2cc"
ACCENT = "#2f6f73"
ACCENT_2 = "#8a5a2b"
BLUE = "#315f8c"


def font(name: str, size: int) -> ImageFont.FreeTypeFont:
    base = Path("/usr/share/fonts/truetype/noto")
    paths = {
        "regular": base / "NotoSans-Regular.ttf",
        "bold": base / "NotoSans-Bold.ttf",
        "mono_bold": base / "NotoSansMono-Bold.ttf",
        "mono": base / "NotoSansMono-Regular.ttf",
    }
    return ImageFont.truetype(str(paths[name]), size * SCALE)


F_TITLE = font("bold", 56)
F_SUB = font("regular", 24)
F_ROLE = font("mono_bold", 23)
F_TAG = font("mono", 18)
F_SMALL = font("regular", 19)


@dataclass(frozen=True)
class Rect:
    x: int
    y: int
    w: int
    h: int

    @property
    def x2(self) -> int:
        return self.x + self.w

    @property
    def cy(self) -> int:
        return self.y + self.h // 2


class Canvas:
    def __init__(self) -> None:
        self.image = Image.new("RGB", (WIDTH * SCALE, HEIGHT * SCALE), BG)
        self.draw = ImageDraw.Draw(self.image)

    def srect(self, r: Rect) -> tuple[int, int, int, int]:
        return tuple(v * SCALE for v in (r.x, r.y, r.x2, r.y + r.h))

    def text(self, xy, value, fill=INK, font_obj=F_SMALL, anchor=None) -> None:
        self.draw.text((xy[0] * SCALE, xy[1] * SCALE), value, fill=fill, font=font_obj, anchor=anchor)

    def box(self, r: Rect, title, lines=None, fill="#ffffff", outline=LINE, title_fill=INK, radius=12, font_obj=F_ROLE) -> None:
        self.draw.rounded_rectangle(self.srect(r), radius=radius * SCALE, fill=fill, outline=outline, width=2 * SCALE)
        self.draw.text(((r.x + 18) * SCALE, (r.y + 14) * SCALE), title, fill=title_fill, font=font_obj)
        if lines:
            y = r.y + 48
            for ln in lines:
                self.draw.text(((r.x + 18) * SCALE, y * SCALE), ln, fill=MUTED, font=F_TAG)
                y += 28

    def pill(self, r: Rect, text, fill="#eef4f8", outline=LINE, color=INK) -> None:
        self.draw.rounded_rectangle(self.srect(r), radius=r.h // 2 * SCALE, fill=fill, outline=outline, width=2 * SCALE)
        self.draw.text(((r.x + r.w // 2) * SCALE, (r.cy) * SCALE), text, fill=color, font=F_TAG, anchor="mm")

    def arrow(self, start, end, fill=LINE, width=3) -> None:
        self.draw.line([(start[0] * SCALE, start[1] * SCALE), (end[0] * SCALE, end[1] * SCALE)], fill=fill, width=width * SCALE)
        ex, ey = end
        d = 1 if end[0] >= start[0] else -1
        head = [(ex, ey), (ex - 11 * d, ey - 7), (ex - 11 * d, ey + 7)]
        self.draw.polygon([(x * SCALE, y * SCALE) for x, y in head], fill=fill)

    def save(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        self.image.resize((WIDTH, HEIGHT), Image.Resampling.LANCZOS).save(path, optimize=True)


def main() -> None:
    c = Canvas()
    # Title block.
    c.text((70, 44), "llmff v1.2", fill=INK, font_obj=F_TITLE)
    c.text((74, 122), "declared multi-agent topology", fill=MUTED, font_obj=F_SUB)
    c.draw.line([(70 * SCALE, 170 * SCALE), (1530 * SCALE, 170 * SCALE)], fill="#d7dee5", width=1 * SCALE)

    # Agents bundle panel feeding the pipeline.
    agents = Rect(70, 235, 250, 250)
    c.box(agents, "agents:", fill="#eaf3f3", outline=ACCENT, title_fill=ACCENT, font_obj=F_ROLE)
    for i, role in enumerate(["generator", "critic", "reviser"]):
        c.pill(Rect(92, 300 + i * 56, 206, 38), role, fill="#ffffff", outline=ACCENT, color=ACCENT)

    # Declared role pipeline: generator -> critic -> reviser.
    roles = [
        ("draft", "agent: generator", 430),
        ("review", "agent: critic", 790),
        ("revise", "agent: reviser", 1150),
    ]
    boxes = []
    for label, tag, x in roles:
        r = Rect(x, 300, 300, 118)
        c.box(r, label, [tag], fill="#ffffff", outline="#c5cfd8", font_obj=F_ROLE)
        boxes.append(r)
    c.arrow((agents.x2, 360), (boxes[0].x, boxes[0].cy), fill=ACCENT)
    for a, b in zip(boxes, boxes[1:]):
        c.arrow((a.x2, a.cy), (b.x, b.cy), fill=BLUE)

    # Bounded-loop hint over the role pipeline.
    c.pill(Rect(430, 248, 330, 34), "op: loop  max_iterations", fill="#fff8ed", outline=ACCENT_2, color=ACCENT_2)

    # Bottom pills: the contract.
    tags = ["agent: <name>", "system persona", "role-stamped traces", "bounded route handoff"]
    x = 430
    for t in tags:
        w = 22 + len(t) * 11
        c.pill(Rect(x, 485, w, 40), t, fill="#ffffff")
        x += w + 24

    c.text((70, 565), "roles are declared and inspectable; the host still owns which role runs next", fill=MUTED, font_obj=F_SMALL)
    c.save(OUT)
    print(f"wrote {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
