# Leslie speed and acceleration choices of the System menu (ARM firmware)

- **Checked:** 2026-11-14
- **Sources (local, never committed):** OS v1.66 `target/fw_unpacked/bank0_CODE.bin` (host ARM image, loads at `0xC0000000`), the settled ARM harness (`tools/arm_leslie_speeds.py`), the capstone listing of `0xC0112AC0..0xC0112FE8`.
- **Question:** what do the System menu entries Rotary Horn Speed / Horn Acceleration / Rotor Speed / Rotor Acceleration (Low, Normal, High) do to the rotor speeds and ramps, so that `Level3::speed_scale` (+-15 %, ASSUMED) and `tau_scale` (1.5 / 1 / 0.67, ASSUMED) can be replaced?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. Result

One host function (`0xC0112AC0..0xC0112FE8`, a switch on the edit id) handles `Edt_Sys_kLeslieHornSpeed` (0x91), `HornAcc` (0x92), `RotorSpeed` (0x93) and `RotorAcc` (0x94). Each handler maps the menu value through the word table `0xC02BBC30` (`0, 1, 2`: the identity) and stores doubles into the rotor state block of [arm-part-activation](arm-part-activation.md) s2. The block is the one the harness shows in the settled state, which is the Normal choice (horn 0.8 / 7.0 Hz, rotor 0.73 / 6.4 Hz, steps 0.06 and 0.01 / -0.0218).

| Setting | Field (block offset) | Low | Normal | High |
|---|---|---|---|---|
| Horn Speed | slow Hz (`+0x48`) | 0.74 | 0.80 | 0.86 |
| Horn Speed | fast Hz (`+0x50`) | 6.5 | 7.0 | 7.5 |
| Rotor Speed | slow Hz (`+0xB0`) | 0.65 | 0.73 | 0.76 |
| Rotor Speed | fast Hz (`+0xB8`) | 5.7 | 6.4 | 6.65 |
| Horn Acceleration | step per update up / down (`+0x18` / `+0x28`, `+0x38`) | 0.03 / -0.03 | 0.06 / -0.06 | 0.12 / -0.12 |
| Rotor Acceleration | step per update up / down (`+0x80` / `+0x90`, `+0xA0`) | 0.006 / -0.012 | 0.01 / -0.0218 | 0.02 / -0.024 |

All VERIFIED (static): the values are IEEE doubles built from `ldr rN, [pc, #imm]` literal pairs and `strd` stores; `tools/arm_leslie_settings.py` evaluates them without emulation (test `tools/tests/test_arm_leslie_settings.py`, Rust fingerprint `organ::rotary_laws::tests::table_equals_the_committed_csv` against [data/leslie-settings.csv](data/leslie-settings.csv)). Normal equals the settled harness block (`tools/arm_leslie_speeds.py`).

```bash
python tools/arm_leslie_settings.py target/fw_unpacked/bank0_CODE.bin
```

## 2. What changed in the engine

`organ/rotor_arm.rs` (PR 357, same day) already carries the Normal section (0.80 / 7.0 and 0.73 / 6.4 nominal Hz, steps 0.06 and 0.01 up / -0.0218 down, the 10 ms update from the period word `0xF0` = 240 ticks at 24,000 per second, physical ratio `F / 44,100 = 0.98418`). This note adds the other two choices of each setting:

| Constant | Before | After | Tag |
|---|---|---|---|
| Speed choices (Low / High) | x 0.85 / 1.15 of both speeds, same for horn and rotor | per setting tables above (`organ/rotor_choices.rs`, `horn_section` / `rotor_section`) | ASSUMED -> VERIFIED |
| Acceleration choices | step stretch 1 / 1.5, 1 / 0.67 (`tau_scale`) | the ARM step per update: horn 0.5x / 1x / 2x, rotor up 0.6x / 1x / 2x, down 0.55x / 1x / 1.1x | ASSUMED -> VERIFIED |
| `OrganGlobals` | `horn_speed_scale`, `rotor_speed_scale`, `horn_tau_scale`, `rotor_tau_scale` | `horn_speed_choice`, `rotor_speed_choice`, `horn_acc_choice`, `rotor_acc_choice` (0..2, default 1) | |

The default engine is unchanged by this note (the Normal section is the one `rotor_arm` already used), so no golden or `ns4 bench` hash moves from it.

## 3. Not settled

- Whether the menu value 0 is labelled Low on the panel (the table is the identity and the default 1 is Normal; the choice names come from the manual).
- The Low and High rotor braking step (0.012 and 0.024 against 0.0218 at Normal) is not monotone in the ARM; the engine copies it.

## Implications

- Rotary speed choices no longer need a hardware timing run; only the absolute ramp time does.
- The firmware stores the Low and High choices as plain constants: no hardware timing run is needed for them.

## Follow-ups

1. None: the update interval (10 ms) is settled in [fx-leslie](fx-leslie.md) s11.
