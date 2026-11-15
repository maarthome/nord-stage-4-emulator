# Grand Pad (A:43) against the Part A demo video: piano + pad balance

- **Checked:** 2026-11-15
- **Sources (local, never committed):** Part A demo video (`_zksT69H30c`, program A:43 on screen 1168 to 1204 s), factory backup `Nord Stage 4 Factory Restore RevK v1.66.ns4b` (`Program/Bank A/Grand Pad.ns4p`, 868 bytes), Transkun note stream of PR 381 (`piano_events__zksT69H30c.json`, 127 notes in the 1171 to 1201 s window, median velocity 50; no pedal replay in this run), `tools/emu_vs_hw.py` (new segment `pianopad_grand`), `ns4 render-patch`. Related: [dsoundman-piano-pad-layer](dsoundman-piano-pad-layer.md), [piano-note-velocity](piano-note-velocity.md), [emulator-vs-hardware-audio](emulator-vs-hardware-audio.md).
- **Question:** The user hears the emulated Grand Pad as odd against the hardware at 19:29 (1169 s). What is the program, and which layer, level or stage is wrong?

Tags follow `.agents/rules/provenance-in-code.md`. Hardware audio is a lossy live recording and the notes are a piano-trained estimate (pad notes are junk-prone), so every number is shape and level evidence only.

## 1. The program (VERIFIED, `ns4 convert --to-json`)

| Item | Value |
|---|---|
| Sections | `section_enable_i = 3` (piano A + synth A); organ off, split off, scene 1 mask 20 |
| Piano A | Grand, model 3 variation 1, level 1.0, string resonance on, acoustic level 2, no timbre, no dyn comp; Reverb type 3, amount 0.236 |
| Synth A | analog SuperSaw (detune 3.46, unison 3), Lp12 cutoff 294 Hz, resonance 0.097, filter env amount 0.35 (attack 68 ms, decay 9.5 s), amp attack 19 ms, decay 45 s, release 399 ms, **layer level 0.567**, cutoff morph (wheel) +0.14; FX A: amp enabled (type 0, drive 0), EQ mid -5 dB, Reverb type 4 colour 1 amount 0.37 |
| Synth B / C, piano B | off |

So it is a **piano + dark pad layer**; the pad filter at 294 Hz leaves almost nothing above 400 Hz.

## 2. Comparison (VERIFIED, run; deltas are emulator minus hardware, dB re each signal's RMS)

Replay of the same 127 Transkun notes at their own velocities (`tk_*` renders; the first run with the `video_ref` notes at the uniform velocity 100 read RMS +14.9 dB for the same reason as the Velvet Grand clip: the performer plays soft, median 50).

| Variant | sub | low | mid | pres | brill | RMS | width hw / emu | T60 hw / emu s | score |
|---|---|---|---|---|---|---|---|---|---|
| default (both layers) | +3.2 | +0.3 | +1.3 | -2.1 | -0.3 | +2.3 | 0.54 / 0.37 | 13.4 / 3.7 | 1.46 |
| piano layer only | -1.6 | +0.7 | +3.6 | +0.7 | +2.4 | -0.6 | 0.54 / 0.52 | 13.4 / 1.8 | 1.78 |
| synth layer only | +5.9 | -0.1 | -6.2 | -15.7 | -12.3 | -1.4 | 0.54 / 0.13 | 13.4 / 4.1 | 8.04 |
| `--piano-slew-fw` | +5.4 | +0.1 | -4.0 | -10.6 | -7.8 | -0.7 | | | 5.57 |
| piano chain + slew | +5.1 | +0.1 | -3.1 | -9.0 | -6.3 | -0.2 | | | 4.71 |
| `--synth-p verified` | +0.5 | +0.5 | +2.8 | 0.0 | +1.6 | +0.2 | 0.54 / 0.47 | | 1.09 |
| `--legacy-all` | +1.1 | +0.5 | +2.9 | -0.1 | +4.7 | +3.2 | | | 1.85 |
| `--firmware-all` | +3.5 | +0.3 | +1.4 | -2.1 | -0.4 | +2.7 | | | 1.53 |
| `--firmware-piano-layer-law` | identical to default (the piano layer level is 1.0) | | | | | | | | 1.46 |
| synth level 1.0 | +4.7 | +0.1 | -0.8 | -4.9 | -3.0 | +5.2 | | | 2.69 |
| synth level 0.567 (default) | +3.2 | +0.3 | +1.3 | -2.1 | -0.3 | +2.3 | | | 1.46 |
| **synth level 0.3214 = 0.567^2 (`--firmware-synth-layer-law`)** | +1.3 | +0.5 | +2.7 | -0.5 | +1.2 | **+0.6** | 0.54 / 0.44 | 13.4 / 3.3 | **1.24** |
| synth level 0.15 | -0.6 | +0.6 | +3.4 | +0.3 | +2.0 | -0.2 | | | 1.40 |

