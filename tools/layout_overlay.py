"""Validate the panel layout against high-resolution real-panel crops.

    cargo run -p nord-gui --example layout_dump_gui > target/layout_check/layout_gui.json
    python tools/layout_overlay.py organ [--source gui|layout] [--anchors "px,py,mx,my;..."]
                                         [--shots] [--out DIR] [--crops DIR]

For one section it
  1. maps crop pixels <-> panel mm: for the HA88 render and the `organ` / `piano` / `master`
     crops with ONE isotropic scale (the crops are registered onto the render, see `ISO`); for
     the other crops with an affine fit per axis from anchor points (default: the crop's section
     outline corners = the section rectangle of the dump; `--anchors` overrides),
  2. draws every control rectangle of the section with its id over the crop
     (`<out>/<crop>_overlay.png`), and, with `--shots`, a side-by-side with our own screenshot
     (`python tools/gui_shots.py --scale 4 --sections <section>`),
  3. reads `crates/nord-panel/data/photo_anchors_<section>.csv` (id, photo_x, photo_y: the
     real control's centre in crop px, read by eye from the crop; `#` comment lines; an optional
     `shared_with` column marks ids that draw on another control's rectangle and are not
     checked) and prints per control the offset in mm between our rectangle centre and the real
     centre, flagging |offset| > 2 mm.

The `effects` crop is mapped isotropically (`ISOTROPIC`), not onto the section rectangle.

The crops live in `.reference/crops` (git-ignored, never committed); output goes to
`target/layout_check/` (git-ignored). Needs Pillow.
"""

import argparse
import csv
import json
import math
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DEFAULT_OUT = os.path.join(ROOT, "target", "layout_check")


def find_crops_dir(start=ROOT):
    """`$NS4_CROPS`, else the nearest `.reference/crops` up the tree (a worktree sits inside
    the main checkout, which owns the git-ignored reference dir)."""
    env = os.environ.get("NS4_CROPS")
    if env:
        return env
    d = start
    while True:
        cand = os.path.join(d, ".reference", "crops")
        if os.path.isdir(cand):
            return cand
        parent = os.path.dirname(d)
        if parent == d:
            return os.path.join(start, ".reference", "crops")
        d = parent


DEFAULT_CROPS = find_crops_dir()
DATA_DIR = os.path.join(ROOT, "crates", "nord-panel", "data")
THRESHOLD_MM = 2.0

