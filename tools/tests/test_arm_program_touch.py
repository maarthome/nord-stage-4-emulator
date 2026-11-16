"""Unit tests for tools/arm_program_touch.py (pure helpers; the firmware run stays local)."""

from __future__ import annotations

import io
import zipfile
from pathlib import Path

import arm_program_touch as pt


def test_x_of_t_matches_the_documented_levels() -> None:
    # research table: X = 74 / 66 / 57 at T = 88 / 78 / 68 (velocity 64, Heavy / Medium / Light)
    assert [pt.x_of_t(t) for t in (88, 78, 68)] == [74, 66, 57]
    assert pt.x_of_t(0) == 0
    assert pt.x_of_t(200) == 170
    assert pt.x_of_t(10_000) == pt.X_MAX


def test_curve_ids_follow_the_edit_value() -> None:
    assert [pt.expected_curve_id(v) for v in (0, 1, 2)] == [10, 11, 13]


def test_max_abs_error() -> None:
    assert pt.max_abs_error([]) == 0
    assert pt.max_abs_error([(5, 5), (3, 8), (9, 7)]) == 5


def test_parse_model_rows_skips_non_numeric_lines() -> None:
    rows = pt.parse_model_rows("noise\n64 88 74 123 456\n1 200 170\n")
    assert rows == {64: (88, 74, [123, 456]), 1: (200, 170, [])}


def test_read_member_returns_the_whole_member_or_a_prefix(tmp_path: Path) -> None:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        z.writestr("a/b.bin", b"0123456789")
    path = tmp_path / "x.zip"
    path.write_bytes(buf.getvalue())
    assert pt.read_member(path, "a/b.bin") == b"0123456789"
    assert pt.read_member(path, "a/b.bin", 4) == b"0123"
