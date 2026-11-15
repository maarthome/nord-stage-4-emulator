# Audio parity: what is still open (consolidated list)

- **Checked:** 2026-11-15
- **Sources:** merged PRs #373 to #391, the open PRs #392 to #395 (their notes and decisions D84 to D86 are not on main yet) and the research notes they cite ([organ-preamp-search](organ-preamp-search.md), `fm-banks-1-2` (PR #392, not on main yet), [fx-chorus-hw-clip](fx-chorus-hw-clip.md), [piano-transcription](piano-transcription.md), [organ-presence-gap](organ-presence-gap.md)), `docs/OPEN-QUESTIONS.md` (OQ20, OQ21, OQ22, OQ24), `docs/ROADMAP.md`, the Part A demo video comparisons (`docs/research/data/emu_vs_hw_segments.json`) and a listening test of 2026-11-15 (Velvet Grand A:61, Felt Upright A:13).
- **Question:** which audio items still stand between the emulator and 1:1 parity with the hardware, what evidence each has, and how each can be closed?

Tags: **VERIFIED** = matches firmware or oracle, **EMPIRICAL** = fitted to a recording, **ASSUMED** = chosen without evidence. A default flips only when verified end to end (D22).

## 1. Open items

| # | Item | State | Evidence so far | How to close | Check |
|---|---|---|---|---|---|
| 1 | Synth layer level law | **closed (D87):** the handler `0xC011BB48` run live stores `(v / 127)^2`; squared is the default (`--legacy-synth-layer-law` restores linear); Leaky Stabs II score 3.19 to 1.95, Grand Pad 1.48 to 1.42 | [synth-layer-level-law](synth-layer-level-law.md) | open only: Reverb GLOBAL on (cell 3.25) and the DSoundman fader taper | ARM harness run (done) |
| 2 | Sample pitch entry `e` | `--sample-pitch fw` stays opt-in (D65, OQ22) | join arithmetic and entry format verified, the ARM writer of the 16-bit table entry at `DM(0x9354A + index)` is not found | trace the ARM writer like the P words ([synth-p-writer](synth-p-writer.md)) | ARM harness |
| 3 | Organ vs piano level `K = 2.8` | EMPIRICAL (OQ24); image audit found no `1 / 0.21875` lane word (D85, PR #394, not on main yet) | [organ-output-law](organ-output-law.md) s4 | find the gain chain (ORG image plus ARM level words) | oracle trace |
| 4 | Touch velocity curves | emulator uses the raw MIDI velocity; bean `ns4-yp85` | listening test: key force changes the piano sound a lot | extract per-program curves from the firmware, apply opt-in | ARM harness |
| 5 | Mouse velocity | position along the key, linear 20 to 127 (D84, ASSUMED; PR #393, not on main yet) | no listening check | listen; replace by the Touch curve when item 4 lands | listening |
| 6 | Modulation FX per type | CHOR (D32), VIBE (D37), FLANG and PHAS 0 (D40) run the firmware routines behind `FirmwareFx::mod2`; ENS and SPIN routines are verified and ported (`fw_ens.rs`, `fw_spin.rs`, D40) but not wired to a stage (ENS output silent, SPIN diverges with the captured words); PHAS 1, Mod 1 RM / A-WAH / PUMP and the remaining types and the ARM parameter mapping not checked per type | [fx-mod2-types](fx-mod2-types.md) | trace the ARM writes per type, compare with the SHARC models | oracle trace |
| 7 | Reverb per type | names, delay unit and tempo sync verified; per-type parameter mapping and tail time not checked (Grand Pad tail 13.4 s hardware vs 3.7 s) | Grand Pad clip | check the ARM parameters against the SHARC reverb for every type and variation | oracle trace |
| 8 | Grand Pad leftovers | **closed as a replay artefact (D93):** with the transcribed pedal the default reads width 0.46 (0.54 hardware) and tail 6.7 s (13.4 s); the synth rack is irrelevant (EQ mid -5 dB at 700 Hz under a 294 Hz pad: at most 0.1 dB); `--synth-sustain-pedal` opt-in gets the tail to 10.9 s but the score worsens 1.42 to 1.68. SUSTPED decoded (D97): firmware default on, stored per program (381 of 384 set, Grand Pad 1), `--synth-sustain-pedal` follows the bit, default stays until the ARM hold logic is traced. Open: the reverb tail (item 7), the synth stereo path | [grand-pad-leftovers](grand-pad-leftovers.md), [synth-sustped](synth-sustped.md) | ARM harness: trace the synth note hold on the `0xF8` flag | clip comparison |
| 9 | B3 presence and Leslie level | **Closed as a search (D94).** The Leslie feeder was traced: `0xB6000..3` is a harness scratch pair; the real input is the chain lane (scaled by `wet`) plus a gated aux pair (gain `0xB2F3C` = 0) and two DMA input lanes `0xB0078 / 0xB008C`; every feed gives the same -48 dB (the processor is linear), the chain mix has no gain above 1 and the 48 dB is the frozen ring ([organ-preamp-search](organ-preamp-search.md) s6.1). With the B3 right hand as 96 sustained keys (the 525 Transkun notes are Leslie re-triggers) firmware Leslie presence is +1.5 .. +2.8 dB (inside 3 dB), RMS -4.9 .. -6.7 dB; the sub +14 dB is a flat 0..90 Hz rumble of the surrogate ring (s6.2) | not a missing stage; remaining: the DMA ring of the history (`0xB0078` fill, 8-pass restart), mid +3 / brilliance +5 dB, one clip | model the ring DMA; a lone held B3 note clip | `tools/note_offsets.py`, `emu_vs_hw.py --events-from` |
| 10 | FM banks 1 and 2 | stages `0x124269` / `0x12453F` readable; the 128 x 129 row table at `0xE0926` is not produced by the harness (`fm-banks-1-2` (PR #392, not on main yet)) | zero in the image, not written by boot, DSP or a wave edit | run the real strip apply `0xC01CE7D8` with an FM program and log DSP stores | ARM harness |
| 11 | Piano T60 and attack | the 3.2 s vs 1.3 s gap did not reproduce with the transcribed pedal and velocities (D86, PR #395, not on main yet); chain plus slew is 5 to 9 dB low with about 2x slower attack, opt-in | [piano-transcription](piano-transcription.md); `piano-decay-attack` and the per-octave T60 tool (PR #395, not on main yet) | quantify the remaining attack gap; fit PNO parameters; keep defaults (D22) | clip comparison |
| 12 | Synth presets vs video | Horizon Whl (D81) and Leaky Stabs II (D83) fixed in part; the LP24 SUPER type factor 4.4 was fitted on its own clip | [synth-validation](synth-validation.md) s12 | independent clips; item 1 may explain Grand Pad and Leaky Stabs together | clip comparison |
| 13 | FX defaults against hardware | only the chorus has a hardware clip; UP2 tables compared on paper only (fx-chorus-hw-clip s.7): chorus rate positions 0 to 9 inside type 4 (0.25 to 1.27 Hz), 10 to 20 above it; UP2 delay reading 0.75 to 3.5 ms against firmware 0.87 to 3.28 ms (ASSUMED third-party law); EQ FREQ2 is `20 * 1000^(v/1e6)` Hz (EMPIRICAL, 6 %); no reverb IR in the library resources | [fx-chorus-hw-clip](fx-chorus-hw-clip.md) | official Kontakt Player oracle (Ultimate Pianos 2) or more video clips | recording |
| 14 | Synth note transcription | Transkun is piano-trained (recall about 40 percent on synth) | [piano-transcription](piano-transcription.md) | MuScriptor (multi-instrument), later Aria-AMT and ByteDance as second opinions | tooling |
| 15 | Real time Bass and Farfisa | bit-exact but not real time | [organ-farfisa-bass](organ-farfisa-bass.md) | performance work, last in the queue | benchmark |

## 2. Reading

1. Items 1, 2, 3, 4, 6, 7, 8, 9 and 10 can be closed with the ARM harness and the SHARC oracle; no hardware recording is needed.
2. Items 11 and 12 need either the local sample library measurement (derived numbers only) or more hardware clips.
3. Nothing in this list needs bypassing encryption; no firmware image, sample or audio is committed.

## Implications

- Do not call the audio done while item 1 is open: it probably explains Grand Pad and Leaky Stabs II together.
- Several verified paths stay opt-in; the everyday sound improves only when they are promoted under D22.

## Follow-ups

1. Keep this table current in the same PR that changes an item (docs rule).
2. After the running sessions report, mark each row delivered or blocked and move closed items to the decisions.
