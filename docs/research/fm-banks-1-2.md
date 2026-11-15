# FM banks 1 and 2 (stages `0x124269` / `0x12453F`): the data is readable

- **Checked:** 2026-11-15
- **Sources (local, never committed):** OS v1.66 `bank0_CODE.bin` / `bank0_SRAM.bin`, the ORG core 2 image (`python tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin --sram DIR`, listing `sharc-disasm DIR/core2_sram.bin 0x2484D2 0x248F6C`), the boot capture `target/bulk/org_core2.script` ([arm-bulk-tables](arm-bulk-tables.md)), `sharc-org` (`--watch`, `--rwatch`), the ARM harness (`tools/synth_p_writer.py` apply call, `tools/synth_fm_tables.py`). Builds on [synth-stage-models-13](synth-stage-models-13.md) s1 and [synth-wave-categories](synth-wave-categories.md).
- **Question:** can the data of the FM oscillator banks (operator or wave tables, ratios) be read from the firmware images the harness already uses, without new decryption, so the stages can be ported and checked against the SHARC oracle?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. Short answer

**The data is readable: the ARM generates it.** The earlier reading (zero table, writer not found, first version of this note) came from looking at the wrong paths. The strip apply `0xC01CE7D8(strip)` with a pending type 49..53 sends **361 float words to `DM(0xE0900 .. 0xE0A68)`** of ORG core 2 (also PNO core 2 and ORG core 1: the same chip messages), and nothing else for types 0..48 and 54..73. No decryption, no firmware image or sample is involved.

| Item | Result | Tag |
|---|---|---|
| Stage code `0x124269` / `0x12453F` | Readable: 598 listing lines `0x2484D2..0x248F6C` in the ORG core 2 image; the second stage starts at `0x248A7E` | VERIFIED (listing) |
| What `DM(0xC15A8) = 0xE0926` is | Not 129-point wave rows. The stage forms `I5 = 0xE0926 + FEXT(R0 BY 25:7)` and reads `DM(I5, 0x81)` three times (stride 129), so the index (a signed 7-bit field, -64..63 around the base) picks one word from each of three **band curves** at `+0`, `+129`, `+258`; they are lerped (`F1 = F1 - F0`, `F1 * F5`) and used as per-oscillator gains / fades. 361 words cover `0xE0900..0xE0A68` | VERIFIED (listing, window of the writes) |
| Writer | ARM only. Boot, DSP passes and the wave edit path (101 strip messages, as in the first version of this note) send nothing; the strip apply sends them (the earlier edit probes never ran the apply for a type 49..53; `tools/synth_p_writer.py` filtered the P block window and dropped these words). The sender is a table-driven bulk send (`0xC01CF358` loop, `0xC01CF644` port store, 192 stores in the loop plus 3 from `0xC01CF9CC`). The values are computed, not rodata (the first three words of each segment occur nowhere in `bank0_CODE`) | VERIFIED (run) |
| Dependence | On the type and on strip `+0xA4` / `+0xA8` (s6; this table is the default `+0xA4 = 0`, `+0xA8 = 0x80000`). `a`, `b` (pending wave variant words) do not change a word (4 variants x 5 types, `tools/synth_fm_tables.py capture` stops on a difference). Types 50, 51, 52, 53 and 49 give 5 tables; segments: ramp `0xE0900 + k` (70 words, `(70 - k) / n`, n = 240 (49), 30 (50, 51, 53), 51 (52), within 2e-6 relative, a float recurrence), zeros, a decay curve at `0xE0967..` (about `A 2^(-k/6) - c`, ratio 0.8909 per step, clipped at zero; A = 7822 for 49 and 53, 3911 for 50 to 52), zeros, a second decay curve at `0xE09E8..` (A = 62582 for 49 and 53, 4651 / 4544 / 14310 for 50 / 51 / 52) | VERIFIED (capture), formula shapes EMPIRICAL |
| Committed data | [data/synth-fm-tables.csv](data/synth-fm-tables.csv): 5 types x 361 words (1,805 derived numbers, about 40 KB; D56 sizing, no firmware bytes), fingerprint `13a83db262affa8a` in `tools/tests/test_synth_fm_tables.py` | VERIFIED (test) |
| Partial ratios | The FM-H partial double table at `bank0_CODE@0x2D82A8` reads `0.5, 1, 2, 3, 4, 5, 6, 7 ...` | VERIFIED (run) |