# crop name -> (file, section whose rectangle the outline maps to, outline in crop px
# (x0, y0, x1, y1) or None = whole image, sections drawn on it, optional window in image px
# (x0, y0, x1, y1) that is the only part drawn and saved). A file name starting with `../` is
# looked up next to the crops dir (`.reference/`).
#
# `program`, `keybed` and `wheels` use the straight-on HA88 top-down render (11600 px wide, body
# x 400..11196 = 1290 mm, 0.1195 mm/px on both axes, panel top edge y 482, keybed boundary
# y 1981 = 179.1 mm); the outline is the section rectangle in that image, so the mapping is
# isotropic (unlike the organ crop, which shows only the sections and no connector strip).
TOPDOWN = "../NS4_HA88_TopDown-01_241008.jpg"
# Millimetres per render pixel (1290 mm / 10796 px) and the render pixel of the panel origin.
MM_PER_TD_PX = 1290.0 / 10796.0
TD_ORIGIN = (400.0, 482.0)
# Section crops registered onto the render: render px = (rx + k * crop_x, ry + k * crop_y), the same
# k on both axes (multi-scale `cv2.matchTemplate` of the crop against the render, TM_CCOEFF_NORMED:
# organ 0.920 at k 0.995, piano 0.919 at k 0.990). Section top edge: render y 691 px = 25.0 mm.
# The older affine outline mapping stretched the 137 mm tall sections onto the 179 mm panel height.
ISO = {
    "organ": (2166.5, 678.75, 0.995),
    "piano": (4112.3, 674.2, 0.990),
}
CROPS = {
    "program": (
        TOPDOWN,
        "program",
        (4880.0, 482.0, 5968.0, 1981.0),
        ("program",),
        (4700, 400, 6150, 2100),
    ),
    "keybed": (
        TOPDOWN,
        "keybed",
        (400.0, 1981.0, 11196.0, 3318.0),
        ("keybed",),
        (300, 1700, 11300, 3400),
    ),
    "wheels": (
        TOPDOWN,
        "master",
        (400.0, 482.0, 2415.0, 1981.0),
        ("master",),
        (1300, 700, 2500, 1550),
    ),
    "organ": (
        "NS4-Panel-organ.max-2000x2000.format-png.png",
        "organ",
        None,
        ("master", "organ"),
        None,
    ),
    "piano": ("NS4-Panel-piano.max-2000x2000.format-png.png", "piano", None, ("piano",), None),
    "synth": ("NS4-Panel-synth.max-2000x2000.format-png.png", "synth", None, ("synth",), None),
    "effects": (
        "NS4-Panel-effects.max-2000x2000.format-png.png",
        "effects",
        None,
        ("effects",),
        None,
    ),
}
# Hero crops that are NOT stretched onto their section: crop -> (mm per crop px, panel x in mm of
# crop x = 0, panel y in mm of crop y = 0). One scale on both axes; the crop shows the section
# header strip but not the 26 mm connector strip above it or the 18 mm lip below it.
#   effects: registered onto the straight-on HA88 render by FFT cross-correlation (peak 0.76 at
#   1.0175 crop px per render px, offset (17.5, 10.0) crop px from render px (8138, 683)):
#   0.119488 mm/px / 1.0175 = 0.11743 mm/px; panel mm = (render px - (400, 482)) * 0.119488.
ISOTROPIC = {"effects": (0.11743, 922.55, 22.85)}
SECTION_CROP = {
    "master": "organ",
    "organ": "organ",
    "piano": "piano",
    "synth": "synth",
    "effects": "effects",
    "program": "program",
    "keybed": "keybed",
    "wheels": "wheels",
}


def fit_axis(pairs):
    """Least-squares `mm = a * px + b` from `(px, mm)` pairs; returns `(a, b)`."""
    if len(pairs) < 2:
        raise ValueError("need at least two anchors per axis")
    n = len(pairs)
    sx = sum(p for p, _ in pairs)
    sy = sum(m for _, m in pairs)
    sxx = sum(p * p for p, _ in pairs)
    sxy = sum(p * m for p, m in pairs)
    den = n * sxx - sx * sx
    if abs(den) < 1e-9:
        raise ValueError("anchors share the same coordinate on one axis")
    a = (n * sxy - sx * sy) / den
    return a, (sy - a * sx) / n


class Mapping:
    """Crop px <-> panel mm, one scale and offset per axis."""

    def __init__(self, anchors):
        self.ax, self.bx = fit_axis([(a[0], a[2]) for a in anchors])
        self.ay, self.by = fit_axis([(a[1], a[3]) for a in anchors])

    def to_mm(self, px, py):
        return self.ax * px + self.bx, self.ay * py + self.by

    def to_px(self, mx, my):
        return (mx - self.bx) / self.ax, (my - self.by) / self.ay

    @classmethod
    def isotropic(cls, rx, ry, k):
        """Mapping of a crop registered onto the straight-on render (same mm/px on both axes)."""
        m = cls.__new__(cls)
        m.ax = m.ay = k * MM_PER_TD_PX
        m.bx = (rx - TD_ORIGIN[0]) * MM_PER_TD_PX
        m.by = (ry - TD_ORIGIN[1]) * MM_PER_TD_PX
        return m

    def anisotropy(self):
        """Vertical over horizontal mm per crop px (1.0 = same scale on both axes)."""
        return self.ay / self.ax


def parse_anchors(text):
    """`"px,py,mmx,mmy;px,py,mmx,mmy"` -> list of 4-tuples."""
    out = []
    for item in text.split(";"):
        item = item.strip()
        if item:
            vals = [float(v) for v in item.split(",")]
            if len(vals) != 4:
                raise ValueError(f"anchor needs 4 numbers: {item!r}")
            out.append(tuple(vals))
    return out


def outline_anchors(outline, rect):
    """Corner anchors mapping the crop outline `(x0, y0, x1, y1)` onto a mm rect dict."""
    x0, y0, x1, y1 = outline
    return [
        (x0, y0, rect["x"], rect["y"]),
        (x1, y1, rect["x"] + rect["w"], rect["y"] + rect["h"]),
    ]


