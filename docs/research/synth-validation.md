# Synth default validation: legacy vs strip vs strip + per-wave P

- **Checked:** 2026-11-14
- **Sources (local, never committed):** Nord Stage 4 demo Part A (`_zksT69H30c`, nine synth programs, `tools/emu_vs_hw.py` segments), the Audible Genius lesson clips in `.reference/audible-genius-nord-4/` (pitch blip `s0OhmMpbscU`, laser zap `0NMvKWDc-Lo`, Super Saw detune `MjkV3qz8Gqk`), the factory backup, `tools/synth_validate.py` (new), example `ag_synth`, `ns4 render-patch`, the program sweep (`crates/nord-tools/src/bin/program_sweep.rs`). Nothing from the clips (audio, video, frames) is committed; the derived numbers are in `docs/research/data/synth_validation_results.json`, the clip set in `docs/research/data/synth_validation.json`.
- **Question:** Does the firmware strip chain with the stage oscillators and an EMPIRICAL per-wave P table reproduce hardware synth sounds better than the legacy chain on clips that were not used for the fit, so that it can become the synth default (D22)? Which P values are per wave and which are common?

Tags follow `.agents/rules/provenance-in-code.md`. The audio is lossy YouTube audio, the replay velocity is one value (100) for every Part A note, and the lesson clips fix no absolute level, so every number is shape evidence with the stated uncertainty.

## 1. Short answer

| Question | Result | Tag |
|---|---|---|
| D22 flip | **Done**: `SynthLayer::new` now plays the strip chain, the stage oscillators and the P table (`FirmwareOptions::default()` the same, `--legacy-all` / `SynthLayer::legacy` keep the classic chain). Over 15 clips: mean score 6.6 (legacy) to 4.1 (default), 9 clips better, 3 equal, 3 worse; over the 9 hold-out clips mean gain 3.8 dB of score, 7 better; no clip worse than 3 dB in presence (all) or RMS (Part A) | EMPIRICAL |
| Common law of the SHAPE oscillator | the shape word equals the **Osc Ctrl knob** (`shape = ctrl`), no second oscillator: Smooth Lead (A:87, fit) 0.69, Crunch Synth (A:66, hold-out) 1.18. The octave-up second oscillator of the earlier fit (ratio 2.0, weight 0.35) was compensating a wrong shape word (idle 0.5) and is dropped; the "factor 2 for pitch" is therefore not needed | EMPIRICAL (two programs) |
| Per-wave level | saw 0.4, pulse 0.27 (= saw x 0.68), SHAPE 0.6, MULTI 0.26, SUPER 2.0 (linear, velocity 100, Part A level convention) | EMPIRICAL |
| Per-wave cutoff factor | SHAPE 1.75, MULTI 1.4, saw / pulse 0.9, SUPER 1.5: **only SHAPE and MULTI are stable**; saw (filter_bank 1.25 vs laser-zap 0.8) and SUPER (pad_silk 0.5 vs mega_line 2.0) had clips that disagree; section 7 splits the saw disagreement into a band pass type factor (saw 0.85, BP 1.4) and shows the SUPER one is the filter envelope of pad_silk, not a factor (SUPER 1.07 x LP12 1.4 = 1.5) | EMPIRICAL, weak |
| Super Saw level against Detune | the SUPER stage keeps the level within 1.2 dB from Detune 0 to 10 (hardware: flat), the legacy 7-voice saw falls 8 dB (-20.1 to -11.9 dB against the hardware) | EMPIRICAL (3 settings) |
| Program sweep, 384 factory programs | 0 NaN / Inf, 0 newly silent, programs with more than 0.1 % clipped samples 18 to 9, none clips more than before; 279 of 384 programs change by 3 dB or less in RMS, the extremes are -23.1 and +15.7 dB (section 6) | measured |

## 2. The validation set

Defined in `docs/research/data/synth_validation.json`; `role` = `fit` when the clip fixed a row of the P table, `holdout` when it never did. Part A clips are the confirmed notes of the program's OLED window rendered with `ns4 render-patch` (same program, same events, replay velocity 100); lesson clips are one steady note rendered with `ag_synth` at the panel values read off the OLED ([audible-genius-synth-lessons](audible-genius-synth-lessons.md) s2).

| Clip | Source | Wave (P row) | Role | What |
|---|---|---|---|---|
| lead_smooth | A:87 Smooth Lead, 22 s | SHAPE triangle | fit | LP24 370 Hz, Osc Ctrl 7.4 |
| lead_crunch | A:66 Crunch Synth, 26 s | SHAPE triangle | holdout | LP12 622 Hz, Osc Ctrl 9.4 |
| pad_silk | A:86 Silk Pad, 18 s | SUPER wave 0 | fit | LP12 294 Hz pad |
| mega_line | A:48 Mega Line Whl, 40 s | MULTI + SUPER x2 | holdout | LP12 1200 Hz, wave 1 of SUPER plays the legacy saw |
| sci_fi_80s | A:44 80s Sci-Fi, 50 s | MULTI | fit | Ladder24 1500 Hz |
| filter_bank | A:75 Filter Bank Ld, 40 s | saw x3 | fit | band pass 165 / 466 / 1300 Hz |
| phat_3osc | A:67 Phat 3 Osc MiniM, 18 s | saw x3 | holdout | Ladder24 13 kHz (open) |
| sweep_12db | A:57 12dB Sweep, 20 s | SHAPE square x2 + saw | holdout | LP12 |
| sine_pluck | A:26 Sine-Saw Plk Whl, 40 s | SHAPE SINE wave 0 | holdout | LP12 185 Hz |
| ag_blip_saw / ag_blip_square | pitch blip clip 20.65 to 21.15 s / 26.5 to 27.25 s, G4 | saw / pulse | fit (level ratio only) | filter open |
| ag_zap_saw | laser zap clip 55.75 to 57.75 s, G4 | saw | holdout | LP24 415 Hz, Res 7.3, Env Amt 7.7, decay 2.07 s |
| ag_supersaw_0 / _3 / _10 | Super Saw clip 0.5 to 1.2 s, 21 to 22.2 s, 46.5 to 48 s, G4 | SUPER wave 0 | holdout | Detune 0.0 / 3.0 / 10.0, filter open |

Part A programs with a clean single synth part were chosen from the 67 programs of the video; programs that mix the synth with organ or piano (Space Drama II, Pinky Lead) cannot be isolated on the hardware audio (the scene / section edits of `emu_vs_hw.py` isolate only the emulator side), so they are **not** in the set. Not run: the DSoundman LFO clip (it measures the LFO depth law, which the strip chain does not change, [dsoundman-synth-lfo](dsoundman-synth-lfo.md): a P table cannot move it) and the DSoundman pad-alone clip (Soft Pad 1 is not a factory program and its panel values are not read), see Follow-ups.

Metrics (`emu_vs_hw.segment_metrics`, emulator minus hardware, level-free band deltas): presence = 1.5 to 3 kHz band, brilliance = 3 to 8 kHz band, centroid (Hz), RMS (dB), attack (10 to 90 % rise of the median onset). **Score** = mean absolute delta of the sub (20 to 120 Hz), low, mid, presence and brilliance bands (the 8 to 20 kHz band is excluded, YouTube audio is low-passed). Variants: `legacy` = classic chain; `strip` = firmware strip chain, unit P, legacy waveforms; `strip+p` = the single Smooth Lead block of [synth-p-fit](synth-p-fit.md) (`--synth-p fitted`); `strip+st` = plus the stage oscillators; **default** = `strip+st` plus the per-wave table (`strip+st+tbl`).

