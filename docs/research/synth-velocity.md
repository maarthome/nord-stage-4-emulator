# Synth velocity from the firmware: amp velocity, filter-envelope velocity, factory census

- **Checked:** 2026-11-15
- **Sources (local, never committed):** OS v1.66 firmware (`target/fw_unpacked/`), ORG core 2 image (`tools/ns4_dsp_ldr.py DSP/dsp_image_2_ORG.bin --sram DIR`), `sharc-org` (`crates/nord-sharc`), factory backup `Nord Stage 4 Factory Restore RevK v1.66.ns4b`, `docs/research/arm-edit-params.csv`, [synth-knob-laws](synth-knob-laws.md), [keydown-zones](keydown-zones.md), [synth-adr-and-strip-law](synth-adr-and-strip-law.md).
- **Question:** How does a synth note's velocity change the level and the filter in the firmware, per program and per layer, and where do better note velocities matter?

Tags follow `.agents/rules/provenance-in-code.md`. Bean `ns4-9l1f`, decision D104.

## 1. Result

| Item | Result | Tag |
|---|---|---|
| Velocity parameters of the Synth section | Three edit ids only: `Edt_Syn_kAmpEnvVel` (`0x131`, 0..3), `Edt_Syn_kFltEnvVel` (`0x13c`, 0..1), `Edt_Syn_kOscEnvVel` (`0x124`, 0..1), per layer. No Synth Touch curve exists (Touch is `Edt_Pno_kKbdTouch` only, [piano-touch-curves](piano-touch-curves.md)). | VERIFIED (registry) |
| What the ARM sends | One key word per synth key, byte 1 = `0x80 + g(vel)`; `g` is the saturating S-curve of the curve-6 velocity table (`synth/key_velocity.rs`, 127 entries, equal to the ARM run). The velocity setting never changes `g`: the setting travels in the strip words | VERIFIED (run) |
| Amp velocity gain `V` | `DM(0xC10B4)` of slot 0 in the ORG core 2 pass: `bits(V) = bits(a) + ((bits(a + b) - bits(a)) * G >> 7)`, `a` = strip 0 word 53, `b` = strip 6 word 53 (`synth-knob-laws.csv`), `G = g`, `G = 128` for `g = 127`. A geometric blend from `-6 s` dB at `g = 0` to `+2 s` dB at the top, `s` = setting; setting 0 is exactly 1.0. 512 of 512 words within 1 float ulp of the oracle | VERIFIED (run) |
| Filter-envelope factor `F` | With `kFltEnvVel` on (strip 0 word 19 = 0.0, strip 6 word 19 = 1.0) the slot word `DM(0xC2723)` = `G / 128` (peak of the filter envelope scales with it, `DM(0xC117C)` follows); off it is 1.0. 256 of 256 words within 1 ulp | VERIFIED (run) |
| Oscillator-envelope velocity | `kOscEnvVel` is carried in strip word 0 (`((v & 0x7F) << 24) | stage`), which the harness holds constant; the law is not isolated | open |
| That `V` multiplies the voice amplitude and `F` the filter-envelope depth | The slot words are read by the envelope loop (`pc 0x1271C3`, `0x127CE7` read `DM(0xC10B6) = 0.0017569 V`); the final multiply is not isolated | ASSUMED |
| File bits = edit value | The layer's `amp_velocity` (2b at `+216`), `filter_velocity` and `osc_env_velocity` equal the edit value by analogy with the Touch field (VERIFIED there, [piano-touch-curves](piano-touch-curves.md) s4); the Synth fields were not recalled one by one | ASSUMED |

## 2. The amp law

`g` is the key byte of the velocity (`0..=127`), `s` the setting. In dB: `V(g = 0) = -6 s`, `V(g = 127) = +2 s`:

| Setting | `V(g = 0)` | `V(g = 64)` | `V(g = 100)` | `V(g = 127)` | strip 0 word 53 (`a`) | strip 6 word 53 (`b`) |
|---|---|---|---|---|---|---|
| 0 | 1.0 | 1.0 | 1.0 | 1.0 | `0x3F800000` | `0` |
| 1 | 0.501 (-6.0 dB) | 0.815 (-1.8) | 0.992 (-0.1) | 1.259 (+2.0) | `0x3F004DCE` | `0x3F41FB22` |
| 2 | 0.251 (-12.0) | 0.647 (-3.8) | 1.020 (+0.2) | 1.585 (+4.0) | `0x3E809BCC` | `0x3FAAB6D5` |
| 3 | 0.126 (-18.0) | 0.501 (-6.0) | 1.123 (+1.0) | 1.995 (+6.0) | `0x3E00E9F9` | `0x3FEF4782` |

The curve is piecewise linear with the slope doubling at every power of two (bit interpolation): the series is `docs/research/data/synth-velocity-law.csv` (768 rows, amp and filter, `tools/synth_velocity_law.py`). `g` already contains the S-curve, so the dB figures above are per key byte, not per MIDI velocity: at MIDI velocity 40 / 64 / 100 the key bytes are 12 / 32 / 98 (`KEY_VELOCITY_G`).

