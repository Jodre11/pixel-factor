#!/usr/bin/env python3
"""Draw the demo image, an abstract harbour scene, for the Keycloak plugin.

Run from the repository root: python3 keycloak/demo/make_image.py
Standard library only. Deterministic: the same output every run. Aspect 0.75 (height over width), matching the demo's
PF_IMAGES entry demo=0.75. No faces and no text (spec section 11); more than 30 distinct features to tap.
"""

import math
import random
from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / "src/main/resources/theme-resources/resources/img/demo.svg"
W, H = 1200, 900

SKY = "#bfdbfe"
SEA = "#1e3a8a"
SAND = "#fde68a"
HILL = "#4d7c0f"
COLOURS = ["#dc2626", "#ea580c", "#facc15", "#16a34a", "#0891b2", "#7c3aed", "#db2777", "#f8fafc", "#0f172a"]


def main():
    rng = random.Random(20260101)
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}">',
        f'<rect width="{W}" height="{H}" fill="{SKY}"/>',
        '<circle cx="1010" cy="140" r="70" fill="#fbbf24"/>',
    ]
    # Clouds.
    for cx, cy in [(180, 110), (480, 70), (760, 150)]:
        for dx, r in [(-40, 30), (0, 42), (40, 32)]:
            parts.append(f'<circle cx="{cx + dx}" cy="{cy}" r="{r}" fill="#f8fafc"/>')
    # Hills, each with a few trees.
    for hx, hr in [(150, 260), (520, 210), (900, 280)]:
        parts.append(f'<ellipse cx="{hx}" cy="430" rx="{hr}" ry="150" fill="{HILL}"/>')
        for k in range(3):
            tx = hx - hr / 2 + k * hr / 2
            parts.append(f'<rect x="{tx - 5:.0f}" y="330" width="10" height="40" fill="#78350f"/>')
            parts.append(f'<circle cx="{tx:.0f}" cy="320" r="24" fill="#166534"/>')
    # Lighthouse.
    parts.append('<polygon points="1080,430 1120,430 1110,250 1090,250" fill="#f8fafc"/>')
    parts.append('<rect x="1086" y="300" width="28" height="20" fill="#dc2626"/>')
    parts.append('<rect x="1084" y="230" width="32" height="20" fill="#facc15"/>')
    # Sea and beach.
    parts.append(f'<rect y="430" width="{W}" height="320" fill="{SEA}"/>')
    parts.append(f'<path d="M0 750 Q300 700 600 740 T1200 730 V{H} H0 Z" fill="{SAND}"/>')
    # Boats: hull, mast and sail, each in its own colours.
    for i, (bx, by) in enumerate([(120, 520), (330, 600), (560, 500), (780, 640), (980, 540), (450, 680)]):
        hull = COLOURS[i % len(COLOURS)]
        sail = COLOURS[(i + 3) % len(COLOURS)]
        parts.append(f'<polygon points="{bx - 50},{by} {bx + 50},{by} {bx + 35},{by + 22} {bx - 35},{by + 22}"'
                     f' fill="{hull}"/>')
        parts.append(f'<rect x="{bx - 2}" y="{by - 80}" width="4" height="80" fill="#0f172a"/>')
        parts.append(f'<polygon points="{bx + 2},{by - 78} {bx + 2},{by - 8} {bx + 45},{by - 8}" fill="{sail}"/>')
    # Buoys.
    for bx, by in [(240, 470), (680, 580), (900, 470), (60, 650), (1130, 620)]:
        parts.append(f'<circle cx="{bx}" cy="{by}" r="12" fill="#ea580c"/>')
        parts.append(f'<rect x="{bx - 12}" y="{by - 3}" width="24" height="6" fill="#f8fafc"/>')
    # Shells, starfish and stones on the beach.
    for _ in range(12):
        sx = rng.randint(40, W - 40)
        sy = rng.randint(785, H - 30)
        kind = rng.choice(["stone", "star", "shell"])
        colour = rng.choice(COLOURS[:7])
        if kind == "stone":
            parts.append(f'<ellipse cx="{sx}" cy="{sy}" rx="18" ry="11" fill="#57534e"/>')
        elif kind == "star":
            pts = []
            for k in range(10):
                r = 18 if k % 2 == 0 else 7
                a = math.radians(-90 + 36 * k)
                pts.append(f"{sx + r * math.cos(a):.1f},{sy + r * math.sin(a):.1f}")
            parts.append(f'<polygon points="{" ".join(pts)}" fill="{colour}"/>')
        else:
            parts.append(f'<path d="M{sx - 14} {sy} A14 14 0 0 1 {sx + 14} {sy} Z" fill="{colour}"/>')
    parts.append("</svg>")
    OUT.write_text("\n".join(parts) + "\n")


if __name__ == "__main__":
    main()