## 3. Result: score of every variant (lower is better)

| Clip | Role | Wave | legacy | strip | strip+p | strip+st | **default** |
|---|---|---|---|---|---|---|---|
| lead_smooth | fit | SHAPE | 4.93 | 4.05 | 0.97 | 4.05 | **0.69** |
| lead_crunch | holdout | SHAPE | 8.33 | 2.07 | 5.04 | 2.07 | **1.18** |
| pad_silk | fit | SUPER | 6.53 | 6.24 | 9.17 | 5.39 | **7.63** |
| mega_line | holdout | MULTI + SUPER | 2.98 | 3.22 | n/a | 3.64 | **2.03** |
| sci_fi_80s | fit | MULTI | 2.03 | 2.01 | n/a | 0.83 | **0.24** |
| filter_bank | fit | saw | 3.15 | 3.82 | 6.65 | 3.82 | **4.46** |
| phat_3osc | holdout | saw | 3.21 | 2.91 | n/a | 2.91 | **2.91** |
| sweep_12db | holdout | SHAPE square | 21.38 | 12.50 | n/a | 4.85 | **4.59** |
| sine_pluck | holdout | SHAPE SINE | 5.99 | 6.54 | 5.15 | 7.03 | **7.03** |
| ag_blip_saw | fit | saw | 5.28 | 5.31 | n/a | 5.31 | **5.31** |
| ag_blip_square | fit | pulse | 3.29 | 3.29 | n/a | 3.29 | **3.29** |
| ag_zap_saw | holdout | saw | 8.99 | 7.27 | n/a | 7.27 | **6.93** |
| ag_supersaw_0 | holdout | SUPER | 11.31 | 11.36 | n/a | 4.38 | **4.38** |
| ag_supersaw_3 | holdout | SUPER | 6.83 | 6.62 | n/a | 6.00 | **6.02** |
| ag_supersaw_10 | holdout | SUPER | 4.96 | 4.79 | n/a | 4.95 | **4.96** |

`strip+p` is only available for the clips of the first run (it is the single-block opt-in of the earlier PR: it overfits Smooth Lead and is worse than plain strip on Crunch Synth, 5.04 against 2.07, because its fixed shape word 0.5 does not follow the Osc Ctrl knob).

Band, level and attack detail, legacy / default:

| Clip | presence dB | brilliance dB | RMS dB | centroid Hz | attack hw / legacy / default ms |
|---|---|---|---|---|---|
| lead_smooth | -16.5 / -2.0 | -6.9 / +0.2 | -6.4 / -0.9 | -232 / -38 | 19.8 / 42.3 / 35.0 |
| lead_crunch | -15.0 / +2.3 | -18.5 / +0.4 | +5.6 / +2.3 | -246 / +147 | 65.5 / 63.7 / 65.4 |
| pad_silk | +12.3 / +13.2 | +14.4 / +15.8 | -11.4 / +1.1 | +21 / -35 | 61.2 / 100.9 / 42.4 |
| mega_line | -0.1 / -1.5 | -4.8 / -3.4 | -18.5 / -5.7 | +128 / +8 | 59.1 / 63.6 / 30.8 |
| sci_fi_80s | +1.7 / +0.4 | -2.0 / -0.1 | -0.4 / +0.0 | -92 / -159 | 97.1 / 113.0 / 77.6 |
| filter_bank | -3.4 / -2.6 | -2.3 / -4.3 | +3.8 / +1.6 | -437 / -425 | 11.0 / 20.8 / 24.7 |
| phat_3osc | +3.6 / +3.5 | +2.6 / +1.4 | +12.1 / +1.0 | +563 / +486 | 26.1 / 87.2 / 66.2 |
| sweep_12db | -26.2 / -3.5 | -36.1 / -10.1 | +19.0 / +7.6 | -652 / -408 | 46.8 / 0.0 / 95.2 |
| sine_pluck | -19.0 / -17.0 | +2.8 / +7.1 | +2.0 / +4.8 | -89 / -90 | 77.8 / 58.8 / 57.4 |
| ag_blip_saw | +0.1 / +0.0 | +0.2 / -0.5 | -5.8 / -14.1 * | +72 / -75 | n/a |
| ag_blip_square | +0.1 / +0.1 | +0.2 / +0.2 | -2.4 / -14.0 * | +49 / +49 | n/a |
| ag_zap_saw | -1.4 / -0.3 | -7.0 / -2.5 | -7.2 / -3.4 * | -625 / -416 | n/a |
| ag_supersaw_0 | +5.3 / +1.0 | +8.9 / +0.8 | -20.1 / -7.9 * | +2296 / +111 | n/a |
| ag_supersaw_3 | +1.4 / -1.5 | +2.0 / -1.6 | -12.6 / -4.8 * | +446 / -186 | n/a |
| ag_supersaw_10 | +0.2 / -0.7 | +0.7 / -0.2 | -11.9 / -6.6 * | +114 / -69 | n/a |

\* RMS of lesson clips is not a level evidence (two Pure Saw notes of the same clip differ by 13 dB, velocity unknown); the P levels come from Part A only, so the lesson clips read 8 to 14 dB low. The RMS bar is applied to Part A clips only.

## 4. The P table per wave (hold-out discipline)

`synth/p_table.rs` (`PWave`, `PTable`, rows tagged in the source). Fit: level from the RMS delta of the fit clip (linear, so exact for a linear path), cutoff factor and shape law by grid on the fit-clip score, second oscillator tested and dropped.

| Row | gain | cutoff factor | Fit clip (score before to after) | Hold-out clips (legacy to default score) |
|---|---|---|---|---|
| SHAPE wave 0 (`0x1212D3`) | 0.6 | 1.75, shape word = Osc Ctrl | lead_smooth 4.05 (strip) to 0.69 | lead_crunch 8.33 to 1.18 |
| MULTI (`0x121C54`) | 0.26 | 1.4 | sci_fi_80s 0.83 to 0.24 (grid: 0.7: 1.86, 1.0: 0.83, 1.4: 0.24, 2.0: 0.34) | mega_line (with SUPER) 2.98 to 2.03 |
| SUPER wave 0 (`0x121D6D`) | 2.0 | 1.5 (compromise) | pad_silk RMS -5.1 to +1.1; cutoff 0.5 alone gives 2.03, 1.5 gives 7.63 | mega_line 2.03, ag_supersaw_0 / 3 / 10: 11.31 to 4.38, 6.83 to 6.02, 4.96 to 4.96 |
| Pure Saw (`0x1215FB`), SHAPE saw | 0.4 | 0.9 (compromise) | filter_bank RMS +5.3 to +1.6 | phat_3osc 3.21 to 2.91 (RMS +12.1 to +1.0), ag_zap_saw 8.99 to 6.93 |
| Pure Square (`0x121B99` family) | 0.27 | 0.9 (ASSUMED from the saw) | ag_blip pair: pulse 3.4 dB under the saw (one pair) | none |
| SHAPE square, SHAPE SINE, others | 1.0 | 1.0 (idle) | none | sweep_12db 12.50 to 4.59 (stage model only), sine_pluck 6.54 to 7.03 |

