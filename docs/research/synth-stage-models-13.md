# Synth stage models, batch 13: the WAVE bank (`0x1247B6`) and its 46 tables

- **Checked:** 2026-11-14
- **Sources (local, never committed):** OS v1.66 `bank0_CODE.bin` (also as the update container `fw_v1.66.bin`), `bank0_SRAM.bin` and the ARM harness (`tools/arm_edit_capture.py` settled snapshot, `tools/synth_wave_probe.py` logic), PNO / ORG core 2 images (`tools/ns4_dsp_images.py`, `tools/ns4_dsp_ldr.py --sram`), `sharc-org`, `sharc-disasm` (listings `0x248F6C..0x24930A`, `0x2484D2..0x248F6C`), the ARM boot capture of [arm-bulk-tables](arm-bulk-tables.md) (`pno_core2.script`, `org_core2.script`), `capstone` on `bank0_CODE.bin`. Builds on [synth-stage-models-12](synth-stage-models-12.md) (the SUPER 2..4 stage), [synth-wave-categories](synth-wave-categories.md) s2 and [synth-sample-kernel-rule](synth-sample-kernel-rule.md) s9.
- **Question:** which DSP stage renders a WAVE (wavetable) layer, where do its 46 tables come from, how does the ARM hand a selected wave to the DSP, and can the stage be modelled word for word?

Tags follow `.agents/rules/provenance-in-code.md`: **VERIFIED** = reproduced by the command shown, **EMPIRICAL** = measured, **ASSUMED** = reading.

## 1. Short answer

| Question | Answer | Tag |
|---|---|---|
| Which strip type is a WAVE layer? | `kOscBank 3` gives pending type **54** (`[strip + 0x98]`), stage `0x1247B6`; the ARM builder of the WAVE tree calls `0xC01ACBF4(this, 0x36, table_id)` ([synth-wave-categories](synth-wave-categories.md) s2): `0x36 = 54`. Type 56 is the same stage; types 55 and 57 resolve to `0x124B6A` (an `RTS`) and `0x124B6D` (not decoded) | VERIFIED (run: pending block of bank 3; code read) |
| Is it SY / SMP? | **No.** The stage runs on PNO / ORG **core 2** next to the ANALOG stages, and reads the same wavetable machinery as SUPER 2..4 (`DM(0xC15A5)` = `0x08030000`, rows at PM `DM(0xC15A6)`, FIR PM `0x9531F`, lattice PM `0x95000`). No SY / SMP message exists for it (the negative run of [synth-sample-kernel-rule](synth-sample-kernel-rule.md) s9 stands) | VERIFIED (listing, run) |
| Where are the 46 tables? | **Firmware rodata**: a registry of 55 records at `bank0_CODE@0xC02FD4B0` (28 bytes each: `name, rows_end, 8, rows, 512, data, count`). Records 0..45 are the WAVE table ids `Second` = 0 .. `Tubular Bells` = 45 (40 of 46 names equal the preset tree, 6 abbreviations; `tools/synth_wave_registry.py`), records 46..54 are nine vowel tables (`AAA .. OE`, not WAVE). 605,164 int16 samples in all. Not sample-library data, not the `.ns4b` | VERIFIED (run) |
| How does the DSP get one? | The ARM boot copier `0xC01C0B3C` copies record 0 (`Second`: `[object + 0x44] = 0xC043FB54` is its `data`, the segment list `0xC0440924` is its `rows`) into the DSP slot (L2 `0x200C0000`, PM rows `0x9631F`). Layout: unique bands of the rodata table re-laid in ascending order with stride `count + 12`, each band followed by its first 12 samples cyclically (a guard for the FIR), the row word 0 rewritten. Rebuilt record 0 equals the boot copy: **1970 of 1970 samples and 512 of 512 row words** (`tools/synth_wave_registry.py CODE --boot-check`). The six boot slots at `+10,000` words hold this one table; the 46 tables are at most 9,010 words, so they fit a slot | VERIFIED (record 0); the copy of a selected wave at run time is not captured (ASSUMED) |
| Is the stage modelled? | Yes: `tools/synth_stage_wave.py`, **320 of 320** oracle words over 40 trials with the boot table, and 96 of 96 each (12 trials) with the rebuilt tables of Bell (40), 2nd Spectra (16), Tubular Bells (45) and Full Organ (7) | VERIFIED (run) |
| Banks 1 and 2 (types 49..53, stages `0x124269`, `0x12453F`) | Not WAVE. The pending block of bank 2 sets word `[+0xC] = 1`; the FM tree has two banks of five groups (`FM-H`, `FM-I`, [synth-wave-categories](synth-wave-categories.md) s1): these are most likely the FM oscillators. The stages are additive: two phase accumulators per lane, eight sine polynomials of `abs(phase) + c`, and 129-point lerp rows through `I1` (`DM(-14, I1) = DM(0xC15A8) = 0xE0926`, zero in the image and not written by the boot: ARM-filled). Not decoded | EMPIRICAL (structure) / open |

## 2. Registry and layout

```bash
python tools/synth_wave_registry.py target/fw_unpacked/bank0_CODE.bin --csv docs/research/data/synth-wave-registry.csv
python tools/synth_wave_registry.py target/fw_unpacked/bank0_CODE.bin --boot-check   # entry 0 vs the boot copy
python tools/synth_wave_registry.py --check                                           # committed CSV
```

