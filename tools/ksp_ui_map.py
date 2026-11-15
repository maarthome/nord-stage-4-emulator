#!/usr/bin/env python3
"""Map the ``on ui_control`` handlers of a decoded Kontakt KSP script to engine parameters.

Each handler of the Ultimate Pianos 2 script looks like::

    on ui_control($v)
    set_engine_par($ENGINE_PAR_SENDLEVEL_1,$v,-1,7,$NI_BUS_OFFSET+$qgbyb)
    set_control_par_str(get_ui_id($lbl),$CONTROL_PAR_TEXT,"Reverb Amount [A]")
    set_control_par_str(get_ui_id($val),$CONTROL_PAR_TEXT,$v*10/62996/10 & "." & ...)
    end on

so the control variable, the Kontakt engine parameter it drives (slot, unit and
bus arguments), its label and the display formula can be read straight from text.
Saved control values come from the ``$name value`` pairs stored after the script in
the decoded ``.nki`` stream (see tools/nki_lz.py).
"""

from __future__ import annotations

import argparse
import csv
import re
import sys
from dataclasses import dataclass
from pathlib import Path

HANDLER_RE = re.compile(r"^on ui_control\((\$\w+)\)\n(.*?)^end on$", re.S | re.M)
ENGINE_RE = re.compile(r"set_engine_par\((\$ENGINE_PAR_\w+),([^\n]*)\)")
LABEL_RE = re.compile(r'\$CONTROL_PAR_TEXT,"([^"]+)"\)')
DISPLAY_RE = re.compile(r"\$CONTROL_PAR_TEXT,([^\"\n][^\n]*)\)\n")
VALUE_RE = re.compile(rb"[\x00-\x20]\x00\x00\x00(\$[A-Za-z0-9_]{3,10}) (-?\d+)")
FIELDS = ["var", "engine_par", "args", "label", "display_formula", "saved_value"]


@dataclass
class Control:
    var: str
    engine_par: str
    args: str
    label: str
    display_formula: str
    saved_value: int | None


def parse_handlers(script: str) -> list[tuple[str, str]]:
    """Return (variable, body) for every ``on ui_control`` handler."""
    return [(m.group(1), m.group(2)) for m in HANDLER_RE.finditer(script)]


def parse_saved_values(tail: bytes) -> dict[str, int]:
    """Saved ``$name value`` pairs (first occurrence wins) from the bytes after the script."""
    vals: dict[str, int] = {}
    for m in VALUE_RE.finditer(tail):
        vals.setdefault(m.group(1).decode(), int(m.group(2)))
    return vals


def build_controls(script: str, saved: dict[str, int]) -> list[Control]:
    """One Control per handler that sets an engine parameter."""
    out: list[Control] = []
    for var, body in parse_handlers(script):
        e = ENGINE_RE.search(body)
        if not e:
            continue
        lab = LABEL_RE.search(body)
        disp = DISPLAY_RE.search(body)
        out.append(
            Control(
                var=var,
                engine_par=e.group(1),
                args=e.group(2),
                label=lab.group(1) if lab else "",
                display_formula=disp.group(1).strip() if disp else "",
                saved_value=saved.get(var),
            )
        )
    return out


def write_csv(path: Path, controls: list[Control]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(FIELDS)
        for c in controls:
            sv = "" if c.saved_value is None else c.saved_value
            w.writerow([c.var, c.engine_par, c.args, c.label, c.display_formula, sv])


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Map KSP ui_control handlers to engine parameters.")
    ap.add_argument("decoded", type=Path, help="decoded stream from tools/nki_lz.py")
    ap.add_argument("script_len", type=int, help="length of the leading KSP script in bytes")
    ap.add_argument("out_csv", type=Path)
    a = ap.parse_args(argv)
    data = a.decoded.read_bytes()
    script = data[: a.script_len].decode("latin1")
    controls = build_controls(script, parse_saved_values(data[a.script_len :]))
    write_csv(a.out_csv, controls)
    with_vals = sum(c.saved_value is not None for c in controls)
    print(f"{len(controls)} engine-parameter controls, {with_vals} with saved values")
    return 0


if __name__ == "__main__":
    sys.exit(main())
