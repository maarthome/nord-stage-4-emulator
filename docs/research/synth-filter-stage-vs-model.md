# Synth filter stages against the Rust model: the port is right, the gap is not in the stages

- **Checked:** 2026-11-16
- **Sources (local, never committed):** ORG core 2 image run in `sharc-org` (`tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin`); Rust stage dumper `crates/nord-tools/src/bin/fw_filter_stage_dump.rs` (`FwCoefs` / `FwFilter`, the code path `--synth-fw-filter` uses); driver `tools/synth_filter_stage_diff.py` (test `tools/tests/test_synth_filter_stage_diff.py`); derived tables [synth-filter-stage-diff.csv](data/synth-filter-stage-diff.csv) and [synth-filter-stage-corner.csv](data/synth-filter-stage-corner.csv) (numbers only). Notes read: [synth-filter-links](synth-filter-links.md), [synth-filter-type-corners](synth-filter-type-corners.md), [synth-cutoff-voice-law](synth-cutoff-voice-law.md), [synth-stage-models-9](synth-stage-models-9.md), [synth-stage-models-10](synth-stage-models-10.md).
- **Question:** D121 left the fitted per-type / per-row cutoff factors (`p_type.rs`, `p_table.rs`) as the gap of `--synth-fw-filter` (3.36 against 2.18 mean gap score). Are they compensation for a port error in the Rust stage model (`fw_filter.rs`: `FwCoefs`, `FwFilter`, `TablePos`; `fw_ladder.rs`)? Per stage `0x1253BD`, `0x12566E`, `0x12549D`, `0x125913`, `0x125A50`, which parameter region departs from the real SHARC stage?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. Short answer

| Item | Result | Tag |
|---|---|---|
| Stage port, per sample | The five Rust stages equal the oracle stage on the frame words a sounding voice gives (`R` from the voice law, `W28 = 128 knob / 127`): 7 cutoffs x 9 resonances x 1024 samples per stage (an impulse, 255 zeros, 768 noise samples at +-0.5). Worst `max abs diff / output peak`: LP12 3.3e-5, LP24 3.9e-5, BP 3.6e-5, HP 2.1e-5, Ladder24 1.4e-4 (-77 dB). No cutoff, resonance or sample region departs structurally: the error is flat over the grid (LP, BP: 0.9e-5 .. 2.5e-5 up to resonance 112, 3.3e-5 .. 3.9e-5 at 127; HP grows from 3.5e-7 to 2.1e-5 with the resonance; Ladder24 1.4e-4 at every resonance) | VERIFIED (`diff`, CSV) |
| Response in Hz, stage against voice chain | At the same display cutoff, the voice chain (`FwCoefs::from_params` at `1.0147 x display`, 48 kHz, the `--synth-fw-filter` unit) has the corner of the real stage within 2.3 % up to 1.4 kHz display and within 9 % at 4.4 kHz (LP12 / LP24 / BP / HP, resonance 0 .. 112). Peak height equal to 0.05 dB up to 4.4 kHz (LP24, BP, HP), 0.3 dB (LP12), at most 0.5 dB at 13 kHz | VERIFIED (`corner`, CSV) |
| Verdict | **No port error in LP12, LP24, BP, HP.** The type and row factors of `p_type.rs` / `p_table.rs` are not compensation for the stage shape. Nothing was changed in the stage model, no default moves, no clip re-scored | VERIFIED |
| Ladder24 in a sounding voice | The stage port is right (row above), but the engine's `from_r` (pole = `2 atan R`, ASSUMED) is not the stage with the voice `R`: the oracle stage fed the literal voice `R` as table position has a corner 2 .. 500 Hz at display 139 .. 4400 Hz, and in the unpatched pass with a voice word its coefficient words do not follow `R` at all (constant position 0.5458 for knobs 40 .. 100). Which word the Ladder24 stage reads in a sounding voice is **open** (new OQ22 item) | VERIFIED (observation), cause open |

## 2. What was run

