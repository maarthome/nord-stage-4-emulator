# Felt Upright sounds softer on the hardware: replay velocity, not a missing stage

- **Checked:** 2026-11-15
- **Sources (local, never committed):** Part A demo video (`_zksT69H30c`, program `A:13 Felt Upright` on the OLED from 146 s, played 150 to 190 s), the OS v1.66 factory backup `Program/Bank A/Felt Upright.ns4p` and `Piano/Upright/Felt Upright Lrg 6.2.npno`, `tools/emu_vs_hw.py --only piano_felt` (new segment in `docs/research/data/emu_vs_hw_segments.json`), `ns4 render-patch`, [piano-downstream-level](piano-downstream-level.md), [piano-note-velocity](piano-note-velocity.md), [smp-slew](smp-slew.md), [emulator-vs-hardware-audio](emulator-vs-hardware-audio.md).
- **Question:** The user compared the GUI with the hardware reference and hears the emulated Felt Upright as much harsher and brighter. Which stage or default makes it so?

Tags follow `.agents/rules/provenance-in-code.md`. The video audio is lossy YouTube audio; the numbers are shape and level evidence only, no audio is committed.

## 1. What "Felt Upright" is

| Item | Result | Tag |
|---|---|---|
| Program | `A:13 Felt Upright` (`Program/Bank A/Felt Upright.ns4p`): section enable I = 2 (piano only), piano layer A on, no FX, no EQ, no reverb | VERIFIED (decoded program) |
| Sound | `Piano/Upright/Felt Upright Lrg 6.2.npno`, resolved from the name hash `1857221708` (category Upright, model 1) | VERIFIED (`ns4 render-patch` prints the resolved entry) |
| Per-sound parameters | Timbre `Off`, touch `Medium` (identity curve), string resonance on, pedal noise on, soft release off, dyn comp 0, unison 0 | VERIFIED (decoded program) |
| Other programs with Felt | `Bright Felt` (A:51, video 1406 s), `Felt Soft Str`, `Big Felt Pad Whl`, `Felt Clouds`, `Felt Tines`; the user means the `Felt Upright` program | VERIFIED (program list) |
| Timbre candidate | The program stores Timbre `Off`, so the dsoundman Normal to Soft shift does not apply to this program | VERIFIED |
| Isolated reference | None: the video holds only music (no single notes, no velocity ramp). The 419 detected notes of the segment give shape and level evidence only | VERIFIED |

## 2. Emulator against the video (VERIFIED, run; 150 to 190 s, 419 notes, uniform replay velocity)

Band deltas are emulator minus hardware, relative to the total of each signal (dB); RMS is absolute (dB); the hardware 8 to 20 kHz band is the YouTube low-pass and is not scored.

| Replay | low 120-400 | mid 400-1.5k | presence 1.5-3k | brilliance 3-8k | RMS | attack hw / emu ms | score |
|---|---|---|---|---|---|---|---|
| velocity 64 | +3.6 | -5.4 | -14.1 | -22.2 | +2.7 | 94 / 73 | 10.66 |
| velocity 80 | +3.3 | -4.0 | -6.0 | -13.9 | +6.8 | 94 / 75 | 6.94 |
| velocity 88 | +3.2 | -3.4 | -3.5 | -9.3 | +8.4 | 94 / 72 | 5.39 |
| velocity 100 (the GUI mouse velocity) | +3.0 | -2.6 | +0.9 | +0.5 | +11.0 | 94 / 67 | 2.84 |
| velocity 127 | +2.4 | -1.7 | +6.9 | +16.7 | +14.5 | 94 / 52 | 6.84 |
| 88, `--legacy-piano-level` | +3.0 | -3.0 | -0.7 | -1.8 | +9.5 | 94 / 68 | 3.19 |
| 88, `--firmware-piano-chain` | +3.2 | -3.4 | -3.5 | -6.4 | +11.1 | 94 / 69 | 4.79 |
| 100, `--piano-slew-fw` | +2.4 | -3.1 | -0.4 | -3.1 | -4.5 | 94 / 110 | 3.54 |
| 88, `--piano-slew-fw` | +1.8 | -3.9 | -4.2 | -10.9 | -5.6 | 94 / 118 | 5.94 |