[data/synth-wave-registry.csv](data/synth-wave-registry.csv) lists index, name, sample count and the two addresses of each of the 55 records (addresses and counts only: the samples are firmware rodata and stay out of the repository, D22). Sample counts: 1,768 (the seven `Second .. Squabble` organ waves and `Flute1`), 3,318, 6,352, 12,360, 14,957 (`Bar2`), 17,282, 8,115 (`Tubular Bells`) and 18,020 (vowels). The 128 band rows of a table are `(offset, count, spare, spare)`; the spare pair `(1921, 1)`, `(1732, 8)`, `(0, 8)` is identical in the rodata and boot rows and is not an offset.

Run-time data path in the emulator: the user passes their own OS image (`--synth-wave-tables PATH`, the update container or `bank0_CODE.bin`), `synth/fw_wave_tables.rs` reads the registry and builds the DSP layout of all 46 tables in memory; nothing derived from the samples is committed.

## 3. The stage `0x1247B6` (listing `0x248F6C..0x24930A`, 190 lines)

It is the two-oscillator sibling of `0x124985` ([synth-stage-models-12](synth-stage-models-12.md) s3); every register juggle is the same, the set-up differs:

| Part | `0x124985` (SUPER 2..4) | `0x1247B6` (WAVE) |
|---|---|---|
| pitch | `f = clip(DM(-29, I2), 0.5)` | the same, written back |
| increments | `inc_j = (rho_j v + 1 - v) 2 f` from the spread word `s5`, four voices | `D_j = fix(f P[-32 + 2 j] 2^31)` (`DM(-10, I0) = 0x1F`), two oscillators, no spread word |
| positions | `ph + D`, `ph + 2 D` per voice | `ph + 2 D`, `ph + 4 D` per oscillator; the phase word becomes `ph + 4 D` |
| band row | `idx` from `2 f` | the same (`0x2493BA` routine inlined), row broadcast |
| buffer, FIR, `a a b b`, three biquads | as in s3 of batch 12 | the same (`DO` loops of 144, 51, 24 and 78 words) |
| mix | four groups `W[j] = P[-16 + 4 j ..]`, `cross + own`, then `+ own`, `+ cross` | groups 0 and 1 only, same order |
| scale | `P[16] g` with `g = C (B + clip(A - f, B))` | the same |

Model: `tools/synth_stage_wave.py` (imports the batch 12 helpers `row_index`, `fir_lane`, `biquad`).

## 4. Oracle comparison

| Run | Result |
|---|---|
| `python tools/synth_wave_oracle.py --image SRAM_ORG/core2_sram.bin --stage 0x1247B6 --trials 40` (boot table) | **320 of 320** output words equal |
| Rebuilt tables 40, 16, 45, 7 (`tables_from_code`-equivalent `build_l2`), 12 trials each | 96 of 96 each |
| Recorded cases | `data/synth-stage-oracle-cases-rest.json` key `0x1247b6` (six cases), replayed by `tools/tests/test_synth_stage_rest.py` without the firmware |

Limits: ratios drawn near 1 (`2^U(-0.5, 0.5)`) so a position pair stays inside the 6-word buffer (as in batch 12); the short-word extension question (zero or sign) of batch 12 s4 applies unchanged (the Rust port sign-extends, ASSUMED).

## 5. Rust (opt-in)

`synth/fw_wave.rs` (`wave_bank_sample`, `WaveBankState`, no heap per sample), `synth/fw_wave_tables.rs` (registry reader and layout, `tables_from_code`, `tables_from_file`, tests on a synthetic image). `SynthLayer::wave_bank` (default off) makes a `Wavetable` layer play the stage instead of the 64-table placeholder; table = `wavetable_index` (the wave id). Flags: `--synth-wave-bank` (boot table for every wave) and `--synth-wave-tables PATH` (all 46 tables from the user's image; implies the first). ASSUMED: the two oscillator ratios (`1, 1`) and weights (`0.5, 0.5`, the P block is open), the last band's guard trim for tables other than record 0 (two samples), the sign extension.

## 6. Coverage

WAVE layers with a bit-exact stage model: **14 of 14 layers in 12 factory programs** (`BELLS/TINES` 3, `DIGITAL` 8, `KEYS` 2, `ORGAN` 1, `ACOUSTIC` 0; [program-coverage](program-coverage.md)): one stage for all, so all 14 are covered by the model. The Rust port plays them correctly only when the user supplies the image; the per-layer table ids are not in `program-sweep.json` (category only). ANALOG 312 of 312 and filters 442 of 442 unchanged.

## Implications

- The WAVE bank was not a missing data path but a firmware-rodata registry plus a stage next to SUPER 2..4; the earlier "WAVE does not read this table" (batch 12 s1) was about the boot slot, which holds `Second` only.
- The remaining parity gap is the P block (ratios, weights) and the run-time hand-over (how the selected record reaches the slot; the ARM sends `aux + base` in strip word 4 and a flag in word `0xA`, [synth-wave-categories](synth-wave-categories.md) s2).
- FM (banks 1 and 2) is the next oscillator family without a model.

## Follow-ups

1. Capture the ARM copy of a selected wave (the block copier with a record other than 0) to turn the layout of s1 into a run-time VERIFIED claim for records 1..45 (needs a loaded WAVE program in the harness).
2. Decode `0x124269` / `0x12453F` (FM-H / FM-I) and fill `0xE0926` rows.
3. Extract the wave id of each of the 14 factory layers from the `.ns4p` files and a hardware clip of one WAVE program to fit ratios and weights.
