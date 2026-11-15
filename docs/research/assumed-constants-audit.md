# Audit of the ASSUMED constants: what the firmware settles without hardware

- **Checked:** 2026-11-14
- **Sources (local, never committed):** OS v1.66 `target/fw_unpacked/bank0_CODE.bin` (host ARM image), the settled ARM harness (`target/settled.snap`, `tools/arm_edit_capture.py`), the DSP images (`target/DSP/`, `tools/ns4_dsp_ldr.py --sram`, `sharc-disasm`), the committed captures under `docs/research/data/`.
- **Question:** which of the `ASSUMED` constants in `crates/nord-core/src` with the highest audible impact (glide, Electric timbre, knob laws, rotary and organ rates, FX rate laws, velocity curves, reverb) can be settled from the firmware alone, and which need hardware?

Tags follow `.agents/rules/provenance-in-code.md`. The census: 291 `ASSUMED` mentions in `crates/nord-core/src` (before this change), most of them in the opt-in firmware stage models (voice weights, P blocks, context words) or in menu behaviours with no audible effect (arp detents, pedal assignments, MIDI menu groups).

## 1. Result table

| # | Constant (file) | Before | After | Source / reproduce | Engine change |
|---|---|---|---|---|---|
| 1 | Rotary Horn / Rotor Speed choices: speed factors 0.85 / 1 / 1.15 (`engine/settings.rs`, `organ/globals.rs`) | ASSUMED | **VERIFIED** | ARM `0xC0112BD0` / `0xC0112C7C`: horn 0.74 / 0.80 / 0.86 and 6.5 / 7.0 / 7.5 Hz, rotor 0.65 / 0.73 / 0.76 and 5.7 / 6.4 / 6.65 Hz. `python tools/arm_leslie_settings.py target/fw_unpacked/bank0_CODE.bin`, [leslie-settings](leslie-settings.md), `organ/rotary_laws.rs` | yes (tables) |
| 2 | Rotary Horn / Rotor Acceleration: tau 1.5 / 1 / 0.67 (`engine/settings.rs`) | ASSUMED | **VERIFIED** (ratios) | ARM `0xC0112C14` / `0xC0112CC8`: step per update horn 0.03 / 0.06 / 0.12, rotor 0.006 / 0.01 / 0.02 up, 0.012 / 0.0218 / 0.024 down; same tool | yes (tau ratios, rotor brakes faster) |
| 3 | Base rotary speeds 0.80 / 0.67 slow, 6.85 / 5.70 fast (`organ/rotary.rs`, spec `organ-engine.md`) | ASSUMED (Hammond 122 textbook) | **VERIFIED** | Normal block of the same handlers = settled harness state (0.8 / 7.0, 0.73 / 6.4 Hz nominal) times the DSP frame ratio 0.98418 (`dsp-frame-rate.md`): 0.787 / 6.889 and 0.718 / 6.299 Hz | yes: `ns4 bench` hashes move |
| 4 | Pitch Stick Range list and ceiling 12 (`engine/pstick.rs`) | ASSUMED | **VERIFIED** | firmware strings `bank0_CODE.bin@0x2D3479`: `1, 2, 3, 4, 5, 7, 10, 12, -12, -24 semi`, id range 0..9, default 1 = 2 semi. `python tools/ns4_fw_choices.py` | list constant and test only (the GUI still steps 1..12; 6, 8, 9, 11 do not exist on the hardware, the negative entries invert the stick: follow-up) |
| 5 | Vibrato delay `0.1 + 2.9 raw / 29` (`cbin/synth_preset.rs`) | untagged | **VERIFIED** | firmware strings `@0x2CD714..0x2CD7FC`: 30 choices `0.1 s .. 3.0 s`, ids 1..30, default 7 = 0.7 s (the 0.7 s of the Audible Genius clip). Same tool | formula unchanged, test added |
| 6 | Strip pass length 2 SY frames, ADR attack / decay mapping (`synth/strip_map.rs`) | ASSUMED | **VERIFIED** (index law) | [synth-knob-laws](synth-knob-laws.md) s3 (ARM writes `knob - 64`; 2 frames per pass reproduce the displayed attack at plateau starts, ratio 1.000 to 1.011) | retag only; `DECAY_TAU_FRACTION` stays EMPIRICAL 0.18, see s2 |
| 7 | Vibrato rate `2 + 6 raw / 127` (`cbin/synth_preset.rs`) | ASSUMED | ASSUMED (shape checked) | ARM words are 61 steps `round(raw 60 / 127)`, 2.0 to 7.90 Hz at `word / 0x30500` ([synth-knob-laws](synth-knob-laws.md) s3); the engine value is within 0.1 Hz. The unit of the word is not measured (clips read x1.01 of the display), and a quantised law would break lossless `.ns4p` round trips | none |

## 2. Findings that keep the old value on purpose

