# Research: does the B3 Leslie run on the FX chip? (negative result, D131)

- **Checked:** 2026-11-17
- **Sources (local, never committed):** the five DSP images (`tools/ns4_dsp_images.py`; flat SRAM per core from `tools/ns4_dsp_ldr.py --sram`: FX core 1 / core 2, PNO core 1 / core 2, ORG core 1 / core 2), the ARM boot logs `fx_core1.script`, `fx_core2.script`, `pno_core1.script`, `org_core*.script` (`tools/arm_bulk_tables.py capture`), the B3 8-key PNO lane (`sharc-sys --pno --epdma --lane 2:0 --wav`, 8,016 passes, peak 0.347, rms 0.0584), `sharc-fxlive` (D125), `sharc-sys --probe`, and the new `tools/fx_code_presence.py`. Numbers only; no audio or firmware content is printed or committed. Builds on [b3-leslie-pno-lane](b3-leslie-pno-lane.md) (D126), [fx-oracle-live](fx-oracle-live.md) (D125), [fx-leslie](fx-leslie.md), [fx-type-map](fx-type-map.md) s8 to s10, [fx-chain-install](fx-chain-install.md), [audio-remaining](audio-remaining.md) row 9.
- **Question:** The Leslie processor `0x1256AD` runs in 0 of 76 passes on the live PNO core 1 lane (D126). Does it, or any other Leslie code path, run on the FX chip, and what drives it (D98 drive words, ring DMA, ARM messages)? If it runs, how does organ B3 plus that Leslie compare with the native `organ_b3` and the hardware clip (sub / mid / brilliance and the 11.02 dB gap)?

Tags follow `.agents/rules/provenance-in-code.md`: **VERIFIED** = reproduced by the commands below, **EMPIRICAL** = measured on this host, **ASSUMED** = reading without a source.

## 1. Short answer

**No.** The hypothesis that the Leslie runs on the FX chip is rejected on independent grounds. No "organ B3 + FX-chip Leslie" render exists, so the band levels and the 11.02 dB gap are unchanged and nothing is ported to `nord-core`.

| # | Check | Result | Tag |
|---|---|---|---|
| 1 | Is the Leslie processor `0x1256AD` code in any FX image? | **No.** Fraction of its 12-byte code pieces (first `0x300` bytes, step 2) found anywhere in the target image: FX core 1 **0.00**, FX core 2 **0.00**, ORG core 1 / core 2 0.00, PNO core 2 0.00. The B3 processor `0x121E2B` scores 0.00 in all of them as well. Both routines live only in the PNO core 1 image | VERIFIED (`tools/fx_code_presence.py`) |
| 2 | Are the Leslie *family* routines in the FX images? | **Yes, as the amp cabinet module.** The PNO core 1 routines `0x1237F8` / `0x1236F8` (cabinet), `0x1235AA` (table LFO), `0x125371` (nonlinear resonant filter) and `0x1250CF` score 1.00 / 1.00 / 1.00 / 0.98 / 1.00 in both FX cores (FX core 1 copies at `0x123391`, `0x123243`, `0x12500A`, `0x124D68`; core 2 copies at `0x122A1D`, `0x1228CF`, `0x124696`, `0x1243F4`). These are the library routines [fx-type-map](fx-type-map.md) s10 measured as the Amp cabinet. The PNO core 1 image carries copies for its own module chain; the ORG images score 0.00 to 0.02 | VERIFIED (same tool) |
| 3 | Does the ARM send the Leslie row to the FX chip? | **No.** In the boot logs the Leslie row words (rotor speeds `0xB2F25` / `0xB2F26`, routine pointer `0xB2F29 = 0x1237F8`, drive words `0xB2F2D` / `0xB2F31` of D98) exist only in `pno_core1.script` (10 words in `0xB2F20..0xB2F40`). `fx_core1.script` and `fx_core2.script` hold 0 words there, and no message of any FX or ORG script carries the value `0x1256AD`, `0x1237F8`, `0x1236F8`, `0x1235AA` or `0x125371` (`pno_core1.script` carries `0x1237F8` x2, `0x1236F8` x1, `0x1235AA` x1, `0x125371` x1, `0x1250CF` x2, never `0x1256AD`). The FX scripts install only FX-address routines (core 1: `0x1231CB` x5, `0x124D68` x2, `0x124E0B` x3, `0x12500A` x5; core 2: `0x1228CF` x5, `0x122A1D` x2, `0x122B1D` x3, `0x1243F4` x2, `0x124497` x3), the amp, filter and pass-through stages | VERIFIED (script scan) |
| 4 | Does any Leslie-family stage run on the FX chip with the B3 lane as input? | **No.** `sharc-fxlive` with the FX boot log (master word `0xB6518 = 1.0`), 3,000 lead passes and the B3 lane as input (input L peak 0.1727, rms 0.0165): "not reached in a pass" for core 1 `0x1231CB`, `0x124D68`, `0x124E0B`, `0x12500A`, `0x123243`, `0x123391` and core 2 `0x1231CB`, `0x124D68`, `0x124E0B`, `0x12500A`, `0x1228CF`, `0x122A1D`, `0x122B1D`, `0x1243F4`, `0x124497`; `0x1256AD` is not reached on either core. The output is exactly 0: the boot-log state is the default program with the effects off | VERIFIED (`--regs-at`) |
| 5 | Is the live-chain harness sound on this input? | **Yes.** Positive control: stage 0 set to the pass-through `0x1231CB` (the identity install of `tools/fx_oracle_live.py`) with the B3 lane in: `0x1231CB` is reached, output L rms 0.003081 against input 0.016514 = **0.1866 x**, the D125 transfer. With the boot log applied first the output is again 0 (the boot state mutes the chain), so "not reached" describes the booted state, not a broken harness | VERIFIED (run) |
| 6 | Does the PNO core 1 pass call `0x1256AD` after the full B3 load and 8 keys? | No: `sharc-sys --probe 0x1256AD` executes it in **0 of 316** passes (D126: 0 of 76) | VERIFIED |