SHAPE shape-word grid (fit clip, gain 0.6, no second oscillator): cutoff 1.0 presence -10.1 dB, 1.35 -3.7, 1.75 -2.0. With the shape word fixed at 0.5 and the octave oscillator (earlier fit) the Crunch Synth presence reads -8.0 dB; with the shape word = Osc Ctrl (0.94) and the same cutoff, +2.3 dB. Crunch Synth with the second oscillator: score 2.11 (weight 0.35) against 1.18 (weight 0).

Disclosure: the SAW and SUPER cutoff compromises (0.9, 1.5) were chosen after seeing `ag_zap_saw` and `mega_line`, so those two hold-outs are no longer independent for those rows; the clean hold-outs are lead_crunch (SHAPE), phat_3osc (saw), sweep_12db, sine_pluck and the three Super Saw clips (filter open, so the cutoff factor does not act). Gains of the Pure Square and the SHAPE SINE / SHAPE square rows have no hold-out.

What is common across waves: the pitch factor 2 of the old fit is not needed; the SHAPE word follows the Osc Ctrl knob; the level is linear per stage family and Part A velocity 100; the unison and Super Saw spread laws of `synth/measured.rs` are untouched (the table acts on gain and cutoff only; `measured_laws` stays opt-in). The pulse sits 3.4 dB under the saw in both clips of the pitch-blip lesson, as the earlier fit found (3.3 dB).

## 5. D22 decision

Bar (task and [synth-p-fit](synth-p-fit.md) s6): the default beats legacy on most validation clips (mean score improvement), and no clip is worse by more than 3 dB in presence (all clips) or RMS (Part A clips); `tools/synth_validate.py summary` computes it.

| Variant | clips | mean score gain vs legacy | clips better | clips over the bar | D22 bar |
|---|---|---|---|---|---|
| strip (all / hold-out) | 15 / 9 | 1.15 / 1.86 | 9 / 6 | 0 / 0 | pass |
| strip + stages (all / hold-out) | 15 / 9 | 2.23 / 3.43 | 10 / 7 | 2 / 1 (mega_line presence -0.1 to -5.7, sci_fi_80s RMS -0.4 to +11.8) | fail |
| **strip + stages + P table (all / hold-out)** | 15 / 9 | **2.50 / 3.77** | **9 / 7** | **0 / 0** | **pass** |

Clips that stay worse than legacy and are named here: **pad_silk** (score 6.53 to 7.63, presence +12.3 to +13.2 dB: the Silk Pad is 13 dB too bright at LP12 294 Hz whatever the stage, the SUPER cutoff factor cannot fix it without breaking mega_line; an LP12 slope or cutoff mapping issue is suspected), **filter_bank** (3.15 to 4.46, band pass of saws, brilliance -4.3 dB), **sine_pluck** (5.99 to 7.03, presence -17 dB for every variant: the SHAPE SINE stage is bit exact but its parameter words are unknown). Three clips are equal (ag_blip x2, ag_supersaw_10).

Decision: the bar is met, so `SynthLayer::new` and `FirmwareOptions::default()` select the strip chain, the stage oscillators and the P table. The classic chain stays one call away (`SynthLayer::legacy`, `--legacy-all`, `--synth-chain legacy`), and the GUI Firmware bar shows the new defaults (Chain: "Firmware strip (default)", Stage oscillators checkbox, P table dropdown). The weak spots above are recorded in `docs/OPEN-QUESTIONS.md` (OQ22) and `docs/TECH_DEBT.md` (TD-11).

`render_hash` (example `render_hash`) changes from `25baa387b0219c0835094150106a35ce751b9c16b68109d2a030596e78e7b59a` to `b1c91b531d850889f169444738c3863ab5e112c0d4b62c79b2a8a28c2e5dc9aa`: every `SynthLayer::new` segment changes (the synth oscillator modes, the looped sample layers at several ratios, the full engine timeline), the organ and piano segments are unchanged. The classic chain itself is unchanged (`SynthLayer::legacy`, covered by the existing unit tests that now build on it).

## 6. Program sweep with the new default

`program_sweep` (384 factory programs, notes 48, 55, 64, 72, velocity 100, 96,000 frames), "default" mode (new default) against the committed pre-flip numbers of [program-coverage](program-coverage.md):

| Check | Before | After |
|---|---|---|
| NaN / Inf in any program | 0 | 0 |
| Silent programs, default mode | 0 | 0 |
| Silent programs, `--firmware-all --firmware-routing` mode | 9 | 9 (same programs, scene masks, OQ25) |
| Programs with more than 0.1 % clipped samples (default) | 18 | 9, none clips more than before |
| Same, firmware mode | 18 | 6, one program (Cinema Rise) 0 to 0.1 % |
| Programs whose RMS changes by at most 3 dB / 6 dB | | 279 / 324 of 384 |
| Largest RMS drops | | Danceflr Peak MW -23.1 dB, Chip Bass -21.1, Short Chords -20.4, Circus Box -20.2, PineScape Synth -18.5 |
| Largest RMS rises | | Horizon Whl +15.7 dB, Dual Gates +15.4, Silk Pad +13.3 (the hardware clip says +12.5), Synth Orchestra +13.0, Syn Flute +11.8 |

The sweep rows of `docs/research/data/program-sweep.json` and `program-coverage.md` were not regenerated (they document the pre-flip default); the per-program level shifts of up to 23 dB are the largest open risk of the flip and need hardware clips of those programs (Follow-ups).

## 7. Filter law investigation (2026-11-14, follow-up 1)

Data: `docs/research/data/synth_filter_law_results.json` (score per clip per configuration). All runs: the 15 clips of section 2, variant `strip+st+tbl`, same notes and velocity 100; the only changes are the named cutoff factors / envelope scale.

**What is established about the stages** (VERIFIED, [synth-stage-equations](synth-stage-equations.md), [synth-filter-type-map](synth-filter-type-map.md)): the stage turns `R = tan(pi fc / F)` into the natural frequency of `1 / (s^2 + P s + 1)` per section; the first -3 dB point sits at `x3(P) * fc` (`P = 2`: 0.64 fc, `P = sqrt 2`: fc, `P = 0.3`: 1.4 fc for one section). What stays unknown (ASSUMED) is the number the panel word stands for: the displayed Hz.

