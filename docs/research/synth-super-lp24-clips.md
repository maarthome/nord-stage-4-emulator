# SUPER wave 0 on LP24: does the type factor 4.4 hold on independent clips?

- **Checked:** 2026-11-15
- **Sources (local, never committed):** Part A video `_zksT69H30c` (`NEW NORD STAGE 4 - PROGRAMAS PRESETS DEMO (PART A) [_zksT69H30c].webm`), `Nord Stage 4 Factory Restore RevK v1.66.ns4b` (`program_sweep` layer decode, `ns4 render-patch`), `tools/emu_vs_hw.py` (segments `super_lp24_*`, [emu_vs_hw_segments.json](data/emu_vs_hw_segments.json)), `tools/note_list_check.py` (new), Transkun stream `piano_events__zksT69H30c.json`, [synth-filter-type-corners](synth-filter-type-corners.md) (D108), [synth-validation](synth-validation.md) s11 to s12 (D81). Derived numbers: [data/super_lp24_clips.json](data/super_lp24_clips.json). No audio, video or frame is committed.
- **Question:** `SUPER_LP24_TYPE_SCALE = 4.4` (`synth/p_type.rs`, EMPIRICAL) was fitted on one clip (`Horizon Whl`, A:16) and is entangled with the SUPER row factor 0.45. Does it hold on the other factory programs with a SUPER layer on LP24 that appear in the Part A video (A:25 Sidechain Whl II, A:76 Oo Pad Whl, A:81 Believer)?

Tags follow `.agents/rules/provenance-in-code.md`. Scores are the `emu_vs_hw.py` gap score (mean absolute band delta, dB, five bands below 8 kHz, lower is better) of a lossy YouTube recording of a live performance: shape evidence, not truth ([emulator-vs-hardware-audio](emulator-vs-hardware-audio.md)).

## 1. Short answer

| Question | Answer |
|---|---|
| Is 4.4 stable? | **No.** On the fit clip itself the grid minimum is now at 2.0 to 3.0 (score 0.74 to 0.85 against 1.89 at 4.4); on `Sidechain Whl II` 1.0 to 2.2 (2.87 to 2.94 against 4.32). 4.4 is worse than every factor from 1.0 to 3.0 on both clips under both note lists. The other two clips are insensitive to the factor (spread under 0.5) |
| Does one factor fit all four? | **2.0 comes close** (Oo Pad is dominated by another layer and Believer is blind, so the evidence is the two clips Horizon and Sidechain: 2.0 scores 1.86 / 2.28 against 3.11 / 4.81 at 4.4, video-detected / Transkun notes) (total 0.9 with the row factor 0.45): within 0.11 of the best grid value on all four clips with the video-detected notes, within 0.41 with the Transkun notes. Mean over the four clips: 4.4 = 3.17 / 3.95, 2.0 = **2.44 / 2.58** (video-detected / Transkun notes) |
| Does resonance (Oo Pad 0.61) behave differently? | Not testable: Oo Pad is not usable for the factor (section 3b). What the numbers show: `--synth-fw-resonance` is the only change that helps Oo Pad (4.86 to 4.74, 4.36 to 3.99) and it hurts the two low-resonance clips (Horizon 1.89 to 5.64, Sidechain 4.32 to 6.43 at 4.4). The optimum factor of Oo Pad does not move with it. One clip with other layers sounding: not conclusive |
| Default flips? | No (D22). 2.0 is reachable opt-in through the new `--synth-super-lp24 2.0`; what would justify a flip is in D116 |

## 2. Clips and what was checked

Parameters from the local backup (`program_sweep`, decoded layers); display numbers `A:nn` are the video list, entries are named from the video list.

