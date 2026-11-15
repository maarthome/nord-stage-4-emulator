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
- The B3 presence gap is the replayed note set: with the Transkun right-hand notes the firmware Leslie reads presence +2.7 .. +5.4 dB instead of -5.7 (section 4.2).

## Follow-ups

1. Find the element that feeds `0x1256AD` (OQ21 area); attempted in s4.1, still open.
2. A hardware recording of a lone note with the rotary on and off, drive 0 and 127, would separate the preamp from the replay problem.
3. B3 right-hand note list for the clip ([organ-presence-gap](organ-presence-gap.md) follow-up 1), then re-measure presence with `--leslie firmware`.

## 4. Follow-up (2026-11-15): feed search and the right-hand note stream

### 4.1 Feed of `0x1256AD` (not found, three more attempts)

| Attempt | Result | Tag |
|---|---|---|
| Static: stores / loads of the Leslie in-place pair `0xB6000..3` in the PNO listing | no immediate `0xB60xx` in PNO core 1 or 2; the caller passes `I0` at run time | VERIFIED (negative) |
| ARM: system address `0x28495AB4` (`0x28000000 + 4 * 0x1256AD`) in `bank0_CODE` / `bank0_SRAM` | absent, so the routine is not named by a literal; table `0xC05450C8` is built in RAM by the static initialiser and indexed by row (writer `0xC01D5C14`: `(row, value)` pairs, [fx-chain-install](fx-chain-install.md) s1) | VERIFIED (negative) |
| Earlier harness work (`arm-part-activation` s4, [fx-leslie](fx-leslie.md) s11.3): chain record inserted at `0xC2E4A`, lane unchanged | still the state of the art; the organ lane copy that feeds the processor is not reachable without the ARM part command consumer ([arm-part-activation](arm-part-activation.md) follow-ups 1, 2) | open |

So the level hand-over stays open and nothing is ported: a gain or `tanh` stage inserted without a hardware observation would be ASSUMED and fitted to one clip.

### 4.2 B3 clip with the Transkun right-hand notes (the presence gap is the note set)

`tools/emu_vs_hw.py --events-from transkun` on `organ_b3` (Part A, 238 s, 25 s) replays 525 notes (MIDI octave 4: 131, 5: 310, 6: 78, 7 and 8: 6) instead of the 101 video-confirmed notes of octaves 3 and 4. Options `--tone-source b3wheels --firmware-routing --b3-tail on --organ-level firmware`; gaps emulator minus hardware in dB re RMS ([data/emu_vs_hw_b3_transkun_leslie.json](data/emu_vs_hw_b3_transkun_leslie.json)). Transkun offsets are piano decays (median note 44 ms, mean polyphony 1.1), so the run is repeated with every note held at least 0.5 and 1.0 s:

| Notes | Variant | sub | low | mid | presence | brilliance | RMS | score |
|---|---|---|---|---|---|---|---|---|
| confirmed 101 (before, [fx-leslie](fx-leslie.md) s11.4) | engine rotary | +3.2 | +1.4 | +2.3 | -19.6 | -13.0 | -6.7 | 7.90 |
| confirmed 101 (before) | firmware, advance | +4.7 | +0.7 | +3.7 | **-5.7** | +2.3 | -10.9 | 3.42 |
| Transkun, as transcribed | default | +7.0 | -4.0 | +4.1 | +2.6 | +10.0 | +0.4 | 5.54 |
| Transkun, as transcribed | engine rotary | +12.2 | -1.2 | +3.4 | -1.4 | +4.5 | -8.9 | 4.53 |
| Transkun, as transcribed | firmware, advance | +11.0 | -3.5 | +3.5 | **+5.4** | +7.3 | -11.0 | 6.15 |
| Transkun, held >= 0.5 s | engine rotary | +12.1 | -0.3 | +2.7 | -9.6 | +0.8 | -2.9 | 5.10 |
| Transkun, held >= 0.5 s | firmware, advance | +12.0 | -2.1 | +3.8 | **+2.7** | +5.3 | -6.4 | 5.20 |
| Transkun, held >= 1.0 s | engine rotary | +11.8 | -0.4 | +2.6 | -7.7 | -0.9 | -0.5 | 4.67 |
| Transkun, held >= 1.0 s | firmware, advance | +12.1 | -2.8 | +3.3 | **+4.0** | +7.6 | -4.1 | 5.97 |

Reading (EMPIRICAL, one clip, note lengths unknown): with the right-hand register the firmware Leslie presence moves from -5.7 to +2.7 .. +5.4 dB (sign flipped, spread 2.7 dB over the three durations) and the engine rotary from -19.6 to -1.4 .. -9.6 dB. The "about -6 dB Leslie presence gap" and most of the B3 presence deficit are therefore the replayed note set, as [organ-presence-gap](organ-presence-gap.md) s1 predicted; the firmware path is the closer model for sustained notes (|presence| 2.7 .. 5.4 against 7.7 .. 9.6 dB). Not closed: sub +11 .. +12 dB (all variants; the Legacy default is +7 .. +9.6, so not a Leslie property alone), RMS -4 .. -11 dB for the firmware path against -0.5 .. -9 for the engine rotary (the ring advance stride and the missing level hand-over, section 4.1), and the note durations (no hardware note list). **No default flips** (D22: one clip, no duration ground truth, presence still 2.7 .. 5.4 dB out, sub 11 dB out).

## 5. Follow-ups after section 4

1. Note durations from the audio (offset detection on the sustained lines) so that the held-length sweep becomes one measurement; then re-run with `--leslie firmware`.
2. Sub band +11 dB on the Transkun replay: check the bass layer / drum path of the firmware Leslie (`--leslie-ring`, rotor drum horn balance) with the lone-note hardware clip.
3. DSP-in-the-loop answers for `0xC0066E64` ([arm-part-activation](arm-part-activation.md) follow-up 1) to reach the part command consumer that installs the Leslie element.