| Hypothesis | Test | Result |
|---|---|---|
| Displayed Hz is the first -3 dB corner (natural fc = display / x3(P)), no per-wave factor | all row factors 1.0 plus the corner law (`x1n`) | **rejected**: mean score 4.80 against 4.11; with the old rows on top 4.45; opposite sign 4.60 |
| One LP24 factor for every wave | LP24 1.3 | **rejected**: `lead_smooth` 0.69 to 2.53, `ag_zap_saw` 6.93 to 13.5 (the two LP24 clips want 1.75 and 0.85 as total factors, so the SHAPE row and the saw row differ at the same type) |
| The saw disagreement (`filter_bank` 1.25, `ag_zap_saw` 0.8) is a filter type effect | BP factor 1.4, saw row 0.85 | **supported**: `filter_bank` presence -2.6 to -0.7 dB, brilliance -4.3 to +1.0, RMS +1.6 to +0.3, `ag_zap_saw` 6.93 to 6.59; no other clip changes |
| LP12 needs a different factor from LP24 | LP12 grid 0.7 / 1.0 / 1.2 / 1.4 / 1.7 (SUPER row rescaled so SUPER x LP12 = 1.5) | **supported for 3 of 5 LP12 clips**: `sweep_12db` 4.59 to 3.04, `mega_line` 2.03 to 1.87, `sine_pluck` 7.03 to 6.40; `lead_crunch` +0.05, `pad_silk` unchanged. 1.7 is better for the first three (2.52, 1.26, 5.80) and worsens `pad_silk` (8.24) |
| The SUPER / LP12 conflict (`pad_silk` 0.5 against `mega_line` 2.0) is a cutoff factor | `NS4_ENVOCT` scan of the filter envelope octaves per full amount (8 default) | **it is the filter envelope**: `pad_silk` (env amount 0.346, decay 9.5 s, filter velocity on) reaches score 1.50, presence -0.2 dB at 2 octaves per full amount, but every other clip with an envelope wants 6 to 8 (`ag_zap_saw` 24.1 at 2, 11.1 at 4, 6.6 at 6.2; `sweep_12db` 17.1 at 2; `filter_bank` 8.5 at 2). No monotonic law in the amount, the decay or the type separates `pad_silk` from them; the replay velocity (100, amp velocity 0 and filter velocity 1 on this program) is the open suspect |

Adopted (`synth/p_table.rs::type_cutoff_scale`, applied with the P table): LP12 1.4, BP 1.4, LP24 / Ladder24 / HP 1.0 (EMPIRICAL); saw and pulse rows 0.9 to 0.85, SUPER row 1.5 to 1.07 (SUPER x LP12 = 1.5, unchanged for the two SUPER clips).

| Clip | Role | Before (PR 261 default) | After | Legacy |
|---|---|---|---|---|
| lead_smooth | fit | 0.69 | 0.69 | 4.93 |
| lead_crunch | holdout | 1.18 | 1.23 | 8.33 |
| pad_silk | fit | 7.63 | 7.62 | 6.53 |
| mega_line | holdout | 2.03 | 1.87 | 2.98 |
| sci_fi_80s | fit | 0.24 | 0.24 | 2.03 |
| filter_bank | fit | 4.46 | 4.37 | 3.15 |
| phat_3osc | holdout | 2.91 | 2.91 | 3.21 |
| sweep_12db | holdout | 4.59 | 3.04 | 21.38 |
| sine_pluck | holdout | 7.03 | 6.40 | 5.99 |
| ag_blip_saw / square | fit | 5.31 / 3.29 | 5.31 / 3.29 | 5.28 / 3.29 |
| ag_zap_saw | holdout | 6.93 | 6.59 | 8.99 |
| ag_supersaw_0 / 3 / 10 | holdout | 4.38 / 6.02 / 4.96 | unchanged | 11.31 / 6.83 / 4.96 |
| **mean (15)** | | **4.11** | **3.93** | 6.6 |

Cross-validation caveat: with 15 clips and five LP12 clips, the LP12 factor is chosen by the grid on `sweep_12db`, `mega_line` and `sine_pluck`, which were hold-outs of the earlier fit and are therefore not independent any more; `lead_crunch` (SHAPE, LP12) and `pad_silk` are the clips that argue against a larger factor (1.7). The BP factor has one clip (`filter_bank`, fit for the saw row); `ag_zap_saw` is the independent check for the saw row (improves). `render_hash` is unchanged (`b1c91b53...c9aa`): its scenarios use the idle row, Ladder24 (factor 1.0) and sample layers.

Still open: `pad_silk` (13 dB too bright, an envelope or velocity issue, not the cutoff factor), `filter_bank` and `sine_pluck` stay above legacy (4.37 against 3.15, 6.40 against 5.99; `sine_pluck` presence -17 dB is the SHAPE SINE words / wheel morph of the program, not a filter effect), the knob to strip word law needs a hardware run with an active voice (the ARM sends the cutoff only with a sounding voice, [synth-voice-control](synth-voice-control.md) s3), and `x3` above shows why the answer cannot come from the stage equations alone.

## Knob-law re-score (D57, 2026-11-14)

Harness restored as `crates/nord-tools` (`ag_synth`, `render_hash`, `program_sweep`, ... as release binaries, D57). All 15 clips re-run with the variant `strip+st+tbl+kl` (the default plus `--synth-knob-laws`, [synth-knob-laws](synth-knob-laws.md)); `python tools/synth_validate.py summary result.json --baseline strip+st+tbl` scores an increment against the default (mean score better, no clip over `WORSE_DB`; the majority rule only applies against `legacy`).

| Clip | Role | Legacy | Default (`strip+st+tbl`) | + knob laws | Delta | Attack ms default / kl (hw) | Presence dB default / kl | RMS dB default / kl |
|---|---|---|---|---|---|---|---|---|
| lead_smooth | fit | 4.93 | 0.69 | 0.69 | 0.00 | 35.0 / 35.0 (19.8) | -2.0 / -2.0 | -0.9 / -0.9 |
| lead_crunch | holdout | 8.33 | 1.23 | 1.23 | 0.00 | 65.3 / 65.3 (65.5) | +2.4 / +2.4 | +2.2 / +2.2 |
| pad_silk | fit | 6.53 | 7.62 | 7.62 | 0.00 | 42.4 / 42.4 (61.2) | +13.2 / +13.2 | +1.1 / +1.1 |
| mega_line | holdout | 2.98 | 2.53 | 2.53 | 0.00 | 30.8 / 30.8 (59.1) | -3.1 / -3.1 | -3.2 / -3.2 |
| sci_fi_80s | fit | 2.03 | 1.39 | 1.39 | 0.00 | 80.7 / 80.7 (97.1) | +1.4 / +1.4 | -10.4 / -10.4 |
| filter_bank | fit | 3.15 | 4.37 | 4.33 | -0.04 | 15.9 / 14.8 (11.0) | -3.9 / -3.3 | +0.6 / +0.5 |
| phat_3osc | holdout | 3.21 | 2.59 | 2.59 | 0.00 | 106.4 / 106.4 (26.1) | +3.3 / +3.3 | +0.9 / +0.9 |
| sweep_12db | holdout | 21.38 | 3.04 | 1.97 | -1.07 | 64.1 / 64.2 (46.8) | -5.1 / -2.4 | +7.6 / +7.3 |
| sine_pluck | holdout | 5.99 | 6.40 | 6.52 | +0.12 | 60.0 / 57.3 (77.8) | -18.4 / -18.6 | +5.3 / +5.3 |
| ag_blip_saw / square | fit | 5.28 / 3.29 | 5.31 / 3.29 | same | 0.00 | none | +0.0 / +0.1 | -14.1 / -14.0 |
| ag_zap_saw | holdout | 8.99 | 6.59 | 6.59 | 0.00 | none | +1.7 | -4.3 |
| ag_supersaw_0 / 3 / 10 | holdout | 11.31 / 6.83 / 4.96 | 4.38 / 6.02 / 4.96 | same | 0.00 | none | +1.0 / -1.5 / -0.7 | -7.9 / -4.8 / -6.6 |
| **mean (15)** | | 6.61 | **4.03** | **3.96** | -0.07 | | | |
| **mean holdout (9)** | | 8.22 | 4.19 | 4.09 | -0.10 | | | |