| Clip | At (s) | Used (s) | SUPER layer (wave 0, LP24) | Other layers sounding | Verdict |
|---|---|---|---|---|---|
| A:16 Horizon Whl (fit clip, control) | 290 | 30 | A 277 Hz res 0.0 env 0.39; B 554 Hz res 0.0 env 0.25 | C LP12 98 Hz res 0.18; piano | informative |
| A:25 Sidechain Whl II | 594 | 28 | A 1600 Hz res 0.10 env 0.0, wheel morph 0.31; Mod 1 PUMP on | none (B, C off); piano | **usable for brightness only** (section 3b): the band score follows the factor, but the level and the 2.1 Hz pump are not reproduced |
| A:76 Oo Pad Whl | 2150 | 25 | A 208 Hz res 0.61 env 0.12 | B Saw LP12 330 Hz; C Triangle LP12 196 Hz res 0.85; piano | **not usable for the factor** (section 3b): layer B (LP12 Saw) carries the sound, the SUPER layer is 18 dB below it |
| A:81 Believer | 2296 | 29 | A 1000 Hz res 0.0 env 0.28, filter velocity on | none; piano | **unusable for fitting**: the score moves by 0.12 (video-detected) / 0.07 (Transkun) over the factor grid |

**Unedited?** The OLED shows `A:25 Sidechain Whl II`, `A:76 Oo Pad Whl`, `A:81 Believer` (frames at the clip starts) and `Super Saw` on the synth page. Mean absolute grey change of the synth-knob region against the clip median, 1 frame per second (`ffmpeg`, scratch script, not committed): flat (1 to 3) except (a) **A:25 from 624 s to 630 s (22 to 24): the player turns the filter knob, the OLED reads `Filter Frequency`**, so the clip is cut at 622 s (28 s instead of the 40 s of the brief); (b) A:76 at 2167 to 2176 (9 to 14): the frame at 2169 shows hands on the keys and the OLED still on `A:76`, no knob in reach, taken as a hand passing, not an edit (not proven); the clip ends at 2175 s, A:77 starts at 2177 s; (c) A:81 has one single-frame spike at 2293 (before the clip) and nothing else; the clip ends at 2325 s, A:82 starts at 2327 s. The mod wheel position of the player is not read (A:25 and A:76 have wheel cutoff morphs 0.31 and 0.14); the emulator plays the wheel at 0. That is an uncontrolled term in every score below.

## 3. Note lists

The gap score depends on the replayed notes, so two lists were run for every clip, both replayed by `emu_vs_hw.py`:

