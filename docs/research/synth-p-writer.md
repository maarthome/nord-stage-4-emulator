# The ARM writes the synth oscillator P block (found, run, tabulated)

- **Checked:** 2026-11-14
- **Sources (local, never committed):** OS v1.66 `bank0_CODE.bin` / `bank0_SRAM.bin` in the ARM harness (settled snapshot), the boot capture of `tools/arm_bulk_tables.py` (`org_core2.script`, `boot_log.jsonl`), `capstone` listings of `0xC01B1E6C..0xC01CB100`, ORG core 2 image (`ns4_dsp_ldr.py --sram`), new `tools/synth_p_writer.py` (`tools/tests/test_synth_p_writer.py`). Follows [synth-wave-models](synth-wave-models.md) s9 / s10, [synth-knob-laws](synth-knob-laws.md) s5, [arm-bulk-tables](arm-bulk-tables.md).
- **Question:** Who writes the oscillator program words (ratios, weights, gain, unison ratios) that every stage reads from its frame, and what are they per wave?

Tags follow `.agents/rules/provenance-in-code.md`. Only derived numbers are committed ([data/synth-p-words.csv](data/synth-p-words.csv)).

## 1. Short answer

| Question | Result | Tag |
|---|---|---|
| Who writes the P block? | **The ARM**, not core 1 and not DMA. The strip apply function `0xC01CE7D8(strip)` (the routine that [synth-wave-stages](synth-wave-stages.md) s13 found only reachable for the state marker) calls the table driven writers `0xC01C72CC`, `0xC01C7550` and `0xC01C94CC`, which call 44 one-word senders (`0xC01B1E6C .. 0xC01C044C`, prologue `cmp r0, #5`, signature `(slot 0..5, float)`, six slot objects at `0xC0549178 + 0x30 slot` and the like). Each sender stores one word at `DM(0xC167A + 0x96 slot + k)` of ORG core 1, ORG core 2 and PNO core 2 | VERIFIED (run) |
| Why was it missed? | (1) the captures drove edit ids, and the wave ids only store the pending type at `[strip + 0x98]` (`0xC00171B0`); (2) the idle census of the P range saw only the **boot defaults** of the same senders (`1.0` ratios, `0.25` gains, 136 + 126 messages in the settle windows `settle 1`, `settle 3` of the boot log, all with the marker type); (3) the earlier note searched the DM windows of the pass, not the ARM message log | VERIFIED (boot log: 262 messages into `0xC1670..0xC1ABC` from 44 senders) |
| How to trigger it | `emu.write32(strip + 0x98, type)`; `0xC01CE7D8(strip)`: 390 to 430 messages, the strip word 0 stage id, the P words, for some types the MULTI record `0xC1596..98`. Variants: `[strip + 0x9C]` (`a`), `[strip + 0xA0]` (`b`) | VERIFIED (run) |
| Idle block against the loaded one | the image block (`P[-32] = 0.077`, ...) is overwritten: after the apply every type has `P[-32 .. -29]` = unison or detune ratios **around 1.0**, not 0.077; the ratio-needs-2 contradiction of [synth-wave-models](synth-wave-models.md) s9 disappears (r = 1 plays the note pitch) | VERIFIED (run) |
| Per-wave words | table below and [data/synth-p-words.csv](data/synth-p-words.csv) (74 types, slot 0, `a = b = 0` rows plus the unison variants) | VERIFIED (run) |
| Knob to word | the oscillator knobs reach these words only through the type index (category, wave -> type, [data/synth-wave-types.csv](data/synth-wave-types.csv)) and the two pending words `a`, `b` (unison, section 4); the oscillator **ratio**, weights and gain are **not** knob values: they are table rows of the firmware | VERIFIED (run) |

## 2. The writer chain (listing facts)

| Address | Role |
|---|---|
| `0xC01CE7D8(strip)` | apply: copies `[strip + 0x98 .. 0xC0)` (type, `a`, `b`, ...) to `[strip + 0xC8 ..)`, calls `0xC01C6EE0`, `0xC01C72CC(strip, type)`, ..., `0xC01C94CC(strip, type, [strip + 0xA0])` (call at `0xC01CEA78`), `0xC01C754C(strip, type, [strip + 0x9C])` (call at `0xC01CEA88`), `0xC01CB0DC`, and sends the strip words (`0xC01CC85C`: word 0 = `0x87 << 24 \| stage`, the idle `0x06` byte is the pending marker) |
| `0xC01C7550` | switch on the type; rows of four float pointers at `0xC02F9968 + 0x200 + 4 (variant + 5 j)` (`bank0_CODE` rodata) feed the senders of `P[-32 .. -29]`, `P[-16 .. -9]`, ... per oscillator group |
| `0xC01C94CC` | the SUPER family: float immediates `0x3F7F42FF`, `0x3F805EC6`, `0x3F811D26` (1.0087), `0x3F7DCAA0` (0.9918), `0x3F7C546C`, `0x3F81DC8E`, ... and the table `0xC02F9E88` rows (`+0x50 .. +0x5C`, `+0xA4`, `+0xB8`, `+0xCC`) |
| `0xC01B1E6C .. 0xC01C044C` | the 44 word senders, one per P word of a slot (stride `0x17C` bytes): `(slot, float)`; each takes the comm object of its word from the table at `0xC0549178 ..` and stores `(system address, value)` through `0xC0062E10` |

