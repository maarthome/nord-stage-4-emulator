#!/usr/bin/env python3
"""Run the firmware's own program load for a `.ns4p` and read the section table it fills (OQ25).

`arm_program_routing.py` writes `(enable, layer mask, focus)` into the section table by hand. This
tool does not: it puts the program into the Store record of the I2C EEPROM image (`arm_store.py`
layout, location `A:11`), boots the full machine with that image (`arm_screens.Session`) and lets
the firmware recall the program itself. The recalled program fills the section table
`[[0xC055B144]] + 0x288` (three sections of `0x28` bytes: view `(enable, mask, focus)` at `+0`,
scene A at `+0xC`, scene B at `+0x18`) and the layer enable handlers write the zone bytes.

Findings (`docs/research/engine-routing.md` section 7):

* the Store record body is the program file from byte `0x2C` (the last 4 header bytes plus the
  820-byte payload), 824 bytes, CRC-32 over the body, name outside the CRC;
* the view refresh `0xC00F8A50` stores `mask = scene mask`, but `mask = 1` when the scene mask is 0,
  so an enabled section whose stored layer enable is 0 plays layer A.

    C=target/fw_unpacked
    python tools/arm_program_load.py $C/bank0_CODE.bin $C/bank0_SRAM.bin \
        --eeprom target/oled/eeprom.bin "factory_programs/Program/Bank A/Vox Like Me.ns4p"

`--eeprom` is the image of `arm_store.py store` (the default program stored once; derived data under
`target/`, never committed). A boot takes about 4 minutes of CPU; run programs in parallel shells.
Needs `unicorn` and the unpacked firmware.
"""

from __future__ import annotations

import argparse
import struct
import sys
import zlib
from pathlib import Path

import arm_program_routing as apr
import arm_store as st

# VERIFIED (run, 2026-11-14): the Store record body is `file[0x2C:]` of the `.ns4p` (the last 4
# bytes of the 48-byte CBIN header, then the 820-byte payload), 824 bytes; the stream is
# `size u32 (860)`, `crc32 u32` of the body, name[24], `p4sn`, 4 zeros, location u32, body.
BODY_START = 0x2C
BODY_SIZE = 824
STREAM_SIZE = 860
SETUP_GLOBAL = 0xC055B144
SECTION_VIEW = 0x288
SECTION_STRIDE = 0x28
SECTION_NAMES = ("organ", "piano", "synth")
# Offsets inside a section entry (words): view, scene A, scene B.
VIEW, SCENE_A, SCENE_B = 0, 3, 6


def program_body(program: bytes) -> bytes:
    """The 824-byte Store record body of a `.ns4p` file."""
    body = program[BODY_START:]
    if len(body) != BODY_SIZE:
        raise ValueError(f"program file of {len(program)} bytes, body {len(body)} != {BODY_SIZE}")
    return body


def patch_stream(stream: bytes, program: bytes, name: str) -> bytes:
    """`stream` (a stored record) with the body, CRC and name of `program`."""
    raw = name.encode("latin-1")
    if len(raw) >= st.NAME_BYTES:
        raise ValueError(f"name longer than {st.NAME_BYTES - 1} bytes")
    body = program_body(program)
    out = bytearray(stream)
    struct.pack_into("<II", out, 0, STREAM_SIZE, zlib.crc32(body))
    out[st.NAME_OFFSET : st.NAME_OFFSET + st.NAME_BYTES] = raw.ljust(st.NAME_BYTES, b"\0")
    out[st.BODY_OFFSET : st.BODY_OFFSET + BODY_SIZE] = body
    return bytes(out)


def build_image(blob: bytes, program: bytes, name: str) -> bytes:
    """The 256 KB EEPROM image `blob` with its stored record replaced by `program`."""
    images = [bytearray(i) for i in st.split_image(blob)]
    stream = st.unpack_clusters(bytes(images[0]))
    pages = st.pack_clusters(patch_stream(stream, program, name))
    images[0][st.CLUSTER_BASE : st.CLUSTER_BASE + len(pages)] = pages
    return st.join_image([bytes(i) for i in images])


def view_mask(scene_mask: int) -> int:
    """View refresh `0xC00F8A50`: `cmp mask, #0; moveq mask, #1` (VERIFIED, disassembly, run)."""
    return scene_mask or 1


def expected_view(payload: bytes, scene: int = 0) -> dict[str, tuple[int, int]]:
    """`{section: (enable, view mask)}` the view refresh must give for scene 0 (I) or 1 (II)."""
    views = apr.scene_views(payload)[scene]
    return {name: (enable, view_mask(mask)) for name, (enable, mask) in views.items()}


def zone_expected(enable: int, mask: int, layers: int) -> list[int]:
    """Zone bytes of the first `layers` layers of a section from its view."""
    return apr.expected_zone_bytes(enable, mask, layers)


def section_words(read32, setup: int) -> dict[str, list[int]]:  # noqa: ANN001 - emulator accessor
    """The 9 words (`view`, scene A, scene B) of every section entry."""
    return {
        name: [read32(setup + SECTION_VIEW + SECTION_STRIDE * i + 4 * k) for k in range(9)]
        for i, name in enumerate(SECTION_NAMES)
    }


def compare(program: bytes, words: dict[str, list[int]], zones: list[int]) -> list[str]:
    """Differences between the decode rule and what the firmware loaded (empty = all equal)."""
    payload = program[apr.HEADER : apr.HEADER + 820]
    problems = []
    first = 0
    for name, (enable, mask) in expected_view(payload).items():
        got = words[name]
        if (got[VIEW], got[VIEW + 1]) != (enable, mask):
            problems.append(f"{name} view {got[VIEW]},{got[VIEW + 1]} want {enable},{mask}")
        layers = len(apr.LAYERS[name])
        want = zone_expected(enable, mask, layers)
        if zones[first : first + layers] != want:
            problems.append(f"{name} zones {zones[first : first + layers]} want {want}")
        first += layers if name != "organ" else 2
    return problems


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the local firmware
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path)
    ap.add_argument("--eeprom", type=Path, required=True)
    ap.add_argument("program", type=Path)
    ap.add_argument("--ticks", type=int, default=300)
    ns = ap.parse_args(argv)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import arm_screens as sc
    import arm_zones as az

    program = ns.program.read_bytes()
    session = sc.Session(ns.image, ns.sram)
    session.load_eeproms(build_image(ns.eeprom.read_bytes(), program, ns.program.stem[:23]))
    session.boot()
    session.run(ns.ticks)
    emu = session.emu
    words = section_words(emu.read32, az.setup_object(emu))
    zones = [int(az.zone_enabled(emu, b)) for b in az.layer_bases(emu)[:7]]
    print(f"{ns.program.stem}: title {[t.text for t in session.texts][-1:]}")
    for name, w in words.items():
        print(f"  {name:5s} view {w[0:3]} scene A {w[3:6]} scene B {w[6:9]}")
    print(f"  zone bytes (organ A B, piano A B, synth A B C): {zones}")
    problems = compare(program, words, zones)
    for p in problems:
        print("  MISMATCH", p)
    return int(bool(problems))


if __name__ == "__main__":
    sys.exit(main())
