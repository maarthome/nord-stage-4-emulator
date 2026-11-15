# SMP voice slew loop and stream edge handling

- **Checked:** 2026-11-14
- **Sources (local, never committed):** SMP image of OS v1.66 (`python tools/ns4_dsp_images.py target/fw_unpacked/bank0_CODE.bin --out target/oracle/img`, `python tools/ns4_dsp_raw.py target/oracle/img/dsp_image_4_SMP.bin --sram target/oracle/smp.bin`; listing `python tools/adi_dis.py target/oracle/smp.bin 0x380600 0x382B10`, CCES 2.12.1; pc = `0x1C0300` + listing offset); `sharc-ref` oracle (`target/release/sharc-ref`, scenarios from `tools/smp_slew_scn.py`); [smp-voice](smp-voice.md), [piano-dsp-path](piano-dsp-path.md), [piano-sound-load](piano-sound-load.md) s11, [piano-bus-convention](piano-bus-convention.md), [piano-receiver-scale](piano-receiver-scale.md).
- **Question:** What does the SMP do between the float fields `+11..+16` of a voice record and the Q31 gains `0 / +1` that the voice loop multiplies in (slew time behaviour), and what does it do at the edges of a sample stream (loop, end, history)?

Tags follow `.agents/rules/provenance-in-code.md`. "Oracle" is the `sharc-ref` core running the real SMP image; the SIMD register swap and the long-word SIMD access are ASSUMED forms of that core ([smp-voice](smp-voice.md) s7), the numbers below hold on it.

## 1. Result

| Item | Result | Tag |
|---|---|---|
| Where the slew runs | Loop `0x1C06B9..0x1C06E6` of the audio interrupt (`LCNTR = 0x39`, SIMD, `M6 = 0x36` = one record per pass, before the voice loops). One interrupt updates records `0..=56`, the next `57..=113` (3 interrupts in a row: 0..56, 57..113, 0..56), so **every record is updated every second audio interrupt** | VERIFIED (run, 114 records poked, fields compared after each of 4 interrupts) |
| Fields read and written | Inputs `-2, -1, +2, +3, +11 .. +18` (floats) and `+19, +20` (Q31 words); outputs `-2, +13, +14, +15, +16` (floats) and the gains `0, +1`. A probe that changes one field per record (52 records, 20 fields) shows that `-20, -19, +4, +5` and the old `0, +1` do not influence any output | VERIFIED (run) |
| Update law | See section 2: one-pole-like x lane `v = max(F11 + F15 F13 F17, 0)`, y lane `u = max(F12 + F16 F14 F18, 0)` with decaying state `F14`, ramp `w = max(clip(F-2 + F-1, F3), F2)`, gains `(fix31(w v) F19) >> 31` and `(fix31(w v) F20) >> 31` | VERIFIED: Rust model equals the oracle in 11 of 11 random records (all 7 outputs) and in 60 of 60 updates of a trajectory record (both gains), `piano/smp_slew.rs` |
| Arithmetic | 40-bit extended float: `MODE1 = 0x1001CFC` (listing `0x1C06AA`) leaves `RND32` clear and sets `TRUNCATE`: intermediates keep 32 mantissa bits, every operation truncates toward zero, a store drops 8 more bits. With 24-bit truncation (or round to nearest) the model misses by 1 ulp in most fields; with `piano/ext40.rs` it is bit exact | VERIFIED (run) |
| Time behaviour | Per update of one record (every 2 interrupts): `w` moves by `F-1` per update toward `F3` (ramp with ceiling, floor `F2`); the x state follows `v' = F11 + v F13 F17` with `F13` replaced by the y state `u` each update, and `u' = F12 + u F14 F18` with `F14' = F14 F18`, so the decay coefficient `F14 F18^t` falls geometrically and `u` falls super-exponentially to the target `F12`. Example trajectory (60 updates, `F-1 = 0.04`, rates `0.979`, `F11 = F12 = 0`): the left gain rises with the ramp to `1.77e8` (0.0825 of full scale) at update 4 and decays to 0 by update 60; table in the goldens of `smp_slew.rs` | VERIFIED (run) for the arithmetic, the physical meaning of the ramp / decay split is ASSUMED |
| What the ARM writes at note-on | Four-word groups per record (`piano-sound-load` s3): record `R = 0x92848 + 54 k`, words `R + 14 = 1.0`, `R + 12 = 0`, `R + 11 = 0`, `R + 16 = 0.9793 (0x3F7AB576)`; so `F14 = 1`, `F11 = F12 = 0`, `y_state = 0.9793`, identical for every velocity and note. The other fields (`F13, F15, F17, F18, F-2, F-1, F2, F3, F19, F20`) are set by other ARM writers (`0xC00B6030`, `0xC00B61AC` tables, `0xC00B64D8` init) that were not run | EMPIRICAL (message list, address arithmetic) |
| Gain word in the record | The level word `level_q23 / 2^23` of [piano-sound-load](piano-sound-load.md) s10 sits at DM `0x9252A` / `0x9251E`, not in the 54-word record; how it reaches `F19 / F20` or `F11` is not traced | open |

