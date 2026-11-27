#!/usr/bin/env python3
"""Draw the geometry diagrams used by docs/HOW-IT-WORKS.md.

Run from the repository root: python3 docs/images/make_diagrams.py
Standard library only. The lattice maths mirrors core/src/geometry.rs (pointy-top hexagons,
basis a1 = (sqrt(3) s, 0), a2 = (sqrt(3) s / 2, 1.5 s)) but is for drawing only.
"""

import math
from pathlib import Path

OUT = Path(__file__).resolve().parent
SQRT3 = math.sqrt(3)

BG = "#ffffff"
GRID = "#c4cad3"
DOT = "#8a93a0"
INK = "#1f2937"
MUTED = "#5b6472"
ACCENT = "#2563eb"
ACCENT_FILL = "#dbeafe"
CELL_FILL = "#fef3c7"
CELL_EDGE = "#d97706"
GOOD = "#15803d"
BAD = "#dc2626"
FONT = "font-family=\"system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif\""


def to_xy(q, t, s, origin):
    """Lattice coordinates to drawing coordinates."""
    ox, oy = origin
    return ox + (q + t / 2) * SQRT3 * s, oy + t * 1.5 * s


def hexagon(cx, cy, s):
    """Points of a pointy-top hexagon with circumradius s."""
    pts = []
    for k in range(6):
        a = math.radians(-90 + 60 * k)
        pts.append(f"{cx + s * math.cos(a):.2f},{cy + s * math.sin(a):.2f}")
    return " ".join(pts)


def svg(width, height, body, title):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" '
        f'height="{height}" role="img" aria-label="{title}">\n'
        f"<title>{title}</title>\n"
        f'<rect width="{width}" height="{height}" rx="12" fill="{BG}"/>\n'
        f"{body}</svg>\n"
    )


def text(x, y, s, size=15, fill=INK, anchor="start", weight="normal"):
    return (
        f'<text x="{x:.1f}" y="{y:.1f}" {FONT} font-size="{size}" fill="{fill}" '
        f'text-anchor="{anchor}" font-weight="{weight}">{s}</text>\n'
    )


def arrow_defs():
    out = "<defs>\n"
    for name, colour in (("a-accent", ACCENT), ("a-edge", CELL_EDGE), ("a-ink", INK)):
        out += (
            f'<marker id="{name}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" '
            f'orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="{colour}"/></marker>\n'
        )
    return out + "</defs>\n"


def grid(s, origin, q_range, t_range, clip, stroke=GRID, dots=True, shift=(0.0, 0.0), width=1.2):
    """Hexagon outlines (and optionally centres) of a lattice, shifted by `shift` in lattice coordinates."""
    x0, y0, x1, y1 = clip
    out = ""
    for t in range(*t_range):
        for q in range(*q_range):
            cx, cy = to_xy(q + shift[0], t + shift[1], s, origin)
            if x0 - s < cx < x1 + s and y0 - s < cy < y1 + s:
                out += f'<polygon points="{hexagon(cx, cy, s)}" fill="none" stroke="{stroke}" stroke-width="{width}"/>\n'
                if dots:
                    out += f'<circle cx="{cx:.2f}" cy="{cy:.2f}" r="2.6" fill="{DOT}"/>\n'
    return out


def clip_path(name, rect):
    x0, y0, x1, y1 = rect
    return (
        f'<clipPath id="{name}"><rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" rx="8"/></clipPath>\n'
    )