1. **Words a program gives.** `settled_words`: the stage in the B slot of channel 0, strip word 3 and common word 1 = `(knob - 52) << 24` (the ARM word, [synth-knob-laws](synth-knob-laws.md) s3), the voice word of note 60 in `DM(0xC1DCC)`, `kFltRes` word `-0.5 + knob / 127` in common word 7, 200 passes. `R = DM(I3 - 30)`: 0.002040 (knob 24), 0.005140 (40), 0.010281 (52), 0.020565 (64), 0.051859 (80), 0.165998 (100), 0.526204 (118); the same for every stage (checked for all five stages at knobs 52, 80, 100) and equal to [synth-cutoff-voice-law](synth-cutoff-voice-law.md). `W28` is the settled `128 knob / 127` ([synth-filter-links](synth-filter-links.md) s2); the sweep uses resonance knobs 0, 16, 32, 48, 64, 80, 96, 112, 127.
2. **`diff`.** Stage installed as in `tools/synth_stage_oracle.py` with the three frame rewriting stores patched out; frame words `-30`, `-28` poked (for Ladder24 also the coefficient words `-18 .. -12` the previous call would have stored, as `tools/synth_filter_ladder_oracle.py`); input = impulse, 255 zeros, LCG noise. The same words and input go to the Rust stage (`FwCoefs::new_strip`, which equals `FwCoefs::new` for the four `R`-driven stages and takes `R` as the Ladder24 table position, as the stage does). Output compared word by word.
3. **`corner`.** Rust stage at the real words against the voice chain at `display_hz x 2 / 1.971` ([synth-cutoff-voice-law.csv](data/synth-cutoff-voice-law.csv), `2 F`), both on a 65,536 sample impulse response; corner = last frequency above `DC / sqrt 2` (LP), first above `HP(0.45) / sqrt 2` (HP), geometric centre of the -3 dB band (BP). Hardware Hz = corner x `2 F` (86,804.69 Hz, ASSUMED unit), model Hz = corner x 48 kHz.

```
cargo build --release -p nord-sharc -p nord-tools
uv run python tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin --sram DIR
export NS4_SHARC_ORG=target/release/sharc-org NS4_ORG_IMAGE=DIR/core2_sram.bin
uv run python tools/synth_filter_stage_diff.py diff   --dumper target/release/fw_filter_stage_dump --csv docs/research/data/synth-filter-stage-diff.csv
uv run python tools/synth_filter_stage_diff.py corner --dumper target/release/fw_filter_stage_dump --csv docs/research/data/synth-filter-stage-corner.csv
```

(`words` prints the frame words, `ladder-voice` the Ladder24 observation of s4. On Windows pass the dumper with its `.exe`.)

## 3. Per stage

Stage output, oracle against Rust, 64,512 samples per stage ([synth-filter-stage-diff.csv](data/synth-filter-stage-diff.csv)):

| Stage | Words bit equal | Worst `max abs / peak` | Where | Earliest differing sample |
|---|---|---|---|---|
| LP12 `0x1253BD` | 1,208 | 3.3e-5 | knob 118, resonance 127 | 1 |
| LP24 `0x12566E` | 633 | 3.9e-5 | knob 40, resonance 127 | 2 |
| Ladder24 `0x12549D` | 6 | 1.4e-4 | every resonance, knobs 64, 80, 118 | 0 |
| BP `0x125913` | 844 | 3.6e-5 | knob 40, resonance 127 | 0 |
| HP `0x125A50` | 3,358 | 2.1e-5 | knob 64, resonance 127 | 0 |

The bit-exact Python model (`tools/synth_filter_model.py`, equal to the oracle word for word) uses 40-bit registers with truncating stores and a seed plus two Newton steps for `1 / sqrt`; `FwCoefs` / `FwFilter` use float32 round-to-nearest and the exact `sqrt` (the documented 1e-7 difference of `FwCoefs::new`). The 1e-5 level is that rounding accumulated in the integrators; it is flat in resonance for the Ladder24 and for LP / BP up to resonance 112, so it is not a coefficient error amplified by the resonance. The cause of the residual is not isolated to one operation (ASSUMED: truncating against rounding stores). The 3e-5 .. 4e-5 at resonance 127 (position 127.999, peak +50 dB, the clip active) is the largest region departure, still -88 dB.

Response at the real words ([synth-filter-stage-corner.csv](data/synth-filter-stage-corner.csv)), `model / hardware` corner, resonance 0 .. 112 (resonance 127 self-oscillates against the clip at +50 .. +76 dB and the linear impulse response method does not apply):

| Stage | display <= 1.4 kHz (`R <= 0.052`) | 4.4 kHz (`R = 0.166`) | 13 kHz (`R = 0.526`) | Peak dB difference (max) |
|---|---|---|---|---|
| LP12 | 0.987 .. 1.004 | 0.958 .. 0.994 | 0.862 .. 1.093 | 0.30 / 0.00 / 0.50 |
| LP24 | 0.986 .. 1.000 | 0.962 .. 0.998 | 0.877 .. 1.157 | 0.03 / 0.00 / 0.00 |
| BP | 0.984 .. 0.994 | 0.969 .. 0.982 | 0.932 .. 0.986 | 0.00 |
| HP | 0.977 .. 0.995 | 0.914 .. 0.993 | 0.751 .. 1.087 | 0.00 / 0.03 / 0.48 |

The 1 .. 2 % low ratio below 1.4 kHz and the larger departures at high `R` are the bilinear warp: the stage runs at `2 F` (86.8 kHz) while the engine runs the same section at 48 kHz, so the corner of the same `R` differs by `atan` warp and the 13 kHz corner cannot be reproduced at 48 kHz. That is a sample-rate limit of the engine, not a stage error; slopes and peaks agree except where `4 x corner` passes the 48 kHz Nyquist.