## 2. The loop as equations

Per record (x lane = word `f`, y lane = word `f + 1` of each field pair read as a SIMD pair):

```text
d_x = F13 * F17                       v  = max(F11 + F15 * d_x, 0)      -> F15
d_y = F14 * F18                       u  = max(F12 + F16 * d_y, 0)      -> F16, F13;   F14 <- d_y
w   = max(clip(F-2 + F-1, |F3|), F2)                                      -> F-2
q   = round_to_nearest(w * v * 2^31)  (saturating, 40-bit float product)
F0  = (q * F19) >> 31     F1 = (q * F20) >> 31                           (floor)
```

The loop copies the x product into the y lane with `r8 <-> s6`, so **both gains come from the x lane**; the y lane only produces `u`, which becomes the coefficient `F13` of the next x update. Not modelled: in one probe the oracle gain was one LSB above the model for products with a Q31 multiplier of exactly `0x40000000` / `0x7FFFFFFF` (17 of 104 values, ties of the integer multiply); with random multipliers the model is exact (142 of 142 gains, the oracle runs behind `CASES`). Saturation (`w v >= 1`, `q` clamps to `0x7FFFFFFF`) is implemented but the oracle capture of that case was lost in the NaN guard of the dump, so it is not golden.

## 3. Stream edge handling (VERIFIED runs, `tools/smp_slew_scn.py edges`)

The state machine subroutine at `0x1C0A89` (listing `0x789`) is called twice per interrupt (records `2 j` and `2 j + 1`, walking from record 0: records 0, 1 on one interrupt, 2, 3 on the next). Observed with `F+8 = 1`, `F-8 = 100`, `F-7 = 40`, `F-6 = 500`, `F-5 = 600`, `F-4 = 7`, `F+6 = 0.5`, `F+7 = 0.25`, `F-24 = 3`:

| Case | Result | Tag |
|---|---|---|
| counter `F-30 >= F-8` (loop end) | `F-30 <- F-30 - F-7` (130 gives 90), `F-24 <- (F-24 + F-7) & 15` (3 gives 11): the position wraps by the loop length and the history slot offset is realigned | VERIFIED (run, 2 of 2 records per interrupt) |
| counter `> F-8 - F-7` | the rate word `F17` is loaded from `F+6` (0.5); at or below it from `F+7` (0.25): a different slew rate inside the loop region | VERIFIED (both branches) |
| counter below `F-8` and at or above `F-9` | the record is cleared by the routine at `0x1C0B08` (`F+8`, `F-30`, `F-24` and the other words of the listing `0x808..0x866` zeroed, `F+8 = 0` marks the voice free): the end of a non-looping stream stops the voice, nothing else is read past the last sample | VERIFIED (run: `F-9 = 0` stops every record) |
| page words | after a loop wrap `F-15 <- F-6` and `F-10 <- F-4 + 1` in the probe (counter 90 below `F-5 = 600`); the listing compares the wrapped counter with `F-5` before choosing `F-6` or `F-5` and incrementing `F-4`, but the oracle incremented `F-4` while keeping `F-6`: the two conditional moves are not both explained | EMPIRICAL |
| step limit | no clamp: `counter += step_int + carry` ([smp-voice](smp-voice.md) s3), so a step above 1 skips residuals and the predictor decodes noise; the ARM keeps the step at or below 1 ([piano-dsp-path](piano-dsp-path.md): cents table index `-cents`) | VERIFIED (oracle equals model at step 1.3, [smp-voice](smp-voice.md) s5), ARM side EMPIRICAL |
| ring end | the 256-word stream ring wraps by `& 0xFF` with no end test; the DSP never reads an end marker, the ARM refills ahead of the position ([smp-voice](smp-voice.md) s4) | VERIFIED (listing and runs) |