Reading:

1. **No missing darkening stage.** At velocity 100 the spectral shape matches the recording within 1 dB (presence +0.9, brilliance +0.5, centroid -124 Hz). At the velocities that match the level (64 to 88) the emulator is darker than the recording, not brighter. The brightness is a property of the velocity layers of the `.npno` (VERIFIED strokes, VERIFIED level law): brilliance moves 39 dB between velocity 64 and 127.
2. **The level is the difference.** At velocity 100 the emulator is +11.0 dB over the recording; the Pipe organ yardstick (oracle-verified absolute scale) reads +7.3 dB for the same recording chain ([piano-downstream-level](piano-downstream-level.md) s3), so the piano at 100 is **+3.7 dB louder** than a hardware note of the same velocity would be, and at 88 it is +1.1 dB (inside the 3 dB target of D22). Velocity 100 against 88 is 2.6 dB louder and 9.8 dB brighter in the 3 to 8 kHz band (-9.3 to +0.5 dB), which reads as harsh against a recording played softer.
3. **The GUI plays every mouse press at velocity 100** (`MOUSE_VELOCITY` in `crates/nord-gui/src/panel/keybed.rs`), the loud, bright layer of every velocity-layered factory piano, on every click. The Velvet Grand clip fits a uniform replay velocity of 88 ([piano-note-velocity](piano-note-velocity.md) s1) and the Felt Upright clip matches the level yardstick at 88 as well.
4. **Attack:** the hardware envelope rises in 94 ms (median 10 to 90 %), the default engine in 67 to 73 ms. The firmware slew voice (`--piano-slew-fw`) gives 110 to 118 ms: the real attack lies between; the slew ramp has no verified absolute level (RMS -4.5 dB at velocity 100, 15.5 dB under the default; with the bus read as `word * 2^-29` it would be +7.5 dB, equal to the yardstick, but that convention is ASSUMED, OQ20), so the slew stays opt-in (D22).
5. **Tail:** T60 3.2 s (hardware) against 1.3 s (emulator): the engine releases earlier on the clip; not a brightness effect, left open (OQ20, damper-down strokes).

## 3. Cause and change

| Candidate | Result |
|---|---|
| Wrong sound or program layers | no: right `.npno`, piano only, no FX |
| Timbre / Soft | no: the program stores Off |
| Missing EQ, dynamics, amp | no: all off in the program; the dyn comp and unison are 0 |
| Velocity to layer map and level law | already the firmware law by default (VERIFIED); the velocity it receives is the cause |
| SMP slew / kernel | slew changes the attack and the absolute level but not the shape; opt-in, absolute level open |
| Mouse velocity of the GUI | 100: loud and bright layer on every click |

Change (D76): the on-screen keybed sends velocity **88** instead of 100 (`MOUSE_VELOCITY`, EMPIRICAL, two clips agree on the level: Velvet Grand best uniform replay 88 and Felt Upright level +1.1 dB against the yardstick). On the Felt Upright clip this lowers the level by 2.6 dB (+11.0 to +8.4) and the 3 to 8 kHz band by 9.8 dB relative. No DSP default changes; `render_hash` and every golden are unchanged. A MIDI keyboard keeps its own velocity.

## Implications

- Nothing in the piano engine is brighter than the firmware at equal velocity; the perceived harshness of the GUI comes from the fixed velocity 100 and the open absolute level (+3.7 dB over the yardstick at 100).
- The shape-versus-level conflict of the recording (shape says 100, level says 88 or lower) is the same as in [piano-downstream-level](piano-downstream-level.md) s3 and stays an open recording-gain question.

## Follow-ups

- Recording needed from the user (hardware, line out or USB audio, no YouTube): Felt Upright, program `A:13`, no FX, single notes C3 and C4 held 4 s at five fixed velocities (soft, about 40, 64, 90, 110, hardest) on a velocity-reporting MIDI input, plus the MIDI file or log of the velocities. That pins level and brightness per velocity and the true attack without the unknown video gain.
- Optional GUI follow-up: a mouse velocity control or velocity from the vertical click position on the key.
