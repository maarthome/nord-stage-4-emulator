# FM banks 1 and 2 (stages `0x124269` / `0x12453F`): feasibility, stopped

- **Checked:** 2026-11-15
- **Sources (local, never committed):** OS v1.66 `bank0_CODE.bin` / `bank0_SRAM.bin`, the ORG core 2 image (`python tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin --sram DIR`, listing `sharc-disasm DIR/core2_sram.bin 0x2484D2 0x248F6C`), the boot capture `target/bulk/org_core2.script` ([arm-bulk-tables](arm-bulk-tables.md)), `sharc-org` (`--watch`, `--rwatch`), the ARM edit harness (`tools/arm_edit_capture.py`, `tools/synth_wave_probe.py` logic). Builds on [synth-stage-models-13](synth-stage-models-13.md) s1 and [synth-wave-categories](synth-wave-categories.md).
- **Question:** can the data of the FM oscillator banks (operator or wave tables, ratios) be read from the firmware images the harness already uses, without new decryption, so the stages can be ported and checked against the SHARC oracle?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. Short answer

**Not feasible this round: the stage code is readable, the table it interpolates is not present in any image or message the harness produces.** No decryption is involved. Nothing was ported; this PR only records the evidence.

| Item | Result | Tag |
|---|---|---|
| Stage code `0x124269` / `0x12453F` | Readable: 598 listing lines `0x2484D2..0x248F6C` in the ORG core 2 image, both stages share the two-accumulator / polynomial / lerp skeleton (the second starts at `0x248A7E`). The only table read is `R5 = DM(-14, I1)` (`DM(0xC15A8)` = `0xE0926`) as base of 129-word rows (`M5 = 0x81`, row index `FEXT R0 BY 25:7`, so up to 128 rows) | VERIFIED (listing) |
| Partial ratios | The double table at `bank0_CODE@0x2D82A8` reads `0.5, 1, 2, 3, 4, 5, 6, 7 ...` (`python`, `struct.unpack('<d')`), the FM-H partials | VERIFIED (run) |
| Row table at `DM(0xE0926..)` in the image | Zero: 40,000 zero words from `0xE0926` (to `0xEA566`) in the flat ORG core 2 image. The pointer word itself (`0xC15A8` = `0xE0926`) is image data; `0xE0926` as a literal occurs in `bank0_CODE` only at `0x4B0A58`, `0x4D553C`, `0x4F9128`, all inside the embedded DSP images | VERIFIED (run) |
| Written by the ARM boot? | No. `target/bulk/org_core2.script` and the other five scripts hold no store in `0xE0926..0xE49A6`; the boot sends no word in that window | VERIFIED (script grep) |
| Written by the DSP? | No. `sharc-org ... --watch 0xE0926:0x4080 --rwatch 0xE0926:0x4080 --boot-stores --passes 2` with the full boot script lists no store and no load site; the stage code stores only through `I5` (the strip state block), never into the row table | VERIFIED (run, idle FM) |
| Sent on a wave edit? | No. `Edt_Syn_kOscWave` dispatched through the real layer client for `kOscBank` 1 and 2, categories 0 and 1, waves 0 and 3 (`kMode` 1) sends 101 messages each, none at DM address `>= 0xE0100`, so no table transfer | VERIFIED (run) |
| Sine / FM table in `bank0_CODE` | No float or double `sin(2 pi k / N)` run for `N = 64 .. 4096` in `bank0_CODE`. A Q31 or int16 table, or a generated one (computed on the ARM at program load), is not excluded | EMPIRICAL (negative scan) |

## 2. Implications

- The 128 x 129 row table (about 16.5 k words) is neither firmware rodata the harness can reach nor computed by the DSP in a pass. Its source is an ARM path that the settled harness does not execute: the program-load / strip apply routine (`0xC01CE7D8`, not reached by the harness, [synth-stage-models-13](synth-stage-models-13.md) s1) or a lazy generator. Without that data a port of `0x124269` / `0x12453F` would invent the waveform (ASSUMED shape), which breaks the audio-parity rule.
- No firmware or table data was committed, no encryption involved or bypassed.

## 3. Follow-ups

1. Run the real strip apply `0xC01CE7D8` (via `tools/arm_program_load.py`) with a program that has a FM layer (find one in the factory banks with `tools/ns4_inspect.py`), and log every DSP store `>= 0xE0926`.
2. Static: xref `bank0_CODE` for the writer of the row region from the strip builder `0xC01ACBF4(this, type 49..53, table_id)`: look for a loop of 129 stores per row and `sin` / `pow` calls.
3. If the fill is a generator, derive the formula from the DSP reads (row index = modulation index?) and check it against `sharc-org` with the table poked; if the table is rodata, extract it as derived numbers only if small (D22 / D56 / D62 sizing).
4. Then decode `0x124269` / `0x12453F` word for word as in batches 11 to 13, opt-in first (D22 defaults). OQ22 and ROADMAP s7 stay open on this item.