What the stages say about the factors: at display 277 Hz (knob 52) and resonance 64 the real corners are LP12 422 Hz (1.52 x display), LP24 375 Hz (1.35), BP 284 Hz (1.03), HP 215 Hz (0.78) at `2 F`, resonance peaks 6.4 / 7.3 / 7.3 dB. The Rust stages already produce exactly these. LP12 over LP24 is 1.12, not the fitted 1.4 (as D108 noted): a multiplication of the display Hz by 1.4 therefore **moves the voice chain away from the real stage**, it does not repair it.

## 4. Ladder24

`FwFilterType::Ladder` (stage port) is right at the 1.4e-4 level (section above). The voice chain does not feed it the stage's own table position; `FwCoefs::new` uses `LadderCoefs::from_r` (position chosen so the section pole equals `exp(-2 atan R)`, ASSUMED). Two measured facts show how open that is:

| Observation | Number |
|---|---|
| The stage with the literal voice `R` (`FwCoefs::new_strip`) at display 139 .. 4400 Hz (knobs 40 .. 100, resonance 0 .. 112) has a -3 dB corner of 1.3 .. 500 Hz (`2 F`); the voice chain at `1.0147 x display` has 61 .. 12,700 Hz | [corner CSV](data/synth-filter-stage-corner.csv), rows `ladder`; ratios 1.1 .. 18,000 |
| Unpatched pass, voice word set, stage `0x12549D` in slot 0: the stored coefficient words stay at `c = -0.9261` (table position 0.5458) for knobs 40, 52, 64, 80, 100 while `R` runs 0.0051 .. 0.164; without the voice word they equal `rows(R)` word for word (`R` 0.266 .. 0.742) | `uv run python tools/synth_filter_stage_diff.py ladder-voice` |

So the table read by the Ladder24 stage at the voice `R` would give a nearly closed filter at 50 .. 500 Hz for knobs below 100, which contradicts the hardware clips (`synth_phat3`, `synth_80s`, `synth_leaky` play open at the displayed Hz with the fitted 0.8). Either the Ladder24 stage is not reached on this frame in a voice state (the stale coefficients suggest it is not called, or not with these words), or its position word comes from another source. This is the Ladder24 position word of D108 open item 4; it now has a measured symptom. The Ladder24 factor 0.8 therefore stays EMPIRICAL and unexplained by the stage.

## 5. Verdict

For LP12, LP24, BP and HP the port **matches the oracle within float32 accumulation (-88 dB of the output peak)** over the whole cutoff x resonance grid a program can produce, and the response in Hz at the displayed cutoff matches the real stage to 2.3 % below 1.4 kHz. The per-type and per-row cutoff factors of `p_type.rs` / `p_table.rs` are **not** compensation for a stage-model error. The remaining gap of `--synth-fw-filter` (3.36 against 2.18, D121 s5) lies outside the filter stages: what enters `R` before the stage (the sum of knob, key tracking, filter envelope amount, LFO and the pitch-note term; the pass adds terms we modelled only at their idle / unmodulated value, [synth-cutoff-voice-law](synth-cutoff-voice-law.md) s4 item 4), the level in front of it (table A input gain `cin`, up to 1.98 for BP at resonance 0, oscillator level), the pre-filter path, and, for the Ladder24 clips, the position word of section 4. This is OQ22.

No fix was needed, so no engine path changed and the 12 clips were not re-scored (the D121 numbers stand: default 2.18, `--synth-fw-filter` 3.36; the engine output is bit-identical to the D121 build). Only `FwCoefs::new_strip` (tool-facing constructor, no caller in the render path) and the dumper were added.

## 6. Implications

- A per-type factor cannot be removed or shrunk by correcting the stage model: the stage model has no error to correct. Replacing the factors needs the terms in front of `R` (item 1 below) or a hardware corner sweep at fixed `R`.
- The `2 F` unit (D121) is consistent with the oracle stages: at `1.0147 x display` the engine corner equals the real stage corner (0.98 .. 1.00 below 1.4 kHz), so a corner sweep clip would confirm `F` against `2 F` directly in Hz.
- Ladder24 needs its own capture before `--synth-stages` is trusted for type 2.

## 7. Follow-ups

1. Capture `R` per pass of a loaded program with a sounding voice (ARM `tools/arm_program_load.py` + `arm_keydown.py`, synth layer) including the filter envelope, LFO and key tracking terms, and compare it with `fc_nat(knob, note)` of the voice law; the difference is the missing term of the clip gap.
2. Ladder24: find the word the stage reads in a sounding voice (watch `DM(I3 - 30)` reads at the stage prologue, or trace why the coefficient words stay at position 0.5458); until then the Ladder24 clips stay on LP24 + 0.8.
3. Model stage `0x1257B2` (raw type 3, 61 factory layers) like the other four (out of scope here).
4. Hardware corner sweep at a fixed `R` to settle `F` against `2 F` (out of scope here).
