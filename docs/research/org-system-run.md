# Research: first system run, ARM firmware driving the SHARC ORG Core2 passes (D119)

- **Checked:** 2026-11-16
- **Sources (local, never committed):** `target/fw_unpacked/bank0_CODE.bin` and `bank0_SRAM.bin` (OS v1.66), the factory restore EEPROM image `target/oled/eeprom.bin`, program `factory_programs/Program/Bank A/B3 Chords n Solo.ns4p`, ORG Core2 image `core2_sram.bin` (`python tools/ns4_dsp_images.py ... --out IMG`, `python tools/ns4_dsp_ldr.py IMG/dsp_image_2_ORG.bin --sram DIR`). Tools: `tools/arm_org_stream.py` (new), `tools/arm_program_load.py`, `tools/arm_keydown.py`, `crates/nord-sharc` `sharc-sys` (new) and `sharc-org`. Background: [dsp-rx-protocol](dsp-rx-protocol.md), [arm-param-messages](arm-param-messages.md), [sharc-jit-real-org-diff](sharc-jit-real-org-diff.md), [sharc-jit-org-profile](sharc-jit-org-profile.md), [dsp-frame-rate](dsp-frame-rate.md), [D119](../DECISIONS.md#d119-the-sharc-oracle-becomes-the-dsp-half-of-a-minimal-system-run-opt-in-org-core2-only-no-default-change).
- **Question:** What does the real ARM to ORG Core2 message stream (program loaded and idle, 1 key down, 8 keys down) do to the SHARC pass, how does that compare with the stage-A poke proxy, and what does each component of one pass interval cost when ARM and SHARC run together?

Tags follow `.agents/rules/provenance-in-code.md`: **VERIFIED** = reproduced by the command shown, **EMPIRICAL** = measured on this host (noisy, see section 4), **ASSUMED** = reading or placeholder.

## 1. Short answer

| Question | Result | Tag |
|---|---|---|
| Does a key press reach ORG Core2? | **No.** 1 key and 8 keys send 126 and 907 messages, all to **PNO core 1**; ORG core 1 and core 2 get 0 (idle: 3 messages, all PNO core 1). The poke proxy's stage-A pointers (`0x1215FB`) are not written by any ARM message of this program. | VERIFIED (section 2) |
| What does the real stream do to ORG Core2? | Program load plus the initial push send **912 messages to ORG Core2** (13 bursts of 7 to 143 words within the first 234 ticks). Steady state is **6,377 steps per pass** (idle proxy 4,265, 8-voice proxy 5,953). | VERIFIED |
| Does the voice routine run? | **No.** `0x1215FB` runs in 0 of 316 passes (proxy: 316 of 316). The stage-A slots stay `0x120BF7` (null) with `0x120300` in slots 2 and 3. The extra 2,112 steps are a new block `0x12566E..0x1257B1` (136 new pcs, 4,200 distinct pcs per pass against 4,068 idle). | VERIFIED |
| Does the real stream run without help? | **No.** Pass 15 reads `DM(0x3E021)` (external-port DMA register block) as the loop count of `DO (PC, 3) UNTIL LCE, LCNTR = R0` at `0x12717F`; the oracle reads 0 and faults. Pre-setting the word to 1 lets 316 passes run. | VERIFIED (fault), ASSUMED (value 1) |
| Audio and DSP state | Pass buffers `0xB0000..0xB2FFF`: **0 words change in every run** (real, proxy, `sharc-org`): ORG Core2 produces no audio in any state tried. Module state differs: 520 / 537 / 656 words against proxy idle / 1 voice / 8 voices. | VERIFIED |
| Real-time | One pass is **46.08 us** of audio. SHARC alone needs **10.7x** (best run) to 11.3x (median) the interpreter speed; the ARM harness runs at **17.7 ms per 1 ms tick (17.7x slower than real time)**. Together **28x**, before DMA, interrupts and the other cores. | EMPIRICAL |

## 2. Real message streams

`tools/arm_org_stream.py` boots the full machine with the program in the Store record, runs 800 ticks (recall plus initial push), 300 idle ticks, then key phases of 40 ticks on + 40 off. Messages per phase and receiving DSP (`docs/research/data/org-system-arm-stream.json`):

| Phase | Ticks | ORG core 2 | ORG core 1 | PNO core 1 | PNO core 2 | FX 1 / 2 | SMP path 1 |
|---|---:|---:|---:|---:|---:|---:|---:|
| load (boot to settle) | 800 | **912** | 912 | 7,340 | 912 | 1,840 / 1,905 | 1,516 |
| idle | 300 | 0 | 0 | 3 | 0 | 0 | 0 |
| 1 key down (C4) | 40 | **0** | 0 | 126 | 0 | 0 | 0 |
| 1 key up | 40 | 0 | 0 | 59 | 0 | 0 | 0 |
| 8 keys down (C4..C5 white) | 40 | **0** | 0 | 907 | 0 | 0 | 0 |
| 8 keys up | 40 | 0 | 0 | 789 | 0 | 0 | 0 |

The 912 ORG core 2 messages are in `docs/research/data/org-system-load-core2.script` (`PASS W1 W2`, W1 = DM word, `--msg-base 0`; pass = `floor(tick * 21.70)`). Key scripts for ORG core 2 are empty, so "program loaded + idle", "1 key" and "8 keys" are the **same ORG Core2 run**; `sharc-sys` confirms it: with the key scripts added all SRAM data words equal the load run (0 of 0x70000 differ, section 3).

Why no ORG voice: in this firmware the note path writes the PNO image (organ models and keys, [keydown-paths](keydown-paths.md), [pno-full-pass](pno-full-pass.md)); [arm-param-messages](arm-param-messages.md) section 5 already found no key-on sender among the ORG strip senders. ORG Core2 receives only strips and controller words. The `0x1215FB` stage of the proxy is the synth saw stage address taken from `tools/synth_wave_oracle.py`; no ARM message in this run writes it into the ORG slot table.

### Step counts and coverage (316 passes, all messages before pass 0, `--pre 0x3E021=1`)

| Run | Steps per pass | `0x1215FB` passes | Distinct pcs (last pass) | Stage-A slots |
|---|---|---:|---:|---|
| proxy idle | 4,264 first, 4,265 | 0 / 316 | 4,068 | all `0x120BF7` |
| proxy 1 voice | 4,475 first, 4,476 | 316 / 316 | n/a | slot 0 `0x1215FB` |
| proxy 8 voices | 5,952 first, 5,953 | 316 / 316 | n/a | slots 0..7 `0x1215FB` |
| **real load = idle = 1 key = 8 keys** | 4,309, then ramp (below), **6,377** steady | **0 / 316** | **4,200** | all `0x120BF7`, `0x120300` in slots 2, 3 |

Real ramp (pass: steps, `sharc-org --coverage`): 0..14 grow 4,309 to 4,940 (+45 each), 15..20 = 3,167 to 3,187 (the `DM(0x3E021)` path, +4 each), 21..22 = 4,985, 23..38 grow +87 each to 6,377, 39 onward 6,377 constant. The steady value is the same for `DM(0x3E021)` = 1, 2, 4, 8 and 16 (min steps per pass 3,171 / 3,172 / 3,174 / 3,178 / 3,186, mean 6,279.6 to 6,279.9), so the stub value changes only the pass 15 to 20 window.

Poke-proxy reference points reproduce ([sharc-jit-real-org-diff](sharc-jit-real-org-diff.md) section 2): 4,265 / 4,476 / 5,953.

## 3. Comparison

| Comparison (316 passes, same buffers) | Result | Tag |
|---|---|---|
| `sharc-sys` vs `sharc-org` with the same real script (`--dump 0xB0000:12288 0xC0000:12288 0xE0000:768`) | identical text (125,392 bytes each) | VERIFIED |
| `sharc-sys --proxy-voices 8` vs `sharc-org` with 8 hand-written `--msg 0:0xc10b8..=0x1215fb` | identical text | VERIFIED |
| real key1 / key8 (load + empty key script) vs real load, all SRAM data words | 0 / 0 words differ | VERIFIED |
| real load vs proxy idle / 1 voice / 8 voices | 520 / 537 / 656 words differ: pass buffers 0 / 0 / 0, module state `0xC0000..0xC2FFF` 454 / 471 / 590, strips `0xE0000..0xE02FF` 66 each, elsewhere 0 | VERIFIED |
| real, messages at their natural passes (13 bursts over 5,078 passes, 17,516 passes) vs flat 316 passes | 297 words differ (285 module state, 12 strips): counters and phases at a different pass count; steady steps 6,377 in both | VERIFIED |
| Hardware clip (Part A demo video, [emulator-vs-hardware-audio](emulator-vs-hardware-audio.md), `tools/emu_vs_hw.py`) | not comparable: the ORG Core2 pass buffers hold 0 changed words, so output RMS is 0 in every run while the hardware segment is non-silent. The B3 sound is rendered by the PNO image, outside this scope. | VERIFIED (0 words), no metric computed |

## 4. Per-component timing

300 measured passes after 16 warm-up passes, interpreter, `target/release`, Windows 11 host with other sessions running (so single runs vary by up to 1.6x; the table gives the best run mean, the median of 7 run means and the worst pass of all runs). `sharc` = interpreter steps from the trigger write to the poll loop, `msg` = message delivery (store of the due words), `stub` = input-refresh stub (cost only), `total` = sum.

| Run | Steps per pass | sharc best / median mean (us) | msg mean (us) | stub mean (us) | total best / median mean (us) | worst pass (us) | total best / 46.08 us |
|---|---:|---:|---:|---:|---:|---:|---:|
| proxy idle | 4,265 | 331.0 / 512.4 | 0.03 | 0.29 | 331.3 / 512.9 | 1,893.6 | 7.2x |
| proxy 1 voice | 4,476 | 470.9 / 589.1 | 0.05 | 0.53 | 471.5 / 589.9 | 2,629.4 | 10.2x |
| proxy 8 voices | 5,953 | 500.9 / 590.8 | 0.06 | 0.49 | 501.5 / 591.6 | 2,873.7 | 10.9x |
| **real load / idle / 1 key / 8 keys** | 6,377 | 492.0 / 521.6 | 0.63 | 0.29 | 492.9 / 522.6 | 2,717.8 | **10.7x** |

- Interpreter speed 12.9 Msteps/s (best run, real: 6,377 / 492.9 us; proxy idle 4,265 / 331.3 us). The real stream needs **6,377 / 46.08 us = 138.4 Msteps/s** on one core.
- Message delivery: the measured window holds no messages for the flat run (the 0.63 us is the per-pass scan of the 912-entry script). With natural pass numbers (17,500 passes, 13 bursts of up to 143 words) the delivery mean is 0.68 us and the worst pass 21.3 us, about 0.15 us per delivered word. The ARM to DSP link itself (SPI frame, 3 words per message, [dsp-rx-protocol](dsp-rx-protocol.md)) is **not timed**.
- DMA / stub: 0.3 to 0.5 us per pass for rewriting 32 input words. This is a cost placeholder; real audio DMA is not modelled.
- Worst passes (2 to 3 ms) are host scheduling spikes (the same script run 7 times gives worst passes from 1.0 to 2.7 ms); they do not come from the DSP work (step counts are constant).

ARM side (`arm_org_stream.py`, Unicorn, 200,000 ARM instructions per 1 ms tick, `docs/research/data/org-system-arm-stream.json`, first 16 ticks of a phase dropped):

| Phase | n ticks | tick wall mean / median / worst (ms) | per pass interval, mean (ms) | per pass interval, worst tick (ms) |
|---|---:|---:|---:|---:|
| load | 784 | 16.3 / 14.6 / 69.2 | 0.753 | 3.19 |
| idle | 284 | 17.7 / 17.0 / 30.1 | 0.815 | 1.39 |
| 1 key down | 24 | 21.9 / 21.7 / 24.7 | 1.009 | 1.14 |
| 8 keys down | 24 | 21.6 / 21.5 / 24.3 | 0.995 | 1.12 |

Pass interval = tick / 21.70 passes per tick. Emulating 200,000 ARM instructions takes 17.7 ms (11.3 M instructions per second in Unicorn); the real ARM would need 200 M instructions per second for the same tick (ASSUMED tick length 1 ms, 200,000 instructions per tick is the harness constant `INSNS_PER_TICK`, not a measured ARM rate).

Whole interval (sequential, one thread): ARM idle 0.815 ms + SHARC 0.493 ms = 1.31 ms for a 46.08 us pass: **28.4x too slow**; ARM key phases 1.0 ms + 0.49 ms = 1.5 ms (32x).

### Pass period

One pass is 2 audio frames at `F = 43,402.34375 Hz` (44,444,000 / 1024) = **46.08 us**. `F` is VERIFIED by five independent readouts ([dsp-frame-rate](dsp-frame-rate.md): ARM literal `0x47298A58`, FX delay words, SY fade constants, Pipe bank ladder, B3 generator), so other paths agree on `F`. **2 frames per ORG Core2 pass is still ASSUMED**: it is read from the B3 generator and the Vox note step, not from an ORG Core2 audio output (none exists, section 3). The poll loop is woken once per pass by the Core1 P4I clock; if the real pass were 1 frame the factors above double.

## 5. What is stubbed or unmodelled

| Part | State in this run |
|---|---|
| ARM side | Unicorn with stubbed peripherals, 1 ms tick = 200,000 instructions (ASSUMED), real firmware code |
| ARM to DSP link | messages applied as `DM(word) = value` before the named pass; SPI/DMA latency, the 3-word frame and ordering against the pass are not modelled; message-to-pass mapping `floor(tick * 21.70)` is ASSUMED |
| Audio DMA | stub: the 32 input words are re-stored each pass; no samples are delivered |
| External-port DMA | not modelled. `DM(0x3E021)` is read at pass 15 as a loop count; run with `--pre 0x3E021=1` (ASSUMED value, no hardware reading). `0x3E002/3`, `0x3E010..12` are written by the same code (store sites `0x12714B`, `0x127150`, `0x127155`) |
| Interrupts | none; the pass is started by the mailbox write (`DM(0xB00E2)`), not by P4I |
| Other cores | ORG Core1, PNO, FX, SY, SMP are not run; PNO core 1 receives the key messages and would carry the organ voice |
| Timing | wall clock of an interpreter on a loaded host, no cycle model; the SHARC cycle count of a pass is not measured |

## Implications

- A key press does not change the ORG Core2 pass in this firmware, so "1 key" and "8 keys" cost the same as idle for this core; the poke proxy overstated it for voices and understated it for the program load (4,265 to 5,953 against 6,377 steps).
- Single-core real time needs 138.4 Msteps/s for the real ORG load (against 92.6 / 97.1 / 129.2 for the proxy states in [sharc-jit-org-profile](sharc-jit-org-profile.md)); the interpreter is 10.7x short, the ARM harness 17.7x short, and neither the DMA nor the other four cores are in the figure.
- The ARM side, not the SHARC, is the larger share of the interval (0.82 ms against 0.49 ms) in this harness.
- Any JIT/AOT comparison on ORG Core2 should use the real-load state (6,377 steps, block `0x12566E..0x1257B1`) in addition to the proxy states.

## Follow-ups

1. Find out what the `0x12566E..0x1257B1` block and the pass 15 window compute (controller words `DM(0xC1DCC + i)`, [org-stage-map](org-stage-map.md)), and the real value of `DM(0x3E021)`; model the external-port DMA registers.
2. Run PNO core 1 with the same ARM stream (that is where the keys land) and measure it; then ORG Core1 and the other images for a multi-core budget (out of scope here).
3. Time the real ARM message delivery (SPI frame) and the P4I interrupt latency against the pass.
4. Repeat the timing on an idle host or pin the process; the 1.6x spread between runs hides differences below 40 %.
5. Check 2 frames per ORG Core2 pass from a real audio-producing state.

## Reproduce

```bash
C=target/fw_unpacked
# ARM side: message streams, per-tick timing (about 5 minutes of CPU, writes under target/org_system)
cd tools && uv run --with unicorn python arm_org_stream.py ../$C/bank0_CODE.bin ../$C/bank0_SRAM.bin \
  --eeprom ../target/oled/eeprom.bin "../factory_programs/Program/Bank A/B3 Chords n Solo.ns4p" --out ../target/org_system
cd ..
cargo build --release -p nord-sharc --bin sharc-sys --bin sharc-org
IMG=core2_sram.bin
S=docs/research/data/org-system-load-core2.script
# real load = idle = 1 key = 8 keys (ORG core 2 gets no key message)
target/release/sharc-sys $IMG --passes 300 --warmup 16 --pre 0x3e021=1 --script-flat $S --probe 0x1215fb --state-out load.state
target/release/sharc-sys $IMG --passes 300 --warmup 16 --pre 0x3e021=1 --script-flat $S --script-at 0:target/org_system/org-sys-key8-core2.script --state-cmp load.state
# poke proxy
target/release/sharc-sys $IMG --passes 300 --warmup 16 --proxy-voices 8 --probe 0x1215fb --state-cmp load.state
# hand-written reference and coverage
awk '{ $1=0; print }' $S > flat.script
target/release/sharc-org $IMG --dev 0x282c0380=1 --dev 0x3108d008=5 --pre 0x3e021=1 --passes 60 --msg-base 0 --msg-script flat.script --coverage --dump 0xC10B8:0xA8
# fault without the stub
target/release/sharc-org $IMG --dev 0x282c0380=1 --dev 0x3108d008=5 --passes 16 --msg-base 0 --msg-script flat.script --watch 0x3e000:0x40 --rwatch 0x3e021:1
# timing table: 7 runs per scenario, then
python tools/org_system_timing.py run1.txt run2.txt ...
```