def split_diagram():
    """Enrolment: a tap splits into a secret anchor (cell index) and a public offset."""
    w, h, s = 760, 440, 46
    panel = (24, 24, 470, 416)
    origin = (40, 60)
    q_tap, t_tap = 1.58, 2.42
    qi, ti = math.floor(q_tap), math.floor(t_tap)

    body = arrow_defs() + "<defs>" + clip_path("c1", panel) + "</defs>\n"
    body += f'<g clip-path="url(#c1)">\n'
    body += grid(s, origin, (-4, 8), (-1, 8), panel)
    corners = [to_xy(qi + dq, ti + dt, s, origin) for dq, dt in ((0, 0), (1, 0), (1, 1), (0, 1))]
    pts = " ".join(f"{x:.2f},{y:.2f}" for x, y in corners)
    body += f'<polygon points="{pts}" fill="{CELL_FILL}" fill-opacity="0.75" stroke="{CELL_EDGE}" stroke-width="2"/>\n'
    ax, ay = corners[0]
    px, py = to_xy(q_tap, t_tap, s, origin)
    body += (
        f'<line x1="{ax:.2f}" y1="{ay:.2f}" x2="{px:.2f}" y2="{py:.2f}" stroke="{ACCENT}" stroke-width="2.5" '
        f'marker-end="url(#a-accent)"/>\n'
    )
    body += f'<circle cx="{ax:.2f}" cy="{ay:.2f}" r="7" fill="{CELL_EDGE}" stroke="{BG}" stroke-width="2"/>\n'
    body += f'<circle cx="{px:.2f}" cy="{py:.2f}" r="8" fill="{INK}" stroke="{BG}" stroke-width="2"/>\n'
    body += "</g>\n"
    body += f'<rect x="{panel[0]}" y="{panel[1]}" width="{panel[2] - panel[0]}" height="{panel[3] - panel[1]}" ' \
            f'rx="8" fill="none" stroke="{GRID}"/>\n'
    body += text(px + 12, py + 22, "tap", 14, INK, weight="bold")
    body += text(ax - 10, ay - 12, "anchor", 14, CELL_EDGE, "end", "bold")

    lx = 500
    body += text(lx, 70, "One tap becomes two values", 18, INK, weight="bold")
    body += f'<circle cx="{lx + 8}" cy="{112}" r="7" fill="{CELL_EDGE}"/>\n'
    body += text(lx + 26, 117, "Anchor: the lattice point at", 15, INK, weight="bold")
    body += text(lx + 26, 139, "the corner of the tap's cell.", 14, MUTED)
    body += text(lx + 26, 161, "Its two whole numbers are the", 14, MUTED)
    body += text(lx + 26, 183, "cell index. Secret: hashed only.", 14, MUTED)
    body += (
        f'<line x1="{lx}" y1="228" x2="{lx + 18}" y2="228" stroke="{ACCENT}" stroke-width="2.5" '
        f'marker-end="url(#a-accent)"/>\n'
    )
    body += text(lx + 26, 233, "Offset: from anchor to tap.", 15, INK, weight="bold")
    body += text(lx + 26, 255, "Where the tap sits inside the", 14, MUTED)
    body += text(lx + 26, 277, "shaded cell, never which cell.", 14, MUTED)
    body += text(lx + 26, 299, "Stored, encrypted after tap 1.", 14, MUTED)
    body += f'<rect x="{lx}" y="330" width="16" height="16" fill="{CELL_FILL}" stroke="{CELL_EDGE}"/>\n'
    body += text(lx + 26, 343, "The cell: one step of each", 14, MUTED)
    body += text(lx + 26, 365, "lattice direction from the anchor.", 14, MUTED)
    body += f'<circle cx="{lx + 8}" cy="392" r="2.6" fill="{DOT}"/>\n'
    body += text(lx + 26, 397, "Lattice point (hexagon centre).", 14, MUTED)
    return svg(w, h, body, "A tap split into a secret anchor and a public offset")


