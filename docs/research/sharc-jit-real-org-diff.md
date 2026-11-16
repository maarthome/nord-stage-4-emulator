# Real ORG (`core2_sram.bin`) JIT vs Interpreter Differential Verification

Date: 2026-11-16
Branch: `sharc-jit-diff-passes`
Target image: `core1_sram.bin` and `core2_sram.bin` extracted from `bank0_CODE.bin` via:

```powershell
python tools/ns4_dsp_images.py "<path>\bank0_CODE.bin" --out "$env:TEMP\ns4_sram\dsp"
python tools/ns4_dsp_ldr.py "$env:TEMP\ns4_sram\dsp\dsp_image_2_ORG.bin" --sram "$env:TEMP\ns4_sram\org_sram"
```

## 1. Strict run without `--lenient` (boot / 200k steps)

Both `core1_sram.bin` and `core2_sram.bin` matched 1:1 for 200,000 steps without `--lenient` (0 faults, 0 skipped ops):

```powershell
cargo run --release -p nord-sharc --features jit --bin sharc_jit_diff -- "$env:TEMP\ns4_sram\org_sram\core1_sram.bin" --pc 0x125c88 --steps 200000 --chunk 1000
# OK: 200000 steps matched 1:1 (chunk=1000, final pc=0x125c1c)

cargo run --release -p nord-sharc --features jit --bin sharc_jit_diff -- "$env:TEMP\ns4_sram\org_sram\core2_sram.bin" --pc 0x125c88 --steps 200000 --chunk 1000
# OK: 200000 steps matched 1:1 (chunk=1000, final pc=0x125da1)
```

## 2. Audio passes with ARM messages (`--msg`, base `0xE0000`), coverage, and strict lockstep diff

Tested on `core2_sram.bin` (`--pc 0x125c88 --boot 100000 --dev 0x282c0380=1 --dev 0x3108d008=5 --trigger 0xB00E2 --idle-pc 0x125DDF --passes 8 --chunk 1000 --coverage --bench`, **no `--lenient`**):

- **State 1 — ORG idle** (no `--msg`):
  - `booted 100000 steps 1:1 (pc=0x125de0)`
  - `pass 0: OK (4264 steps, final pc=0x125ddf)`
  - `pass 1..7: OK (4265 steps, final pc=0x125ddf)` (0 divergences across full architectural state + SRAM)
  - Coverage: 3,716 PC ranges spanning `0x120bf7..0x120bfe`, `0x12538c..0x12539d`, `0x125b79..0x125b7f`, `0x125dad..0x128696` (confirmed execution leaves the `0x125DDC..0x125DE3` idle poll loop).
- **State 2 — One note / strip 0** (`--msg 0:0=0x8012082f`):
  - `booted 100000 steps 1:1 (pc=0x125de0)`
  - `pass 0: OK (4264 steps, final pc=0x125ddf)`, `pass 1..7: OK (4265 steps, final pc=0x125ddf)` (0 divergences)
- **State 3 — 8 voices / strips 0..7** (`--msg 0:0=0x8012082f` .. `--msg 0:434=0x8012082f` at 62-word stride):
  - `booted 100000 steps 1:1 (pc=0x125de0)`
  - `pass 0: OK (4264 steps, final pc=0x125ddf)`, `pass 1..7: OK (4265 steps, final pc=0x125ddf)` (0 divergences)

Note: On this `core2_sram.bin` revision (`dsp_image_2_ORG.bin` from `bank0_CODE.bin`), `sharc-org` with `--msg 0:0=0x8012082f` also executes 4,264 steps on pass 0 and 4,265 steps on subsequent passes without branching to `0x12082F`.

## 3. Pass throughput & latency (`--bench`, 8 passes, release)

`bench` measurements (pass 0 includes JIT block compilation on the JIT side):

| State | Interp steps | Interp mean / worst (ms) | Interp Msteps/s | JIT steps | JIT mean / worst (ms) | JIT Msteps/s |
|---|---:|---:|---:|---:|---:|---:|
| ORG idle | 34,119 | 0.424 / 0.751 | 10.06 | 34,304 | 2.931 / 17.779 | 1.46 |
| 1 note (`strip 0`) | 34,119 | 0.446 / 0.624 | 9.55 | 34,304 | 2.906 / 17.238 | 1.48 |
| 8 voices (`strips 0..7`) | 34,119 | 1.427 / 4.828 | 2.99 | 34,304 | 4.409 / 24.788 | 0.97 |

## 4. Reproducible test

Added `runner::jit_diff::tests::diff_real_org_passes_from_env` (`#[ignore]`) in `crates/nord-sharc/src/runner/jit_diff.rs`:

```powershell
$env:NORD_SHARC_ORG_IMAGE = "$env:TEMP\ns4_sram\org_sram\core2_sram.bin"
cargo test --release -p nord-sharc --features jit -- --ignored diff_real_org_passes_from_env
# test runner::jit_diff::tests::diff_real_org_passes_from_env ... ok (1.57s)
```
