# SUPER wave 0 on LP24: does the type factor 4.4 hold on independent clips?

- **Checked:** 2026-11-15
- **Sources (local, never committed):** Part A video `_zksT69H30c` (`NEW NORD STAGE 4 - PROGRAMAS PRESETS DEMO (PART A) [_zksT69H30c].webm`), `Nord Stage 4 Factory Restore RevK v1.66.ns4b` (`program_sweep` layer decode, `ns4 render-patch`), `tools/emu_vs_hw.py` (segments `super_lp24_*`, [emu_vs_hw_segments.json](data/emu_vs_hw_segments.json)), `tools/note_list_check.py` (new), Transkun stream `piano_events__zksT69H30c.json`, [synth-filter-type-corners](synth-filter-type-corners.md) (D108), [synth-validation](synth-validation.md) s11 to s12 (D81). Derived numbers: [data/super_lp24_clips.json](data/super_lp24_clips.json). No audio, video or frame is committed.
- **Question:** `SUPER_LP24_TYPE_SCALE = 4.4` (`synth/p_type.rs`, EMPIRICAL) was fitted on one clip (`Horizon Whl`, A:16) and is entangled with the SUPER row factor 0.45. Does it hold on the other factory programs with a SUPER layer on LP24 that appear in the Part A video (A:25 Sidechain Whl II, A:76 Oo Pad Whl, A:81 Believer)?

Tags follow `.agents/rules/provenance-in-code.md`. Scores are the `emu_vs_hw.py` gap score (mean absolute band delta, dB, five bands below 8 kHz, lower is better) of a lossy YouTube recording of a live performance: shape evidence, not truth ([emulator-vs-hardware-audio](emulator-vs-hardware-audio.md)).

## 1. Short answer

| Question | Answer |
|---|---|
| Is 4.4 stable? | **No.** On the fit clip itself the grid minimum is now at 2.0 to 3.0 (score 0.74 to 0.85 against 1.89 at 4.4); on `Sidechain Whl II` 1.0 to 2.2 (2.87 to 2.94 against 4.32). 4.4 is worse than every factor from 1.0 to 3.0 on both clips under both note lists. The other two clips are insensitive to the factor (spread under 0.5) |
| Does one factor fit all four? | **2.0 comes close** (total 0.9 with the row factor 0.45): within 0.11 of the best grid value on all four clips with the video-detected notes, within 0.41 with the Transkun notes. Mean over the four clips: 4.4 = 3.17 / 3.95, 2.0 = **2.44 / 2.58** (video-detected / Transkun notes) |
| Does resonance (Oo Pad 0.61) behave differently? | Yes, but weakly: `--synth-fw-resonance` is the only change that helps Oo Pad (4.86 to 4.74, 4.36 to 3.99) and it hurts the two low-resonance clips (Horizon 1.89 to 5.64, Sidechain 4.32 to 6.43 at 4.4). The optimum factor of Oo Pad does not move with it. One clip with other layers sounding: not conclusive |
| Default flips? | No (D22). 2.0 is reachable opt-in through the new `--synth-super-lp24 2.0`; what would justify a flip is in D116 |

## 2. Clips and what was checked

Parameters from the local backup (`program_sweep`, decoded layers); display numbers `A:nn` are the video list, entries are named from the video list.

| Clip | At (s) | Used (s) | SUPER layer (wave 0, LP24) | Other layers sounding | Verdict |
|---|---|---|---|---|---|
| A:16 Horizon Whl (fit clip, control) | 290 | 30 | A 277 Hz res 0.0 env 0.39; B 554 Hz res 0.0 env 0.25 | C LP12 98 Hz res 0.18; piano | informative |
| A:25 Sidechain Whl II | 594 | 28 | A 1600 Hz res 0.10 env 0.0, wheel morph 0.31 | none (B, C off); piano | **informative** (the cleanest clip) |
| A:76 Oo Pad Whl | 2150 | 25 | A 208 Hz res 0.61 env 0.12 | B Saw LP12 330 Hz; C Triangle LP12 196 Hz res 0.85; piano | weak: two LP12 layers dominate and no factor gets below 4.3 |
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
5. **Oo Pad is weak**: the best of every variant is 4.3 to 3.5; the LP12 layers B and C carry the sound and no factor of the LP24 layer fixes that.

## Implications

- SUPER LP24 4.4 is EMPIRICAL and **not independently confirmed**: on the only two informative clips (Horizon, Sidechain) 2.0 (total 0.9) is better under both note lists, and the pooled mean over four clips goes 3.17 to 2.44 and 3.95 to 2.58. The value 2.0 is a candidate, not a fit result: the grid has 1.0 / 1.5 / 2.0 / 2.2 / 3.0 and the optimum moves with the note list.
- The type factor is still not a stage quantity (D108); a fit to two clips with an uncontrolled mod wheel and unverified note lists is not enough to replace a default (D22).

## Follow-ups

1. A trusted note list for these clips (pitch track of the hardware, MuScriptor, or an `agy` excerpt hypothesis checked as in section 3) and the wheel position of the player; rerun the grid.
2. The other seven factory programs with SUPER on LP24 (11 in total) have no Part A clip: any independent clip (a synth lesson video) would be a hold-out.
3. The stage-level work of D108 section 4 (stage call rate, `kFltType` link, LFO scale) remains the way to replace the factor, not a fit.
