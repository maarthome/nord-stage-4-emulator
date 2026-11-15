# FX reverb type names: `kRvbType` value to UI name, and the type 6 block

- **Checked:** 2026-11-14
- **Sources (local, never committed):** `target/fw_unpacked/bank0_CODE.bin` (OS v1.66, ARM base `0xC0000000`), the `unicorn` harness (`tools/fx_clients.py`, `tools/arm_edit_capture.py`), the ARM boot log (`python tools/arm_bulk_tables.py capture ...`). Derived data committed: [data/fx-reverb-type6.csv](data/fx-reverb-type6.csv) (64 `(core, dm_word, word)` rows).
- **Question:** which UI reverb (Spring, Booth, Room, Stage, Hall, Cathedral, each with its variation) is `Edt_Effects_kRvbType` 0..11, and what does type 6 write (it had no ARM capture)?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. Short answer

| Question | Result | Tag |
|---|---|---|
| Name of each `kRvbType` value | The firmware value-name list sits at `0xC02CD2BC`: 12 consecutive pointers to `Spring`, `Spring2`, `Booth`, `Booth Predelay`, `Room`, `Room Predelay`, `Stage`, `Stage Chorale`, `Hall`, `Hall Chorale`, `Cathedral`, `Cathedral Chorale`, followed by the colour list `Normal`, `Dark`, `Bright` (`kRvbColor` 0..2). The list is indexed by the edit value: model = `type / 2` (Spring, Booth, Room, Stage, Hall, Cathedral), variation flag = `type & 1` | VERIFIED (`python tools/fx_reverb_names.py target/fw_unpacked/bank0_CODE.bin`; the same order as the program decode of [effects-and-routing](../specs/effects-and-routing.md) s3 and `ns4decode`, 17,280 of 17,280 fields in [hardware-audio-reference-fx](hardware-audio-reference-fx.md)) |
| Is the stored program value the same number as `kRvbType`? | The edit-id range is 0..11 with the list above as its display names; a program value `v` (bits 405..408) therefore selects the name `v`. No remapping code was found between the two (the harness applies the value cell directly and the handler reads it). | VERIFIED for the list; EMPIRICAL for "no remap" (no program was loaded through the ARM file path) |
| Old engine mapping (size order of the type words) | Booth 2/3, Room 4/5, Stage 6/7, Hall 8/9, Cathedral 10/11: identical to the name list, so `LayerFxChain::fw_multi_type` needs no change; only its tag moves from ASSUMED to VERIFIED | VERIFIED |
| Type 6 (Stage, no variation) | Not "sends nothing": the settled state already holds type 6 (the edit default is 6, `arm-edit-params.csv`), and the earlier capture dropped words equal to the baseline. Applying 6 to the synth unit L6 writes the same **64 words** as the other types (DM `0x94384 ..`, core 2), identical from a settled start and after a switch from type 3 | VERIFIED (run, [data/fx-reverb-type6.csv](data/fx-reverb-type6.csv)) |
| Type 6 against type 7 | 47 of 64 words differ (same length word `0x116` = 278 at index 9); type 6 is the unchanged initial block (`0x80000000`, `0x7FFFFFFE`, `0xFFFFFFFE` ...), so it is the pair partner of 7 (Stage, Stage Chorale) with no variation | VERIFIED (words), EMPIRICAL (role) |
| Organ rows (units whose instance words are zero in the image) | No pointer or length word reaches the DSP for them in the boot log (`boot_log.jsonl` has no write to `0xB0E94 .. 0xB0F1B`, the line pointer and length words `c[-24 .. -13]` of the instance `0xB0F28`; the 80 writes up to `0xB0FE5` are coefficients); the line bases stay unknown | open |

## 2. Reproduce

```bash
python tools/fx_reverb_names.py target/fw_unpacked/bank0_CODE.bin
# table at 0xc02cd2bc ; 0 Spring ... 11 Cathedral Chorale ; 12 Normal 13 Dark 14 Bright
```

Type 6 words: `tools/fx_clients.py` unit L6, `Edt_Effects_kRvbType = 6` through `arm_edit_capture.apply_value` without the baseline noise filter (the filter in `capture_slot` removes every word the boot already sent, which is all of them for the default type).

## 3. What is not done

| Item | Why |
|---|---|
| Type 6 in the Rust port | `ReverbMulti::new(6)` needs the 64 words in `fw_rvbm_words.rs` and an oracle golden (`tools/fx_rvbmulti_check.py`, `fx-rvbmulti-golden.csv`), which the generator reads from `fx-type-words.csv`: add the type 6 rows there, run the generator and the check for type 6 (about 10 min), then widen `FwReverbMulti::TYPES`. Stage without variation stays on the engine FDN until then |
| Hardware check of the names | the strings are the firmware's own display text; a unit photo of the popup would confirm the value to name link, no clip needed |
| Organ rows | see the table above |

## Implications

- The UI to `kRvbType` map is no longer an assumption: the opt-in `FirmwareFx::reverb` maps Booth, Room, Stage, Hall, Cathedral onto the right pairs.
- Type 6 is capturable; the remaining work is mechanical.

## Follow-ups

1. Add type 6 to `fx-type-words.csv`, regenerate `fw_rvbm_words.rs`, run the oracle check and extend the golden test.
2. Find how the organ part's reverb receives its line bases (activation path, [arm-part-activation](arm-part-activation.md)).