Result: the increment passes the D22 bar (`summary --baseline strip+st+tbl`: mean gain +0.07 over 15 clips, +0.11 over the 9 hold-outs, 0 clips over the bar). Only the three Part A clips whose stored knob maps to a different table step than the displayed-time search move; the lesson clips are layers built in code and carry no knobs. `sweep_12db` (hold-out) gains 1.07 (presence -5.1 -> -2.4 dB), `sine_pluck` loses 0.12, `filter_bank` gains 0.04. The path is therefore the default (D57).

Findings of the re-run, not caused by the knob laws:

- The default (`strip+st+tbl`) mean is 4.03, not the 3.93 of the table above: `mega_line` 1.87 -> 2.53 and `sci_fi_80s` 0.24 -> 1.39 moved since that table (PR 292 and PR 319, bisected and refitted in section 9, D59), the other clips match it.
- Against `legacy` the default no longer clears the bar of `summary`: `sci_fi_80s` RMS -0.4 -> -10.4 dB and `sine_pluck` RMS +2.0 -> +5.3 dB (the bar counts RMS for Part A clips). The scores themselves are better than legacy on 9 of 15 clips.

## 9. Regression bisect and refit (D59, 2026-11-14)

The default of the D57 re-run (`strip+st+tbl`, knob laws on) scored 3.96 against the 3.93 of the PR 275 table, with two clips moved and two RMS bars missed against legacy. Bisect (one release `ns4` per commit of the first-parent history after PR 275, same two clips, variant `strip+st+tbl`, replay velocity 100):

| Clip | Last good | First bad | Cause |
|---|---|---|---|
| `mega_line` (1.87 to 2.53) | 83c6c425 PR 297, 1.87 | 9d7e4045 PR 292 (`synth-stages-rest-5`), 2.53 | SUPER wave 1 (`Super1`) became a stage model; before it played the legacy saw at the idle row, now it plays the stage at the idle block (gain 1.0, factor 1.0) |
| `sci_fi_80s` (0.24 to 1.39) | 54644a52 PR 321, 0.24 | 3a5143fc PR 319 (`synth-stages-rest-9`), 1.39 | Ladder24 stage `0x12549D` replaced the older filter: RMS +0.0 to -10.4 dB, brilliance -0.1 to +3.3 dB; the MULTI gain 0.26 and the Ladder factor 1.0 were fitted for the old filter |

Other bisect points (all `mega_line` 1.87 / `sci_fi_80s` 0.24 unless named): PR 275 (270efa29), PR 286 (8d47618c), PR 295 (1d4167ef), PR 293 (9dd4ab28), PR 297; `mega_line` 2.53 from PR 292 on, `sci_fi_80s` 1.39 from PR 319 on (PR 309 cc925db5, PR 324 8c12e839, PR 337 18d036c3). The knob laws (D55 / D57) and the kFltFreq law are not involved: they do not touch these clips (the knob-law default is off in the bisect builds and `strip+st+tbl` reads the same on them).

Diagnosis: (1) stage model: both stages are bit exact; the change of the clip is the P block they now reach (rows tuned for what played before). (2) P block (ASSUMED ratios, weights, shape word): not the cause, the SHAPE and saw rows are untouched and their clips did not move. (3) Filter law: the tangent reading `R = tan(pi fc / fs)` is inconsistent with the displayed Hz ([synth-knob-laws](synth-knob-laws.md) s6, ARM law `i = knob - 52`, `R(i) = 0.36083 + 0.0079301 i`), but replacing the fitted type factors by it needs a hardware cutoff sweep per type (the displayed 277 Hz at knob 52 reads 4.8 kHz): not done, the type factors stay EMPIRICAL and only the Ladder24 one (the stage that reads `R` as a table position, D54) changed. (4) Knob laws: no effect on these clips.

Refit (grids per clip, rows rendered in parallel with `--p-table table:<row>=g/0/1/shape/cutoff`; the per-type factors through a temporary environment hook, removed):

| Row / factor | Old | New | Grid (score) | Clip |
|---|---|---|---|---|
| SUPER (wave 0) cutoff | 1.07 | 0.45 | 0.25 / 0.36 / 0.42 / 0.45 / 0.5 / 0.7: 4.69 / 1.97 / 1.88 / 2.36 / 3.08 / 5.24 | `pad_silk` (fit) |
| SUPER1 (waves 1 to 4) gain / cutoff | idle 1.0 / 1.0 | 1.0 / 3.2 | with SUPER 0.45: 2.6 / 3.2: 1.91 / 1.69; gain 1.6 worse (3.29 at factor 1.0) | `mega_line` (was hold-out) |
| MULTI gain | 0.26 | 0.75 | 0.26 / 0.7 / 0.9: RMS -10.4 / -1.3 / +1.2 dB, score 1.39 / 0.82 / 0.99 | `sci_fi_80s` |
| Ladder24 type factor | 1.0 | 0.8 | 0.5 / 0.65 / 0.8 / 1.0: brilliance -4.4 / -3.8 / -1.6 / +3.3 dB (with MULTI 0.9 for the last three), `phat_3osc` 2.59 to 2.63 | `sci_fi_80s`, `phat_3osc` |
| SHAPE SINE gain / cutoff | idle 1.0 / 1.0 | 0.5 / 3.0 | cutoff 1.0 / 2.0 / 3.0 / 4.0: 6.52 / 4.73 / 4.75 / 4.83; gain 0.55 = RMS +0.8, 0.45 = -1.0 dB | `sine_pluck` (one clip, no hold-out) |

`pad_silk` is the pad whose SUPER wave 0 level and cutoff are independent of the 3 lesson clips (filter open); the old conflict with `mega_line` came from the Super row serving wave 1 of that clip (the clip also contains a wave 0 oscillator, so the two rows interact: SUPER 0.4 / SUPER1 2.2 gave 2.17, 0.45 / 3.2 gives 1.69). Disclosure: `mega_line` is a fit clip of SUPER1 now, `sine_pluck` has no hold-out for the SHAPE SINE row, `sci_fi_80s` is the only Ladder24 clip with a filter in range (`phat_3osc` is open, 13 kHz), so these rows are EMPIRICAL and weak. BP stays 1.4 (`filter_bank`: BP 1.0 / 1.8 give 4.64 / 5.52 against 4.33); its error is not in the presence band.

15-clip table (default `strip+st+tbl`, knob laws on; before = D57 re-run on the merged branch, after = this PR; legacy for reference):