Against the engine's earlier law (`amp_vel = (1 - s/3) + (s/3) g / 127`, strip chain default): setting 3 is 15.8 dB louder at velocity 100 than at 40 (was 18.2), settings 1 and 2 are steeper (4.9 and 10.0 dB, were 2.4 and 6.6), and the whole curve sits higher: at `g = 64` setting 3 is -6.0 dB (was -12.0 dB).

## 3. Method

1. `tools/synth_adr_oracle.py` framework: `sharc-org` runs full ORG core 2 passes (`--dev 0x282c0380=1 --dev 0x3108d008=5`); `--msg 0:53=<word>` writes strip words, `--pre 0xC1DCC=<word>` sets the slot-0 controller word `0x01000000 | (0x80 + g) << 8`.
2. Diff of the data memory `0xC0000..0xC4000` after 40 passes between setting 0 and 3 and between `g = 100` and 10 found `DM(0xC10B4)` (and `DM(0xC10B6) = 0.0017569 x`), `DM(0xC272F)` and the controller word itself as the only movers. `kFltEnvVel` on moves `DM(0xC2723)`, `DM(0xC117C)` and the cutoff coefficient `DM(0xC2924)` (0.01014 to 0.01012).
3. `g` swept 0..=127 with one poke per 4 passes (settled in under 4): the series are the CSV. The bit-linear fit of `bits(V)` has a maximum error of 1 ulp, with the single step at `g = 127` doubled (`G = 128`).
4. Census: `cargo run --release -p nord-tools --bin synth_velocity_census -- <backup> --out docs/research/data/synth-program-velocity.csv` (`--check` re-reads).

Reproduce the series: `python tools/synth_velocity_law.py oracle` (local `sharc-org` and ORG image), check without firmware: `python tools/synth_velocity_law.py --check`.

## 4. Factory census (384 programs, 1152 layers)

Layers counted when their bit (4..6) is set in a scene mask of the program: 579 layers in 287 programs.

| Fact | Count |
|---|---|
| Layers with amp velocity 0 / 1 / 2 / 3 | 230 / 166 / 157 / 26 |
| Layers with filter envelope velocity on (and filter on) | 144 (139) |
| Layers with oscillator envelope velocity on | 42 |
| Programs with at least one velocity sensitive layer (amp or filter) | 213 of 287 |
| Programs with amp velocity on a layer | 189 |

Amp gain change from velocity 40 to 100 (`amp_fw_db_v40_v100` column): setting 1 +4.9 dB, setting 2 +10.0 dB, setting 3 +15.8 dB. Rows: [synth-program-velocity.csv](data/synth-program-velocity.csv), one per program and layer with the flags and the dB values; `sample = 1` marks a SAMPLES layer, `0` an oscillator layer.

### The three programs of the listening check

Whole-program RMS re velocity 100 (`program_sweep`, notes C3 G3 E4 C5, default engine options, `--synth-velocity` = firmware law; organ / piano layers of the program are included):

| Program (synth layer) | Velocity 20 | 40 | 70 | 127 | Reading |
|---|---|---|---|---|---|
| Horizon Whl, default | -1.43 | -1.31 | -0.87 | +0.43 | layers B and C amp 1 (+4.9 dB over 40 to 100 for the synth part), layer A amp 0; the organ layers of the program dilute the change |
| Horizon Whl, firmware | -2.65 | -2.43 | -1.62 | +1.50 | |
| Leaky Stabs II, default | -2.58 | -2.33 | -1.47 | +0.66 | layer A amp 1 (+4.9 dB over 40 to 100 for the synth part), B amp 0 |
| Leaky Stabs II, firmware | -5.22 | -4.64 | -2.79 | +2.08 | |
| Grand Pad, default | -13.39 | -10.27 | -5.51 | +3.12 | layer A amp 0, filter velocity on: the level change is the filter |
| Grand Pad, firmware | -13.96 | -10.75 | -5.64 | +3.12 | filter factor `F` 0.094 at 40, 0.766 at 100 |

So the velocity sensitive content of the three is: Leaky Stabs II and Horizon Whl through the amp (setting 1, +4.9 dB for the synth part), Grand Pad through the filter envelope (`F`). Better note velocities matter most for the 26 amp-3 layers, the 157 amp-2 layers and the filter-velocity pads.

## 5. Implications

- Opt-in only: `SynthLayer::firmware_velocity` / `--firmware-synth-velocity`. Off, the render is bit identical (`render_hash`, goldens unchanged). On, the amp gain is `V(setting, g)` and the filter-envelope depth follows `F`.
- Default stays off (D22, D104): the application of `V` and `F` inside the voice (ASSUMED), the setting mapping of the file bits (ASSUMED) and the oscillator-envelope velocity (open) are not verified end to end, and no clip varies the synth velocity.

## 6. Follow-ups

1. Verify the file bits against the edit value with the program recall (as in [piano-touch-curves](piano-touch-curves.md) s4) and isolate the multiply of `V` (read `DM(0xC10B6)` consumers `0x1271C3`, `0x127CE7`).
2. Oscillator-envelope velocity (`kOscEnvVel`, strip word 0 selector) and its effect on `Edt_Syn_kOscEnvAmt` depth.
3. A hardware clip of one synth program at three velocities to decide the default flip and re-baseline `render_hash`.
4. GUI Firmware bar switch for `--firmware-synth-velocity`.
