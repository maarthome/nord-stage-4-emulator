import os
import struct
import sys
import wave

import numpy as np
import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import ncw  # noqa: E402


def _pack(deltas, bits):
    if bits == 0:
        return b""
    v = 0
    for i, d in enumerate(deltas):
        v |= (d & ((1 << bits) - 1)) << (i * bits)
    return v.to_bytes((len(deltas) * bits + 7) // 8, "little")


def _encode_block(samples, prev):
    """Encode 512 samples: base = previous sample, deltas, placeholder last delta 0."""
    deltas = []
    last = prev
    for x in samples[:511]:
        deltas.append(int(x) - last)
        last = int(x)
    deltas.append(0)
    bits = 0 if not any(deltas) else max(abs(d) for d in deltas).bit_length() + 1
    head = ncw.BLOCK_MAGIC + struct.pack("<iHHI", prev, bits, 0, 0)
    return head + _pack(deltas, bits)


def make_ncw(samples, rate=44100, bits=16):
    """Build an NCW file from an int array of shape (frames, channels)."""
    frames, chans = samples.shape
    groups = -(-frames // 512)
    pad = np.zeros((groups * 512, chans), dtype=np.int64)
    pad[:frames] = samples
    body = b""
    offsets = []
    for g in range(groups):
        offsets.append(len(body))
        for c in range(chans):
            prev = int(pad[g * 512 - 1, c]) if g else 0
            body += _encode_block(pad[g * 512 : (g + 1) * 512, c], prev)
    offsets.append(len(body))
    table = struct.pack(f"<{len(offsets)}I", *offsets)
    table_off = ncw.HEADER_SIZE
    data_start = table_off + len(table)
    head = ncw.NCW_MAGIC + struct.pack(
        "<IHHIIIII", 0x131, chans, bits, rate, frames, table_off, data_start, len(body)
    )
    head += b"\x00" * (ncw.HEADER_SIZE - len(head))
    return head + table + body


def test_stereo_roundtrip_across_block_boundaries():
    rng = np.random.default_rng(1)
    t = np.arange(1300)
    x = np.stack(
        [
            (8000 * np.sin(t / 20)).astype(np.int64) + rng.integers(-3, 4, t.size),
            (-5000 * np.sin(t / 7)).astype(np.int64),
        ],
        axis=1,
    )
    out = ncw.decode(make_ncw(x))
    assert out.sample_rate == 44100 and out.bits == 16
    assert out.samples.shape == (1300, 2)
    np.testing.assert_array_equal(out.samples, x)


def test_last_sample_of_block_comes_from_next_block_base():
    x = np.arange(1024).reshape(-1, 1) * 7 + 3
    x = np.vstack([x, [[x[-1, 0] + 7]] * 100])  # pad so the final block is not the last sample
    out = ncw.decode(make_ncw(x))
    assert out.samples[511, 0] == x[511, 0]
    np.testing.assert_array_equal(out.samples, x)


def test_mono_and_silent_block_with_zero_bit_width():
    x = np.zeros((512, 1), dtype=np.int64)
    x[:, 0] = 1234  # constant: every delta zero -> bits == 0
    out = ncw.decode(make_ncw(x))
    np.testing.assert_array_equal(out.samples, x)


def test_bit_unpack_sign_extension():
    d = [-4, 3, -1, 0, 2, -3] + [0] * 506
    np.testing.assert_array_equal(ncw._unpack_deltas(_pack(d, 3), 3), d)


def test_payload_that_contains_block_magic_does_not_break_group_walk():
    # 32-bit-wide deltas let us embed the magic bytes in the payload of block 0.
    magic = int.from_bytes(ncw.BLOCK_MAGIC, "little", signed=True)
    x = np.zeros((512, 2), dtype=np.int64)
    x[5, 0] = magic // 2
    out = ncw.decode(make_ncw(x))
    np.testing.assert_array_equal(out.samples, x)


def test_bad_magic_and_truncation_raise():
    with pytest.raises(ncw.NcwError):
        ncw.decode(b"nope" * 64)
    good = make_ncw(np.zeros((512, 2), dtype=np.int64) + 5)
    with pytest.raises(ncw.NcwError):
        ncw.decode(good[: ncw.HEADER_SIZE + 4])


def test_nonzero_flags_rejected():
    blob = bytearray(make_ncw(np.zeros((512, 1), dtype=np.int64)))
    table_len = 4 * 2
    first = ncw.HEADER_SIZE + table_len
    struct.pack_into("<H", blob, first + 10, 1)
    with pytest.raises(ncw.NcwError):
        ncw.decode(bytes(blob))


def test_write_wav_and_cli_roundtrip(tmp_path):
    x = (np.arange(600).reshape(-1, 1) % 200 - 100).astype(np.int64)
    src = tmp_path / "a.ncw"
    src.write_bytes(make_ncw(np.hstack([x, -x])))
    dst = tmp_path / "a.wav"
    assert ncw.main([str(src), str(dst)]) == 0
    with wave.open(str(dst)) as w:
        assert (w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()) == (
            2,
            2,
            44100,
            600,
        )
        pcm = np.frombuffer(w.readframes(600), dtype="<i2").reshape(-1, 2)
    np.testing.assert_array_equal(pcm[:, 0], x[:, 0])
    out_dir = tmp_path / "wavs"
    assert ncw.main([str(tmp_path), str(out_dir)]) == 0
    assert (out_dir / "a.wav").exists()
