# Grand Pad (A:43) leftovers: synth FX rack, pedal, stereo width and tail

- **Checked:** 2026-11-15 (OS v1.66, HEAD `d5c9c210` plus this change).
- **Sources (local, never committed):** Part A demo video (`_zksT69H30c`, 1171 to 1201 s), factory backup `Nord Stage 4 Factory Restore RevK v1.66.ns4b` (`Program/Bank A/Grand Pad.ns4p`), the committed Transkun stream [data/piano_events__zksT69H30c.json](data/piano_events__zksT69H30c.json) (notes, velocities, pedal events), `tools/emu_vs_hw.py --events-from transkun` (segment `pianopad_grand`, variants of [data/emu_vs_hw_segments.json](data/emu_vs_hw_segments.json)), `ns4 convert --to-json`, the manual (v1.6X, pp. 18, 23, 28). Related: [grand-pad-vs-video](grand-pad-vs-video.md), [fx-eq-words](fx-eq-words.md), [fx-type-map](fx-type-map.md), [fx-reverb-leslie](fx-reverb-leslie.md), [audio-remaining](audio-remaining.md) item 8.
- **Question:** which of the four Grand Pad leftovers of D82 (synth FX rack not decoded, stereo width 0.54 against 0.37, tail 13.4 s against 3.7 s, sustain pedal not replayed) are real emulator gaps, and which came from the replay?

Tags follow `.agents/rules/provenance-in-code.md`. The hardware clip is a lossy live recording and the notes are a piano-trained estimate; clip numbers are shape and level evidence only. The ARM harness was **not** re-run for this note: the rack words are already tabulated (s2) and the clip runs show that no decoding of them can move this program. The clip only checks.

## 1. Answer

| Leftover | Result | Status |
|---|---|---|
| Sustain pedal not replayed | Replaying the transcribed CC 64 events (the committed stream, `--events-from transkun`; the D82 run used the audio-detected notes without pedal) moves the default tail **3.7 to 6.7 s**, the width **0.37 to 0.46**, RMS to -1.0 dB, score 1.42. It was a replay artefact of the earlier run, not an engine gap. | VERIFIED (run, s3) |
| Synth FX rack: amp, EQ mid -5 dB | **No effect on this clip.** Amp type 0, drive 0 is the firmware amp already (default). EQ mid -5 dB sits at index 53 (about 700 Hz, `eq_mid_freq_norm` 0.417) where the 294 Hz low-pass pad has no energy: setting the mid to 0 dB changes every band by at most 0.1 dB and the score by 0.04; the `eq` port and `--firmware-fx` give the same bound. Decoding the synth unit EQ words (open in [fx-eq-words](fx-eq-words.md)) cannot move this clip. | VERIFIED (run) |
| Synth reverb 0.37 (type 4, colour 1) | The emulated reverbs contribute almost nothing here: synth reverb off, piano reverb off and both off move the T60 by 0.0 to 0.25 s and the score by 0.01 to 0.07. The 13.4 s hardware tail is not produced by the reverb stage the emulator has. Per type parameters belong to the reverb per type work (audio-remaining item 7); not changed here. | VERIFIED (run), open |
| Pedal on the synth layers | The manual (p. 28) gives the Synth section its own SUSTPED button; the program file does not store it (no field in the program JSON; engine default: Piano only, [midi-and-control](../specs/midi-and-control.md)). With the pedal also holding the three synth layers (`--synth-sustain-pedal`, opt-in) the tail reads **10.9 s** (hardware 13.4), hold -4.0 dB (hardware -3.2), release -2.6 dB (hardware -0.8), RMS +0.2 dB, but the spectral score rises 1.42 to 1.68 (sub +2.4, presence -3.3) and the width falls to 0.40. Mixed evidence, one clip: opt-in only. | EMPIRICAL |
| Stereo width 0.54 vs 0.46 | Closed from 0.37 by the pedal replay; the piano alone reads 0.51 and the synth alone 0.13 (hardware: the mix only). The synth stereo path (amp pan words `0xC02BB10C`, `0xC02BB2C4`, unison spread, reverb stereo) is not decoded and no variant moves the width above 0.48. | open |

## 2. The rack, from the words already read

| Item | Program value | Where it is decoded | Effect on this clip |
|---|---|---|---|
| Amp | enabled, `kAmpTypeNew` 0, drive 0 | [fx-type-map](fx-type-map.md) s8 (drive stage verified against the oracle, `FirmwareFx::amp` default on) | none (variant `amp-off` equals `synth-eq-neutral`) |
| EQ | treble 0, mid -5 dB at index 53, bass 0 | [fx-eq-words](fx-eq-words.md) (tone stack `0x1243F4`; synth unit words differ, not wired) | at most 0.1 dB per band |
| Synth reverb | type 4, colour 1, amount 0.37 | [fx-reverb-leslie](fx-reverb-leslie.md) s14 (`FwReverbMulti`, opt-in `--firmware-reverb`) | T60 +0.2 s, width -0.01 |
| Piano reverb | type 3, amount 0.236 | same | T60 0.25 s |

