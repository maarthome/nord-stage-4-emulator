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

| Constant | Before | After | Tag |
|---|---|---|---|
| Horn slow / fast (engine Hz) | 0.80 / 6.85 (Hammond 122 textbook) | `0.80 x r`, `7.0 x r` = 0.787 / 6.889 | ASSUMED -> VERIFIED |
| Rotor slow / fast | 0.67 / 5.70 | `0.73 x r`, `6.4 x r` = 0.718 / 6.299 | ASSUMED -> VERIFIED |
| Speed choices | x 0.85 / 1 / 1.15, both speeds | tables above, per speed and per rotor | ASSUMED -> VERIFIED |
| Acceleration choices | tau x 1.5 / 1 / 0.67 | horn tau x 2 / 1 / 0.5; rotor tau up x 1.667 / 1 / 0.5 | ASSUMED -> VERIFIED (ratios) |
| Rotor braking | same tau as speed-up | tau x `0.01 / 0.0218` (x 0.46 at Normal) | ASSUMED -> VERIFIED (ratio) |
| Base tau 0.7 s horn, 4.2 s rotor | | unchanged | ASSUMED (the ARM update period is not read) |

`r = (44_444_000 / 1024) / 44_100 = 0.98418`: the ARM converts Hz to the speed word with the nominal 44.1 kHz frame rate, the DSP runs at 43,402.34 Hz ([dsp-frame-rate](dsp-frame-rate.md)), so the horn that the firmware calls 7.0 Hz turns at 6.889 Hz. The firmware Leslie path (`--leslie firmware`) maps the engine Hz back to the nominal Hz with `1 / r`; with Normal it produces the same words as before (the old piecewise map ended at the same points), so those renders are unchanged.

## 3. Not settled

- The update period of the rotor tick in seconds (the steps are Hz per update): the absolute ramp times stay at the engine's 0.7 s / 4.2 s (ASSUMED). The ratio between choices and between speed-up and braking is verified.
- Whether the menu value 0 is labelled Low on the panel (the table is the identity and the default 1 is Normal; the choice names come from the manual).

## Implications

- Rotary speed choices no longer need a hardware timing run; only the absolute ramp time does.
- The engine fast rates were 0.6 % (horn) and 9.5 % (rotor) off the firmware values; the rotor slow rate was 7 % low.

## Follow-ups

1. Read the periodic rotor update interval (the caller of `0xC0209728` and the timer period) to turn the steps into seconds.
