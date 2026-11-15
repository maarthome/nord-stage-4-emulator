# Synth layer level law: the firmware stores `(v / 127)^2`

- **Checked:** 2026-11-15
- **Sources (local, never committed):** `target/fw_unpacked/bank0_CODE.bin` (ARM, capstone, unicorn); `tools/arm_layer_level.py`; Part A video `_zksT69H30c` and the factory backup (`tools/emu_vs_hw.py`); [piano-receiver-scale](piano-receiver-scale.md) s5.2 / s6.2; [grand-pad-vs-video](grand-pad-vs-video.md); [synth-validation](synth-validation.md) (Leaky Stabs II).
- **Question:** Does the synth layer level (edit ids `0xA4..0xA6`, `Edt_SynCom_kLayerLevelSynA/B/C`, 0..127) reach the DSP as `v / 127` or as `(v / 127)^2` like the organ and piano layers (D26)?

## 1. Which handler (VERIFIED, listing)

Two earlier notes named two handlers. The one that stores the level is **`0xC011BB48`** (inside the dispatcher `0xC011BA38`, ids `<= 0xA6`; `cmp r2, #0xA3` sends ids `0xA4..0xA6` to this code). `0xC0114588` (also reached with ids `0xA4..0xA6`, from a different dispatcher) rounds `v * [this + 0xB4]` into a 16-bit word at `[[this + 0xA8] + 0x48]` and never calls the sender: it is a display / edit-state path, not the DSP level.

The body of `0xC011BB48` is the organ body `0xC011AC38..0xC011AC98` instruction for instruction: `x = float(q16 value) * 0x34010204` (`1 / (127 * 65536)`); `x < 0x33D6BF95` (1e-7) selects the mute sentinel `-0.05` (`0xBD4CCCCD`), else `x * x`; store at `table[7 * layer + slot + 0x34]`, then `0xC023195C(table, layer, slot)`. The slot comes from `[this + 0x64]` (0..5): `{0, 1}` slot 4, `{2, 3}` slot 5, `{4, 5}` slot 6 (synth A, B, C).

## 2. The run (VERIFIED)

`tools/arm_layer_level.py` calls the handler in the unicorn harness with a fabricated controller (layer `[this + 0x90]`, table `[this + 0xB0]`, event kind 8, sender stubbed) and reads the stored word. For v = 0, 1, 2, 10, 32, 63, 64, 100, 126, 127 and the three ids (30 cases) the word equals `(v / 127)^2` within 1e-6 relative (v = 100 gives `0x3F1EB866` = 0.6200012, v = 127 gives 1.0, v = 64 gives 0.2539525); v = 0 stores `-0.05`. With the organ and piano handlers the tool checks 70 of 70 cases (`uv run --with unicorn python tools/arm_layer_level.py target/fw_unpacked/bank0_CODE.bin`; test `tools/tests/test_arm_layer_level.py`). The sender receives `(table, 3, 4 / 5 / 6)`.

## 3. How the DSP consumes it (VERIFIED structure, earlier runs)

[piano-receiver-scale](piano-receiver-scale.md) s5.2 / s6.2: the sender writes the word times a scale cell into FX core 1 rows 17..19 (pointers `0xC0458 / 60 / 68`, the PNO buses). The FX consumer (listing `0x1A5B..0x1A86`) forms `g = max((1 - c) t + c s, 0)` (one-pole smoother, `c = 0.99`) and multiplies the bus lanes by `g`: the DSP is **linear in the word** (piano rows 15, 16 and the organ words were measured the same way, [arm-boot-rederive](arm-boot-rederive.md) s1). With Reverb GLOBAL off the cell is 1.0 (3.25 with it on). So the fader gain is `(v / 127)^2`; nothing squares or linearises it again. The synth rows 17..19 use the same consumer code as rows 15 and 16; no separate SHARC run was made for them.

## 4. Clips (EMPIRICAL, one video, `tools/emu_vs_hw.py`, gap score, lower is better)

Layer levels: 0.567 (Grand Pad A), 0.82 / 0.32 (Leaky Stabs II A / B).

| Segment, notes | Linear (old default) | Squared (new default) |
|---|---|---|
| `synth_leaky`, audio notes (552) | 3.19 (RMS -2.9 dB, presence +4.3, brilliance +7.5) | **1.95** (RMS -5.2, presence +2.7, brilliance +3.3) |
| `synth_leaky`, Transkun notes (162) | 5.38 | **3.97** |
| `pianopad_grand`, audio notes (311) | 2.14 | 2.19 (tie) |
| `pianopad_grand`, Transkun notes (127) | 1.48 (RMS +0.4) | **1.42** (RMS -1.0) |

The Leaky Stabs II excess of D83 (about 8 dB of noise-layer level in the 1.5 to 8 kHz bands) is closed by the verified law, no fit; the Grand Pad optimum of D82 is the same law. The DSoundman clip of [dsoundman-piano-pad-layer](dsoundman-piano-pad-layer.md) s4 (fader 0.7: -2.7 dB measured, -3.1 linear, -6.2 squared) is not explained; its fader taper (OLED value against cap position) is unknown, so the firmware word outranks it.

## Implications

- `Stage4Engine::synth_layer_law` and `FirmwareOptions::synth_layer_law` default to `true`; `--legacy-synth-layer-law` restores linear (`--legacy-all` includes it); `--firmware-synth-layer-law` stays accepted (D87).
- `render_hash` segment `engine` moves (peak 0.995080 to 0.951197); the other four segments are unchanged.

## Follow-ups

- Reverb GLOBAL on (scale cell 3.25) is not modelled in the engine.
- A hardware sweep at fixed fader positions would calibrate the fader taper of the DSoundman clip.