| Clip | Role | Legacy | Before | After | Presence dB (before to after) | RMS dB (before to after) |
|---|---|---|---|---|---|---|
| lead_smooth | fit | 4.93 | 0.69 | 0.69 | -2.0 / -2.0 | -0.9 / -0.9 |
| lead_crunch | holdout | 8.33 | 1.23 | 1.23 | +2.4 / +2.4 | +2.2 / +2.2 |
| pad_silk | fit | 6.53 | 7.62 | **2.36** | +13.2 / +2.2 | +1.1 / +0.5 |
| mega_line | holdout | 2.98 | 2.53 | **1.69** | -3.1 / -2.9 | -3.2 / -2.7 |
| sci_fi_80s | fit | 2.03 | 1.39 | **0.86** | +1.4 / +1.8 | -10.4 / -0.7 |
| filter_bank | fit | 3.15 | 4.33 | 4.33 | -3.3 / -3.3 | +0.5 / +0.5 |
| phat_3osc | holdout | 3.21 | 2.59 | 2.63 | +3.3 / +3.1 | +0.9 / +0.8 |
| sweep_12db | holdout | 21.38 | 1.97 | 1.97 | -2.4 / -2.4 | +7.3 / +7.3 |
| sine_pluck | holdout | 5.99 | 6.52 | **4.75** | -18.6 / -12.2 | +5.3 / -0.0 |
| ag_blip_saw / square | fit | 5.28 / 3.29 | 5.31 / 3.29 | same | | |
| ag_zap_saw | holdout | 8.99 | 6.59 | 6.59 | +1.7 | -4.3 |
| ag_supersaw_0 / 3 / 10 | holdout | 11.31 / 6.83 / 4.96 | 4.38 / 6.02 / 4.96 | same | | |
| **mean (15)** | | 6.61 | 3.96 | **3.40** | | |

`python tools/synth_validate.py summary result.json` against legacy: the default beats legacy on 11 of 15 clips (mean score gain 3.21, hold-outs 8 of 9 with gain 4.42), two are equal (`ag_blip_square`, `ag_supersaw_10`), two are worse (`filter_bank`, `ag_blip_saw` by 0.03), the D22 bar passes (no clip over 3 dB in presence, no Part A clip over 3 dB in RMS: `mega_line` presence -0.1 to -2.9). Still above legacy: `filter_bank` 4.33 against 3.15 (BP of saws, mid / low bands, not presence). `sine_pluck` presence is still -12.2 dB (the SHAPE SINE word, not the filter). `render_hash` whole `63923599...50a16` (D59).

## Stop point

Time box (about 120 min) reached after: the 15-clip set and the scoring tool, the per-wave table with the SHAPE law, the flip, the workspace checks and the sweep. Not done: second hold-out clips for Pure Square, SHAPE SQUARE, SHAPE SINE and MULTI (no program with a single such wave and a clean video window was found among the 67 Part A programs); the Part A programs that mix synth with organ or piano; the DSoundman LFO and pad clips; the Ladder24 filter (type 2 is not a low pass, `phat_3osc`, `sci_fi_80s` and `mega_line` use it or LP12 with the strip law as is); a per-filter-type cutoff factor.

## Implications

- The SHAPE oscillator needs no second oscillator once the shape word follows Osc Ctrl; the P block of the firmware is probably just `ctrl` plus a level, not an octave partner.
- Level and cutoff factor are separable only per program, not per wave: SUPER and saw rows have clips with opposite optima, which points at the filter (slope or cutoff map of LP12 / LP24 / BP) rather than the oscillators.
- The SUPER stage reproduces the flat level against Detune that the legacy 7-voice saw does not.

## Follow-ups

1. Per-filter-type cutoff factor (LP12 / LP24 / BP / Ladder24): pad_silk, mega_line, filter_bank and ag_zap_saw disagree by a factor 2 to 4 on one wave, which is the signature of a filter-law error; test with the oracle response at the displayed cutoffs.
2. Hardware level of the biggest movers of the sweep (Chip Bass, Danceflr Peak MW, Horizon Whl, Dual Gates): a Part A segment or a lesson clip per program.
3. A clip with a known velocity (a controller-recorded note) to fix the saw gain, and a filtered Pure Square clip for the pulse row.
4. The DSoundman LFO and pad-alone clips once Soft Pad 1's panel values are read.
5. The legacy 7-voice Super Saw (waves 1 to 4) and SHAPE SINE waves 3, 4 still play legacy waveforms at unit P; model the stages before giving them rows.

## Reproduce

```bash
cargo build --release -p nord-core --bins -p nord-tools --bins
python tools/synth_validate.py run --video <PartA.webm> --backup <factory.ns4b> --workdir <scratch> \
    --ag-dir <dir with s0OhmMpbscU.wav 0NMvKWDc-Lo.wav MjkV3qz8Gqk.wav> --out result.json
python tools/synth_validate.py summary result.json
python tools/synth_validate.py summary result.json --baseline strip+st+tbl   # an increment over the default
# fit one row: replaces "table" in the table variants, e.g. the SUPER row
python tools/synth_validate.py run ... --only pad_silk mega_line --variants strip+st+tbl \
    --p-table "table:super=2/0/1/0.5/1.5"
ns4 render-patch "Program/Bank A/Smooth Lead.ns4p" --backup <factory.ns4b> --events events.json --wav out.wav --synth-p table:shape=0.6/0/1/1/1.75
```

The lesson WAVs are `ffmpeg -i clip.webm -vn -ac 2 -ar 48000 <id>.wav`.

## 10. Verified P words against the 15 clips (`--synth-p verified`, D62)

The ARM writes the oscillator words per wave ([synth-p-writer](synth-p-writer.md)). The opt-in table `--synth-p verified` (`synth/p_words.rs`) takes the gain of the rows that a single oscillator describes from them (`1.6 x P[16] x weight`; the saw calibrates the level, EMPIRICAL) and keeps the cutoff factors and the MULTI / SUPER rows. Run: `python tools/synth_validate.py run ... --variants strip+st+tbl --p-table "table:saw=0.4/0/1/0.5/0.85;pulse=0.28/0/1/0.5/0.85;shape=0.4/0/1/1/1.75;shapesaw=0.4/0/1/0.5/0.85;shapesq=0.4/0/1/0.5/1;shapesine=0.4/0/1/1/3"` against the default (same clips, same binaries).

| Clip | wave | default score / RMS | verified score / RMS |
|---|---|---|---|
| lead_smooth | shape | 0.69 / -0.94 dB | 0.69 / -4.47 dB |
| lead_crunch | shape | 1.23 / +2.24 dB | 1.23 / -1.21 dB |
| sweep_12db | shape square | 1.97 / +7.28 dB | 2.72 / +1.33 dB |
| sine_pluck | shape sine | 4.75 / -0.04 dB | 4.75 / -1.98 dB |
| ag_blip_square | pulse | 3.29 / -13.97 dB | 3.29 / -13.65 dB |
| the other 10 clips | | identical | identical |

15-clip mean score 3.40 (default) against 3.45 (verified), hold-out mean gain over legacy 4.42 against 4.33; the D22 bar against legacy holds (0 clips over it). Reading: the three SHAPE rows are level-consistent with the words (0.4) for `lead_crunch` and `sweep_12db` (RMS error 2.2 / 7.3 dB down to 1.2 / 1.3 dB) but `lead_smooth` loses 3.5 dB because its EMPIRICAL gain 0.6 and cutoff factor 1.75 were fitted jointly (the cutoff factor must be refitted with the verified gain); `sweep_12db` loses 0.75 in score for the same reason (its filter, not its level). The Pure Square level of the pitch-blip clip is unaffected (velocity unknown). No flip: the table is not better than the default by score (+0.05 worse), it only replaces fitted levels by verified ones; the default keeps the EMPIRICAL table.