## 3. Runs (VERIFIED; deltas emulator minus hardware in dB; transcribed notes, velocities and pedal; segment `pianopad_grand`, `tools/emu_vs_hw.py --events-from transkun`)

| Variant | sub | low | mid | pres | brill | air | RMS | width hw / emu | T60 hw / emu s | hold dB hw / emu | release dB hw / emu | score |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| default | +0.8 | +0.1 | +1.1 | -2.1 | +2.9 | -1.6 | -1.0 | 0.54 / 0.46 | 13.43 / 6.74 | -3.23 / -6.67 | -0.76 / -4.19 | 1.42 |
| piano layer only | -0.7 | +0.4 | +1.8 | -1.5 | +3.7 | -1.3 | -1.7 | 0.54 / 0.51 | 13.43 / 6.48 | -3.23 / -5.91 | -0.76 / -3.83 | 1.61 |
| synth layer only | +4.1 | +1.6 | -5.4 | -14.7 | -11.8 | -6.2 | -8.0 | 0.54 / 0.13 | 13.43 / 4.09 | -3.23 / -0.28 | -0.76 / -5.83 | 7.51 |
| synth EQ mid 0 dB (equal to amp off) | +0.7 | +0.1 | +1.1 | -2.1 | +2.9 | -1.6 | -0.9 | 0.54 / 0.46 | 13.43 / 6.54 | -3.23 / -6.88 | -0.76 / -4.29 | 1.38 |
| `eq` port only | identical to default within 0.01 dB | | | | | | | 0.54 / 0.46 | 13.43 / 6.73 | | | 1.42 |
| synth reverb off | +1.0 | +0.1 | +1.0 | -2.2 | +2.8 | -1.6 | -0.8 | 0.54 / 0.45 | 13.43 / 6.74 | -3.23 / -6.56 | -0.76 / -4.28 | 1.43 |
| piano reverb off | +1.0 | -0.1 | +1.2 | -2.0 | +3.1 | -1.5 | -0.7 | 0.54 / 0.48 | 13.43 / 6.98 | -3.23 / -6.30 | -0.76 / -3.96 | 1.48 |
| both reverbs off | +1.1 | -0.1 | +1.1 | -2.1 | +3.0 | -1.6 | -0.5 | 0.54 / 0.47 | 13.43 / 6.99 | -3.23 / -6.21 | -0.76 / -4.07 | 1.49 |
| `--firmware-reverb` | +1.0 | +0.1 | +1.1 | -2.2 | +2.9 | -1.8 | -0.8 | 0.54 / 0.45 | 13.43 / 6.94 | -3.23 / -6.48 | -0.76 / -4.34 | 1.47 |
| `--firmware-fx` (all ports) | +1.1 | +0.0 | +1.2 | -2.1 | +3.0 | -1.8 | -0.5 | 0.54 / 0.47 | 13.43 / 7.12 | -3.23 / -6.45 | -0.76 / -4.30 | 1.48 |
| `--synth-sustain-pedal` | +2.4 | +0.7 | +0.2 | -3.3 | +1.8 | -2.6 | +0.2 | 0.54 / 0.40 | 13.43 / 10.91 | -3.23 / -4.03 | -0.76 / -2.62 | 1.68 |
| `--synth-sustain-pedal --firmware-fx` | +2.6 | +0.6 | +0.3 | -3.3 | +1.8 | -2.9 | +0.7 | 0.54 / 0.39 | 13.43 / 10.79 | -3.23 / -3.88 | -0.76 / -2.43 | 1.72 |

Reading: the emulated tail ends where the piano pedal and the 399 ms synth release end; the hardware keeps the pad (45 s decay) audible longer, which the synth pedal hold reproduces (10.9 s) but with a darker spectrum (the held pad piles up below 130 Hz: sub +2.4). The remaining 2.5 s and the width are not explained by the amp, the EQ or the reverb types the emulator has.

## Implications

- Close the pedal and rack items of D82: pedal replay is the default of `emu_vs_hw.py --events-from transkun`; the rack (amp, EQ) is irrelevant for this program.
- Whether the hardware synth layers hold on the pedal (the SUSTPED default of the Synth section, manual p. 28) decides about 4 s of tail: decode the ARM default of that flag and its program bit before any flip (D22).
- The reverb stage is nearly inaudible against the hardware here (T60 and hold barely move with either reverb off): the per type and amount mapping is the open lever for tail and width (audio-remaining item 7, reverb per type work). Recorded in OQ21 (addendum).

## Follow-ups

1. ARM harness: find the synth SUSTPED default and the program bit (Synth section toggle) and the DSP side release hold; flip `--synth-sustain-pedal` only with that and a second clip.
2. After the reverb per type result lands, re-run `pianopad_grand` and read T60 and width again.
3. Decode the synth unit stereo path (amp pan words `0xC02BB10C`, `0xC02BB2C4`, the SuperSaw unison spread) with the SHARC oracle if the width stays below 0.5.