## 2. Reproduce

```bash
uv run --with unicorn --with capstone python tools/synth_fm_tables.py capture     target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin     --cache target/settled.snap --out docs/research/data/synth-fm-tables.csv
python -m pytest tools/tests/test_synth_fm_tables.py
```

Disassembly: `python tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin --sram DIR` then `sharc-disasm DIR/core2_sram.bin 0x2484D2 0x248F6C`.

## 3. Stage models (added 2026-11-15)

| Step | Result | Tag |
|---|---|---|
| Listing interpreter `tools/sharc_text_vm.py` (SIMD move / exchange rules, `BDCST1` broadcast loads through `I1`) | equal to `sharc-org` on 620 of 620 (stage 1, types 49 and 53) and 520 of 520 (stage 2, types 50 to 52) written words, 10 trials each (`tools/synth_fm_oracle.py`) | VERIFIED (run) |
| Python equation models `tools/synth_stage_fm.py` | equal to the interpreter on 1240 of 1240 (stage 1) and 1040 of 1040 (stage 2) words, 20 random trials x 5 types (`synth_fm_oracle.py --model`), plus random-word cases of the goldens | VERIFIED (run) |
| Rust `synth/fw_fm/` (f32) | 20 goldens (`tools/synth_fm_golden.py`) within 1e-3, measured 6e-7; no allocation per call; not reachable from the default path (D22) | tolerance VERIFIED |

Findings of the decode: stage 1 chains A -> B and C -> D (four samples per operator, two passes at `a1`, `a2`), stage 2 chains A -> B -> C; the sine is the verified helper `0x12522E`; the band curves are lerped with `frac = (W & 0x1FFFFFF) 2^-25` (`FDEP R0 BY 6:25` deposits 25 bits at bit 6, not 6 bits at bit 25); loads through `I1` broadcast the PEx word (this fixed 70 percent of the first interpreter mismatches); lane 1 of the curve 2 lerp runs reversed because `R3 = S2` reads the other lane.

## 4. Implications

- The "table not reachable" blocker is closed. What remains for a port is the stage maths itself (two SIMD stages of 290 and 300 lines: four phase accumulators per lane, eight sine polynomials of `abs(phase) + c`, the band-curve lerps), the P words of the FM wave types (already in [data/synth-p-words.csv](data/synth-p-words.csv), types 49..53) and the ARM writer of the per-wave ratio (`0x2D82A8` table).
- Open (unchanged by the models): the curve index source (which register carries the pitch / note term into `FEXT R0 BY 25:7`), whether the table depends on strip state other than the type (only `a`, `b` were varied), and the analytic generator (values are float recurrences; the CSV is the source).

## 5. Follow-ups

1. Decode the writers of the position word `W = DM(I2-31)`, the pitch term `DM(I2-29)`, the ARM block `DM(0xC1599 ..)` and the P words of types 49..53, then wire `fw_fm` to an FM voice (opt-in first, D22).
2. Vary strip state other than `a`, `b` in the table capture (the models take the table as input, so this only changes the CSV).
3. Validate against a hardware FM clip before any default change.

## 6. Strip state beyond the type, and the voice wiring (added 2026-11-15, D96)

An outside review noted that section 1 said "type only" while only `a` and `b` were varied. Probe `python tools/synth_fm_tables.py probe CODE SRAM --cache SNAP` (needs the settled harness): it pokes each of the 38 strip offsets the apply `0xC01CE7D8` reads (five values each: 0, 1, `1.0f`, `2.0f`, `0x7FFFFFFF`) for types 49 and 52 and compares the 361 words.

