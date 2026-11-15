# Piano: the program Touch velocity curves (KB TOUCH)

- **Checked:** 2026-11-15
- **Sources (local, never committed):** OS v1.66 firmware (`target/fw_unpacked/bank0_CODE.bin`, `bank0_SRAM.bin`, ARM base `0xC0000000`), factory backup `Nord Stage 4 Factory Restore RevK v1.66.ns4b` (Clavinet D6 `.npno`, Felt Upright program) under `.reference/`, `docs/research/arm-edit-params.csv`, `ns4decode` (`ns4names.py`, `ns4maps.py`).
- **Question:** Which velocity curves does the per-program Touch setting (Piano `KB TOUCH`, Organ, Synth) select in the firmware, do they apply to MIDI notes, and what are the tables?

Tags follow `.agents/rules/provenance-in-code.md`. Bean `ns4-yp85`, decision D88.

## 1. Result

| Item | Result | Tag |
|---|---|---|
| Which sections have a Touch | Only the Piano section: edit id `0x159` `Edt_Pno_kKbdTouch`, values 0..=2, default 1. No Organ or Synth Touch id exists in the 422-entry registry (`arm-edit-params.csv`); organ key messages carry no velocity (OQ23) and the Synth keeps its own amp velocity parameter. | VERIFIED (registry, disassembly) |
| What the value selects | Handler `0xC011AA24` stores `curveId = u32[0xC02BBF98 + 0x678 + 4 * value]` = `10, 11, 13` into field `+0x1C` of the piano layer object | VERIFIED (disassembly; run: the program recall calls the handler with index 1 for the default program and `[ctx + 0x1C]` reads 11; the table words read `10, 11, 13, 4, 4, 5`) |
| How it is used | Voice init `0xC00AA520`: `T = byte[0xC0532664 + (curveId << 10) + r8]` (`ldrb lr, [ip, r8]` at `0xC00AA654`), stroke selection index = `T`, level attenuation `X = clamp(((128 - 0x13) T) >> 7, 0, 249)` | VERIFIED (disassembly, `piano-sound-load.md` s7, s8) |
| Applies to MIDI notes | Yes: a MIDI note-on through the UART2 parser (`midi_parser` -> `midi_note_on_post`) reaches the voice init and reads the curve of the layer object. The Touch is part of the sound, not only of the keybed profile | VERIFIED (run) |
| Tables | `T(v)` for `v = 1..=127`, three curves, in `piano/touch_curve.rs` (curve 11 is `T_BY_VELOCITY`, unchanged); fingerprint pinned by a unit test | VERIFIED |
| End to end | With `[ctx + 0x1C]` set to 10, 11 and 13 and velocities 1, 2, 10, 33, 64, 90, 100, 120, 127 the byte the ARM loads equals the composed table: 27 of 27 (`tools/arm_touch_curves.py arm-check`) | VERIFIED (run) |
| Names | Edit 0 = heavy, 1 = medium, 2 = light | EMPIRICAL (monotone order of the curves, `ns4decode` `PianoTouch`) |
| Program file bits equal the edit value | File bit `1991..1992` (layer A), `+96` (layer B) = edit value: 6 of 6 in the program recall (s4); factory set 156 / 208 / 20 for 0 / 1 / 2 | VERIFIED (run) |

## 2. Tables (derived numbers)

`r8(v) = s16[0xC02B3128 + 0x18 + 2 (v + 128 * 6)]` (`r8(1) = 17`, `r8(64) = 203`, `r8(127) = 656`; curve 6 of the 8 internal velocity curves is in force at the default program, `[manager + 0x134]`). `T = curve[curveId][r8]`.

| v | T heavy (id 10) | T medium (id 11) | T light (id 13) | X heavy / medium / light |
|---|---|---|---|---|
| 1 | 200 | 200 | 200 | 170 |
| 10 | 192 | 178 | 163 | 163 / 151 / 138 |
| 33 | 143 | 131 | 118 | 121 / 111 / 100 |
| 64 | 88 | 78 | 68 | 74 / 66 / 57 |
| 90 | 49 | 40 | 32 | 41 / 34 / 27 |
| 100 | 35 | 27 | 19 | 29 / 22 / 16 |
| 120 | 8 | 2 | 0 | 6 / 1 / 0 |
| 127 | 0 | 0 | 0 | 0 |

`X` steps of 2 per level index (0.1003 dB each, `level_q23.rs`), so at velocity 64 the light curve is 1.8 dB louder than medium and the heavy one 1.6 dB quieter; at velocity 10 the spread is +2.6 / -2.4 dB, at 120 +0.2 / -1.0 dB. `T` also moves the stroke layer (the record threshold is compared with `T`), so the timbre changes with it.

