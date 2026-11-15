"""Unit tests for tools/fx_defaults.py."""

import fx_defaults as fd


def test_literal_keeps_identifiers_and_strips_numeric_underscores():
    assert fd.literal("default_delay_ms,") == "default_delay_ms"
    assert fd.literal("1_000.0") == 1000.0
    assert fd.literal("true") == 1.0


def test_committed_files_match_the_generator():
    assert fd.main(["--check"]) == 0


def test_delay_default_word_is_250_ms():
    rows = {(r["emu_field"], r["source"]): r for r in fd.build()}
    assert float(rows[("delay_time_ms", "chain")]["fw_value"]) == 250.0


def test_rust_table_lists_only_applied_chain_rows():
    text = fd.rust_text(fd.build())
    assert '("delay_time_ms", 250.0)' in text and "#[test]" in text