| Result | Tag |
|---|---|
| Only strip `+0x00`, `+0xA4` and `+0xA8` change a word; the other 35 offsets (including the pending words `+0x98`, `+0x9C`, `+0xA0`) do not | VERIFIED (run, types 49 and 52) |
| `+0xA4` (boolean mode flag, default 0): any nonzero value (1, 2, 3, 4, 8 tried, type 49) gives the same table; the ramp and the decay curve grow by x10.08 (first word 0.29167 -> 2.93982, `A` 7822.35 -> 78848.1), the segment at `+300` (23.68) and the zero tail stay | VERIFIED (run) |
| `+0xA8` (scale, default `0x80000`), `r = a8 / 2^19`: the ramp is `(70 - k) / n` with `n = 240 r` (type 49) and `n = 27 + 24 r` (type 52), the decay curve amplitude `A` goes as `1 / r` (type 49: 31290.6, 15645.1, 7822.35, 3911.0, 2607.2, 1955.3, 1303.4 at `a8` = 0x20000, 0x40000, 0x80000, 0x100000, 0x180000, 0x200000, 0x300000; type 52: 15645.1 .. 651.5). Only the ramp and decay segments move, the rest of the 361 words does not (7 values per type) | VERIFIED (run) |
| The apply loads `+0xA4`, `+0xA8` at `0xC01CB1A0 / 0xC01CB1A4`, `0xC01CFE78 / 0xC01CFE7C` and `0xC01CF5E4 / 0xC01CF5EC`; the first generator converts `a8` with `float(a8) * 1.2014794e-7 * 127` (literals `0x34010204`, `0x42FE0000` at `0xC01CBEC0 / 0xC01CBEC4`) = `8 a8 / 2^19`, which is 8.0 at the default | VERIFIED (listing) |
| No Edt_Syn edit id 1..0x15F (values 1 and 7, write watch on `+0xA4..+0xAB`) writes them (351 ids, 0 hits), so they come from the strip constructor or the wave select path | VERIFIED (run, negative) |
| Which ARM code writes `+0xA4` / `+0xA8` (the wave id to partial ratio path, a mode switch of the FM bank), and the closed form of all 361 words as a function of `(type, a4, a8)` | OPEN (OQ22) |

The words the stage needs besides the table, all located:

| Stage word | Source | Tag |
|---|---|---|
| `DM(I2-31)` position word `W` | the clipped sum of the pitch integer words, `0x01000000` per semitone, zero at C3 (MIDI 48): pitch stage listing `0x24C4FC..0x24C5D6`, written at pc `0x1262EB` to `DM(0xC1F77)` | VERIFIED (listing, [synth-stage-models-7](synth-stage-models-7.md)); the clip bounds are ASSUMED (saturating `i32`) |
| `DM(I2-29)` pitch term `p0` | `DM(0xC1F79) = 0.006028 2^(semi / 12) = 2 f / F_SY` (`F_SY = 43402.34375 Hz`; 0.0060281 at C3) | VERIFIED (equation, same word as the SYNC stages) |
| ARM block `DM(0xC1599 ..)` (`DM(I1-29 .. I1-18)`) | `P[-257 .. -246]` of the P capture, `p_index = dm - 0xC169A`; already in [data/synth-p-words.csv](data/synth-p-words.csv) per type | VERIFIED (run) |
| `DM(I2-5)` depth source `x` | image constant 0.2 | ASSUMED (the hardware may modulate it) |
| Ratios `P[-32 .. -29]`, weights `P[-16 .. -9]`, gain `P[16 .. 17]` | the same CSV | VERIFIED (run) |

Rust: `synth/fw_fm/voice.rs` (`FmVoice`: `set_note(midi)` writes `W`, `p0`; `render4()` runs stage 1 for types 49 and 53, stage 2 for 50 to 52) with the default table. Unit tests: pitch words, input wiring, finiteness, determinism, note dependence. It is opt-in (D22): not reachable from the render path; no hardware clip of an FM program exists to validate level and timbre end to end.

## 7. Follow-ups after the wiring

- Find the writer of strip `+0xA4` / `+0xA8` (the FM wave id to partial ratio path via the FM-H table `bank0_CODE@0x2D82A8`) and decode the generator of all 361 words; then `fm_table` takes `(type, a4, a8)`.
- The ARM writer of the note term `DM(0xC2716)` and of `x`.
- A hardware FM clip to validate level, the table family and the voice mix before the default can flip.
