#!/usr/bin/env python3
"""Extract literal integer arrays from a KSP script (``declare %name[N] := (...)``).

The Ultimate Pianos 2 script (``tools/nki_lz.py --script``) keeps its effect tables as
literal arrays: chorus rate and amount, phaser rate / amount / feedback / center,
flanger rate / amount / feedback, and 128-entry curves. Names are obfuscated
(``%idia3``); the effect they feed comes from where ``set_engine_par`` reads them
(``ENGINE_PAR_*``), found with :func:`engine_par_uses`.

Arrays that hold ASCII codes (file paths, strings) are flagged ``text`` and decoded.
"""

from __future__ import annotations

import argparse
import csv
import re
import sys
from dataclasses import dataclass
from pathlib import Path

DECL_RE = re.compile(r"^declare\s+%(\w+)\[(\d+)\]\s*:=\s*\(", re.M)
USE_RE = re.compile(r"set_engine_par\(\$(ENGINE_PAR_\w+)\s*,\s*([^\n]*)")
CONT_RE = re.compile(r"\.\.\.\s*\r?\n")  # KSP line continuation
TEXT_MIN = 32
TEXT_MAX = 126


@dataclass
class KspArray:
    name: str
    size: int
    values: list[int]

    @property
    def is_text(self) -> bool:
        """True when every value is printable ASCII and the array is long enough to be a string."""
        v = self.values
        return len(v) >= 8 and all(TEXT_MIN <= x <= TEXT_MAX for x in v)

    @property
    def text(self) -> str:
        return "".join(chr(x) for x in self.values) if self.is_text else ""


def parse_arrays(script: str) -> list[KspArray]:
    """All ``declare %x[N] := (...)`` arrays; the value list may span several lines."""
    out: list[KspArray] = []
    for m in DECL_RE.finditer(script):
        end = script.find(")", m.end())
        if end < 0:
            continue
        body = CONT_RE.sub(" ", script[m.end() : end])
        try:
            values = [int(t) for t in body.replace("\n", " ").split(",") if t.strip()]
        except ValueError:
            continue
        out.append(KspArray(m.group(1), int(m.group(2)), values))
    return out


def engine_par_uses(script: str) -> dict[str, list[str]]:
    """Map array name -> engine parameters whose value expression reads it (``%name[``)."""
    uses: dict[str, list[str]] = {}
    for m in USE_RE.finditer(script):
        par, expr = m.group(1), m.group(2)
        for name in re.findall(r"%(\w+)\[", expr):
            lst = uses.setdefault(name, [])
            if par not in lst:
                lst.append(par)
    return uses


def write_csv(path: Path, arrays: list[KspArray], uses: dict[str, list[str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(["name", "size", "parsed", "engine_par", "text", "values"])
        for a in arrays:
            w.writerow(
                [
                    a.name,
                    a.size,
                    len(a.values),
                    " ".join(uses.get(a.name, [])),
                    a.text,
                    " ".join(str(v) for v in a.values),
                ]
            )


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Extract literal arrays from a KSP script.")
    ap.add_argument("script", type=Path)
    ap.add_argument("out_csv", type=Path)
    a = ap.parse_args(argv)
    text = a.script.read_text(encoding="utf-8", errors="replace")
    arrays = parse_arrays(text)
    uses = engine_par_uses(text)
    write_csv(a.out_csv, arrays, uses)
    fx = [x for x in arrays if x.name in uses]
    print(
        f"{len(arrays)} arrays, {sum(x.is_text for x in arrays)} text, {len(fx)} feed engine params"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
