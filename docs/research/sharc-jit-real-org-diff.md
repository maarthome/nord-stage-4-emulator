# Real ORG (`core2_sram.bin`) JIT vs Interpreter Differential Verification

Date: 2026-11-16
Branch: `sharc-jit-diff-passes`
Target image: `core1_sram.bin` and `core2_sram.bin` extracted from `bank0_CODE.bin` via:

```powershell
python tools/ns4_dsp_images.py "<path>\bank0_CODE.bin" --out "$env:TEMP\ns4_sram\dsp"
python tools/ns4_dsp_ldr.py "$env:TEMP\ns4_sram\dsp\dsp_image_2_ORG.bin" --sram "$env:TEMP\ns4_sram\org_sram"
```

## 1. Strict boot run without `--lenient` (200k steps)

Both `core1_sram.bin` and `core2_sram.bin` matched 1:1 for 200,000 steps without `--lenient` (0 faults, 0 skipped ops):

```powershell
cargo run --release -p nord-sharc --features jit --bin sharc_jit_diff -- "$env:TEMP\ns4_sram\org_sram\core1_sram.bin" --pc 0x125c88 --steps 200000 --chunk 1000
# OK: 200000 steps matched 1:1 (chunk=1000, final pc=0x125c1c)

cargo run --release -p nord-sharc --features jit --bin sharc_jit_diff -- "$env:TEMP\ns4_sram\org_sram\core2_sram.bin" --pc 0x125c88 --steps 200000 --chunk 1000
# OK: 200000 steps matched 1:1 (chunk=1000, final pc=0x125da1)
```

## 2. Audio passes (idle, 1 voice, 8 voices) — strict lockstep diff & coverage

In `core2_sram.bin`, the 8 stage-A slot routine pointers sit at `DM(0xC10B8 + k * 0x18)` (`k = 0..7`, default `0x120BF7` null stage). Writing `STAGE_SAW` (`0x1215FB`, per `tools/synth_wave_oracle.py`) via `--msg-base 0` activates the saw oscillator (`0x1215FB..0x12169C`) and its helper (`0x12501F..0x1250B0`), adding **+211 steps/pass per active voice**.

All three states ran for 8 passes after `--boot 100000` (`pc=0x125de0`) **without `--lenient`**, comparing full architectural state + SRAM every 1,000 steps and stopping on first divergence (0 divergences observed):

- **State 1 — ORG idle** (`--msg-base 0`, no `--msg`):
  - `pass 0: OK (4264 steps, final pc=0x125ddf)`, `pass 1..7: OK (4265 steps, final pc=0x125ddf)` (34,119 steps total)
  - `0x1215FB` / `0x12501F` not executed.
- **State 2 — 1 voice** (`--msg-base 0 --msg 0:0xc10b8=0x1215fb`):
  - `pass 0: OK (4475 steps, final pc=0x125ddf)`, `pass 1..7: OK (4476 steps, final pc=0x125ddf)` (35,807 steps total, +211/pass)
  - Coverage includes `0x1215fb..0x12169c` and `0x12501f..0x1250b0` outside the `0x125DDC..0x125DE3` idle poll loop.
- **State 3 — 8 voices** (`--msg-base 0 --msg 0:0xc10b8=0x1215fb` .. `--msg 0:0xc1160=0x1215fb`, `0x18` stride):
  - `pass 0: OK (5952 steps, final pc=0x125ddf)`, `pass 1..7: OK (5953 steps, final pc=0x125ddf)` (47,623 steps total, +1,688/pass = 8 × 211)
  - Coverage includes `0x1215fb..0x12169c` and `0x12501f..0x1250b0` across all 8 slot call sites.

Reproducible command:

```powershell
$img = "$env:TEMP\ns4_sram\org_sram\core2_sram.bin"
cargo run --release -p nord-sharc --features jit --bin sharc_jit_diff -- $img `
  --pc 0x125c88 --boot 100000 --dev 0x282c0380=1 --dev 0x3108d008=5 `
  --trigger 0xB00E2 --idle-pc 0x125DDF --passes 8 --chunk 1000 --msg-base 0 --coverage `
  --msg 0:0xc10b8=0x1215fb --msg 0:0xc10d0=0x1215fb --msg 0:0xc10e8=0x1215fb --msg 0:0xc1100=0x1215fb `
  --msg 0:0xc1118=0x1215fb --msg 0:0xc1130=0x1215fb --msg 0:0xc1148=0x1215fb --msg 0:0xc1160=0x1215fb
```

## 3. Why `--bench` JIT step count rounds up (`34,304` vs `34,119`)

In `sharc_jit_diff`'s `run_pass_burst_timed`, the interpreter steps 1 instruction at a time and stops on the exact step `pc == idle_pc && DM(trigger) == 0` (`4,264 + 7 * 4,265 = 34,119` idle steps). The JIT path calls `Core::run_jit(64)` in 64-step chunks before checking the idle condition, so each pass rounds up to `ceil(4265 / 64) * 64 = 4,288` steps (`8 * 4,288 = 34,304`, +185 idle-poll steps across 8 passes).

## 4. Reproducible test

`runner::jit_diff::tests::diff_real_org_passes_from_env` (`#[ignore]`) in `crates/nord-sharc/src/runner/jit_diff.rs` verifies all three states (idle, 1 voice, 8 voices) without `--lenient`, asserting `0` divergences, strict step-count growth (`idle < 1-voice < 8-voices`), and `0x1215FB` coverage:

```powershell
$env:NORD_SHARC_ORG_IMAGE = "$env:TEMP\ns4_sram\org_sram\core2_sram.bin"
cargo test --release -p nord-sharc --features jit -- --ignored diff_real_org_passes_from_env
# test runner::jit_diff::tests::diff_real_org_passes_from_env ... ok (1.39s)
```

## 5. Warmed-up pass throughput & latency (`--warmup 16 --passes 300 --bench`, release)

Measured after 16 unmeasured warm-up passes (so JIT block compilation completes before timing) over 300 consecutive passes without `--lenient`:

| State | Interp steps | Interp mean / worst (ms) | Interp Msteps/s | JIT steps | JIT mean / worst (ms) | JIT Msteps/s |
|---|---:|---:|---:|---:|---:|---:|
| ORG idle | 1,279,500 | 0.411 / 1.317 | 10.38 | 1,286,400 | 0.737 / 1.616 | 5.82 |
| 1 voice (`0xc10b8=0x1215fb`) | 1,342,800 | 0.510 / 1.265 | 8.77 | 1,344,000 | 0.928 / 2.158 | 4.83 |
| 8 voices (`0xc10b8..0xc1160=0x1215fb`) | 1,785,900 | 0.554 / 1.349 | 10.74 | 1,785,600 | 1.006 / 2.144 | 5.92 |

Reproducible command (append `--msg 0:0xc10b8=0x1215fb` .. `--msg 0:0xc1160=0x1215fb` for 1 or 8 voices):

```powershell
$img = "$env:TEMP\ns4_sram\org_sram\core2_sram.bin"
cargo run --release -p nord-sharc --features jit --bin sharc_jit_diff -- $img `
  --pc 0x125c88 --boot 100000 --dev 0x282c0380=1 --dev 0x3108d008=5 `
  --trigger 0xB00E2 --idle-pc 0x125DDF --warmup 16 --passes 300 --chunk 1000 --msg-base 0 --bench
```