Consequences for the metric: the D126 lane against `organ_b3` stays the only measured pair (sub +27.54 dB, mid +1.04, brilliance +15.41, gap 11.02 dB). There is no FX-chip Leslie render to compare, and the native `organ_b3` render is untouched.

## 2. Method

1. **Code presence.** `tools/fx_code_presence.py REF PC[:name].. --target IMG..` cuts `0x300` bytes at `2 * pc` of the reference image into 12-byte pieces (step 2 bytes) and reports the fraction found anywhere in each target image. A copy linked at another address scores about 1.0 (only pieces that contain an absolute address break), absent code about 0.0. The 1.00 of `0x1237F8` in both FX cores is the positive control for the 0.00 of `0x1256AD`. Unit tests: `tools/tests/test_fx_code_presence.py` (copy at another address, patched operand, absent code).
2. **ARM side.** Script scan of the boot logs: final value per DM word and the routine values installed.
3. **Live FX chip.** `sharc-fxlive IMG --core N --passes 3300 --lead 3000 --msg 0xB6518=f1.0 --script-flat fx_coreN.script --in-wav b3lane.wav --regs-at PC` per routine; `--regs-at` runs one more pass and reports whether the PC executed.
4. **Lane.** `sharc-sys PNO --pno --epdma --passes 8000 --warmup 16 --script-flat pno_core1.script --script-flat org-system-load-pno1.script --script-at 16:org-system-key8-pno1.script --lane 2:0 --wav b3lane.wav --rate 43402`.

## 3. What this leaves

- `0x1256AD` exists only in the PNO core 1 image and is named only by the RAM-built channel table `0xC05450C8` (row 6, [fx-leslie](fx-leslie.md)); nothing in the captured ARM logs links it to a chain. The open question moves **back to the ARM install path on the PNO chip** (which part command or recall phase writes `0x1256AD` into `0xC2F48..`), not to a second DSP.
- The FX-chip Leslie-family routines are the Amp cabinet. If B3 programs use a cabinet-based rotary (amp type 7 "To Rotary"; [fx-type-map](fx-type-map.md) says its words are not in the drive-stage context), the FX chip would carry a different stage list than the boot default. No ARM capture of that exists; not tested here.
- The hardware clip `organ_b3` is the final mix after the FX chip, the lane is the dry DSP output (D120). With the FX chain at its default recall silent in the oracle, the FX chip is not what turns the dry lane into the clip in the current oracle state.

## Implications

- Do not wire a Leslie into `nord-core` from FX-chip evidence: the FX chip has no Leslie processor and receives no Leslie row traffic.
- The B3 presence gap (mid +1.04, brilliance +15.41, gap 11.02 dB on the lane) stays an organ-side / ARM-install question ([audio-remaining](audio-remaining.md) row 9).

## Follow-ups

1. Find the ARM call that writes the module install for the organ part's Leslie (row 6 of `0xC05450C8`, `0x1256AD`) into a PNO core 1 chain head: run the part activation of [arm-part-activation](arm-part-activation.md) with the DSP read-back stub `0xC0066E64` answered from the oracle (its follow-up 1), then diff the PNO core 1 message log against `org-system-load-pno1.script`.
2. Capture an ARM recall of a program with amp type 7 (To Rotary) and `kLeslieEnable = 1` and check the FX scripts for new stage installs.
3. If a message ever installs `0x1256AD` on PNO core 1, re-run `sharc-sys --probe 0x1256AD` and `tools/dsp_lane_vs_hw.py --segment organ_b3`; port only with VERIFIED provenance.

## Reproduce

```bash
python tools/fx_code_presence.py PNO/core1_sram.bin 0x1256AD:leslie 0x1237F8:cabB 0x125371:nl 0x1235AA:lfo 0x121E2B:b3proc \
    --target FX/core1_sram.bin --target FX/core2_sram.bin --target ORG/core1_sram.bin
target/release/sharc-fxlive FX/core1_sram.bin --core 1 --passes 3300 --lead 3000 --msg 0xB6518=f1.0 \
    --script-flat fx_core1.script --in-wav b3lane.wav --regs-at 0x1231cb
target/release/sharc-sys PNO/core1_sram.bin --pno --epdma --passes 300 --script-flat pno_core1.script \
    --script-flat org-system-load-pno1.script --script-at 16:org-system-key8-pno1.script --probe 0x1256AD
```