def load_photo_anchors(path, shared=False):
    """`{id: (photo_x, photo_y)}` from the committed CSV (`#` lines are comments).

    Rows with a `shared_with` column value name an id that draws on another control's rectangle
    (a Shift function or LED choice): they are left out unless `shared` is true, because their
    real position cannot be checked against a rectangle that is not theirs.
    """
    with open(path, newline="", encoding="utf-8") as f:
        lines = (ln for ln in f if not ln.lstrip().startswith("#"))
        rows = list(csv.DictReader(lines))
    return {
        r["id"]: (float(r["photo_x"]), float(r["photo_y"]))
        for r in rows
        if bool((r.get("shared_with") or "").strip()) == shared
    }


def pick_rect(control, source):
    """Rectangle of a control: the drawn one (`gui`) when asked and present."""
    if source == "gui" and "gui" in control:
        return control["gui"]
    return control["rect"]


def centre(r):
    return r["x"] + r["w"] / 2, r["y"] + r["h"] / 2


def compute_offsets(controls, photo, mapping, source="gui"):
    """Per control with a photo anchor: our centre minus the real centre, in mm.

    Returns rows `(id, dx, dy, dist)`; positive dx means ours sits right of the real control.
    """
    by_id = {c["id"]: c for c in controls}
    rows = []
    for cid, (px, py) in photo.items():
        c = by_id.get(cid)
        if c is None:
            continue
        ox, oy = centre(pick_rect(c, source))
        rx, ry = mapping.to_mm(px, py)
        rows.append((cid, ox - rx, oy - ry, math.hypot(ox - rx, oy - ry)))
    return rows


def format_report(rows, threshold=THRESHOLD_MM):
    """Text table sorted by distance; flagged rows are marked `!`."""
    lines = [f"{'id':34s} {'dx mm':>7s} {'dy mm':>7s} {'|d| mm':>7s}"]
    for cid, dx, dy, d in sorted(rows, key=lambda r: -r[3]):
        flag = " !" if d > threshold else ""
        lines.append(f"{cid:34s} {dx:7.2f} {dy:7.2f} {d:7.2f}{flag}")
    if rows:
        flagged = sum(1 for r in rows if r[3] > threshold)
        mean = sum(r[3] for r in rows) / len(rows)
        lines.append(
            f"{len(rows)} controls, {flagged} over {threshold} mm, "
            f"mean {mean:.2f} mm, max {max(r[3] for r in rows):.2f} mm"
        )
    return "\n".join(lines)


