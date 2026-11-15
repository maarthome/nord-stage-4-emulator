# NCW sample format (Kontakt export)

Checked: 2026-11-14
Sources: user-exported `.ncw` files from the Ultimate Pianos 2 library (Kontakt edit mode, `.reference/`, git-ignored); `tools/ncw.py`; `tools/tests/test_ncw.py`.
Question: how are `.ncw` samples exported from Kontakt laid out, so reference piano samples can be decoded to PCM for parity measurements?

## Layout (little-endian)

| Offset | Type | Field | Value seen |
|---|---|---|---|
| 0x00 | bytes | magic | `01 A8 9E D6` |
| 0x04 | u32 | version | 0x131 |
| 0x08 | u16 | channels | 2 |
| 0x0A | u16 | bits per sample | 16 |
| 0x0C | u32 | sample rate | 44100 |
| 0x10 | u32 | frames per channel | 661500 (15 s) |
| 0x14 | u32 | block table offset | 0x78 |
| 0x18 | u32 | first block offset (= table end) | 0x14AC |
| 0x1C | u32 | block data length | file size - 0x14AC |
| 0x78 | u32[] | block-group offsets rel. to 0x18 value, +1 end entry | |

One table entry per 512 frames, holding `channels` consecutive blocks.

Block: 16-byte header `16 0C 9A 3E`, `i32 base`, `u16 bits`, `u16 flags` (0 seen), `u32` reserved; payload = 512 signed deltas, `bits` wide, LSB-first (`ceil(512*bits/8)` bytes).

Decode: `x[i] = base + cumsum(delta)[i]` for i = 0..510. `delta[511]` is always 0; `x[511]` is the next block's `base` (the block `base` is the previous sample). Last block holds sample 510.

## Evidence (VERIFIED)

- Block sizes match `16 + ceil(512*bits/8)` for 2584 blocks of `Astoria Grand DN 021 043.ncw`; the table end entry equals the file size.
- Smoothness check (`tools/ncw.py` decode, mean |2nd difference|): inside blocks 1.55, at block boundaries 1.58 with the next-base rule, vs 72.6 without it (ch0).
- 20 random files of 8130 exported so far: boundary ≈ inside, peaks ≤ 32768, 44.1 kHz / 16 bit stereo.
- `uv run pytest tools/tests/test_ncw.py`: 8 passed.

## Not verified

- Flags != 0 (probably mid/side) and 24-bit files: not present in this export; decoder rejects flags != 0.
- Sample-exactness against Kontakt's own WAV export: not compared (no WAV reference). The smoothness test shows a consistent, continuous stream, not bit-exactness.

## Implications

The decrypted export can be converted with `uv run python tools/ncw.py <folder> .reference/UP2_wav`. Zone mapping, velocity layers and effects live in the `.nki`, not the `.ncw`.

## Follow-ups

- Parse the re-saved `.nki` (key ranges, velocity splits, envelopes, effects).
- Name pattern `<set> <note> <velocity>` (`021` = MIDI note, `043` = velocity layer upper bound?) is unconfirmed.
