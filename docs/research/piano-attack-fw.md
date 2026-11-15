# Piano attack: what the firmware says, and the key-up sample record wired into the slew path

- **Checked:** 2026-11-15
- **Sources (local, never committed):** `sharc-ref` / ARM replay results already recorded in [smp-slew](smp-slew.md) s6 (note-on and note-off message lists, 4 sounds, 4 velocities), [piano-decay-attack](piano-decay-attack.md), [piano-transcription](piano-transcription.md), Part A clips through `tools/emu_vs_hw.py --events-from transkun` (Velvet Grand A:61, Felt Upright A:13), `tools/piano_decay.py`; code `crates/nord-core/src/piano/slew_voice.rs`, `piano.rs::spawn_release_sample`.
- **Question:** Which firmware word or equation makes the chain + slew attack slower than the hardware (and the default engine), and can it be fixed opt-in?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. The firmware has no attack-shaping word

| Stage | Result | Tag |
|---|---|---|
| ARM note-on to SMP record | no ramp: `F-2 = 0.25`, `F-1 = 0`, `F2 = 0`, `F3 = 0.5` (handler), gain `0.21875 * level` from the first update | VERIFIED (4 sounds x 4 velocities, [smp-slew](smp-slew.md) s6) |
| Extra voice | `F3 = F-1 = 0`, gain 0 | VERIFIED |
| Sample onset | the decoded stroke itself; the slew does not touch it | VERIFIED words; consistent with the renders below |
| Hammer / string stages in the PNO or FX image | not found (unison, dynamics and timbre send nothing, [piano-dsp-path](piano-dsp-path.md)) | open |

So the slew voice has the same attack as the default engine **by construction**: single notes at MIDI 48 / 60 / 72 on Felt Upright and Velvet Grand give identical 10 to 90 % rise times for the default and `--piano-slew-fw` (16 / 6 / 17 ms and 12 / 44 / 6 ms, [piano-decay-attack](piano-decay-attack.md) s1). The slower attack of chain + slew in the clips (68 to 70 ms on Velvet / Felt against 54 to 55 ms default, hardware 56 to 64 ms) is not a firmware attack word.

## 2. What differs in the mix (VERIFIED renders)

1. **Key-up record not wired.** In slew mode the key-up sample played with the engine constants (`amp * 0.30`, decay 1.15 s) on a main voice whose `amp` is 1.0 (the level lives in the slew), i.e. about `1 / level` too loud against the note and several times longer than the firmware record (`F17 = 0x3F7FF38E`, time constant 0.24 s; `F15 = 1.0`, 0.922 at velocity 127, `F-2 = 0.25`, `F19 = F20 = 0.875`: gain `0.21875 * F15`). A short Velvet Grand note (MIDI 55, 170 ms) under slew peaked at -22.7 dB at velocity 64 and 81 alike (the key-up sample, independent of velocity), 10 to 14 dB over the same note's own slew peak (-33 dB at velocity 80 for a 1 s note).
2. **Pedal-down notes bypass the slew** (25 to 77 % of the clip notes; [piano-decay-attack](piano-decay-attack.md) s6): the chain + slew clips mix slew and legacy voices.
3. **Absolute slew level** is 19.4 dB under the default at every velocity (0.21875 and the muted extra voice; bus scale open, OQ20).

## 3. Change (opt-in, `--piano-slew-fw` only)

`SlewVoice::start_release_sample` plays the key-up sample of a slewed voice with the firmware record: gain `0.21875 * F15(velocity)` (`F15` words `0x3F7FFFFE` up to velocity 100 and `0x3F6C1CEA` at 127, linear between: ASSUMED), decay by `F17 = 0x3F7FF38E`, no note-off write (`release()` is ignored). A pedal-down voice (legacy path) keeps the engine constants. Tests: `slew_voice::tests::release_sample_level_words_and_law`, `release_sample_slew_decays_by_its_rate_and_ignores_the_release_write`, `extra_voice::tests::slew_note_off_starts_a_firmware_release_sample_not_the_engine_constant`. No default changes; `render_hash` is unchanged (the golden render uses the synthetic sound without slew).

## 4. Clip comparison, before and after (transcribed velocities and pedal, `emu_vs_hw.py`)

Emulator minus hardware; RMS dB, attack ms (hardware / emulator), score (lower is better).

| Clip, variant | RMS | presence / brilliance | attack | hold dB hw / emu | release dB hw / emu | score |
|---|---|---|---|---|---|---|
| Velvet default | -2.6 | +1.9 / +3.9 | 64.4 / 54.2 | -6.37 / -3.89 | -4.17 / -3.11 | 1.37 |
| Velvet chain + slew, before | -4.0 | +5.0 / +7.9 | 64.4 / 67.8 | -6.37 / -3.59 | -4.17 / -2.90 | 3.25 |
| Velvet chain + slew, after | -5.4 | +5.6 / +9.2 | 64.4 / 68.9 | -6.37 / -3.57 | -4.17 / -2.90 | 3.74 |
| Felt default | -4.2 | +2.3 / +2.0 | 55.7 / 54.8 | -6.09 / -5.14 | -4.08 / -3.74 | 1.37 |
| Felt chain + slew, before | -8.0 | -1.5 / -4.7 | 55.7 / 68.2 | -6.09 / -5.32 | -4.08 / -4.87 | 2.18 |
| Felt chain + slew, after | -9.7 | -0.7 / -3.1 | 55.7 / 62.1 | -6.09 / -5.35 | -4.08 / -4.06 | 1.31 |

Reading: the firmware key-up record moves Felt Upright toward the hardware (score 2.18 to 1.31, attack 68.2 to 62.1 ms, release metric -4.87 to -4.06 against -4.08) and Velvet Grand away from it (score 3.25 to 3.74, brilliance +7.9 to +9.2). Level drops 1.4 to 1.7 dB on both (the shorter, quieter key-up). The two clips disagree, so the path stays opt-in (D22); the attack stays 7 to 14 ms slower than the hardware and 8 to 14 ms slower than the default engine because of the mix (items 2 and 3), which no per-note attack word explains.

## 5. Ensemble check of the hardware attack

Averaging the 4 ms envelope of all notes aligned at the transcribed onset (Velvet 188 notes, Felt 144), the hardware reaches 50 % of its rise 14 to 17 ms later than the default engine (Velvet +24 against +7 ms, Felt +20 against +7 ms). This bounds the hardware-minus-default attack at about 15 ms, but the transcriber onset jitter smears the hardware average and the emulator has none, so it is an upper bound, not a measurement (EMPIRICAL).

## Implications

- No firmware word shapes the attack: the default engine and the slew voice play the sample's own onset, and the remaining hardware difference (at most about 15 ms) is inside the transcriber's onset jitter.
- The chain + slew attack and level gap of the clips is a mix of path effects (pedal bypass, key-up record, open bus scale), not a mis-set decay or attack constant.

## Follow-ups

- Decode the firmware voice list of damper-down strokes so the slew covers pedal-down notes ([piano-decay-attack](piano-decay-attack.md) s6); re-run the clips afterwards.
- ARM note-off lists at velocities 101 to 126 for the `F15` law; the rate table of sounds other than Velvet Grand.
- Single-note hardware recording (known velocity, pedal up) for the true attack and absolute level.
