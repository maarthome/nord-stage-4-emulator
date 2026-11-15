# Reverb per type: colour words and tail times

- **Checked:** 2026-11-15
- **Sources (local, never committed):** the ARM image `target/fw_unpacked/bank0_CODE.bin` and `bank0_SRAM.bin` (OS v1.66), the settled snapshot, `tools/fx_rvb_color_capture.py` (the `unicorn` harness of [fx-contexts](fx-contexts.md)). Derived data committed: [data/fx-reverb-colors.csv](data/fx-reverb-colors.csv) (the colour deltas, 306 words).
- **Question:** audio item 7 of [audio-remaining](audio-remaining.md): which parameters does the firmware write per reverb type and variation, and which tail time does it use (Grand Pad clip: 13.4 s hardware, 3.7 s emulated)?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. Short answer

| Question | Result | Tag |
|---|---|---|
| Type and variation to `kRvbType` | model `t / 2`, variation `t & 1` (value-name list `0xC02CD2BC`) | VERIFIED ([fx-reverb-names](fx-reverb-names.md)) |
| Does `kRvbColor` (Normal, Dark, Bright) reach the DSP? | **Yes.** The handler rewrites a full 64 word block per `(type, colour)`; against the type's own block 11 to 20 words differ: the output filter words `DM 0xE0C46 .. 0xE0C4C` (7 words, every type), 4 to 5 instance words (`0xC1A2D`, `0xC1A3D`, `0xC1A3F` (Bright only), `0xC1A40`, `0xC1A41`) and, for the pair types 7, 9, 11, 6 table words (`0x9438E`, `0x94390`, `0x94391`, `0x9439F`, `0x943A1`, `0x943A2`). Three more words (`0xB2F25`, `0xB2F26`, `0xB2FA1`, types 7, 9, 11) lie outside the reverb instance and are kept in the CSV only | VERIFIED (run, 10 types x 2 colours, window 20 M instructions) |
| Colour names | the list after the type names is `Normal, Dark, Bright` = value 0, 1, 2 | VERIFIED ([fx-reverb-names](fx-reverb-names.md)) |
| Tail time of the firmware routine | 0.52 to 1.35 s (T60, Schroeder T20 extrapolated, at the 43,402 Hz DSP frame rate); colour moves it by up to 36 % (type 7: 0.995 s Normal, 1.35 s Dark and Bright), the other types by under 15 % | words and routine VERIFIED, tails measured by `tests_fw_rvbm_color.rs` |
| Tail of the engine FDN against it | Booth 0.18 s (firmware 0.56), Room 0.60 (0.75), Stage 1.55 (1.55; 0.90 with the variation), Hall 3.44 (1.20), Cathedral 7.93 (1.19) | MEASURED, `tests_fw_rvbm_tail.rs` (Hall, Room asserted) |
| Grand Pad tail 13.4 s | **not the reverb**: the program's reverbs are piano type 3 (Booth Predelay, T60 0.62 s) and synth type 4 colour 1 (Room, 0.76 s); no firmware reverb tail exceeds 1.4 s. The hardware figure includes the sustain pedal of the player (not replayed) and a pad with a 45 s amp decay | EMPIRICAL (structural: bounded by the routine; no clip isolates the reverb) |

## 2. Method

`tools/fx_rvb_color_capture.py`: restore the settled firmware, apply a different type, then the type under test (colour 0 block), or type then `kRvbColor` 1 or 2, and record every DSP word message of the last switch on the synth unit L6 (core 2 row 0). `tools/fx_rvb_colors.py` keeps the words that differ from the same type's colour 0 block and writes the CSV and `fx/fw_rvbm_colors.rs` (words inside the port's windows only).

Rust: `ReverbMulti::set_color` restores the type's words at every address a colour touches, then writes the delta (the ARM does not clear the delay line on a colour switch, neither does the port). `set_type` keeps the colour. `LayerFxChain` maps `ReverbColor::{Normal, Dark, Bright}` to 0, 1, 2; `Chorale` is the engine's name of the variation flag already in the type, so it keeps colour 0.

## 3. Open and not claimed

| Item | State |
|---|---|
| Oracle run of a colour block | not done: the FX SRAM images of `tools/fx_rvbmulti_check.py` are not in this checkout. The routine is the code already verified; only data words change, so the word table is VERIFIED against the ARM and the output is **not** compared word for word with the oracle |
| Tap table words | this harness writes the initial block (`0x80000000`, `0x7FFFFFFE`, ...) plus the length words at `0x94384 ..` for every type, while the committed type words (`fx-type-words.csv`, the golden source) carry type specific tap numbers there (39 of 64 words differ, the same for every type). The origin of the committed tap words was not reproduced by this harness. The colour delta is taken against the harness's own colour 0 block, so it holds either way |
| Type 6 (Stage, no variation) | its 64 words (`fx-reverb-type6.csv`) are the same harness block, not the tap form of the other types, so it is not ported; its colour delta is in the CSV only |
| Spring types 0, 1 | colour writes 39 words each; not ported (the engine FDN colour damping applies) |
| Organ rows | unchanged (zero in the image) |
| Rate | the routine runs at the DSP frame rate 43,402 Hz; the engine calls it at 48 kHz, so its tails are 10 % short in time (not corrected) |
| Hardware clip | none isolates a reverb tail; defaults unchanged (D22) |

## Implications

- The per-type reverb parameters of a program are `kRvbType` plus `kRvbColor`; the port now covers both for the nine multi-tap types behind `FirmwareFx::reverb`.
- The engine FDN is 2.9 times (Hall) and 6.7 times (Cathedral) longer than the firmware routine. Nothing here shows the hardware tail, so the FDN stays the default.

## Follow-ups

1. A hardware clip with a clean key release into a dry piano or organ with Hall / Cathedral reverb at amount 127 to measure the real tail.
2. Reproduce the tap words (harness state) and run the oracle on the colour blocks.
3. Port type 6 and the Spring colour words.
