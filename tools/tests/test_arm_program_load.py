"""Unit tests for tools/arm_program_load.py (record patching and the loader rule; no firmware)."""

from __future__ import annotations

import struct
import zlib

import arm_i2c as ai
import arm_program_load as apl
import arm_program_routing as apr
import arm_store as st
import pytest
from test_arm_program_routing import put


def program(section_i: int = 0, org_mask: int = 0, pno_mask: int = 0, syn_mask: int = 0) -> bytes:
    """A 868-byte `.ns4p` with scene I section enables and layer enables."""
    payload = bytearray(820)
    put(payload, apr.SECTION_ENABLE_I, 3, section_i)
    put(payload, apr.ORGAN * 8 + 9, 2, org_mask)
    put(payload, apr.PIANO * 8 + 9, 2, pno_mask)
    put(payload, apr.SYNTH * 8 + 10, 3, syn_mask)
    return bytes(48) + bytes(payload)


def blank_image() -> bytes:
    """Four EEPROMs; device 0 holds one record of the default program."""
    stream = struct.pack("<II", apl.STREAM_SIZE, 0) + b"Unnamed".ljust(24, b"\0")
    stream += b"p4sn" + bytes(4) + struct.pack("<I", 11) + bytes(apl.BODY_SIZE)
    first = bytearray(b"\xff" * ai.EEPROM_SIZE)
    pages = st.pack_clusters(stream)
    first[st.CLUSTER_BASE : st.CLUSTER_BASE + len(pages)] = pages
    return st.join_image([bytes(first)] + [b"\xff" * ai.EEPROM_SIZE] * (ai.EEPROM_COUNT - 1))


def test_body_is_the_file_from_byte_0x2c() -> None:
    data = program()
    assert apl.program_body(data) == data[0x2C:]
    assert len(apl.program_body(data)) == apl.BODY_SIZE


def test_body_of_a_short_file_is_refused() -> None:
    with pytest.raises(ValueError):
        apl.program_body(bytes(100))


def test_patch_stream_sets_size_crc_name_and_body() -> None:
    data = program(0b100, 0b01)
    stream = apl.patch_stream(bytes(st.BODY_OFFSET + apl.BODY_SIZE), data, "Vox Like Me")
    rec = st.unpack_record(stream)
    assert rec["name"] == "Vox Like Me"
    assert rec["size"] == apl.STREAM_SIZE
    assert rec["crc"] == zlib.crc32(data[0x2C:])
    assert rec["crc_ok"] is True


def test_patch_stream_refuses_a_long_name() -> None:
    with pytest.raises(ValueError):
        apl.patch_stream(bytes(900), program(), "x" * 24)


def test_build_image_replaces_the_stored_record() -> None:
    data = program(0b010, 0, 0b10)
    image = apl.build_image(blank_image(), data, "Piano B")
    first = st.split_image(image)[0]
    rec = st.unpack_record(st.unpack_clusters(first))
    assert rec["name"] == "Piano B" and rec["crc_ok"] is True
    assert len(image) == ai.EEPROM_SIZE * ai.EEPROM_COUNT
    assert st.unpack_clusters(first)[st.BODY_OFFSET :][: apl.BODY_SIZE] == data[0x2C:]


def test_view_mask_reads_zero_as_layer_a() -> None:
    assert apl.view_mask(0) == 1
    assert apl.view_mask(0b10) == 0b10
    assert apl.view_mask(0b111) == 0b111


def test_expected_view_defaults_an_enabled_empty_section_to_layer_a() -> None:
    # `Vox Like Me`: organ section on, layer enable 0; `Tines Layer`: piano on with mask 0.
    views = apl.expected_view(program(0b100)[48:])
    assert views["organ"] == (1, 1)
    assert views["piano"] == (0, 1)
    views = apl.expected_view(program(0b011, 0, 0, 0b011)[48:])
    assert views["piano"] == (1, 1) and views["synth"] == (1, 0b011)


def words_for(views: dict[str, tuple[int, int]]) -> dict[str, list[int]]:
    return {name: [e, m, 0, e, 0, 0, 0, 0, 0] for name, (e, m) in views.items()}


def test_compare_accepts_what_the_loader_gives() -> None:
    data = program(0b100)
    words = words_for(apl.expected_view(data[48:]))
    assert apl.compare(data, words, [1, 0, 0, 0, 0, 0, 0]) == []


def test_compare_reports_a_wrong_view_and_wrong_zones() -> None:
    data = program(0b100)
    words = words_for(apl.expected_view(data[48:]))
    words["organ"][1] = 0  # a loader that kept the stored mask 0
    problems = apl.compare(data, words, [0, 0, 0, 0, 0, 0, 0])
    assert any("organ view" in p for p in problems)
    assert any("organ zones" in p for p in problems)


def test_compare_checks_piano_and_synth_zone_slices() -> None:
    data = program(0b011, 0, 0b01, 0b110)
    words = words_for(apl.expected_view(data[48:]))
    assert apl.compare(data, words, [0, 0, 1, 0, 0, 1, 1]) == []
    assert apl.compare(data, words, [0, 0, 1, 0, 1, 1, 1]) != []


def test_section_words_reads_nine_words_per_section() -> None:
    mem = {0x1000 + 0x288 + 4 * k: k for k in range(3 * 10)}
    words = apl.section_words(lambda a: mem.get(a, 0), 0x1000)
    assert words["organ"] == list(range(9))
    assert words["piano"][0] == 10  # next entry starts 0x28 bytes (10 words) later


def test_expected_view_of_scene_two_uses_the_scene_two_enables() -> None:
    payload = bytearray(820)
    put(payload, apr.SECTION_ENABLE_II, 3, 0b001)
    put(payload, apr.SYNTH * 8 + 15, 3, 0b100)
    views = apl.expected_view(bytes(payload), 1)
    assert views["synth"] == (1, 0b100)
    assert views["organ"] == (0, 1)
