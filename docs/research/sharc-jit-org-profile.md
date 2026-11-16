# ORG Core2 Pass Audio Budget & JIT vs Interpreter Profile

Date: 2026-11-16
Branch: `sharc-jit-profile-org`
Target image: `core2_sram.bin` extracted from `bank0_CODE.bin` via `tools/ns4_dsp_images.py` and `tools/ns4_dsp_ldr.py`.

## A. Pass duration in audio time & real-time budget

### 1. Samples per pass and sample rate

From `docs/research/org-dsp-map.md`, `docs/research/synth-super-lp24-clips.md` (s5, s7), and direct `sharc-org` verification on `core2_sram.bin`:

- **4 stage samples per pass at `2F` = 2 audio frames per pass at `F`**:
  - Each channel's filter stage runs **once per ORG Core2 pass** and writes **4 consecutive samples** across 2 SIMD lanes (24 store PCs, 2 stores each, for LP24 `0x12566E`):
    ```powershell
    .\target\release\sharc-org.exe $img --dev 0x282c0380=1 --dev 0x3108d008=5 --passes 1 --msg-base 0 --msg 0:0xc0b8c=0x12566e --msg 0:0xc10b8=0x125b79 --watch 0xC2938:12
    # 24 store PCs (0x1256d1..0x1257ac), 2 stores each, skipped ops 0
    ```
  - Note 60 (`--pre 0xC1DCC=0x0B008000`) sets the oscillator pitch word `DM(0xC1F79) = 0x3bc585c6` (`0.0060279 = 261.6256 / 43402.34375`, relative error `7e-7`), and the oscillator advances `note / (2F)` cycles per stage sample (`2F = 86,804.69 Hz`, base rate `F = 43,402.34375 Hz`):
    ```powershell
    .\target\release\sharc-org.exe $img --dev 0x282c0380=1 --dev 0x3108d008=5 --passes 5 --pre 0xC1DCC=0x0B008000 --dump 0xC1F78:4
    # DM(0xc1f78..): 0 3bc585c6 3bc585c6 0
    ```
  - The 40-word pass buffers (`DM(0xB2EF4..F8) = 0xB0050 0xB0000 0xB0028 0xB0078 0xB00C8`, `DM(0xB2EF3) = 0x282c01e0`) hold **5 groups × 8 words** (4 stage samples × 2 lanes each), not 20 or 40 frames of one source.
- **Pass cadence & wall-clock budget**:
  - `2 frames / pass` at `F = 43,402.34375 Hz` -> **`21,701.17 passes/s`** -> **`46.08 µs` (`0.04608 ms`) per pass**.

### 2. Real-time SHARC step budget (ORG Core2 alone)

| State | Steps / pass (pass 1+) | Real-time step rate needed | Unprofiled interp (`--warmup 16 --passes 300`) | Unprofiled JIT (`--warmup 16 --passes 300`) |
|---|---:|---:|---:|---:|
| ORG idle | 4,265 | 92.55 Msteps/s | 0.411 ms (10.38 Msteps/s, 8.9× slow) | 0.737 ms (5.82 Msteps/s, 16.0× slow) |
| 1 voice (`0xc10b8=0x1215fb`) | 4,476 | 97.13 Msteps/s | 0.510 ms (8.77 Msteps/s, 11.1× slow) | 0.928 ms (4.83 Msteps/s, 20.1× slow) |
| 8 voices (`0xc10b8..0xc1160=0x1215fb`) | 5,953 | 129.19 Msteps/s | 0.554 ms (10.74 Msteps/s, 12.0× slow) | 1.006 ms (5.92 Msteps/s, 21.8× slow) |

## B. Profile of why JIT is slower than the interpreter on real ORG Core2

Measured with measurement-only instrumentation (`Core::run_jit_profiled` via `sharc_jit_diff --warmup 16 --passes 300 --bench --profile-jit`, release, no `--lenient`):

