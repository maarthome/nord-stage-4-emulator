# Piano attack against the firmware oracle: the rate word is a stroke record field

- **Checked:** 2026-11-15
- **Sources (local, never committed):** the ARM harness (`tools/arm_smp_slew_init.py capture` / `rates`, OS v1.66 image and the settled snapshot), the SMP image on `sharc-ref` (`replay`), the factory backup `Nord Stage 4 Factory Restore RevK v1.66.ns4b` (`Piano/Upright/Felt Upright Lrg 6.2.npno`, `Piano/Grand/Velvet Grand Med 6.1.npno`, extracted to a scratch directory), the stroke records read with `NpnoLevel`; earlier work [smp-slew](smp-slew.md), [piano-attack-fw](piano-attack-fw.md), [piano-decay-attack](piano-decay-attack.md), [smp-voice](smp-voice.md), [sy-smp-audio](sy-smp-audio.md). Derived numbers only.
- **Question:** Does the firmware oracle (ARM note-on words into the SMP image) give a slower attack than the emulator, and which word of the emulator differs from it?

Tags follow `.agents/rules/provenance-in-code.md`. `agy` was not consulted: the call to send notes to the external model was blocked by the permission classifier, so no hypotheses from it are used.

## 1. Result

| Item | Result | Tag |
|---|---|---|
| Voice records and levels | the emulator's stroke record and Q23 level equal the ARM's level word `F15` in 54 of 54 grid points (Felt Upright 48, Velvet Grand 6) | VERIFIED (runs below) |
| Sample start, interpolation, decode | the SMP voice loop equals the Rust sample path bit for bit on real pages ([smp-voice](smp-voice.md)); the gain has no ramp ([smp-slew](smp-slew.md) s6) | VERIFIED |
| **Hold-decay rate word `F17`** | `F17 = 1 - d / 2^24`, `d = 2 * round(-s * 2^(semitones / 12))`, `s` the big-endian `i16` at stroke record byte `0x3C`, `semitones` the playback pitch above the stroke root: **48 of 48** Felt Upright points (notes 40 / 48 / 57 / 66 / 72 / 84 x velocities 10 / 30 / 50 / 64 / 80 / 100 / 115 / 127, all on the root pitch) and **7 of 7** Velvet Grand points (notes 60 and 72 on the root; note 57 is 1 semitone above its root: `-2s = 336 / 288 / 180` becomes `d = 356 / 306 / 190`) | VERIFIED (rounding nearest and the cents term ASSUMED) |
| Old engine law | `rate_for` interpolated Velvet Grand note-on words over velocity and key and was used for every sound: up to 21 % off on Velvet (note 72, velocity 30: 550 against 454) and **73 % off on Felt Upright** (velocity 100, note 57: `d = 184` against 318, time constant 4.2 s against 2.4 s) | VERIFIED runs |
| Attack | the oracle gain is flat from the first update (no ramp), so the attack of the oracle note is the decoded sample's own onset, the same as the emulator's: **the oracle does not reproduce a slower or faster attack than the emulator** | VERIFIED (single-note renders, [piano-attack-fw](piano-attack-fw.md) s1) |

## 2. Method

1. `tools/arm_smp_slew_init.py rates` runs the real firmware note-on (library stub, bound `.npno`) for each (note, velocity) and returns the level word and `d` of the rate word. Felt Upright: 8 velocities x 6 notes; Velvet Grand: velocities 30 / 64 / 100 x notes 57 / 72 (plus the earlier note 60 points).
2. The same (note, velocity) is run through `NpnoLevel::voices` and the 118-byte stroke record of the main voice is compared: the level equals, and the byte pair `0x3C..0x3E` determines `d` (the only other record byte that separates `d`, `0x71`, is unique per record). The three Velvet note-57 points differ from `-2s` by the factor 1.0595 = `2^(1/12)` (the note is one semitone above its root): the rate scales with the playback step.
3. `capture` + `replay` of Felt Upright note 57 at velocity 100 through the SMP image on `sharc-ref`: level word `0x3EF74664`, rate word `0x3F7FFEC2` (`d = 318`), gains `226871467 / 226867156 / 226862844 / 226858532 / 226854220`. The Rust record with the word the new law derives from the stroke field (`-159`) reproduces them within 1 LSB (`slew_voice::tests::felt_upright_note_on_gains_follow_the_oracle`); the Velvet words of `rate_law_matches_the_firmware_words` cover velocities 20 / 64 / 100 / 127.

## 3. Change (opt-in, `--piano-slew-fw` only)

`VoiceLevel::decay` carries the record field; `slew_voice::rate_from_record(decay, semitones)` replaces the Velvet-only `rate_for` table in `extra_voice.rs`. No default, `render_hash` or golden changes.

## 4. Effect on the clips (transcribed velocities and pedal, `emu_vs_hw.py`, chain + slew)

Emulator minus hardware; the rate law changes the hold decay (time constants 2 to 5 s), so the 0.5 s hold and 0.25 s release windows and the attack barely move.

| Clip | Score before / after | Attack ms (hw) | Release dB (hw -4.08 / -4.17) | Tail T60 hw / emu s | Per-octave T60 hw / emu, 48-59 and 60-71 |
|---|---|---|---|---|---|
| Velvet Grand | 3.74 / 3.74 | 68.9 / 68.9 (64.4) | -2.90 / -2.90 | n/a / 0.80 | 7.71 / 5.29 to 7.56 and 5.35 / 8.31 to 7.03 |
| Felt Upright | 1.31 / 1.31 | 62.1 / 62.1 (55.7) | -4.06 / -3.99 | 3.34 / 4.46 | 9.01 / 28.06 to 14.72 and 6.26 / 18.15 to 18.24 |
| Felt Upright, uniform 88 | 1.69 / 1.69 | 55.1 / 55.0 (55.7) | -4.24 / -4.24 | 3.34 / 6.37 to 5.31 | n/a |

The per-octave T60 of the Velvet Grand slew render now lies near the hardware (48-59: 7.56 against 7.71 s). On Felt Upright it is still 1.6 to 3 times too long: the pedal-down notes (25 to 77 % of the clip notes) play the legacy voice, not the slew ([piano-decay-attack](piano-decay-attack.md) s6), so a clip replay mixes both paths.

## 5. Where the remaining attack gap can come from

The oracle chain (record selection, level word, rate word, sample decode and interpolation, slew gain) now equals the emulator's end to end for pedal-up notes, and none of its stages shapes an attack. A hardware attack slower than the sample's own onset would have to come from stages the oracle path here does not contain: the PNO or FX image after the SMP bus (not found to hold a piano attack stage, [piano-dsp-path](piano-dsp-path.md)), the recording chain (YouTube codec, room), or the transcriber's onset jitter (the onset-aligned hardware average rises about 15 ms later than the emulator's, [piano-attack-fw](piano-attack-fw.md) s5, an upper bound). A single-note hardware recording is the only direct test; until then the attack difference is not attributed to a firmware word (D22).

## Implications

- The slew voice now uses the real per-stroke decay for every sound (and pitch); the velocity layer selection and level are already the firmware's.
- The emulator's attack equals the oracle's. A remaining hardware attack difference is downstream of the SMP, in the recording, or in the measurement.

## Follow-ups

- Decode the firmware voice list of damper-down strokes, then re-run the clips with the slew on pedal-down notes.
- ARM runs on notes 2 to 11 semitones above the root and on notes with a tuning offset to confirm the rounding and cents terms of the rate law.
- Single-note hardware recording for the attack time and the absolute level.