def shifted_diagram():
    """Verification: the offset shifts the grid so the accepted hexagon is centred on the enrolled tap."""
    w, h, s = 760, 440, 62
    r = s * SQRT3 / 2
    panel = (24, 24, 470, 416)
    origin = (40, 40)
    shift = (0.58, 0.42)
    centre_q, centre_t = 1 + shift[0], 2 + shift[1]

    body = arrow_defs() + "<defs>" + clip_path("c2", panel) + "</defs>\n"
    body += f'<g clip-path="url(#c2)">\n'
    body += grid(s, origin, (-4, 8), (-1, 7), panel, stroke="#e2e6eb", dots=False)
    px, py = to_xy(centre_q, centre_t, s, origin)
    body += f'<polygon points="{hexagon(px, py, s)}" fill="{ACCENT_FILL}" stroke="none"/>\n'
    body += grid(s, origin, (-4, 8), (-1, 7), panel, stroke=ACCENT, dots=False, shift=shift, width=1.6)
    body += f'<circle cx="{px:.2f}" cy="{py:.2f}" r="{r:.2f}" fill="none" stroke="{INK}" stroke-width="1.4" ' \
            f'stroke-dasharray="5 4"/>\n'
    body += f'<circle cx="{px:.2f}" cy="{py:.2f}" r="{s:.2f}" fill="none" stroke="{MUTED}" stroke-width="1.2" ' \
            f'stroke-dasharray="2 4"/>\n'
    body += f'<circle cx="{px:.2f}" cy="{py:.2f}" r="8" fill="{INK}" stroke="{BG}" stroke-width="2"/>\n'
    gx, gy = px + 0.55 * r, py - 0.5 * r
    body += f'<circle cx="{gx:.2f}" cy="{gy:.2f}" r="7" fill="{GOOD}" stroke="{BG}" stroke-width="2"/>\n'
    bx, by = px - 1.25 * s, py + 0.35 * s
    body += f'<circle cx="{bx:.2f}" cy="{by:.2f}" r="7" fill="{BAD}" stroke="{BG}" stroke-width="2"/>\n'
    body += (
        f'<line x1="{px:.2f}" y1="{py:.2f}" x2="{px + r:.2f}" y2="{py:.2f}" stroke="{INK}" stroke-width="1.6" '
        f'marker-end="url(#a-ink)"/>\n'
    )
    body += text(px + r / 2, py + 18, "r", 15, INK, "middle", "bold")
    body += "</g>\n"
    body += f'<rect x="{panel[0]}" y="{panel[1]}" width="{panel[2] - panel[0]}" height="{panel[3] - panel[1]}" ' \
            f'rx="8" fill="none" stroke="{GRID}"/>\n'

    lx = 500
    body += text(lx, 70, "The accepted hexagon", 18, INK, weight="bold")
    body += text(lx, 92, "is centred on the enrolled tap", 18, INK, weight="bold")
    body += f'<line x1="{lx}" y1="130" x2="{lx + 18}" y2="130" stroke="{ACCENT}" stroke-width="2"/>\n'
    body += text(lx + 26, 135, "This point's grid: the fixed", 14, MUTED)
    body += text(lx + 26, 157, "grid (grey) moved by the offset.", 14, MUTED)
    body += f'<circle cx="{lx + 8}" cy="190" r="7" fill="{INK}"/>\n'
    body += text(lx + 26, 195, "Enrolled tap.", 14, MUTED)
    body += f'<circle cx="{lx + 8}" cy="222" r="7" fill="{GOOD}"/>\n'
    body += text(lx + 26, 227, "Later tap inside: accepted.", 14, MUTED)
    body += f'<circle cx="{lx + 8}" cy="254" r="7" fill="{BAD}"/>\n'
    body += text(lx + 26, 259, "Later tap outside: rejected.", 14, MUTED)
    body += f'<line x1="{lx}" y1="290" x2="{lx + 18}" y2="290" stroke="{INK}" stroke-dasharray="5 4"/>\n'
    body += text(lx + 26, 295, "r: always accepted within this.", 14, MUTED)
    body += text(lx + 26, 317, "2.2% of the image width.", 14, MUTED)
    body += f'<line x1="{lx}" y1="348" x2="{lx + 18}" y2="348" stroke="{MUTED}" stroke-dasharray="2 4"/>\n'
    body += text(lx + 26, 353, "s = 2r/√3: never accepted", 14, MUTED)
    body += text(lx + 26, 375, "beyond this (the corners).", 14, MUTED)
    return svg(w, h, body, "The grid shifted so the accepted hexagon is centred on the enrolled tap")