def draw_overlay(img, controls, photo, mapping, source="gui", frames=(), origin=(0, 0)):
    """Semi-transparent control rectangles with ids, real centres as crosses; returns RGB."""
    from PIL import Image, ImageDraw

    ox, oy = origin

    def to_px(mx, my):
        x, y = mapping.to_px(mx, my)
        return x - ox, y - oy

    base = img.convert("RGBA")
    layer = Image.new("RGBA", base.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    for fr in frames:
        r = fr["rect"]
        x0, y0 = to_px(r["x"], r["y"])
        x1, y1 = to_px(r["x"] + r["w"], r["y"] + r["h"])
        d.rectangle([x0, y0, x1, y1], outline=(255, 255, 0, 150), width=2)
    for c in controls:
        r = pick_rect(c, source)
        x0, y0 = to_px(r["x"], r["y"])
        x1, y1 = to_px(r["x"] + r["w"], r["y"] + r["h"])
        est = c.get("estimated")
        fill = (255, 140, 0, 70) if est else (0, 180, 255, 70)
        edge = (255, 140, 0, 230) if est else (0, 120, 255, 230)
        d.rectangle([x0, y0, x1, y1], fill=fill, outline=edge, width=2)
        d.text((x0 + 2, y0 + 1), c["id"].split(".", 1)[-1], fill=(255, 255, 255, 255))
        if c["id"] in photo:
            px, py = photo[c["id"]][0] - ox, photo[c["id"]][1] - oy
            cx, cy = to_px(*centre(r))
            d.line([cx, cy, px, py], fill=(255, 60, 60, 220), width=2)
            d.line([px - 8, py, px + 8, py], fill=(60, 255, 60, 255), width=2)
            d.line([px, py - 8, px, py + 8], fill=(60, 255, 60, 255), width=2)
    return Image.alpha_composite(base, layer).convert("RGB")


def side_by_side(left, right):
    """Two images next to each other, scaled to the same height."""
    from PIL import Image

    h = min(left.height, right.height)
    a = left.resize((round(left.width * h / left.height), h))
    b = right.resize((round(right.width * h / right.height), h))
    out = Image.new("RGB", (a.width + b.width + 10, h), (20, 20, 20))
    out.paste(a, (0, 0))
    out.paste(b, (a.width + 10, 0))
    return out


def load_layout(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def build_mapping(layout, crop, anchors_text):
    _, sec, outline, _, _ = CROPS[crop]
    if anchors_text:
        return Mapping(parse_anchors(anchors_text))
    if crop in ISOTROPIC:
        scale, x0, y0 = ISOTROPIC[crop]
        return Mapping([(0.0, 0.0, x0, y0), (1000.0, 1000.0, x0 + 1000 * scale, y0 + 1000 * scale)])
    if crop in ISO:
        return Mapping.isotropic(*ISO[crop])
    if outline is None:
        raise SystemExit(f"crop {crop!r} has no built-in outline: pass --anchors")
    return Mapping(outline_anchors(outline, layout["sections"][sec]))


def main(argv=None):
    from PIL import Image

    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("section", choices=sorted(SECTION_CROP))
    ap.add_argument("--layout", default=os.path.join(DEFAULT_OUT, "layout_gui.json"))
    ap.add_argument("--source", choices=["gui", "layout"], default="gui")
    ap.add_argument("--anchors", default="", help='"px,py,mm_x,mm_y;..." (>= 2)')
    ap.add_argument("--crops", default=DEFAULT_CROPS)
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--csv", default="", help="photo anchor CSV (default: crates/nord-panel/data)")
    ap.add_argument(
        "--shots", action="store_true", help="also write the side-by-side with our shot"
    )
    args = ap.parse_args(argv)

    if not os.path.exists(args.layout):
        sys.exit(f"missing {args.layout}: run cargo run -p nord-gui --example layout_dump_gui")
    layout = load_layout(args.layout)
    crop = SECTION_CROP[args.section]
    fname, _, _, _, window = CROPS[crop]
    mapping = build_mapping(layout, crop, args.anchors)
    csv_path = args.csv or os.path.join(DATA_DIR, f"photo_anchors_{args.section}.csv")
    photo = load_photo_anchors(csv_path) if os.path.exists(csv_path) else {}
    controls = [c for c in layout["controls"] if c["section"] == args.section or c["id"] in photo]
    frames = [f for f in layout["frames"] if f["section"] == args.section]
    os.makedirs(args.out, exist_ok=True)

    Image.MAX_IMAGE_PIXELS = None
    img = Image.open(os.path.join(args.crops, fname))
    origin = (0, 0)
    if window:
        img = img.crop(window)
        origin = window[:2]
    over = draw_overlay(img, controls, photo, mapping, args.source, frames, origin)
    if over.width > 5000:
        over = over.resize((5000, round(over.height * 5000 / over.width)))
    png = os.path.join(args.out, f"{args.section}_overlay.png")
    over.save(png)
    print(png)
    if args.shots:
        subprocess.run(
            [
                sys.executable,
                os.path.join(ROOT, "tools", "gui_shots.py"),
                "--scale",
                "4",
                "--sections",
                args.section,
                "--no-build",
                "--out",
                os.path.join(args.out, "shots"),
            ],
            check=True,
        )
        shot = Image.open(os.path.join(args.out, "shots", f"{args.section}.png"))
        both = os.path.join(args.out, f"{args.section}_side_by_side.png")
        side_by_side(over, shot).save(both)
        print(both)
    print(f"scale mm/px: x {mapping.ax:.5f} y {mapping.ay:.5f} (y/x = {mapping.anisotropy():.3f})")
    print(format_report(compute_offsets(controls, photo, mapping, args.source)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