The senders are generated code of one parameter each; the rodata rows are firmware numbers, only the words they produce are committed.

## 3. Words per wave (slot 0, `a = b = 0`, VERIFIED run)

`P[16]` = gain (lane x = lane y), `P[-32 + lane]` / `P[-30 + lane]` = oscillator 1 / 2 ratio, weights `P[-14 + i]`, `P[-16 + i]`, `P[-12 + i]`, `P[-10 + i]` (`i` = lane; the lane mix of [synth-wave-models](synth-wave-models.md) s2: lane x output = `P[16] (o1 (P[-16] + P[-13]) + o2 (P[-12] + P[-9]))`).

| Types (stage) | Waves | gain `P[16]` | ratio 1 / ratio 2 (lane x) | weights osc 1 / osc 2 | Remarks |
|---|---|---|---|---|---|
| 0, 1, 2 (sine, triangle, saw) | PURE 0..2 | 0.25 | 1 / 1 | 1 / 0 | one oscillator |
| 3, 4, 5 (`0x121B99`) | PURE square / pulse 33 / pulse 10 | 0.175 / 0.2 / 0.25 | 1 / 1 | 1 / 0 | **the pulse levels differ by wave**: square 0.70 of the saw (fitted 0.68 from the pitch-blip clip) |
| 6, 14, 15, 40, 45 .. 48, 55 .. 73 | noise, misc, sample / WAVE types | 0.25 | 1 / 1 | 1 / (not written) | 30 words only, the stage does not read the second group |
| 7, 8, 9, 10, 11, 12 | SHAPE, SUB OSC | 0.25 | 1 / 1 | 1 / 0 | same as the saw |
| 16 .. 21 (`0x121C54`) | MULTI 0..5 | 0.1768 (waves 2, 5: 0.1443) | 1 / 1.0087 | 1 (0.9) / 0.85 | plus the record `DM(0xC1596 .. 98)` (section 5); lane y ratios 1 / 0.99137 |
| 22 (`0x121D6D`) | SUPER 0 | 0.08839 | sixteen unison ratios | all 0.70711 | 8 voices per lane |
| 23 (`0x121E9D`) | SUPER 1 | 0.06614 | same sixteen | 0.70711 | |
| 24, 25, 26 (`0x124985`) | SUPER 2..4 | 0.125 | same sixteen | 0.70711 | four voices read `P[-32 + 2 j + lane]`, `j = 0 .. 3` |
| 27 .. 39, 41 .. 44, 49 .. 54 | SYNC, SHAPE SINE, others | 0.25 (types 30, 31: 0.175, 0.2) | 1 / 1 | 1 / 0 | |
| 57 (state marker `0x39`) | | 0.35 | 1 / 1 | | the marker block |

SUPER unison ratios `P[-32 .. -17]` (lane x, lane y alternate): `0.99712 1.00289 | 1.0087 0.99137 | 0.98566 1.01454 | 1.02042 0.97999 | 0.97434 1.02633 | 1.03228 0.96873 | 0.96315 1.03826 | 1.04427 0.9576`. The pairs `(1 - d, 1 + d)` are roughly `d = 0.0029, 0.0087, 0.0143, 0.0200, 0.0257, 0.0313, 0.0370, 0.0427`: eight voices spaced by about 0.0057 in ratio (about 10 cents). The ASSUMED voice ratios of the Rust stages (`SUPER_RATIOS` -1 .. 1 for SUPER 0, `SUPER24_RATIOS` 0.97 / 0.99 / 1.01 / 1.03) are replaced by these words in the spread law `inc_j = (rho_j u + 1 - u) f`, `u = 0.35 (t + 2 t^4)`, `t = s5 + 0.06`: the detune at full spread is `(rho_j - 1) u`, so the knob scale `SUPER_SPREAD_MAX` (ASSUMED 0.1, with rho = +-1) must become about 1.0 (open, needs one clip).

## 4. The two pending words (unison variants)

`[strip + 0x9C]` (`a`) and `[strip + 0xA0]` (`b`), values 0 .. 4, recorded as deltas against `a = b = 0` in the CSV. For every type except the SUPER family (type 2 shown):

| Variant | Words that change |
|---|---|
| `b = 1, 2, 3` | ratio pair lane x / y of oscillator 1 `P[-32], P[-31]` = `1.001156 / 0.998845`, `1.002313 / 0.997692`, `1.003472 / 0.996540` (`2^(+-2k / 1200)`, 2 / 4 / 6 cents) and, for `b = 2, 3`, oscillator 2 `P[-30], P[-29]` = `0.999236 / 1.000770`, `0.998845 / 1.001156`; the same pairs at `P[18 .. 19]` and `P[68 .. 69]` (the other two 50-word sub-blocks) |
| `b = 4` | `P[-31] = 2.0` (lane y one octave up), also `P[19]`, `P[69]` |
| `a = 1, 2, 3` | the lane weights become a rotation: `P[-16], P[-15] = 0.923880` and `P[-14], P[-13] = 0.382683` (`cos`, `sin` of pi / 8) and, for `a >= 2`, `P[-12], P[-11] = 0.923880` |
| `a = 4` | `P[-15] = 1`, `P[-14] = 0`: the lane y output takes oscillator 1 of lane y |

