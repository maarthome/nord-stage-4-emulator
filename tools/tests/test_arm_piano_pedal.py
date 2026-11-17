"""Unit tests for tools/arm_piano_pedal.py on synthetic `.npno` header bytes (no firmware)."""

from __future__ import annotations

import struct

import arm_piano_pedal as pp


def npno_with(strokes: dict[int, list[tuple[int, int, int, int]]], cls: int = 5) -> bytes:
    """Header image: `strokes[root]` = list of (kind, level offset, threshold, _unused)."""
    total = sum(len(v) for v in strokes.values())
    image = bytearray(pp.RECORDS + total * pp.RECORD_LEN)
    image[pp.CLASS_BYTE] = cls
    counts = [len(strokes.get(r, [])) for r in range(128)]
    image[pp.COUNTS : pp.COUNTS + 256] = struct.pack(">128H", *counts)
    at = pp.RECORDS
    for root in sorted(strokes):
        for kind, offset, threshold, _ in strokes[root]:
            image[at + pp.KIND_BYTE] = kind
            image[at + pp.LEVEL_FIELD : at + pp.LEVEL_FIELD + 2] = struct.pack(">h", offset)
            image[at + 0x74] = threshold
            at += pp.RECORD_LEN
    return bytes(image)


def test_parse_records_reads_kind_level_offset_and_threshold_in_root_order():
    image = npno_with({60: [(0, 56, 0, 0), (1, 53, 7, 0), (2, -31, 9, 0)], 62: [(0, 40, 3, 0)]})
    recs = pp.parse_records(image)
    assert [(r.index, r.root, r.kind) for r in recs] == [
        (0, 60, 0),
        (1, 60, 1),
        (2, 60, 2),
        (3, 62, 0),
    ]
    assert [r.level_offset for r in recs] == [56, 53, -31, 40]
    assert [r.threshold for r in recs] == [0, 7, 9, 3]


def test_kind_counts_split_one_root_by_stroke_kind():
    image = npno_with({60: [(0, 1, 0, 0), (0, 2, 1, 0), (1, 3, 2, 0), (2, 4, 3, 0)]})
    recs = pp.parse_records(image)
    assert pp.kind_counts(recs, 60) == (2, 1, 1)
    assert pp.kind_counts(recs, 61) == (0, 0, 0)


def test_extra_voice_kind_follows_the_class_and_the_damper_strokes():
    assert pp.extra_voice_kind((9, 9, 6), 5) == "damper-stroke"
    assert pp.extra_voice_kind((9, 0, 6), 5) == "normal+20"
    assert pp.extra_voice_kind((9, 9, 6), 4) == "none"
    assert pp.extra_voice_kind((3, 0, 0), 14) == "none"


def test_row_joins_the_derived_fields():
    line = pp.row("Sound.npno", 5, 60, (9, 9, 6), ["same=True"])
    assert line == "Sound.npno,5,60,9,9,6,damper-stroke,same=True"
