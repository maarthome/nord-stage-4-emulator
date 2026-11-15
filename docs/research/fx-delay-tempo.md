# FX delay: tempo sync (Master Clock), the time unit, and what is still open

- **Checked:** 2026-11-14
- **Sources (local, never committed):** `target/fw_unpacked/bank0_CODE.bin` (OS v1.66), the `unicorn` harness settled snapshot (`tools/fx_clients.py`, `tools/arm_edit_capture.py`), `capstone` listings of `0xC010C514`, `0xC010D3C4`, `0xC0024CC8`, `0xC01FF70C`, the boot log of `tools/arm_bulk_tables.py capture`, the FX core 2 flat SRAM (`tools/ns4_dsp_ldr.py --sram`, listing `tools/adi_dis.py 0x24156A 0x241A2A` = `0x120AB5..0x120D15`). Derived data only (two small tables) in `crates/nord-core/src/fx/fw_delay_sync.rs`.
- **Question:** how does `kDlyClkEnable` (tempo sync to the Master Clock) change the delay time word `c[0x15]`, and what are the unit and the rest of the delay's ARM-supplied words?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. Short answer

| Question | Result | Tag |
|---|---|---|
| Unit of the table index `k` of [fx-delay-time](fx-delay-time.md) | **milliseconds**: `c[0x15] = min(61438, floor(K ms))` with `K = F / 1000`, `F = 43,402.34375 Hz` the DSP frame rate ([dsp-frame-rate](dsp-frame-rate.md)). The delay unit method `0xC01FF70C` receives `ms * 5555.4993` (`= 128 K`; listener argument `8333249` for `k = 1500`) and shifts it right by 7. Hence the delay is **20 ms to 1.416 s** (not 18 ms to 1.28 s: fx-delay-time assumed 48 kHz) | VERIFIED (hook on `0xC01FF70C`; 262 of 264 captured words equal `floor(F/1000 k)`, 2 are one count lower: the `128 K` scale is `5555.4993`, 2e-7 below `5555.5`) |
| `kDlyClkEnable` encoding | the handler `0xC010D3C4` tests `value == 1` after the dispatcher's Q16 shift: the cell must hold `1 << 16` (a raw `1` leaves it off; a first harness run with raw `1` saw no sync). It stores the flag at `[delay object + 0x88]`, `[clock + 0x148]`, and when the state changes re-sends the time through `0xC01FF70C` | VERIFIED (run) |
| Sync branch of the rate handler | `0xC010C514` branches on `[obj + 0x88]` to `0xC010C828`: `n = DIVISION[v >> 7]` (table `0xC02B8B40 + 0xCC0`, 128 words, values 0..16), `ratio = RATIO[n]` (`+ 0xEC0`, 17 `f32`), `ms = ratio * 4.0 * 60000.0 / bpm` in `f64`, then `d2f` / `f2d`, stored as a double at `[clock + 0x140]`; `bpm` is `[clock + 4]` (120 at boot, Master Clock rate) | VERIFIED (disassembly; doubles captured at 120 bpm: 1000.0, `0x4084D55560000000`, 250, 125, ...) |
| Ratios | `1/2, 3/8, 1/3, 0.29, 1/4, 3/16, 1/6, 0.145, 1/8, 3/32, 1/12, 0.0725, 1/16, 1/24, 1/32, 1/48, 1/64` of a whole note = beats `2, 1.5, 4/3, 1.16, 1, 0.75, 2/3, 0.58, 0.5, ...`. The three swing entries are `0.29`, `0.145`, `0.0725` whole notes (1.16 beats), not the engine's `8/7` placeholder (`tables.rs`); all other entries equal the engine table, and the engine's step map `idx * 17 / 128` equals the firmware `DIVISION` table for all 128 steps | VERIFIED (unit tests) |
| Sync time word | the unit method fed with `ms` gives `c[0x15] = floor(K ms)` in sync mode and in free mode alike (8 times, unit L3: 1000 ms 43402, 500 ms 21701, 250 ms 10850, 100 ms 4340, 1415 ms 61414, 1500 and 2000 ms clamped to 61438) | VERIFIED (direct calls) |
| Hand-off of the sync time to the unit by the clock object `0xC0024CC8` | in the harness the clock object does not notify the unit after a rate write in sync mode (it is rate limited by a time stamp and by `[state + 0x5c]`; forcing the flag did not make it call `0xC01FF70C`); the argument is therefore ASSUMED to be the same `ms * 5555.5` | ASSUMED |
| Tap tempo / MIDI clock | not decoded: the Master Clock tempo ids are `Edt_Com_kMasterClockRate` (30..240), `Edt_Com_kMasterClockTapState` (0..4), `Edt_Effects_kDlyRateTap_DNU`, `kDlyTap_Signal`; they feed `[clock + 4]` (bpm) through a path not traced | open |

## 2. Rust

`fx/fw_delay_sync.rs` (`division`, `beats`, `sync_ms`, `sync_word`, `sync_echo_samples_at`; tests: 128-step table equals the engine map, 17 ratios, captured doubles bit exact, 8 unit-method words) and the corrected unit in `fx/fw_delay_time.rs` (`word_from_ms`, `FW_SAMPLE_RATE = 43,402.34 Hz`). `FirmwareFx::delay_time` (opt-in, D22) now also handles `delay_clock_sync`: the sync time comes from `delay_rate_raw` and the Master Clock bpm. Default unchanged; the engine's sync path keeps running when the switch is off or `delay_rate_raw` is missing.

## 3. Not achieved in this pass

| Item | State |
|---|---|
| Feedback-effect types (`kDlyFbFxType`), their instance words, the activation path | not modelled; boot log words for the delay row are the same offset words as before (`c[0x10..0x13]`, block count) and the image values `c[0x15] = 32000.5`, ring pointers; no activation path found |
| Ring variant `0x120AB5` end to end, feedback loop period | listing dumped (288 instructions), not translated to the model pipeline (`tools/fx_listing_gen.py` `ROUTINES` has no delay entry); first-echo law only (fx-delay-time s3) |
| Tap tempo, MIDI clock into `[clock + 4]` | not decoded |
| Mod 1 PUMP / A-PAN Master Clock counters | not touched |

## Implications

- The engine's delay range and the old 1.28 s statement are off by the factor `48000 / 43402`; the opt-in firmware time is now in firmware frames and scaled to the engine rate.
- Tempo sync in the firmware is a 17-entry division ladder with three swing entries; the engine's ladder matches except for those entries.

## Follow-ups

1. Make the clock object notify the unit in sync mode (advance the harness time base) to turn the ASSUMED hand-off into VERIFIED, and trace tap tempo into `[clock + 4]`.
2. Translate `0x120AB5` (feedback loop, mode word) with the `fx_listing_gen.py` pipeline and capture the activation of the delay row for the feedback-effect types.