def image_diagram():
    """A whole image at true scale with a five-tap sequence and each tap's accepted hexagon."""
    pw, ph = 640, 480
    ox, oy = 24, 24
    w, h = pw + 2 * ox, ph + oy + 70
    r = 0.022 * pw
    s = 2 * r / SQRT3

    body = arrow_defs() + "<defs>" + clip_path("c3", (ox, oy, ox + pw, oy + ph)) + "</defs>\n"
    body += '<g clip-path="url(#c3)">\n'
    body += f'<rect x="{ox}" y="{oy}" width="{pw}" height="{ph}" fill="#e0f2fe"/>\n'
    body += f'<circle cx="{ox + 520}" cy="{oy + 95}" r="46" fill="#fde68a"/>\n'
    body += f'<path d="M{ox},{oy + 330} C{ox + 140},{oy + 250} {ox + 260},{oy + 300} {ox + 380},{oy + 280} ' \
            f'S{ox + 560},{oy + 230} {ox + pw},{oy + 290} L{ox + pw},{oy + ph} L{ox},{oy + ph} z" fill="#bbf7d0"/>\n'
    body += f'<path d="M{ox},{oy + 400} C{ox + 200},{oy + 360} {ox + 420},{oy + 430} {ox + pw},{oy + 380} ' \
            f'L{ox + pw},{oy + ph} L{ox},{oy + ph} z" fill="#86efac"/>\n'
    for tx, ty, tr in ((120, 300, 34), (175, 290, 24), (455, 262, 30)):
        body += f'<rect x="{ox + tx - 4}" y="{oy + ty}" width="8" height="40" fill="#92400e"/>\n'
        body += f'<circle cx="{ox + tx}" cy="{oy + ty - tr / 2}" r="{tr}" fill="#4ade80"/>\n'
    body += f'<rect x="{ox + 290}" y="{oy + 330}" width="70" height="52" fill="#fca5a5"/>\n'
    body += f'<path d="M{ox + 282},{oy + 332} L{ox + 325},{oy + 298} L{ox + 368},{oy + 332} z" fill="#b91c1c"/>\n'
    body += f'<rect x="{ox + 316}" y="{oy + 352}" width="18" height="30" fill="#7c2d12"/>\n'
    body += "</g>\n"
    body += f'<rect x="{ox}" y="{oy}" width="{pw}" height="{ph}" fill="none" stroke="{GRID}"/>\n'

    taps = [(520, 95), (120, 284), (325, 312), (455, 246), (594, 412)]
    pts = " ".join(f"{ox + x},{oy + y}" for x, y in taps)
    body += f'<polyline points="{pts}" fill="none" stroke="{INK}" stroke-width="1.5" stroke-dasharray="6 5"/>\n'
    for i, (x, y) in enumerate(taps, 1):
        cx, cy = ox + x, oy + y
        body += f'<polygon points="{hexagon(cx, cy, s)}" fill="{BG}" fill-opacity="0.55" stroke="{ACCENT}" ' \
                f'stroke-width="1.8"/>\n'
        body += f'<circle cx="{cx}" cy="{cy}" r="3" fill="{INK}"/>\n'
        body += f'<circle cx="{cx + 24}" cy="{cy - 20}" r="11" fill="{INK}"/>\n'
        body += text(cx + 24, cy - 15.5, str(i), 13, BG, "middle", "bold")

    sy = oy + ph + 38
    body += f'<line x1="{ox}" y1="{sy}" x2="{ox + r:.2f}" y2="{sy}" stroke="{INK}" stroke-width="3"/>\n'
    body += text(ox + r + 10, sy + 5, "r = 2.2% of the image width. The hexagons are drawn to scale.", 14, MUTED)
    return svg(w, h, body, "An image with a five-tap sequence and each tap's accepted hexagon, to scale")


def main():
    (OUT / "tap-split.svg").write_text(split_diagram())
    (OUT / "shifted-grid.svg").write_text(shifted_diagram())
    (OUT / "sequence-on-image.svg").write_text(image_diagram())


if __name__ == "__main__":
    main()
