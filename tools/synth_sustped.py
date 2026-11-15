#!/usr/bin/env python3
"""Per-program Synth SUSTPED bit (`Edt_Syn_kSustainEnable`, id `0xF8`) of every factory program.

Evidence for D97: the firmware edit-parameter table (`docs/research/arm-edit-params.csv`) gives
`Edt_Syn_kSustainEnable` the range 0..1 and the default 1 (piano 1, organ 0); the program stores
the flag as the 1-bit field at block offset +71 of each Synth layer block (the `0x32` Layer chain
checked by `tools/ns4_fw_order_check.py`; blocks at payload section 327, bit 248 / 656 / 1064, the
layer blocks of `cbin/kb_zone.rs`). This tool reads that bit from every `.ns4p` of a backup and
writes derived numbers only (a CSV of program name, three bits and the layer enables).

    python tools/synth_sustped.py "Nord Stage 4 Factory Restore RevK v1.66.ns4b" \
        --csv docs/research/data/synth-sustped-programs.csv
"""

from __future__ import annotations

import argparse
import csv
import sys
import zipfile
from pathlib import Path

HEADER_LEN = 48  # CBIN header; the payload is the next 820 bytes
SYNTH_SECTION = 327  # payload byte offset of the synth section
LAYER_BLOCKS = (248, 656, 1064)  # bit offsets of synth layers A, B, C in the section
SUSTAIN_FIELD = 71  # `Edt_Syn_kSustainEnable`, 1 bit (`ns4_fw_order_check.LAYER_CHAIN`)
FIRMWARE_DEFAULT = 1  # arm-edit-params.csv row 307: min 0, max 1, default 1


def read_bit(payload: bytes, section: int, bit: int) -> int:
    """MSB-first bit `bit` of the section starting at byte `section` of `payload`."""
    pos = section * 8 + bit
    return (payload[pos // 8] >> (7 - pos % 8)) & 1


def synth_sustain_bits(payload: bytes) -> tuple[int, int, int]:
    """The SUSTPED bits of synth layers A, B and C of an 820-byte program payload."""
    return tuple(read_bit(payload, SYNTH_SECTION, b + SUSTAIN_FIELD) for b in LAYER_BLOCKS)


def scan(archive: Path) -> list[dict]:
    """One row per `Program/*.ns4p` entry of the backup."""
    rows = []
    with zipfile.ZipFile(archive) as z:
        for info in z.infolist():
            if not (info.filename.startswith("Program/") and info.filename.endswith(".ns4p")):
                continue
            raw = z.read(info)
            payload = raw[HEADER_LEN : HEADER_LEN + 820]
            if len(payload) < 820:
                continue
            a, b, c = synth_sustain_bits(payload)
            rows.append({"program": info.filename, "synth_a": a, "synth_b": b, "synth_c": c})
    return rows


def summary(rows: list[dict]) -> dict:
    """Counts of set bits per layer and the number of programs that differ from the default."""
    n = len(rows)
    per_layer = {k: sum(r[k] for r in rows) for k in ("synth_a", "synth_b", "synth_c")}
    off_default = sum(
        1 for r in rows if any(r[k] != FIRMWARE_DEFAULT for k in ("synth_a", "synth_b", "synth_c"))
    )
    return {"programs": n, "set": per_layer, "off_default": off_default}


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("backup", help="Factory .ns4b backup (a zip).")
    p.add_argument("--csv", help="Write the per-program bits here (derived numbers only).")
    args = p.parse_args(argv)
    if not Path(args.backup).exists():
        print(f"missing backup: {args.backup}", file=sys.stderr)
        return 2
    rows = scan(Path(args.backup))
    if args.csv:
        with open(args.csv, "w", newline="", encoding="utf-8") as f:
            w = csv.DictWriter(f, fieldnames=["program", "synth_a", "synth_b", "synth_c"])
            w.writeheader()
            w.writerows(rows)
    print(summary(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main())