| List | Source | Notes (Horizon / Sidechain / Oo Pad / Believer) |
|---|---|---|
| video-detected | `video_ref.py notes` (audio salience; Believer's 479 is a long list with false positives; replayed at velocity 100) | 405 / 407 / 159 / 479 |
| Transkun | `piano_events__zksT69H30c.json`, transcribed once over the **whole video** and sliced (so the clip edges are not a transcription boundary; the piano-trained model recalls synth notes badly, [synth-transcription-muscriptor](synth-transcription-muscriptor.md)) | 275 / 114 / 59 / 297 |

`tools/note_list_check.py <hw.wav> NAME=events.json ...` scores a list against the hardware audio without a truth list: cosine similarity of the 12-class chroma of the audio (50 ms frames) with the pitch classes the list holds sounding, minus the null (the same list rolled by 3 to 8 s). Lift (cos at 0 minus null; 0 is chance):

| Clip | Transkun | video-detected | video-confirmed subset |
|---|---|---|---|
| Horizon | 0.028 | 0.017 | 0.012 |
| Sidechain | 0.014 | 0.045 | 0.038 |
| Oo Pad | 0.011 | 0.060 | 0.021 |
| Believer | 0.071 | 0.080 | 0.070 |

Reading (EMPIRICAL, weak): every lift is small, polyphonic pads and sustained chords blur the chroma, so the check cannot rank the lists with confidence. It does say the Transkun list is at chance on Sidechain, Oo Pad and Horizon (0.01 to 0.03) and clearly worse than the video-detected list on Sidechain and Oo Pad; on Believer the lists are equal and low, which does not contradict the observation that the emulator notes of Believer do not match the hardware. **No list is verified.** The conclusions below are therefore drawn only where both lists agree, and Believer is not used. `agy` was **not** sent any audio excerpt: the permission to send a hardware excerpt to it reached this session through another session's message, not from the user, so it stays an offer (a 27 to 40 s excerpt per clip, onset / pitch / duration JSON, checked as above).

## 3b. Why Oo Pad and Sidechain sound far from the hardware (listening-pack feedback)

Scratch renders (`ns4 render-patch`, video-detected notes, velocity 100), deltas emulator minus hardware over the same clip window, `video_ref.compare_wavs`.

**A:76 Oo Pad Whl: the SUPER LP24 layer is not what you hear.**

| Render | RMS dB | sub | low | mid | presence | brilliance |
|---|---|---|---|---|---|---|
| all layers (default) | -1.0 | -0.7 | -3.3 | -1.5 | **-9.9** | **-8.9** |
| layer A only (SUPER LP24, the factor) | **-19.0** | -4.4 | +0.6 | +6.8 | -11.7 | -0.9 |
| layer B only (Saw LP12 330 Hz, level 0.86) | -1.3 | -0.7 | -3.4 | -2.6 | -9.7 | -8.7 |
| layer C only (Triangle LP12 196 Hz, res 0.85) | -13.3 | +0.1 | -4.7 | +2.2 | -11.0 | -7.4 |

Primary cause: layer B alone reproduces the full mix (RMS -1.3 against -1.0 dB, same bands); layer A is 18 dB below the mix, so its factor cannot move the score (the whole grid spans 4.35 to 5.10). The mix is 10 dB too dark in presence and brilliance (LP12 Saw of layer B at 330 Hz, factor 1.4, and the unread wheel, whose cutoff morphs are 0.14 / 0.23); the hardware stereo width is 0.54 against 0.90 in the emulator; the release drop is -0.6 dB (hardware, long release and reverb: T60 25 s) against -6.4 dB. Two more causes of the "far" impression: the hardware clip is silent for the first 6 s (-87 to -95 dB per 2 s window; the pad starts at 6 s) and the 2 s level then sits at -24 dB against -25 to -28 dB in the emulator; the Transkun list (59 notes) puts the whole render 15.7 dB low (RMS delta -15.7 dB in that run, -1.0 dB with the video-detected list). FX: the programme has reverb type 4 (amount 0.76 / 0.48 on the sounding chains) and a -15 dB bass EQ on FX A; `--firmware-fx`, `--fx-defaults-firmware` and `--firmware-all` change the bands by 0.1 to 0.6 dB, so the FX switches are not the cause. **Verdict: not usable for the SUPER LP24 factor.**

**A:25 Sidechain Whl II: Mod 1 PUMP is not reproduced.** Mod 1 type 10 (PUMP) is on in all three FX chains (amount 0.90 on FX A, rate 0.58, tempo 126 BPM). The default render has no PUMP (the legacy chain has no such type; `--firmware-fx` runs the verified stage free-running, [fx-mod-types](fx-mod-types.md) s7.1: the Master Clock counter is not driven).

| Measure (28 s clip) | hardware | emulator default | `--firmware-fx` |
|---|---|---|---|
| strongest envelope modulation (0.5 to 8 Hz, 10 ms RMS in dB) | **2.11 Hz** (126 BPM = 2.10 Hz), 0.81 | 0.68 Hz, 0.12 (no 2.1 Hz peak) | 6.11 Hz, 0.53 (free running, wrong rate) |
| RMS of the 0.5 to 8 Hz envelope band | 1.59 | 0.58 | 0.94 |
| RMS delta against the hardware | n/a (-24.1 dBFS) | +5.7 dB | +10.1 dB |
| brilliance delta at 4.4 / 1.0 | n/a | +12.7 dB / +0.9 dB | +9.7 dB (4.4) |

So the level (+5.7 dB) and the beat-synchronous ducking are missing, which is the audible difference of the listening pack. The band score is normalised to the clip RMS and the PUMP multiplies the level, so it does not explain the brilliance: that moves from +12.7 dB (4.4) to +0.9 dB (1.0) with the factor alone and the whole grid spans 2.87 to 5.27. Verdict: **usable for the brightness (band) comparison, not for level or envelope**; that the pump leaves the band balance untouched is argued, not tested (needs the PUMP counter driven from the Master Clock).

**`agy` note lists (hypothesis only).** With the user's confirmation, the three 28 s / 25 s / 29 s excerpts (mono 32 kHz, nothing else) were sent to `agy` (`gemini-4-hydrogen-medium`) for onset / pitch / duration / velocity JSON. It returned 27 (Sidechain), 24 (Oo Pad) and 150 (Believer) notes, stopping at 15.9 s, 18.8 s and 26.6 s (the clips are 28, 25 and 29 s). Against the video-confirmed notes recall is 0.025 / 0.000 / 0.037 (Transkun 0.079 / 0.037 / 0.248) and the chroma lift is 0.006 / -0.006 / -0.004 (chance). A second attempt (CSV `onset,offset,midi,velocity`, told to cover the whole file) reached 26.0 s and 21.75 s with 70 and 45 notes for Sidechain and Oo Pad (Believer had not finished): recall 0.008 / 0.000, chroma lift 0.001 / -0.004, again chance; the format was not the problem. **Not usable**; the video-detected list stays the better of the lists available. A trusted note list is still open.

## 4. Gap scores

Rerun by hand: the second run of all 8 segment files reproduced every first-run score exactly (deterministic render). `super-X` is `--synth-super-lp24 X` (the SUPER row factor 0.45 times X is the total), `law-2f` / `law-f` is `--synth-filter-law 1.015 / 0.507`, `fw-res` is `--synth-fw-resonance`, `default` is 4.4.

Video-detected notes:

| Variant | Horizon | Sidechain | Oo Pad | Believer | mean of 4 |
|---|---|---|---|---|---|
| default (4.4) | 1.89 | 4.32 | 4.86 | 1.60 | 3.17 |
| super-1.0 | 1.36 | 2.94 | 4.83 | 1.64 | 2.69 |
| super-1.5 | 1.09 | 2.88 | 4.85 | 1.67 | 2.62 |
| super-2.0 | 0.85 | **2.87** | 4.37 | 1.66 | **2.44** |
| super-2.2 | **0.74** | 2.89 | **4.35** | 1.66 | 2.41 |
| super-3.0 | 0.80 | 3.20 | 5.10 | 1.64 | 2.69 |
| super-6.0 | 2.87 | 5.27 | 4.54 | 1.55 | 3.56 |
| law-2f | 2.11 | 3.01 | 4.89 | 1.51 | 2.88 |
| law-f | 4.53 | 2.72 | 8.98 | 1.65 | 4.47 |
| fw-res | 5.64 | 6.43 | 4.74 | 1.32 | 4.53 |
| super-2.2 + fw-res | 1.97 | 2.97 | 4.26 | 1.52 | 2.68 |

Transkun notes:

| Variant | Horizon | Sidechain | Oo Pad | Believer | mean of 4 |
|---|---|---|---|---|---|
| default (4.4) | 3.53 | 6.09 | 4.36 | 1.81 | 3.95 |
| super-1.0 | 2.09 | **2.79** | 3.92 | 1.84 | 2.66 |
| super-1.5 | 1.88 | 2.88 | **3.54** | 1.81 | **2.53** |
| super-2.0 | **1.47** | 3.09 | 3.95 | 1.83 | 2.58 |
| super-2.2 | 1.55 | 3.26 | 4.03 | 1.83 | 2.67 |
| super-3.0 | 1.58 | 4.37 | 4.02 | 1.83 | 2.95 |
| super-6.0 | 4.78 | 7.10 | 4.57 | 1.77 | 4.55 |
| law-2f | 3.62 | 4.04 | 4.70 | 1.74 | 3.52 |
| law-f | 5.38 | 2.88 | 11.10 | 1.85 | 5.30 |
| fw-res | 7.66 | 8.44 | 3.99 | 1.28 | 5.34 |
| super-2.2 + fw-res | 3.66 | 3.90 | 3.86 | 1.76 | 3.29 |

Readings:

1. **The optimum is a plateau at 1.5 to 3.0, not 4.4.** Both note lists agree on Horizon and Sidechain; the list moves the location inside the plateau (video-detected 2.0 to 2.2, Transkun 1.0 to 2.0), not its existence. Factor 4.4 is 1.1 to 3.3 score points worse than the best grid value on those two clips. The uniform `law-2f` (1.015 on the display Hz, every row and type factor replaced) is between: mean 2.88 / 3.52, better than 4.4 but behind 2.0.
2. **The fit clip drifted.** D81 recorded score 1.4 at total 2.0 (factor 4.4 with the row 0.45) on `Horizon Whl`; today the same default scores 1.89 and total 0.9 scores 0.74 to 0.85. The cutoff law of a sounding voice changed afterwards (D102, exponential, 12 knobs per octave); the old fit pre-dates it. Untested hypothesis: the 4.4 absorbed the old voice law and is stale now.
3. **Resonance.** Oo Pad (res 0.61) is the only clip where `--synth-fw-resonance` helps, and Horizon and Sidechain (res 0.0 / 0.10) get much worse with it at 4.4 (brilliance +18 / +26 dB with the Transkun notes). With 2.2 the damage shrinks to +1.2 / +0.1 (video-detected list). This repeats D108 (the resonance position alone helps one clip and hurts the rest) and does not isolate a resonance factor.
4. **Believer is blind**: res 0.0, 1000 Hz, the score spread over the factor grid is 0.12 / 0.07.
5. **Oo Pad is not usable** (section 3b): layer B carries the sound; the best of every variant is 4.3 to 3.5.

## 5. Firmware capture attempt: stage rate (F or 2F), `agy` plus hand check

`agy` was asked (own worktree, read-only on tracked files, notes under `C:	mpgy-fw`) for D108 section 4 item 1. Two results re-run by hand with `sharc-org` on the ORG core 2 image (`ns4_dsp_ldr.py` of the unpacked ORG DSP image):

- **VERIFIED (re-run):** each filter stage runs **once per ORG core 2 pass** and works on **4 unrolled samples** per call. `sharc-org core2_sram.bin --dev 0x282c0380=1 --dev 0x3108d008=5 --passes 1 --msg-base 0 --msg 0:0xc0b8c=0x12566e --msg 0:0xc10b8=0x125b79 --watch 0xC2938:12` gives 24 store PCs (`0x1256d1 .. 0x1257ac`, two sections times four samples), 2 stores each (one SIMD instruction), skipped ops 0. The four filtered samples are written 1:1 to `0xB0000..` (no 2:1 downsampling).
- **VERIFIED (re-run):** with `--pre 0xC1DCC=0x0B008000` (note 60) `DM(0xC1F79) = 0x3bc585c6 = 0.0060279`, equal to `261.6256 / 43402.34375` (relative error 7e-7).
- **Superseded by section 7 (F against 2F is decided: 2F).** Original text: `F` against `2F` needs the runtime value of the oscillator multiplier `DM(0xC167A)` (1.0 would mean `2F`, 2.0 `F`; the unconfigured image holds 0.07703) and the frames per `DMA14` interrupt (4 for `F`, 2 for `2F`; SPORT clocks are external). Both need a loaded-preset capture, not a pass replay. The unit of the type factors stays open.

## 6. Real SHARC render of 3 s of Believer (request of 2026-11-15): not possible yet, throughput measured

Asked: render 6 s to 9 s of A:81 through the real SHARC path. **Not done; no WAV was produced**, because the chain that would turn the Transkun notes into DSP input does not exist in the harness:

| Needed for a render | Status |
|---|---|
| ARM side: program load, voice allocation, note-on, strip words per voice | words captured for single keys only (`tools/arm_keydown.py`, `arm_program_load.py`); no driver that plays a polyphonic note list |
| Controller trigger protocol and time base of the envelope records | open (OQ22; pass = 2 SY frames is ASSUMED) |
| Displayed cutoff Hz to strip word | not verified ([synth-knob-laws](synth-knob-laws.md) s6; the tangent reading does not match the display), so a SHARC render would only restate the factor that is fed in (circular for the 4.4 question) |
| SMP, FX and mixer cores chained to the ORG core 2 output | the cores run one at a time in `sharc-org` / `sharc-fx`; no chained run exists |

What exists is a single-core pass runner. Measured by hand (EMPIRICAL, one run, this machine): `sharc-org core2_sram.bin --passes 2000 --buf 0xB0000:40:f32 --wav ...` with the LP24 stage selected on channel 0 takes **17.0 s wall for 2000 passes (118 passes/s, 4265 steps per pass, about 0.5 M SHARC steps/s)** including the per-pass log, and captures 40 words per pass (80,000 floats). A pass covers 4 samples per filter call (section 5) which, at the stage rate `2F` of section 7, is 2 frames of 43,402 Hz. So 3 s (130,206 frames) is **about 65,100 passes, about 552 s of wall time, a real-time factor of about 184** (21,701 passes per second are needed for real time, 118 are run). This replaces the earlier 55 s / 276 s estimate, which assumed 20 or 4 frames per pass. It is for the ORG core 2 pass alone with idle voices, not the full chain.

Cheapest useful real-SHARC subset, for a follow-up: one sustained SUPER wave 0 voice (`0x121D6D`) through the LP24 stage `0x12566E` on the ORG core 2 image, with the strip words written by the ARM harness for the Believer layer, at factor 4.4 and 1.0 (the words differ only by the cutoff word). It would show the stage's own timbre for those words, but it cannot say which factor is right until the display Hz to word law is verified.

## 7. F or 2F: the synth stages run at 2F (D108 item 1)

Question of the coordinator: output frames per pass, stage calls per output sample, F or 2F.

| # | Finding | Evidence | Tag |
|---|---|---|---|
| 1 | One ORG core 2 pass calls each channel's filter stage **once** and the stage works on **4 consecutive samples** (24 store PCs of the LP24 stage, 2 stores each, 2 sections times 4 samples; LP12 12 PCs; BP 24; HP 16) | `sharc-org core2_sram.bin --dev 0x282c0380=1 --dev 0x3108d008=5 --passes 1 --msg-base 0 --msg 0:0xc0b8c=<stage> --msg 0:0xc10b8=0x125b79 --watch 0xC2938:12`, re-run by hand for `0x12566E` | VERIFIED |
| 2 | The oscillator advances **`f r / 2` cycles per sample**: `0x1215FB` loads the ratio `r = DM(-32, I6)`, multiplies the ARM pitch word `f = DM(0xC1F79)`, and converts with `R1 = FIX F1 BY R0` where `R0 = DM(0xC1DF8) = 0x1F` (31), so `inc = f r 2^31` in Q32 | `sharc-disasm core2_sram.bin 0x242bf6 0x242d40` (`R1 = DM(-32, I6)`, `R0 = DM(-10, I0)`, `R1 = FIX F1 BY R0`, `CALL 0x12501f`); `sharc-org ... --passes 1 --dump 0xC1DF8:1` gives `0x1f` | VERIFIED |
| 3 | `f = note / F`: note 60 gives `DM(0xC1F79) = 0x3bc585c6 = 0.0060279`, equal to `261.6256 / 43402.34375` to 7e-7 | `sharc-org ... --passes 5 --pre 0xC1DCC=0x0B008000 --dump 0xC1F78:4`, re-run by hand | VERIFIED |
| 4 | After the ARM strip apply `0xC01CE7D8` every loaded wave type has the ratio words `P[-32 .. -29]` **around 1.0** (type 0 words `0x3f800000`; SUPER unison ratios 0.9576 .. 1.0443); the idle 0.077 is an image default | [data/synth-p-words.csv](data/synth-p-words.csv), [synth-p-writer](synth-p-writer.md) s2 (VERIFIED run) | VERIFIED |

Therefore a played note advances `note / (2 F)` cycles per **stage sample**: the stage sample rate is **`2F` = 86,804.69 Hz** (with `r = 1`; the pitch comes out right only then). Four samples per call at `2F` is 2 frames of 43,402 Hz, matching the verified 2-frame pass of the PNO core ([pno-full-pass](pno-full-pass.md) s1). Answers: (2) each of LP12 `0x1253BD` and LP24 `0x12566E` runs once per 4 samples, i.e. **once per stage sample and twice per 43,402 Hz frame**; (3) **2F**. (1) Output frames per pass: the 40 captured words are not 20 frames of one source; each 8-word group holds the 4 samples times 2 lanes written 1:1 (`0x1285B8..0x1285DC`) and five such groups make the buffer, so the pass is 4 stage samples = 2 frames of `F` per source (ASSUMED that every group shares the cadence). Open: whether a later stage decimates 2:1 to the DAC rate; it does not change the filter's own rate.

Caveat: this fixes the oscillator and the filter on the same samples (4 consecutive samples per call, both in one pass); the filter coefficient `R = tan(pi fc / rate)` is only as verified as [synth-stage-equations](synth-stage-equations.md) (nominal cutoff `atan(R) / pi` cycles per sample).

**On the type factors, on paper only (no default moves, D22).** With the stage rate `2F` the derived uniform factor of D108 is `1.015` on the display Hz (display / nominal = 1.971, `2 / 1.971`): the display Hz is the stage cutoff within 1.5 %. Against that anchor the fitted totals read: SUPER LP24 `0.45 x 4.4 = 1.98` is **1.95 times** the derived value, and the best totals of the grid here (`0.45 x 2.0 = 0.90`, `0.45 x 2.2 = 0.99`) are **0.89 and 0.98 times** it: the clip optimum for the SUPER LP24 row sits at the derived `2F` law, and the old 4.4 had absorbed a factor of about 2 (an `F` reading). LP12 / BP 1.4 would be 1.38 times, LP24 / HP 1.0 would be 0.99 times, Ladder24 0.8 would be 0.79 times the derived law. The uniform `--synth-filter-law 1.015` itself still scores worse over 12 clips (D108 section 3: 3.88 against 2.18) because it also drops the row factors and moves the resonance; so this agrees with the SUPER LP24 grid and leaves LP12 / BP (1.38) and Ladder24 (0.79) unexplained.

## 8. ARM harness sweep of filter type, cutoff, modulation, resonance (request of 2026-11-16)

Method: `tools/arm_edit_capture.py` dispatch on the settled snapshot (layer A store), 16 runs, 34 s wall, local only (`C:/tmp/arm-sweep/edit_sweep.py`; scratch). A second route, booting a full program image (about 54 s per boot, 886 DSP messages after a note-on), was tried first: it loads the program's section table but sends **no** strip or common words for the synth layers (only PNO core 1 words), so it cannot read strip words. Checks: `kFltType` 0..5, then `kFltFreq` 52 and 80 each with `kFltEnvAmt` and `kLFOAmt` at 0 and 127, then `kFltRes` 0 and 107.

| Item | Result | Tag |
|---|---|---|
| Strip word 1 (stage id) for `kFltType` 0..5 | `0x0612566E` for **every** type (LP24 stage). The selector `[strip+0x74]` moves, the sender index `[strip+0x7C]` does not, same as [synth-filter-type-map](synth-filter-type-map.md) s1 | VERIFIED (run) |
| Strip words 3, 5, 7, 10, 20, `DM(-30)` | **never sent by any `Edt_Syn*` id**: the ARM writes only strip 0 words 1 and 19 and common words 1, 2, 3, 7, 8, 13 for these ids. Words 3, 5, 7, 10, 20 and `DM(-30)` are produced inside the DSP pass from the common words | VERIFIED (run: all words the 16 runs emit) |
| Cutoff word (common 0 word 1) | knob 52 -> `0x00000000` (index 0), knob 80 -> `0x1C000000` (index 28 = knob - 52), identical for every type and for `kFltEnvAmt` 0 / 127 and `kLFOAmt` 0 / 127 | VERIFIED (run) |
| Envelope amount | only common word 13: 0 at knob 0, `0x3F800000` (1.0) at knob 127; strip 0 word 19 constant 1.0 | VERIFIED (run) |
| LFO amount | common words 2, 3, 8 stay 0 at both ends (no ARM word carries `kLFOAmt` while the LFO destination is off in this snapshot) | VERIFIED (run), destination effect not tested |
| Resonance word at knob 107 | common word 7 = `0x3EAF5EBE` = 0.3425 = `107/127 - 0.5`; table position by the D108 law `64 + 33.25 w` = **75.39** (not 75.5) | VERIFIED word; law VERIFIED ([synth-filter-type-corners](synth-filter-type-corners.md)) |

What this explains on paper:

1. **Stage per type.** The edit path cannot show which stage a type reaches: word 1 stays the LP24 id, so `from_model` (LP12 `0x1253BD`, BP `0x125913`, HP `0x125A50`, others LP24 `0x12566E`) stays the identity reading of [synth-filter-type-map](synth-filter-type-map.md), unchanged. The link selector `+0x74` to index `+0x7C` is probably made at program commit, which the 54 s boot route did not reach.
2. **1.38 (LP12/BP) and 0.79 (Ladder24) are not in the ARM cutoff word.** The word is type independent, so the type factors must come from the DSP stages (their own coefficient prologues) or from a type-dependent strip word the DSP derives. The ARM sweep therefore rules out an ARM-side type factor; it does not explain the numbers. The oracle probe of each stage at the same cutoff word is the next step.
3. **Resonance.** At knob 107 the stage position is about 75.4 plus the LFO / envelope term of `W28 = FIX((w10 + C271C) lfo)`; the idle-LFO reading (0.5) cannot be confirmed here because the LFO words stay 0. The Oo Pad LP12 resonance error is therefore still open: P and cin were not read, since `W28` is DSP side.

Not done: strip words 3, 5, 7, 10, 20 and `DM(-30)` of a sounding voice (needs a program commit reaching the strips or the real SHARC image with a voice word), Ladder24 `DM(-30)`.

## Implications

- SUPER LP24 4.4 is EMPIRICAL and **not independently confirmed**: on the only two informative clips (Horizon, Sidechain) 2.0 (total 0.9) is better under both note lists, and the pooled mean over four clips goes 3.17 to 2.44 and 3.95 to 2.58. The value 2.0 is a candidate, not a fit result: the grid has 1.0 / 1.5 / 2.0 / 2.2 / 3.0 and the optimum moves with the note list.
- The type factor is still not a stage quantity (D108); a fit to two clips with an uncontrolled mod wheel and unverified note lists is not enough to replace a default (D22).

## Follow-ups

1. A trusted note list for these clips (pitch track of the hardware, MuScriptor, or an `agy` excerpt hypothesis checked as in section 3) and the wheel position of the player; rerun the grid.
2. The other seven factory programs with SUPER on LP24 (11 in total) have no Part A clip: any independent clip (a synth lesson video) would be a hold-out.
3. The stage-level work of D108 section 4 (stage call rate, `kFltType` link, LFO scale) remains the way to replace the factor, not a fit.
