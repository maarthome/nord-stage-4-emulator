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

## 6. Follow-up (2026-11-15, task `org-leslie-feeder`): the feeder, the note set, the sub band

Tags as above. Data: [data/emu_vs_hw_b3_merged_leslie.json](data/emu_vs_hw_b3_merged_leslie.json).

### 6.1 (a) Who feeds the Leslie: `0xB6000..3` is the harness, not the firmware

| Item | Result | Tag |
|---|---|---|
| `0xB6000..3` | Scratch pair chosen by `tools/fx_leslie_run.py` (`IO`). No PNO instruction stores to or loads from it (no immediate in either core) | VERIFIED (negative) |
| Real input pair | The module chain loop (`0x2427D2..`, listing bytes) reads the lane pair `DM(-31, I1)` of the record, multiplies it by `wet = g' + c` and stores it at the scratch `0xC2E1A`; it then calls the row routine with `I0 = 0xC2E1A`. The routine saves that `I0` at `DM(0xB4CCC)` (`0x24AD5A`) and reads it back as `R0, R1 = DM(I2, M2)` (`0x24B260`) | VERIFIED (listing) |
| Two more additive feeds | `0x24B254..0x24B2A0`: (1) an aux pair at `DM(DM(0xB2F3B))` (image `0xC0142`) times `DM(0xB2F3C)`; the gain is **0 in the image and in the boot log**, so it is off; (2) the two stereo DMA input lanes at `DM(0xB00E5)` and `+0x14` (`0xB0078` / `0xB008C`, or `0xC0078` / `0xC008C`: the ping-pong input base chosen at `0x2421B2`). No CPU store to `0xB0078 / 0xB008C` exists (only the base immediates), so they are filled by a DMA (ASSUMED) | VERIFIED (listing, boot log), DMA ASSUMED |
| Do the feeds differ in gain? | **No.** Seeded noise (amp 0.3, 300 calls, boot log replayed) into the in-place pair gives last-call outputs `-7.6e-4 / -8.1e-4 / -1.0e-4 / -4.1e-4`; the same noise into `0xB0078 / 0xB007A` gives `-7.2e-4 / -7.9e-4 / -3.4e-5 / -4.4e-4` (same level, -48 dB re the input; the pair sum is symmetric: `0xB008C` gives the same words). The processor is linear in the sum, so **the feed point cannot explain a level gap** | VERIFIED (run) |
| Chain mix after the call | `out = proc * wet + lane * DM(0xB00EE)` (`0x242840..0x24286E`, `I3 = 0xB0106`); `wet` is the ramped `g' + c` of the record; the weight word `0xB00EE` is neither in the boot log nor written by code (image value). With `wet + dry = 1` no stage above unity exists in the chain (reading of the multiply order) | VERIFIED (listing), mix reading ASSUMED |
| The "48 dB" | Reproduced as the ring model: with the history ring frozen the firmware Leslie reads **-47.8 dB RMS** against the clip, with the plain advance **-4.9 dB** (s6.2). Confirms [fx-leslie](fx-leslie.md) s10: a frozen history, not a missing gain | VERIFIED (run) |

So the Leslie level hand-over is closed as a search target: no feeder carries a gain, the chain does not amplify, and the missing piece is the DMA filled history ring (open, `RingAdvance`). No stage to port; no code change.

### 6.2 (b) Note set, durations and residuals

Transkun hears the Leslie amplitude modulation as re-attacks: the 525 notes of the clip are **96 sustained keys** once same-key notes closer than 0.3 s are joined (gap 0.1 s: 261, 0.5 s: 76, 1.0 s: 55); midi 62 alone has 91 detections. The replay of 525 short notes also re-triggers key click and envelope 525 times. `tools/note_offsets.py` (new, tests `tools/tests/test_note_offsets.py`) joins the re-triggers (`--merge-gap`) and replaces each offset by an audio estimate (partials 1..3, 12 dB drop from the attack level; an **upper bound**: shared partials of other notes keep the level up). Hold median: 0.04 s as transcribed, 0.34 s merged, 0.86 s with audio offsets (mean 1.21 s).

Gaps emulator minus hardware, dB re RMS (`tools/emu_vs_hw.py --events-from`, same options as s4.2):

| Stream | Variant | sub | low | mid | presence | brilliance | RMS | score |
|---|---|---|---|---|---|---|---|---|
| transcribed, 525 | default | +10.0 | -1.2 | +4.1 | -2.4 | +6.8 | +0.3 | 4.92 |
| transcribed, 525 | engine rotary | +13.8 | 0.0 | +3.0 | -3.6 | +3.1 | -7.6 | 4.72 |
| transcribed, 525 | firmware, advance | +13.3 | -2.2 | +3.4 | +5.2 | +7.6 | -10.4 | 6.35 |
| merged, 96 | default | +11.1 | -1.4 | +3.1 | -4.7 | -0.4 | +4.8 | 4.14 |
| merged, 96 | engine rotary | +13.8 | -0.1 | +1.8 | -8.9 | -6.0 | -3.1 | 6.11 |
| merged, 96 | firmware, advance | +14.5 | -1.5 | +3.1 | **+1.5** | +4.5 | -6.7 | 5.02 |
| audio offsets, 96 | default | +11.0 | -1.2 | +3.4 | -4.3 | +2.6 | +6.6 | 4.52 |
| audio offsets, 96 | engine rotary | +13.5 | +0.3 | +2.6 | -10.5 | -4.2 | -1.2 | 6.22 |
| audio offsets, 96 | firmware, advance | +13.7 | -2.0 | +3.8 | **+2.8** | +5.8 | -4.9 | 5.60 |
| audio offsets, 96 | firmware, ring frozen | +10.9 | -1.9 | +4.2 | -7.6 | +0.9 | -47.8 | 5.11 |
| audio offsets, 96 | firmware, pass-routine ring | +4.1 | -7.7 | -0.6 | +8.9 | +23.3 | -11.8 | 8.91 |