## 11. The verified words in the MULTI / SUPER stages (D70)

`--synth-p verified` now also feeds the unison stages with the ARM words ([synth-p-writer](synth-p-writer.md) s8): sixteen SUPER voices at the verified ratios and equal weights, the two-voice (or three-voice) MULTI of the record, the spread word `s5 = Osc Ctrl` (scale 1.0, was the ASSUMED 0.1) and pseudo-random start phases. The rows of the unison waves carry only the level constant (`GAIN_PER_P16 = 1.6`); the cutoff factors are the EMPIRICAL ones until refitted. Same 15 clips and binaries as s9 and s10 (`strip+st+tbl`, `--p-table verified:<rows>`, one release build per run), default = the EMPIRICAL table (mean 3.40).

Cutoff factor grids at the verified level 1.6 (score, presence dB, RMS dB):

| Row, clip | factor | score / presence / RMS |
|---|---|---|
| SUPER, `pad_silk` (fit) | 0.30 / 0.45 / 0.70 / 1.0 / 1.5 / 2.2 | 2.60 / 1.92 / 4.72 / 6.78 / 8.68 / 9.93; presence -4.3 / +1.8 / +8.1 / +12.4 / +15.8 / +17.7; RMS -5.1 .. -3.7 |
| SUPER, `mega_line` (hold-out, MULTI + SUPER 1 stay at their rows) | same | 2.88 / 2.98 / 3.12 / 3.33 / 3.72 / 4.22 (default table 1.69) |
| SUPER, `ag_supersaw_0 / 3 / 10` (filter open, factor irrelevant) | | 6.05 / 5.55 / 4.91 (default table 4.38 / 6.02 / 4.96) |
| MULTI, `sci_fi_80s` (fit) | 0.7 / 1.0 / 1.4 / 2.0 / 3.0 | 4.12 / 2.17 / 1.77 / 1.80 / 1.42 (default table 0.86); RMS -4.1 .. -5.3 |
| MULTI, `mega_line` | same | 2.64 / 2.82 / 2.98 / 3.15 / 3.30 |

Reading: (1) the SUPER cutoff factor that the words prefer on `pad_silk` is **0.45**, the same value the old fit found with the ASSUMED stage, so the factor was not compensating the stage (score 2.36 to 1.92); (2) the verified stages are 4 to 5 dB below the hardware level on `pad_silk`, `sci_fi_80s` and `lead_smooth` at 1.6, while `sweep_12db` is 1.3 dB high: one global constant does not fit all rows, so `GAIN_PER_P16` is the weakest EMPIRICAL number left and was not changed here; (3) `mega_line` (two SUPER waves and a MULTI) gets worse by about 1 score with every setting tried (2.64 best against 1.69): its EMPIRICAL rows (SUPER1 factor 3.2, MULTI gain 0.75 and factor 1.4) were fitted against the placeholder stages, and they interact; (4) the Super Saw lesson clips move 4.38 / 6.02 / 4.96 to 6.05 / 5.55 / 4.91: at Detune 0 the hardware spectrum is cleaner than the coherent-ish pseudo-random start of the model; the lesson clips fix no absolute level.

Decision: **no flip**. The six unison clips (`pad_silk`, `mega_line`, `sci_fi_80s`, `ag_supersaw_0 / 3 / 10`) get worse by 2.2 score in total at the best setting of each grid (the other nine clips are unchanged from s10, +0.05), so the 15-clip mean is about 3.6 against 3.40 and the D22 bar (mean better than the default) is not met; the words are VERIFIED, the rows around them are not refit. `--synth-p verified` stays opt-in (CLI, GUI Firmware bar dropdown "ARM words (verified)", `FirmwareOptions::default` unchanged, golden hashes unchanged). Not done (the full 15-clip re-run of a level grid 2.0 / 2.4 / 2.8 did not finish in the time box: every run re-extracts the video frames, about 30 min each on a loaded machine; the grids above are the 5-clip and 3-clip runs): a joint refit of the level constant against all rows with cross-validation (fit clips against hold-outs), the kFltFreq law as a replacement of the type factors (needs a hardware cutoff sweep per type, D59), a clip of SUPER 1 / SUPER 2..4 with a filter in range.

## 12. Horizon Whl (A:16) and the SUPER / MULTI words as default (D81, 2026-11-15)

User report: the factory program `Horizon Whl` sounds far from the hardware; the Part A video shows it from 287 s (OLED `A:16`, still on screen at 330 s). The program has **no hardware clip in the 15-clip set** and moved +15.7 dB at the D59 flip (s6).