The engine's edge rule (repeat the edge sample, [piano-engine](../specs/piano-engine.md) s9) has no hardware counterpart in these words: the hardware loops (`F-8`, `F-7`) or stops (`F-9`) a stroke by position; whether a factory `.npno` stroke sets a loop at all is not read here (the stroke descriptor fields behind `F-9 .. F-4` are not mapped).

## 4. Damper-down strokes, unison, dynamics compressor, EQ

No new hardware evidence. The slew loop and the voice loops are the whole per-record path of the SMP image (listing `0x1C0680..0x1C0A70`): there is no biquad coefficient, detune table or compressor state in them, and the post stage is the unity one-pole of [piano-bus-convention](piano-bus-convention.md) s2b. A counterpart of Unison, Dynamics or Timbre in the PNO or FX image is **not found** (consistent with [piano-dsp-path](piano-dsp-path.md): those ids send nothing); the engine's versions stay `ASSUMED`. Damper-down strokes (`kind = 1` harp strokes selected with the pedal down) have no list or record of their own in the SMP: they are ordinary strokes with a different level (`F-2..F3` ramp and `F19 / F20`), so the selection rule stays on the ARM side (stroke search, [piano-sound-load](piano-sound-load.md) s8) and is not decoded.

## 5. Level chain (item 6)

The slew does not change the steady-state level of any stroke at the engine's unit of measure unless `F11`, `F19 / F20` and the ramp ceiling differ from unity, which the note-on message does not show. The master chain (`DM(0xB6518)` word, layer words 1.6 / 2.5 / 3.25) was not re-run against the clips in this pass: `tools/emu_vs_hw.py` now lists the verified-chain variants for the Velvet Grand segment (`piano-chain`, `vel-88-piano-chain`), and the standing result of [piano-receiver-scale](piano-receiver-scale.md) s6.3 (piano chain +10 to +12 dB, organ chain -14 to -15 dB, same video, same path) holds. Defaults stay (D62).

## Reproduce

```bash
python tools/smp_slew_scn.py random target/exp/slew.scn --records 10 57   # random records, one interrupt
python tools/smp_slew_scn.py trajectory target/exp/traj.scn               # 60 updates of record 20
python tools/smp_slew_scn.py edges target/exp/edges.scn                   # loop wrap / rate / end
target/release/sharc-ref target/oracle/smp.bin target/exp/slew.scn --out target/exp/slew
cargo test -p nord-core --lib piano::smp_slew piano::ext40
```

## Implications

- The voice gain the DSP applies is a product of a ramp and a decaying state, updated every second interrupt; a faithful engine envelope needs the ARM-side initial values (the other record writers), which are not decoded. The engine keeps its own envelope; `piano/smp_slew.rs` is the verified building block and is not wired into the render path.
- Loop and end are position rules of the stroke, not sample repeats; the engine's edge-sample repeat is an approximation.

## Follow-ups

1. Run the ARM writers `0xC00B6030`, `0xC00B61AC`, `0xC00B64D8` (settled emulator) to get `F-2, F-1, F2, F3, F13, F15, F17, F18, F19, F20` for a real note, then drive the slew model with them and compare with the engine envelope.
2. Map the stroke descriptor fields to `F-9 .. F-4` (loop end, loop length, page words) on a real `.npno` and replay a page switch on the oracle.
3. Re-run the clip comparison with the new `piano-chain` variants once the video is at hand (`tools/emu_vs_hw.py --only piano_dry`).
4. Check the SIMD register swap and the SIMD long-word load of the oracle against the ADI simulator (the slew loop is the first consumer that depends on both).