## 3. Method

1. `tools/arm_xref.py dis` of `0xC00D1FD0..0xC00D2130` (keybed event handler) and `0xC00AA520..0xC00AA6D0` (voice init) gave the table reads; `0xC011AA24..0xC011AA3C` the Touch handler. The words at `0xC02BBF98 + 0x678` read `10, 11, 13, 4, 4, 5`.
2. A run of the settled firmware (`tools/arm_keydown.py` rig, Clavinet D6 loaded with `tools/arm_piano_load.py`) with a MIDI note-on over UART2 logged `curveId = [r6 + 0x1C] = 11` and `r8 = 17 / 203 / 656` at velocity 1 / 64 / 127 for the default program.
3. A write watch on `[ctx + 0x1C]` showed the single writer `0xC011AA38` (the handler above), called by the edit dispatch with `curveId` from the table.
4. `[ctx + 0x1C]` set to each curve id, same note-on: the byte at `0xC00AA658` (`T`) equals `curve[id][r8(v)]` for 27 of 27 pairs. Reproduce: `uv run --with unicorn --with capstone python tools/arm_touch_curves.py arm-check <CODE> <SRAM> --cache target/keydown.snap --ns4b <backup> --npno "Piano/Clavinet/Clavinet D6  6.1.npno"`.
5. `python tools/arm_touch_curves.py rust <CODE>` regenerates the arrays and the fingerprint `0xc8293ca1773436e4` the Rust test pins.

## 4. Program file bits (VERIFIED by the program recall; the Rust decoder reads other bits)

The firmware recall (`tools/arm_program_load.py` boot with Felt Upright, then `session.run(300)`, hook on the Touch handler `0xC011AA2C` and on the store setter `0xC00D051C`) with the two bits at file bit `1991` of layer A and `2087` of layer B set to 0 / 1 / 2 (layer A) and 1 / 2 / 0 (layer B): the setter value and the handler index equal the file value in all 6 of 6 cases (layer A index = value, layer B index = value of its own bits). So the file value is the edit value: **0 heavy (curve 10), 1 medium (11), 2 light (13)**, the field sits at file bit `1991..1992` (piano payload bit `167`, layer base + 63; layer B `+96`). That is the `ns4decode` position (`249-8 .. 250-1`).

Over the 384 factory programs the field reads 156 x 0, 208 x 1, 20 x 2 and never 3 (Felt Upright layer A reads 1, medium). The Rust decoder (`cbin/piano_preset.rs`) reads its `touch` 10 bits later (layer base + 73, always 0 in the factory set, decoded as `Medium`), so every factory program decodes as Medium and the program Touch is lost; its neighbouring fields (`variation`, `acoustic_level` = string resonance and soft release, `pedal_noise`) sit on the same shifted bits: against `ns4decode` the bits `1988 / 1989 / 1990` are soft release / string resonance / pedal noise (4 / 241 / 257 ones in 384 programs), and the decoder's `variation` pair at `1988..1989`, `acoustic_level` pair at `1990..1991` and `pedal_noise` at `1992` read the same bits under other names. That alignment is not touched here (it changes string resonance, soft release and pedal noise of every program); follow-up 1.

## 5. Implications

- Opt-in only: `PianoLayer::firmware_touch` / `--firmware-piano-touch` / GUI Firmware bar "Touch curves". Off: every `.npno` note plays the Medium curve (the factory default), the other sounds the ASSUMED power curves. The default stays off because the decoder does not yet read the program's Touch (s4): until it does, the switch changes the sound only for a Touch set by hand (GUI / control edit), and no program loaded from a file.
- On: `.npno` sounds with a level model play the layer's curve (stroke and level); other sounds scale the Medium response by `2^(-2 (X - X_medium) / 60)` (only the Touch difference is firmware derived).
- Defaults and `render_hash` do not change.

## 6. Follow-ups

1. Realign the Piano layer decoder and encoder (`cbin/piano_preset.rs`, layer base + 60..+75): soft release `1988`, string resonance `1989`, pedal noise `1990`, touch `1991..1992` (file bits), variation at `1938..1939`; check each against the firmware recall and the factory set (`ns4 preset-check` must stay 937 of 937), then decide the default flip of the Touch (D22) and re-baseline `render_hash`.
2. The MIDI velocity curve of the System menu (`Edt_Sys_kMIDIVelCurve`, `Edt_Ext_kVelCurve`, `Edt_Com_kAuxKbdVelCurve`: three values each) selects the other velocity table (`r8` curves 1, 2 and the default 6, `0xC00D2024`); not modelled.
3. Compare the Felt Upright and Velvet Grand clips under the three curves once the file bits are known (the clips are Medium).