**The program** (decoded with `ns4 convert`): zone split at key 53 (`kb_zone` synth A 4 = lower, B and C 6 = upper). Synth A: SUPER wave 0 (Detune 1.4), **LP24 277 Hz**, -12 semitones, amp attack 0.5 ms, filter envelope amount 0.39 (decay 340 ms), arpeggiator on. Synth B: SUPER wave 0 (Detune 4.3), unison 3, LP24 554 Hz, -12 semitones. Synth C: MULTI (category 5 wave 4), LP12 98 Hz, drive 0.33, +12 semitones, arpeggiator on. Piano A (White Grand) at level 0 with the **mod wheel** raising it (wheel morph 1.0); wheel morphs also move the synth cutoffs (+0.08 A, +0.10 C), the C level and the delay mix of the C effects (delay ping-pong, reverb, EQ bass -15 dB). The video plays low arpeggio notes on keys 28 to 37 (E1 to C#2, which sound at 20 to 35 Hz through the -12 transpose, the hardware spectrum peaks at 21.2 / 27.1 / 43.2 / 54.6 Hz) with right-hand notes above; the mod wheel is up from about 5:30 (piano meters lit).

**Clip** `horizon_whl` (hold-out): `tools/emu_vs_hw.py` segment `synth_horizon`, 290 to 320 s, the video-confirmed notes (`video_ref.py notes`: 891 audio notes, 396 confirmed over 60 s). Cross-check with the Transkun stream of PR 381 (275 notes in the same 30 s, uniform velocity 100): default RMS +5.5, presence -17.5 dB, mid -11.2; legacy RMS -7.8, presence -1.7, so the note source does not change the finding. The arpeggiator is not run by `render-patch`; the transcribed (sounding) notes are replayed instead.

**Diagnosis** (hardware minus emulator, absolute level per band over the clip): the default was **+12 dB** hot at 20 to 120 Hz, matched at 120 to 500 Hz and **10 to 15 dB low above 500 Hz** (per-second levels, 20 to 120 / 120 to 500 / 500 to 8000 Hz: hardware -30 / -33 / -42 dBFS, default -18 / -34 / -52, legacy -35 / -38 / -50). A table run on the single stage knobs cannot fix it: SUPER cutoff factor 0.45 / 0.9 / 1.4 gives mid -20.1 / -16.3 / -14.3 dB, SUPER gain 0.5 / 1.0 gives mid -13.5 / -15.6 (the filter is not the limit: with the filter open the SUPER stage of layer A still falls 6 dB per octave above its 20 Hz fundamental, 33 dB down at 1 to 2 kHz, where a plain saw falls 3 dB per band octave). The ASSUMED unison constants of the SUPER stage (voice ratios and weights, spread scale 0.1, `p_stage_words.rs`) produce too few partials at deep-bass pitch. With the VERIFIED ARM words (sixteen voices at ratios 0.9576 .. 1.0443, equal weights 0.7071, spread = Osc Ctrl) and the cutoff factor of the row scaled 4.4 for LP24, score 10.23 to 1.57.

**Root cause:** the D22 default read the SUPER and MULTI stages with ASSUMED constants; `pad_silk` (LP12, mid-register notes) hid the error because its fit absorbed it into the gain 2.0 and the cutoff factor 0.45, and no clip had a SUPER LP24 patch at deep-bass pitch. D70 had the words as opt-in because the neighbouring rows were fitted against the placeholder stages.

**Fix (D81):** `PTable::FITTED.words = true`: the MULTI / SUPER 0 / SUPER 1 to 4 stages read the verified words in the default; rows refit jointly on `pad_silk`, `mega_line`, `sci_fi_80s`, `horizon_whl` (CLI grids `--synth-p verified:super=g/0/1/0.5/c;multi=..;super1=..`): SUPER 2.4 / 0.45, MULTI 3.8 / 2.0, SUPER1 1.6 / 2.2 (gain / cutoff factor); SUPER wave 0 on **LP24** takes the type factor 4.4 (`p_type::row_type_cutoff_scale`, total 1.98; grid on the row at LP24 factor 1: 0.45 / 1.0 / 2.0 / 4.0 gives score 3.46 / 3.5 / 1.42 / 3.44).

Result, 30 s clip (emulator minus hardware, bands level-free):

| Variant | score | sub | low | mid | presence | brilliance | RMS | centroid |
|---|---|---|---|---|---|---|---|---|
| default before | 10.23 | +0.8 | -8.4 | -20.1 | -16.0 | -5.8 | +9.9 | -97 Hz |
| legacy | 1.80 | -0.4 | +1.6 | -2.2 | -1.4 | -3.3 | -6.9 | +8 Hz |
| **default after (D81)** | **1.57** | +0.2 | -1.3 | -2.2 | +0.7 | +3.5 | +1.1 | -22 Hz |

(60 s, whole demo section, default before: score 15.06, RMS +10.5, presence -25.1; `--synth-p verified` alone: 11.07, RMS -2.9, presence -20.6; `--legacy-all` 6.08, `--no-synth-knob-laws` 15.24, `--no-synth-stages` 10.23, `--legacy-routing` 15.48: none of the other switches is the cause. Attack 78 ms on the hardware against 98 ms: the arpeggio retrigger, not modelled.)

16-clip set, default `strip+st+tbl` before to after (score; presence and RMS in dB; `python tools/synth_validate.py run ... --variants strip+st+tbl`, cached video windows):

| Clip | before | after | presence before / after | RMS before / after |
|---|---|---|---|---|
| lead_smooth, lead_crunch, filter_bank, phat_3osc, sweep_12db, sine_pluck, ag_blip x2, ag_zap_saw | unchanged | unchanged | | |
| pad_silk | 2.36 | 1.92 | +2.2 / +1.8 | +0.5 / -1.0 |
| mega_line | 1.69 | 1.60 | -2.9 / +3.1 | -2.7 / -5.3 |
| sci_fi_80s | 0.86 | 1.09 | +1.8 / +1.3 | -0.7 / +3.4 |
| **horizon_whl** (new) | 10.23 | **1.57** | -16.0 / +0.7 | +9.9 / +1.1 |
| ag_supersaw_0 / 3 / 10 | 4.38 / 6.02 / 4.96 | 6.06 / 5.56 / 4.92 | +1.0 / -1.5 / -0.7 to +2.8 / -0.8 / 0.0 | lesson clips: no RMS bar |
| **mean (16)** | 3.82 | **3.34** | | |

D22 bar against the previous default: no Part A clip worse by more than 3 dB in presence or RMS (largest: `mega_line` RMS +2.6, `sci_fi_80s` RMS +2.7); the mean of the 16 improves 3.82 to 3.34, 13 of 16 clips beat legacy (mean gain 3.01, hold-outs 10 of 10 with 3.95); the **15 original clips** read 3.40 to 3.46 (`ag_supersaw_0` 4.38 to 6.06 is the cost: the Super Saw lesson clip with Detune 0 reads 11 dB low in RMS, lesson RMS is not a level evidence). Disclosure: `horizon_whl` is both the new clip and one of the four clips the rows were refit on (no independent hold-out for the LP24 SUPER factor; one clip, ASSUMED for other LP24 SUPER programs). Not covered: the mod wheel (played up from about 5:30, the clip starts with the wheel down), the arpeggiator timing (replayed notes, not the arp), the piano layer (level 0 until the wheel), the 20 to 60 Hz region of the recording chain (the emulator is sub-heavy relative to the hardware by 3 dB after the fix).
`Leaky Stabs II` (A:46), white-noise (D83, `leaky_stabs` (`emu_vs_hw_segments.json` `synth_leaky`, 1275 + holdout): SHAPE 0.82, PURE white noise 0.32, Ladder24 2500 filter-env decay 1.21 0.74, release ms. Notes: audio-to-MIDI estimate `video_ref.py` (552 events, many false) (162 events, recall audio, median 66); replayed, deltas (dB re RMS), `tools/emu_vs_hw.py --only synth_leaky`.

(audio-estimated pres brill |---|---|---|---|---|---|---|---|
(noise idle 1.0) -3.0 -3.5 +7.0 +12.1 +1317 5.19 noise 0.4 (D83) -1.9 +0.1 +4.3 +7.5 +502 3.19 only -1.6 -1.8 +0.3 +1.5 +129 1.49 only (idle) -6.4 -7.8 -1.2 +9.4 +15.4 +2405 8.05 x 0.25 0.12 0.06 (idle row) +4.7 +3.2 +2.5 +2.3 +8.3 +4.6 +2.5 3.47 2.32 1.75 1.53 noise 0.4 + levels squared -1.6 -1.8 +0.3 +2.7 +3.2 +194 1.93 notes: 7.53 before, 5.66 noise (D83), 4.50 squared alone 4.09 recall drops stab chords, shape less reliable above). Other variants (`legacy` chain, `--no-synth-knob-laws`, verified`, `--firmware-fx`, `--firmware-routing`) less 0.1 except `legacy` (2.09, RMS: unusable level). decay key-up (release drop T60 36 s): performance keeps ringing, metric usable this clip.

Findings: (1) right (A alone within every band); (2) noise 12 loud 1.5 8 kHz bands, 8 (0.4 1.0) idle P block: give noise (VERIFIED, [synth-p-writer](synth-p-writer.md) s3); (3) rest (about 8 between 0.25 0.12 rows) fits squared `(v/127)^2` (as organ handlers), but handler (`0xC0114588`, ids `0xA4..0xA6`: `v` rounded, float constant `[this+0xB4]`, stored 16-bit integer `[ip+0x48]`) only read, run, DSP side traced: **not applied** (OQ22). Reproduce: tools/emu_vs_hw.py --video <Part A> --backup <backup> --only synth_leaky`.
