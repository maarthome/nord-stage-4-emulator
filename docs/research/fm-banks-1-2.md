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
| Dependence | Only on the type: `a`, `b` (pending wave variant words) do not change a word (4 variants x 5 types, `tools/synth_fm_tables.py capture` stops on a difference). Types 50, 51, 52, 53 and 49 give 5 tables; segments: ramp `0xE0900 + k` (70 words, `(70 - k) / n`, n = 240 (49), 30 (50, 51, 53), 51 (52), within 2e-6 relative, a float recurrence), zeros, a decay curve at `0xE0967..` (about `A 2^(-k/6) - c`, ratio 0.8909 per step, clipped at zero; A = 7822 for 49 and 53, 3911 for 50 to 52), zeros, a second decay curve at `0xE09E8..` (A = 62582 for 49 and 53, 4651 / 4544 / 14310 for 50 / 51 / 52) | VERIFIED (capture), formula shapes EMPIRICAL |
| Committed data | [data/synth-fm-tables.csv](data/synth-fm-tables.csv): 5 types x 361 words (1,805 derived numbers, about 40 KB; D56 sizing, no firmware bytes), fingerprint `13a83db262affa8a` in `tools/tests/test_synth_fm_tables.py` | VERIFIED (test) |
| Partial ratios | The FM-H partial double table at `bank0_CODE@0x2D82A8` reads `0.5, 1, 2, 3, 4, 5, 6, 7 ...` | VERIFIED (run) |

## 2. Reproduce

```bash
uv run --with unicorn --with capstone python tools/synth_fm_tables.py capture     target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin     --cache target/settled.snap --out docs/research/data/synth-fm-tables.csv
python -m pytest tools/tests/test_synth_fm_tables.py
```

Disassembly: `python tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin --sram DIR` then `sharc-disasm DIR/core2_sram.bin 0x2484D2 0x248F6C`.

## 3. Implications

- The "table not reachable" blocker is closed. What remains for a port is the stage maths itself (two SIMD stages of 290 and 300 lines: four phase accumulators per lane, eight sine polynomials of `abs(phase) + c`, the band-curve lerps), the P words of the FM wave types (already in [data/synth-p-words.csv](data/synth-p-words.csv), types 49..53) and the ARM writer of the per-wave ratio (`0x2D82A8` table).
- Open: the curve index source (which register carries the pitch / note term into `FEXT R0 BY 25:7`), whether the table depends on strip state other than the type (only `a`, `b` were varied), and the analytic generator (values are float recurrences; the CSV is the source).

## 4. Follow-ups

1. Decode `0x124269` word for word (Python model + `sharc-org` oracle with the table poked), then `0x12453F`.
2. Rust opt-in port under `crates/nord-core/src/synth/`, tables loaded from the CSV derived numbers, defaults unchanged (D22).
3. Specs, D-entry, OQ22, ROADMAP once a stage is bit exact.
