#!/usr/bin/env python3
"""Decode Native Instruments Kontakt NCW sample files to PCM / WAV.

Layout (reverse engineered from an exported Ultimate Pianos 2 library, see
docs/research/ncw-format.md). All integers little-endian.

Header (0x78 bytes, only the fields used here)::

    0x00 u32  magic, bytes 01 A8 9E D6
    0x04 u32  version (0x131 seen)
    0x08 u16  channels
    0x0A u16  bits per sample
    0x0C u32  sample rate
    0x10 u32  frames per channel
    0x14 u32  block-table offset (0x78)
    0x18 u32  block-table length in bytes = offset of the first block
    0x1C u32  block-data length

Block table: u32 offsets relative to ``data_start`` (header 0x18), one per
group of ``channels`` blocks, plus a final end offset.

Block: 16 byte header + packed payload, 512 frames per channel::

    u32 magic 0x3E9A0C16 (bytes 16 0C 9A 3E)
    i32 base    value of the sample just before the block (0 for the first block)
    u16 bits    width of each packed delta (0 = all deltas zero)
    u16 flags   0 seen; non-zero (mid/side) is rejected
    u32 reserved
    payload     512 two's-complement deltas, ``bits`` wide, LSB first

Sample ``i`` (0..510) of a block is ``base + sum(delta[0..i])``. ``delta[511]`` is
always 0 (placeholder); sample 511 is the ``base`` of the next block of the same
channel. Without a next block (end of file) sample 511 holds sample 510.
Checked on 2584 blocks of Astoria Grand DN 021 043: mean |2nd difference| at block
boundaries 1.58 vs 1.55 inside blocks.
"""

from __future__ import annotations

import argparse
import struct
import sys
import wave
from dataclasses import dataclass
from pathlib import Path

import numpy as np

NCW_MAGIC = b"\x01\xa8\x9e\xd6"
BLOCK_MAGIC = b"\x16\x0c\x9a\x3e"
BLOCK_FRAMES = 512
BLOCK_HEADER = 16
HEADER_SIZE = 0x78


class NcwError(ValueError):
    """Raised for malformed or unsupported NCW data."""


@dataclass
class Ncw:
    """Decoded NCW file: ``samples`` has shape (frames, channels), int32."""

    sample_rate: int
    bits: int
    samples: np.ndarray


def _unpack_deltas(payload: bytes, bits: int) -> np.ndarray:
    """Unpack ``BLOCK_FRAMES`` signed ``bits``-wide values, LSB first."""
    if bits == 0:
        return np.zeros(BLOCK_FRAMES, dtype=np.int64)
    need = (BLOCK_FRAMES * bits + 7) // 8
    if len(payload) < need:
        raise NcwError(f"block payload {len(payload)} bytes, need {need} for {bits} bits")
    raw = np.frombuffer(payload[:need], dtype=np.uint8)
    bitstream = np.unpackbits(raw, bitorder="little")[: BLOCK_FRAMES * bits]
    groups = bitstream.reshape(BLOCK_FRAMES, bits).astype(np.int64)
    values = (groups << np.arange(bits, dtype=np.int64)).sum(axis=1)
    sign = 1 << (bits - 1)
    return np.where(values & sign, values - (1 << bits), values)


def _read_block(data: bytes, start: int, end: int) -> tuple[int, np.ndarray]:
    """Return (base, deltas) of one block."""
    if data[start : start + 4] != BLOCK_MAGIC:
        raise NcwError(f"bad block magic at {start}")
    base, bits, flags = struct.unpack_from("<iHH", data, start + 4)
    if flags != 0:
        raise NcwError(f"unsupported block flags {flags:#x} at {start}")
    return base, _unpack_deltas(data[start + BLOCK_HEADER : end], bits)


def decode(data: bytes) -> Ncw:
    """Decode an in-memory NCW file."""
    if len(data) < HEADER_SIZE or data[:4] != NCW_MAGIC:
        raise NcwError("not an NCW file (bad magic)")
    channels, bits, rate, frames, table_off, data_start = struct.unpack_from("<HHIIII", data, 8)
    if channels < 1:
        raise NcwError("zero channels")
    groups = -(-frames // BLOCK_FRAMES)
    need = table_off + 4 * (groups + 1)
    if len(data) < need or data_start < need:
        raise NcwError("truncated block table")
    table = struct.unpack_from(f"<{groups + 1}I", data, table_off)
    out = np.zeros((groups * BLOCK_FRAMES, channels), dtype=np.int64)
    bases = np.zeros((groups + 1, channels), dtype=np.int64)
    for g in range(groups):
        start = data_start + table[g]
        group_end = data_start + table[g + 1]
        starts = _block_starts(data, start, group_end, channels)
        for c, s in enumerate(starts):
            e = starts[c + 1] if c + 1 < channels else group_end
            base, deltas = _read_block(data, s, e)
            bases[g, c] = base
            out[g * BLOCK_FRAMES : (g + 1) * BLOCK_FRAMES, c] = base + np.cumsum(deltas)
    for g in range(groups - 1):
        out[(g + 1) * BLOCK_FRAMES - 1] = bases[g + 1]
    return Ncw(rate, bits, out[:frames].astype(np.int32))


def _block_starts(data: bytes, start: int, end: int, channels: int) -> list[int]:
    """Offsets of the ``channels`` blocks packed in ``data[start:end]``.

    Each block's size follows from its ``bits`` field, so payload bytes that
    happen to look like the block magic cannot confuse the walk.
    """
    starts = [start]
    pos = start
    while len(starts) < channels:
        (bits,) = struct.unpack_from("<H", data, pos + 8)
        pos += BLOCK_HEADER + (BLOCK_FRAMES * bits + 7) // 8
        if pos >= end:
            raise NcwError(f"expected {channels} blocks in group at {start}")
        starts.append(pos)
    return starts


def write_wav(path: Path, ncw: Ncw) -> None:
    """Write PCM WAV (16 or 24 bit, from the NCW header)."""
    width = {16: 2, 24: 3}.get(ncw.bits)
    if width is None:
        raise NcwError(f"unsupported bit depth {ncw.bits}")
    s = np.clip(ncw.samples, -(1 << (ncw.bits - 1)), (1 << (ncw.bits - 1)) - 1)
    le = s.astype("<i4").tobytes()
    raw = np.frombuffer(le, dtype=np.uint8).reshape(-1, 4)[:, :width].tobytes()
    with wave.open(str(path), "wb") as w:
        w.setnchannels(ncw.samples.shape[1])
        w.setsampwidth(width)
        w.setframerate(ncw.sample_rate)
        w.writeframes(raw)


def convert(src: Path, dst: Path) -> Ncw:
    ncw = decode(src.read_bytes())
    write_wav(dst, ncw)
    return ncw


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Convert NCW files (file or folder) to WAV.")
    ap.add_argument("src", type=Path)
    ap.add_argument("dst", type=Path, help="output .wav (file input) or folder (folder input)")
    args = ap.parse_args(argv)
    if args.src.is_dir():
        args.dst.mkdir(parents=True, exist_ok=True)
        files = sorted(args.src.glob("*.ncw"))
        for f in files:
            convert(f, args.dst / (f.stem + ".wav"))
        print(f"converted {len(files)} files")
    else:
        n = convert(args.src, args.dst)
        print(f"{n.samples.shape[0]} frames, {n.samples.shape[1]} ch, {n.sample_rate} Hz")
    return 0


if __name__ == "__main__":
    sys.exit(main())