For the SUPER types the variants `a = 1 .. 4` change only the weight groups: `0.816088 / 0.577062` (a = 1 at `P[-12 .. -9]`, a = 2 at `P[-16 .. -13]`), `0.912688 / 0.407431` (a = 3), `1 / 0` (a = 4). So `a` is a mixing law between the two output lanes (stereo spread of the unison, listed per type) and `b` a detune pair ladder. The assignment of `a`, `b` to the panel ids (`kOscUnison` 0 .. 3, the Unison dropdown) is **not** verified here: the pending words are written by the unison path of a loaded program, which the harness does not run (section 6).

## 5. Per-type records outside the P block

The MULTI types 16 .. 21 additionally send the voice multiplier record `DM(0xC1596 .. 0xC1598)` (CSV indices `-260 .. -258`): `1, 1, 0.1` / `1, 2, 0.1` / `2, 4, 1` / `1, 1.5, 0.1` / `1, 3, 0.1` / `2, 6, 1` for MULTI waves 0 .. 5 (VERIFIED run). The idle image holds `1, 2, 0.5` (`MULTI_OCTAVES` of the Rust stage, tagged VERIFIED for the image words): it is the default, **not** what a MULTI wave plays. Waves 2 and 5 (octave stacks `x2 x4`, `x2 x6`) also have the smaller weights 0.9 and the third multiplier 1.0 instead of the weight 0.1.

## 6. What the knob captures could not see

The edit ids `kOscBank / Cat / Wave` store the pending type (`[strip + 0x98]`) and mark the strip; the apply function runs from the main loop when the strip is marked. In the harness the marker state never completes, so the old captures saw only the boot defaults. `tools/synth_p_writer.py` calls the apply function directly, which is the effect of a loaded program. The remaining edit ids of the task list (`kOscDetune`, `kOscSub`, `kOscSync`) do not exist in OS v1.66 (`arm-edit-params.csv` has `kOscUnison`, `kOscUnison_obsolete`, `kOscEnvAmt`, tune); the oscillator knobs that exist are Bank, Cat, Wave, Ctrl, Tune, Unison and the env ids ([synth-knob-laws](synth-knob-laws.md)): Ctrl writes common word 6 only (the shape word), tune writes common word 0.

## 7. Consequence for the engine

| Item | Before | Now |
|---|---|---|
| Pure Saw gain | EMPIRICAL 0.4 | one unit of `P[16]` = 1.6 engine gain (EMPIRICAL level constant, the saw calibrates it); everything else follows from the words |
| Pure Pulse gain | EMPIRICAL 0.27 | `1.6 x 0.175 = 0.28` from the words (square), 0.32 (pulse 33), 0.40 (pulse 10) |
| SHAPE, SHAPE SINE gain | EMPIRICAL 0.6 / 0.5 | `0.4` from the words (the fits were confounded with the cutoff factor) |
| MULTI, SUPER rows | EMPIRICAL, ASSUMED voice ratios | words known (section 3, 5); the Rust stage normalisations still differ, rows stay EMPIRICAL |
| Cutoff factors | EMPIRICAL | not P words, unchanged |

Opt-in `--synth-p verified` (`synth/p_words.rs`) builds the gain column of the rows a single oscillator describes from the words; scores in [synth-validation](synth-validation.md) s10.

## Implications

- The oscillator program is a firmware table indexed by wave type; no knob changes ratios or weights, and neither the DSP side nor DMA writes the block.
- The idle block of the image is a default that every loaded program replaces; models built on `P[-32] = 0.077` were built on the wrong block.
- The gain levels of Pure Pulse (and the SHAPE family) now have a source; the cutoff factors do not.

## Follow-ups

1. Replace the ASSUMED voice ratios, weights and spread scale of the SUPER stages and the MULTI records by the words of section 3 and 5 (stage-level unit conversion, one SUPER clip for `SUPER_SPREAD_MAX`).
2. Trace `a`, `b` (`[strip + 0x9C]`, `[strip + 0xA0]`) to `kOscUnison` and `kOscCtrl` in a loaded-program capture.
3. Lanes y words and the other slots (`slot 1 .. 5`, other layers) are the same senders with `slot` as the first argument; checked for slot 0 only.

## Reproduce

```bash
uv run --with unicorn --with capstone python tools/synth_p_writer.py capture target/fw_unpacked/bank0_CODE.bin \
    target/fw_unpacked/bank0_SRAM.bin --cache target/settled.snap --variants 5 --shard 0/6 --out part0.csv   # shards 0..5, then concatenate
python tools/synth_p_writer.py --help
uv run --with unicorn python tools/arm_bulk_tables.py capture target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin --out target/bulk   # the boot defaults
```
