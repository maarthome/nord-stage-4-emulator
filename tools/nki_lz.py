#!/usr/bin/env python3
"""Decompress the preset stream of a Kontakt ``.nki`` (unprotected, Kontakt 8 era).

The preset body is one LZ77 stream. Token layout, from the ``ni-file`` project's
notes (github.com/gitter-badger/ni-file) plus two extensions found on a Kontakt
8.4 file (see docs/research/nki-lz-stream.md).

Control byte ``c`` with ``q = c & 0x1F``::

    c < 0x20            literal run of 1+q bytes follow
    0x20..0xDF          copy; length 3..8 by the top 3 bits (0x20->3 ... 0xC0->8),
                        offset = (q << 8) + next byte + 1
    0xE0..0xFF          copy; length = 9 + sum of extension bytes (a byte of 0xFF
                        means another extension byte follows, LZ4 style),
                        offset = (q << 8) + next byte + 1

If the offset computed above is exactly 8192, two more bytes (big endian) follow
and the real offset is ``8192 + that value``. Copies may overlap (``length > offset``
repeats the window).

VERIFIED: decoding ``Ultimate Pianos 2.1.1 Yuri Total.nki`` (1003776 bytes) from offset
2490 up to the footer ``hsin`` block consumes the stream exactly to its end, never
references before the start, and yields a 180766 byte KSP script (clean ASCII, 1 run)
followed by instrument tables.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

LIT_MAX = 0x1F
LONG_OFFSET = 8192
SCRIPT_MARKER = b"on init"


class NkiLzError(ValueError):
    """Raised for a stream that cannot be decoded."""


def copy_length_class(c: int) -> int:
    """Return 1 for literal runs, 3..8 for short copies, 9 for extended copies."""
    if c <= LIT_MAX:
        return 1
    if c >= 0xE0:
        return 9
    return 3 + ((c - 0x20) >> 5)


def decompress(data: bytes, start: int = 0, end: int | None = None) -> bytes:
    """Decode the LZ stream in ``data[start:end]``; it must end exactly at ``end``."""
    out = bytearray()
    i = start
    n = len(data) if end is None else end
    try:
        while i < n:
            c = data[i]
            i += 1
            q = c & LIT_MAX
            kind = copy_length_class(c)
            if kind == 1:
                length = 1 + q
                chunk = data[i : min(i + length, n)]
                if len(chunk) < length:
                    raise NkiLzError(f"literal run overruns input at {i}")
                out += chunk
                i += length
                continue
            if kind < 9:
                length = kind
            else:
                length = 9
                while True:
                    r = data[i]
                    i += 1
                    length += r
                    if r != 0xFF:
                        break
            offset = (q << 8) + data[i] + 1
            i += 1
            if offset == LONG_OFFSET:
                offset += (data[i] << 8) | data[i + 1]
                i += 2
            if offset > len(out):
                raise NkiLzError(f"copy offset {offset} before stream start at {i}")
            pos = len(out) - offset
            if length <= offset:
                out += out[pos : pos + length]
            else:
                for k in range(length):
                    out.append(out[pos + (k % offset)])
    except IndexError as e:
        raise NkiLzError(f"stream truncated near {i}") from e
    if i != n:
        raise NkiLzError(f"stream ended at {i}, expected {n}")
    return bytes(out)


def find_stream_start(data: bytes) -> int:
    """Offset of the control byte of the literal run that holds the first script text."""
    pos = data.find(SCRIPT_MARKER)
    if pos < 1:
        raise NkiLzError("no 'on init' script marker; pass an explicit start")
    return pos - 1


def find_stream_end(data: bytes, start: int) -> int:
    """Start of the trailing ``hsin`` footer block (8-byte length precedes its tag)."""
    pos = data.rfind(b"hsin")
    if pos - 8 <= start:
        return len(data)
    return pos - 8


def script_length(decoded: bytes) -> int:
    """Length of the leading run of printable ASCII (the KSP script)."""
    n = 0
    for b in decoded:
        if not (32 <= b < 127 or b in (9, 10, 13)):
            break
        n += 1
    return n


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Decompress a Kontakt .nki preset stream.")
    ap.add_argument("nki", type=Path)
    ap.add_argument("out_bin", type=Path, help="decoded stream")
    ap.add_argument("--script", type=Path, help="also write the leading KSP script here")
    ap.add_argument("--start", type=lambda s: int(s, 0), help="stream offset (default: auto)")
    a = ap.parse_args(argv)
    data = a.nki.read_bytes()
    start = a.start if a.start is not None else find_stream_start(data)
    out = decompress(data, start, find_stream_end(data, start))
    a.out_bin.write_bytes(out)
    n = script_length(out)
    if a.script:
        a.script.write_bytes(out[:n])
    print(f"stream at {start}: {len(out)} bytes decoded, script {n} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
