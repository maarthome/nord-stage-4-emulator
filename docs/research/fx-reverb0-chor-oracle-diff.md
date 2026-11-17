# Reverb type 0 and CHOR in the live FX oracle against the native ports: causes and fix

- **Checked:** 2026-11-17
- **Sources (local, never committed):** the FX Core 1 flat SRAM image (`tools/ns4_dsp_ldr.py --sram` of the v1.66 FX image), the oracle `sharc-fxlive` (`crates/nord-sharc`, [fx-oracle-live](fx-oracle-live.md), D125), the native renderer `fx_live_native` (`crates/nord-tools`), `tools/fx_oracle_live.py`, `sharc-disasm` listings of the pass routine. Only numbers derived from runs are quoted; no firmware, sample or audio content is printed or committed.
- **Question:** why do the live oracle responses of reverb type 0 (routine `0x124AF2`) and CHOR (`0x123798`) differ from `FwReverb` and `FirmwareFx::mod2`, and which side is wrong?

## 1. Short answer

| Item | Result | Tag |
|---|---|---|
| Reverb type 0 | **No port error.** The D125 mismatch was the harness: the dispatcher called the installed stage **35 times per pass**, not once. Reverb onset 38 passes = 1352 calls / 35. With one call per pass the oracle equals `FwReverb`: lag 0, gain 0.1866, null depth **-137.0 dB** (impulse), **-130.2 dB** (noise), 1.0 s each | VERIFIED (`tools/fx_oracle_live.py reverb0`) |
| CHOR type 4 | **Port bug, fixed.** After the harness fix the right channel matched, the left wet tap was missing in the port (oracle: wet at 32/33 frames at phase 0, 77/78 after 12000 frames; port: none). Cause: the port wrapped the register pre-modify store `DM(M6, I6)` (`I6 + 0x202`, `L6 = 0x200`) into the first ring, so the guard copy was never written (the error D114 removed from ENS and SPIN). Fixed with `Pe::plain`. Null depth before / after: -18.0 / **-125.3 dB** (noise), -19.7 / **-153.6 dB** (burst), -18.9 / **-126.2 dB** (impulse) | VERIFIED |
| CHOR variation (type 5) | Same bug, same fix. Null depth -26.2 / **-89.5 dB** (noise), -92.8 dB (impulse), -91.9 dB (burst). The residual is the f32 against 40-bit arithmetic of the port | VERIFIED |
| State-by-state check | The port started from the live oracle state (context, ring, input block at routine entry, pass 3000) and run for 80 calls: every output word within 9.2e-7 absolute, the ring equal at every call entry | VERIFIED (scratch probe, not committed) |
| Latency | The port stage adds 4 frames (it collects a block); the oracle chain has none. The comparison script finds lag -4 for CHOR. A port property, not a defect of the routine | observed |
| Provenance | `FwReverb` stays VERIFIED (golden, and now the live chain). CHOR stays VERIFIED; the old goldens came from a Python twin with the same wrap error and were regenerated | - |

## 2. Why the stage ran 35 times

The Core 1 pass routine is unrolled: 35 `CALL (M8, I8)` sites in 10 groups (5, 2, 5, 2, ...). Each group starts with `I1 = DM(header)` (headers at `DM(0xB63EC, 0xB6416, 0xB6428, 0xB6452, 0xB6464, 0xB648E, 0xB64A0, 0xB64CA, 0xB64DC, 0xB6506)`), every later site takes `I1 = DM(state - 0x1D)`-style links. In the image all headers and the head record's link (`DM(0xB62D1)`) point at the head record `0xB62EE`, the record D125 installed the stage in. `sharc-fxlive --calls-at 0x124AF2` (new) counts 35 entries per pass, all with state `0xB27E8`; with only the head link fixed 10; with the link and the nine later headers pointed at a copy of the null stub record (8 words at free `DM(0xB6600)`, new `null_chain()` in `fx_oracle_live.py`) exactly 1. The identity stage is unaffected (0.1866 either way: the stage output replaces the bus, it is not added), which is why the D125 identity check passed.

So the three D125 candidates (scratch protocol, instance layout, CHOR context) were not the cause: the scratch holds the scaled input (the probe pass had no signal), and the CHOR context location does not change the result (`0xC13A4` and `0xB2040` give identical numbers).

## 3. The CHOR bug

Ring stores in `0x122E24` (core 1 `0x123798`): `DM(I6, M6) [pre-mod] = R` with `M6 = 0x202` and `L6 = 0x200`, then `DM(I6, M1) = R` (post-modify). PRM fig 6-5: only the post-modify and `MODIFY` update wrap; the pre-modify address is `I + M`. The ports used `dmod` (wrapping) in address positions, so the mirror store hit `I6` again. Ring difference at routine entry (oracle against port): one word, the mirror of the left sample, at ring offset `0x2E2` (0x200 + 0xE2). The left tap reads the mirror region once the read pointer passes the ring end; without the copy it read zeros. Right reads the primary copy, hence matched.

## Implications

- Reverb type 0 and CHOR 4 / 5 now match the oracle through the real dispatcher (`tools/fx_oracle_live.py`: reverb0, chor4, chor5).
- CHOR on the `FirmwareFx::mod2` path (opt-in, D22) changes in the left wet channel; defaults and `render_hash` are unchanged.
- No other generated port has `dmod` in an address position left (checked `fw_flang`, `fw_vibe`, `fw_phas1`, `fw_pump`, `fw_rm`, `fw_wah`, `fw_awah`, `fw_spin`, `fw_ens`).

## Follow-ups

1. ENS, FLANG, PHAS, VIBE through the live chain (the null-chain install makes them one call per pass).
2. Feed a PNO lane WAV as FX input; decide whether the 7-layer block needs more than layer 0 (D125 open item).
3. A hardware clip for CHOR only after this; OQ21a unchanged.

## Reproduce

```bash
cargo build --release -p nord-sharc -p nord-tools
python tools/ns4_dsp_ldr.py <fx>/dsp_image_0_FX.bin --sram <dir>
python tools/fx_oracle_live.py reverb0 --image <dir>/core1_sram.bin --signal impulse --seconds 1.0
python tools/fx_oracle_live.py chor4   --image <dir>/core1_sram.bin --signal noise   --seconds 1.0
python tools/fx_oracle_live.py chor5   --image <dir>/core1_sram.bin --signal noise   --seconds 1.0
```