The synth-only row is scored against the full mix, so its band deltas are the pad's share of the hardware spectrum, not an error.

Absolute band power, emulator minus hardware (dB, same clip):

| Band | piano layer | synth layer | default mix |
|---|---|---|---|
| 60 to 130 Hz | -2.1 | +4.1 | +5.2 |
| 130 to 400 Hz | -2.4 | -2.4 | +0.9 |
| 400 to 1500 Hz | +1.5 | -11.0 | +1.8 |
| 1.5 to 5 kHz | -0.9 | -15.9 | -0.8 |

## 3. Reading

1. **The piano layer is right.** Above 400 Hz the piano alone matches the recording within 1.5 dB (the pad filter at 294 Hz contributes nothing there on the hardware either). The piano level at the transcribed velocities agrees with the four piano clips of PR 381 (-0.6 dB).
2. **The pad is too loud in the sub band.** The emulated pad carries +4.1 dB over the whole hardware clip below 130 Hz; the mix is +5.2 dB there. Lowering the synth layer level lowers the sub excess and the RMS together: the score falls 2.69 (level 1.0) to 1.46 (0.567, default) to **1.24 (0.3214)** and rises again at 0.15 (1.40). The optimum is the **square law**: `0.567^2 = 0.3214`, the law of the piano and organ layer handlers (`0xC011A7A4`, D26). The linear power fit of the two layers to the 24-band hardware spectrum gives piano -2.3 dB and synth -4.4 dB against the default; the square law asks -4.9 dB of the synth. EMPIRICAL: one clip, estimated notes, the synth layer handler (`0xC011BB48`) not run, and the DSoundman fader move at 0.7 (-2.7 dB) read linear ([dsoundman-piano-pad-layer](dsoundman-piano-pad-layer.md) s4, confounded), so the default stays linear (D82: opt-in `--firmware-synth-layer-law`).
3. **The 130 to 400 Hz pad level is right**, so the sub excess is not a pure level error: the Lp12 at 294 Hz with the SuperSaw leaves the 60 to 130 Hz partials 4 dB high against the hardware (low-note content of the transcription is partly junk, so this is not pinned down). `--synth-p verified` (ARM P words, D70) lands closer on the sub (+0.5) and the RMS (+0.2) and scores best (1.09), but it stays opt-in (D22 bar not met on the 15-clip set).
4. **Stereo width and tail.** Hardware width 0.54 against 0.37 emulated, and a 13.4 s tail time against 3.7 s (hardware includes the sustain pedal, which this render does not replay, and a plate-like synth Reverb type 4 colour 1; the emulated synth layer alone reads width 0.13). Open: the synth FX rack of this program (amp type 0 on, EQ mid -5 dB, reverb 0.37) against the hardware width; not changed here.
5. **Not at fault:** `--piano-slew-fw` and the chain + slew path (4.7 to 5.6, 7 to 10 dB too dark above 400 Hz because they drop the piano level), `--legacy-all` (1.85), `--firmware-all` (1.53), `--legacy-routing` and `--firmware-routing` (identical to the default: the routing default is already D73), `--no-synth-knob-laws` (identical: the program uses no knob-law path).

## 4. Implications

- Root cause of the audible oddness at the uniform replay velocity: the loud piano (velocity 100 against a played median of 50) and a pad that is about 5 dB hot in the sub band relative to the hardware mix. At the played velocities the default is within 2.3 dB RMS and 3.2 dB per band.
- The layer level law of the synth stays an open choice (OQ22): this clip supports the square law, the DSoundman fader move the linear law. A hardware sweep at fixed settings decides.
- Segment `pianopad_grand` (A:43, 1171 s, 30 s) is in `emu_vs_hw_segments.json` with the layer-split variants (`piano_only`, `synth_only` by `section_enable_i`), every opt-in path and four synth level variants, so it stays covered. Notes: `python tools/emu_vs_hw.py --events-from transkun ...` after PR 381 merges.

## 5. Follow-ups

1. Replay the pedal of the transcription (PR 381 `render-patch` pedal support) and re-measure the tail time and width.
2. Done (D87, [synth-layer-level-law](synth-layer-level-law.md)): the handler `0xC011BB48` stores `(v / 127)^2`; the law is the default.
3. Decode the program's synth FX rack against the hardware width (Reverb type 4 colour 1, EQ mid -5 dB).