- **Decay time constant fraction (0.18).** The firmware law is not a constant fraction. The displayed decay is the time to fall 40 dB at the first knob of each plateau of the rate table (`tau = display / ln 100 = 0.2171 x display`) and the ratio drifts from 0.225 down to 0.169 across the plateau, because the rate index repeats while the display keeps rising. The laser-zap clip sits mid-plateau (display 2.07 s, tau 0.38 s, 0.18). Setting 0.2171 in `strip_map.rs` moved the `engine` golden segment by 0.12 % RMS (the `golden_render` fingerprint failed) and would make that clip worse, so the constant stays and layers with stored knobs use the exact index (`knob_laws`, default, D57).
- **Vibrato rate quantisation** (item 7): the committed `.ns4p` encoding is lossless only with the 128-step mapping.

## 3. What was tried and not settled

| Constant | What was tried | Why it stays ASSUMED |
|---|---|---|
| Glide `(knob/10)^2 * 2 s` per octave (`synth_params.rs`) | The ARM sends the rate index `knob - 64` to strip 0 word 55 with the ADR law (`synth-knob-laws.md` s3). Listed ORG core 2 (`sharc-disasm`, 69,000 lines) for readers of strip offset 55: one unrelated hit (`DO (PC, 55)`). The strip words are reached through pointer plus modifier sequences, so no immediate offset exists to grep | The DSP code that turns the index into a pitch slew was not located. Next step: a read watch on `DM(0xE0037)` in `sharc-pass` with a key-down and a glide note, then the slew equation (constant rate in semitones or per pass) |
| Electric Soft / Mid / Bright gains and the 2.2 kHz bell (`piano.rs`) | `docs/research/piano-dsp-path.md` and this pass: the Piano timbre ids send no DSP word in the harness (`kPianoTimbre`, `kSmpTimbre` write the same words for every value); `pno-module-chain.md` lists `0x1229B3` (FIR plus three first-order sections) as the candidate with no numeric tie | The timbre curves are not in an ARM table; they live in the sound data (`.npno`) or in DSP coefficients loaded by SPI download that the harness does not run. Needs a hardware sweep of the three timbres on an electric model |
| Aux KB Soft / Hard velocity exponents 0.7 / 1.4 (`engine/aux_kb.rs`) | The ARM holds nine velocity curves, `[0xC02B3128 + 0x18 + 2 (128 curve + vel)]` (`data/keydown-velocity-curve.csv`). Fitted `v^p` to each normalised curve: curve 5 is linear (p 0.98, top 1016), curves 1, 2, 3 are p 1.76, 1.43, 1.25 (tops 478, 560, 672), 6, 7, 8 are 1.6 to 1.8, curves 0 and 4 are identical (p 3.0) | No firmware curve is concave, so a "Soft" exponent below 1 has no counterpart; the writer of the selector `[manager + 0x134]` and the map from the three menu choices (`kMIDIVelCurve`) to the nine curves were not found (the Sys store is not a dispatcher client in the harness, `find_stores` lists only the ten layer clients) |
| Tonewheel Mode crosstalk factors, Click Level (`engine/settings.rs`) | Handler of `Edt_Sys_kToneWheelMode` (`0xC011CBFC`): stores the mode `0..3` into the per-layer objects (`[obj + 0x34] + 0x1C`) and sets a dirty flag; the effect is applied later | The consumer of that field and the DSP words it selects were not traced |
| FX Mod 1 rate law `0.1 + 14.9 x` Hz (`engine/program_fx.rs`) | [fx-mod-types](fx-mod-types.md) s2.3: the firmware A-PAN / TREM word is `12800 + 16384 v`, 0.0323 Hz at 0 and 5.29 Hz at 127 for one call of 4 frames | Whether the piano / organ units call every 2 frames (a factor 2) is open (OQ21) and no hardware recording of the rate exists; the firmware path stays opt-in (`FirmwareFx::mod1`) |
| B3 vibrato / chorus scanner depths and 6.85 Hz (`organ/scanner.rs`), organ percussion times | `b3-vibrato-tail.md`: the vibrato type words are zero in the harness (the tables come from SPI downloads the harness stubs); percussion ids move about 250 weight words, the decay itself is DSP code (OQ19) | No ARM-side table; needs the download path or the DSP percussion stage |
| Trigger point delay 8 ms (`engine/organ_trigger.rs`) | Handler `0xC01200AC` ff. for `Edt_Sys_kOrganTrigPoint` | It selects a key contact for the organ trigger, no time constant exists in the handler; a keybed timing is hardware |
| Reverb sizes, delay mix, shelf corners (`fx/tables.rs`, `fx/delay.rs`) | Not retried: the firmware stage models (`fx/fw_*`) already carry the verified words and are opt-in pending hardware clips (D22, D60) | See D22 |

## Implications

- The Leslie menu choices needed no hardware: the firmware stores them as constants, and the engine defaults for the rotor were 0.6 % (horn) to 9.5 % (rotor) off.
- Two audible laws remain gated on one DSP read (glide) and one clip (FX rate), not on the ARM.

## Follow-ups

1. Glide: watch reads of strip word 55 in the ORG core 2 pass (see s3).
2. GUI: step the Pitch Stick Range through `PSTICK_RANGE_LIST` and model the inverted `-12` and `-24` entries.
3. Find the writer of the velocity curve selector (`[manager + 0x134]`) by driving the MIDI menu in `tools/arm_menu_edit.py` (needs the booted OLED state).
4. Read the rotor update period (caller of `0xC0209728`, timer) to turn the Leslie steps into seconds.