Reading (EMPIRICAL, one clip): with a realistic note set the firmware Leslie presence is **+1.5 .. +2.8 dB** (inside 3 dB; it was -5.7 on the 101 confirmed notes and +2.7 .. +5.4 on the 525 chatter notes) and the engine rotary stays 9 to 10 dB under; RMS -4.9 .. -6.7 dB (it shrinks with held length). Mid +3.1 .. +3.8, brilliance +4.5 .. +5.8 and sub +13.7 are still outside the bar.

**Sub +14 dB is an artifact of the surrogate ring.** The replayed set has no note below E3 (midi 52), so there is no 20 to 120 Hz fundamental to excite. The firmware advance render carries -22 to -25 dB re total flat from 0 to 90 Hz (hardware -55 to -43, engine rotary -56 to -41) with lines at 6.8 and 13.7 Hz (the horn fast speed and its double) and a mean of -1.5e-3. The ring variants move it: pass-routine ring +4.1 (but presence +8.9, brilliance +23.3), frozen +10.9 (silent). The sub excess therefore belongs to the unmodelled DMA ring ([fx-leslie](fx-leslie.md) s11.2), not to a Leslie gain or a preamp.

D22 bar not met (sub, mid and brilliance outside 3 dB, RMS 4.9 dB under, one clip, ring unmodelled): **no default changes**; `--leslie firmware` stays opt-in (D63, D94).

## 7. Follow-ups after section 6

1. Model the DMA ring that fills `0xB0078 / 0xB008C` and the 8-pass restart; then re-run the table (the sub excess and the brilliance should move with it).
2. The value of `DM(0xB00EE)` (chain dry weight) and a run of `sharc-pass` with a Leslie record in the organ chain, to confirm the mix reading of s6.1.
3. A hardware clip of a lone held B3 note, rotary slow and fast: fixes the speed history and removes the note-set uncertainty.

## 8. Review follow-up (2026-11-15): drive words, caveat, open probes

**Correction to s1.** The linearity probe ran with the drive words `0xB2F2D` / `0xB2F31` at the boot value 0. With them poked at call 0 (seeded noise, 300 calls, last-call output words `0xB6000..3`, same method as s2):

| Drive | out(0.3) / out(0.03) | out(1.0) / out(0.3) | level at input 0.03 vs drive 0 |
|---|---|---|---|
| 0 | 9.91, 9.84, 9.76, 9.92 | 3.24, 3.17, 3.10, 3.25 | 0 dB |
| 0.5 | 7.84, 6.72, 3.84, 8.26 | 1.60, 1.41, -0.40, 1.75 | +4 dB |
| 1.0 | 2.08, 1.66, -0.84, 2.32 | 1.02, 1.01, 1.21, 1.04 | +9.6 dB |

(ideal 10.0 and 3.33.) So the processor **is** a saturating stage once a drive word is non-zero: small-signal gain up by about 10 dB and hard compression above input 0.3 (output ratio 1.02 for x3.3 input). VERIFIED (run, `/tmp` driver on `fx_leslie_run.py` arguments). The "no saturation, drive changes nothing" rows of s1 hold only for drive 0, the boot value. What sets the words at run time (`Edt_OrgCom_kEffectsLeslieDrv` is `v / 127` into the row, [fx-leslie](fx-leslie.md) s4) and the B3 program value are the open link; the engine's firmware path uses the word for word port with the boot words, so a program with Leslie drive > 0 would differ. No code change here.

**Caveat (point 4 of the review).** Transkun is a piano model on a sustained organ clip: the +1.5 .. +5.4 dB presence and the sub excess are partly note-set artefacts (s6.2 shows 525 notes are 96 keys); durations are an upper bound.

## 9. Not run (follow-ups)

1. Boot-installed chain elements `[0x1250CF, 0x125371, 0x121BBA]` at `0xB0476` and `[0x1250CF, 0x1235AA, 0x1236F8]` at `0xB047E` through `sharc-pass` with noise / sine at 0.03, 0.3, 1.0 (`0x125371` nonlinear resonant filter; `0x1235AA` shares the 0.9908 smoothing constant of row `0xB2F39`). Needs a pass harness with the organ lane wired: not cheap here.
2. Sweep `kEffectsLeslieDrv` 0..127 from the settled snapshot (`tools/arm_edit_capture.py`) and log which DSP words it writes; then re-run s8 with the real word values and the B3 program value.
