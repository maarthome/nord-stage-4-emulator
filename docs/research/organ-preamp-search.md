# Organ preamp / tube stage: is there one in the ORG SHARC image?

- **Checked:** 2026-11-15
- **Sources (local, never committed):** `target/DSP/dsp_image_{0_FX,1_PNO,2_ORG}.bin` (OS v1.66) cut with `tools/ns4_dsp_ldr.py --sram`, `sharc-disasm` listings of both cores of each image, `sharc-fx` oracle through `tools/fx_leslie_run.py`, boot log `target/bulk/pno_core1.script`. Builds on [fx-leslie](fx-leslie.md), [fx-type-map](fx-type-map.md) s9, [organ-presence-gap](organ-presence-gap.md), [org-stage-map](org-stage-map.md), [b3-rotary](b3-rotary.md).
- **Question:** the B3 clip reads presence 5.7 dB (firmware Leslie, plain advance) to 20 dB (engine rotary) under the hardware, and the booted firmware Leslie is 48 dB under the engine path. Is the missing piece a separable preamp / saturation / EQ stage in the ORG image that the ARM drives with the Leslie drive word (`Edt_OrgCom_kEffectsLeslieDrv`)?

Tags: **VERIFIED** = reproduced by the commands below, **ASSUMED** = reading.

## 1. Answer in short

| Item | Result | Tag |
|---|---|---|
| Does ORG code touch the Leslie row (`0xB2F21..0xB2F3F`, drive words `0xB2F2D / 0xB2F31`) or the rotor tables (`0xB4E97`)? | **No.** Zero immediate loads of any address in `0xB2F20..0xB2F3F`, `0xB2F8A / B` or `0xB4E97` in either ORG core (`sharc-disasm` of `0x240000..0x2C0000`). Same in both FX cores. The only hit is PNO core 1 (`I6 = 0xB4E97` at `0x24AD6A`). The Leslie is a PNO core 1 processor ([fx-leslie](fx-leslie.md)) | VERIFIED (negative) |
| Is the ORG image an organ tone / preamp program? | No: a channel-strip / parameter engine with no tonewheel generator ([org-stage-map](org-stage-map.md) s1). Its drive-curve tables are the Amp tables (`0xE0802`, `0xE0883`, `0xE0200`) already ported as the Amp drive stage (D22); the B3 and Vox-less programs have the Amp off | VERIFIED (code census), amp off for B3 VERIFIED ([organ-presence-gap](organ-presence-gap.md) s4b) |
| Does the PNO Leslie processor (`0x1256AD`) saturate? | **Barely.** Seeded noise at three levels through the booted oracle, 300 calls: output ratio for x10 input is 9.72 to 9.99 (-0.2 dB at most) and for x3.33 input 3.05 to 3.31 (against 3.33, -0.8 dB at most) on the four output words `0xB6000..3`. A tube-style stage would compress by several dB at these levels, and the engine's `tanh((1 + 3 d) x)` clips hard | VERIFIED (run) |
| Does the drive word change the processor? | No beyond 3 dB ([fx-leslie](fx-leslie.md) s4, s7); the processor also stores into those words | VERIFIED (earlier run) |
| Where do the missing 48 dB and the presence go? | Not in any block found. The booted processor has per channel gains `c[-30] = 0.2016 / 0.01375` and FIR taps of 0.005 to 0.02; the level hand-over into it ([fx-leslie](fx-leslie.md) s11.3) is a mixer / chain element not found. A drive `tanh` placed before it would be a guess (ASSUMED) and is not added | open |
| Code or default change | **None.** `render_hash`, defaults and every opt-in unchanged. Docs only | VERIFIED |

## 2. Commands

```bash
python tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin --sram DIR/org      # likewise PNO, FX
target/release/sharc-disasm.exe DIR/org/core1_sram.bin 0x240000 0x2C0000 > c1.lst   # likewise core2
grep -i -E "= 0x(b2f2[0-9a-f]|b2f3[0-9a-f]|b2f8[ab]|b4e97)\b" c1.lst c2.lst        # ORG: no hit
```

Linearity probe: `tools/fx_leslie_run.py` with `noise_pokes(amp = 0.03, 0.3, 1.0)` (seed 3, 300 calls, boot log replayed, `target/release/sharc-fx.exe`), compare the last call's `0xB6000..3`:

| word | out(0.3) / out(0.03) | out(1.0) / out(0.3) |
|---|---|---|
| `0xB6000` | 9.72 | 3.05 |
| `0xB6001` | 9.87 | 3.20 |
| `0xB6002` | 9.99 | 3.31 |
| `0xB6003` | 9.89 | 3.22 |

(ideal 10.0 and 3.33; input, seed and call count are identical between runs, so the deviation is the processor).

## 3. Reading

1. The hypothesis fails at its first step: no ORG routine reads a Leslie / drive state, so there is nothing to locate or port from that image.
2. The only firmware Leslie is the linear-ish PNO processor, already ported (`fw_leslie_core.rs`, word for word). Its presence (-5.7 dB) and its level (-10.9 dB RMS) come from what feeds it and what follows it, not from a missing block inside it.
3. The hardware drive nonlinearity, if it exists, lives in a stage not located: candidates are the chain elements installed around it (`0x125371` nonlinear resonant filter, `0x1250CF`, [fx-leslie](fx-leslie.md) s1), the mixer element that feeds `0x1256AD`, or the ARM-side level staging. None is separable with the current evidence.

## Implications

- Do not add a tanh preamp to the firmware Leslie path: it would be fitted to one clip (ASSUMED). The engine rotary keeps its `tanh` as a Legacy model.
- The B3 presence gap is dominated by the replayed note set ([organ-presence-gap](organ-presence-gap.md) s1), which this note does not change.

## Follow-ups

1. Find the element that feeds `0x1256AD` (OQ21 area): run the full PNO pass with the boot log and trace who writes `0xB6000..3` before the Leslie call.
2. A hardware recording of a lone note with the rotary on and off, drive 0 and 127, would separate the preamp from the replay problem.
3. B3 right-hand note list for the clip ([organ-presence-gap](organ-presence-gap.md) follow-up 1), then re-measure presence with `--leslie firmware`.