```powershell
$img = "$env:TEMP\ns4_sram\org_sram\core2_sram.bin"
.\target\release\sharc_jit_diff.exe $img --pc 0x125c88 --boot 100000 --dev 0x282c0380=1 --dev 0x3108d008=5 `
  --trigger 0xB00E2 --idle-pc 0x125DDF --passes 300 --warmup 16 --chunk 1000 --msg-base 0 --bench --profile-jit
```

### 1. Steps per pass: JIT-compiled vs interpreter fallback

| State | Total JIT-run steps/pass | JIT-compiled steps/pass | Fallback `Core::step()` steps/pass | `run_jit(64)` calls/pass | `short_chunk` skips/pass | `guard` fallbacks/pass |
|---|---:|---:|---:|---:|---:|---:|
| ORG idle | 4,288.0 | 143.2 (3.3%) | 4,144.8 (96.7%) | 67.0 | 4.2 | 90.0 |
| 1 voice | 4,480.0 | 133.0 (3.0%) | 4,347.0 (97.0%) | 70.0 | 5.0 | 90.0 |
| 8 voices | 5,952.0 | 131.2 (2.2%) | 5,820.8 (97.8%) | 93.0 | 4.2 | 90.0 |

### 2. Block-cache lookups, invalidations, and compile time after warm-up

| State | Lookups/pass | Cache hits/pass | Compiled/pass | Ineligible first op/pass (SIMD) | `invalidate_range` calls/pass | Blocks evicted/pass | `code_gen` resets |
|---|---:|---:|---:|---:|---:|---:|---:|
| ORG idle | 4,123.2 | 72.8 | 0.000 | 4,050.5 (3,896.0 SIMD) | 1,196.0 | 0.000 | 0 |
| 1 voice | 4,316.0 | 64.0 | 0.000 | 4,252.0 (4,107.0 SIMD) | 1,224.0 | 0.000 | 0 |
| 8 voices | 5,787.2 | 60.8 | 0.000 | 5,726.5 (5,584.0 SIMD) | 1,420.0 | 0.000 | 0 |

### 3. Time breakdown per pass (profiled run, ms/pass)

| State | Profiled total (ms) | Pre-lookup scan + `Vec` alloc (`scan_ns`) | Compile (`compile_ns`) | JIT block exec (`jit_exec_ns`) | Fallback `Core::step()` (`fallback_step_ns`) | Loop / `Instant` / chunking (`other`) | Unprofiled `Core::run` baseline (ms) |
|---|---:|---:|---:|---:|---:|---:|---:|
| ORG idle | 1.353 | 0.443 (32.7%) | 0.000 | 0.008 (0.6%) | 0.533 (39.4%) | 0.368 (27.2%) | 0.500 |
| 1 voice | 1.311 | 0.426 (32.5%) | 0.000 | 0.007 (0.5%) | 0.507 (38.7%) | 0.370 (28.2%) | 0.489 |
| 8 voices | 1.658 | 0.505 (30.5%) | 0.000 | 0.008 (0.5%) | 0.658 (39.7%) | 0.486 (29.3%) | 0.581 |

### Summary of findings

1. **96.7–97.8% of every pass falls back to `Core::step()`** because `can_jit_op(op, simd)` rejects all instructions when `simd == true` (`MODE1` PEY bit set), and ORG Core2 runs almost the entire pass (including all 8 `STAGE_SAW` voices) in SIMD mode.
2. **Pre-lookup `DecodeCache` scan + `Vec` allocation costs ~0.43–0.51 ms/pass (31–33%)** because `compile_or_lookup_jit_block` allocates a `Vec` and scans instructions **before** checking `jit.blocks.get(&key)` — 4,123–5,787 times per pass, of which 98%+ immediately fail on instruction 0 (`simd`).
3. **Zero compile or eviction overhead after warm-up**: `compiled = 0.000`, `evicted = 0.000`, `resets = 0` (`invalidate_range` is called 1,196–1,420 times/pass on data SRAM writes, but evicts 0 blocks).
4. **64-step chunking (`run_jit(64)`) is minor**: only `4.2–5.0` blocks/pass are skipped at chunk boundaries (`short_chunk`), and `67–93` `run_jit` calls/pass add `<= 63` extra poll-loop steps per pass.
