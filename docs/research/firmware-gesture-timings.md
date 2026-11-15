# Research: firmware key gesture timings (hold, repeat, double press) and pot behaviour

Checked: 2026-11-14.
Sources: `bank0_CODE.bin` and `bank0_SRAM.bin` (host ARM image v1.66, local only under `target/fw_unpacked/`, never committed), `capstone` disassembly, `unicorn` runs of `tools/arm_screens.py` states (booted key object read back), `tools/arm_xref.py`, `tools/arm_key_timing.py`.
Question: which hold, auto-repeat, double-press and debounce thresholds does the firmware's key pipeline use (OQ13), what does the "Program buttons only act after 40 to 60 ticks" note of [arm-oled-render](arm-oled-render.md) s7 mean, and does the firmware show pot pick-up ("catch") behaviour?

Reproduce: `python tools/arm_key_timing.py table target/fw_unpacked/bank0_CODE.bin` (prints the table of section 2 and the defaults of section 1).

## 1. The key pipeline (VERIFIED by disassembly, live arrays read after the boot)

| Stage | Address | What it does |
|---|---|---|
| Scan | SRAM `0x800014E8` (ROM twin `0xC024DA6C`), hook slot 2 | one column per call, posts `(column, word)` to the key queue `0x80001D20` -> `0xC0083138` when the stored word changed ([arm-oled-render](arm-oled-render.md) s7) |
| Edge handler | `0xC0083138(this, column, &new, &old)` | per changed bit, key id `k = column * 32 + bit` (port `0x10` bits at 16..23). Stored bit set: calls listener slot 1 (`0xC007B45C`, key up) and clears the key's countdown `[this + 0x20][k]`. Stored bit clear (key down, section 4): starts the debounce if `[this + 0x2C][k]` is non-zero, else calls `0xC007B204` at once |
| Key down | `0xC007B204(this, k, tick64)` | calls listener slot 0 (`0xC007B668`, key down), loads the countdown `[this + 0x20][k] = [this + 0x24][k]`, zeroes the repeat count `[this + 0x1C][k]`; if the same key was pressed before within `[this + 0x38]` tick64 units it calls listener slot 2 (`0xC007B3C8`, double press, event kind 4) |
| Poll | `0xC0082EC8(this)`, veneer `0x80001D50`, hook slot 7 (`0x80000AF4`) | skips columns whose stored word is `0xFFFFFFFF`; per key: decrement the debounce countdown, else decrement the hold countdown; at 0 increment the repeat count; count 1 posts the held event (kind 3, inlined `0xC00830B8..0xC00830C4` when the listener slot 4 `0xC007B2A0` is the default), then if `[this + 0x28][k] != 0` the repeat event (kind 2, inlined `0xC0082FBC..0xC0082FC8`) and reloads `[this + 0x20][k] = [this + 0x28][k]` |
| Event | listener slots post `{tag 0xC053F9D0, modifier word, key id, kind}` on the UI ring ([arm_events](../../tools/arm_events.py)); kinds 3 / 2 / 4 = held / repeat / double press from the `mov r2, #n` before each `str r2, [r0, #0xC]` |

The key object (instance `0x900434C0`, 7 columns x 32 keys, constructor `0xC0083278`, config from `0xC0083410`):

| Field | Value | Tag |
|---|---|---|
| default hold / first delay | 300 ms (`mov r2, #0x12C`) | VERIFIED |
| default repeat interval | 80 ms (`mov r1, #0x50`) | VERIFIED |
| double-press word | 300, stored as `300 * 24 = 7200` at `[this + 0x38]` (`0xC0083314`) | VERIFIED (value), unit unresolved (section 4) |
| ms to poll steps | setter `0xC0083248(this, k, delay_ms, repeat_ms)` stores `ms / 6` (`umull 0xAAAAAAAB`, `lsr 2`): 500 -> 83, 1000 -> 166, 350 -> 58, 300 -> 50, 80 -> 13 | VERIFIED (exact vs the live arrays of 224 keys) |
| debounce `[this + 0x2C][k]` | 0 for all 224 keys; nothing writes it | VERIFIED (live read, `0xC0083248` is the only setter of the arrays) |

## 2. The per-key table (VERIFIED)

The init routine `0xC024C560..0xC024CA00` calls the setter 49 times. Keys not in the table keep the 300 ms hold and the 80 ms repeat.

| Hold | Keys | Notes |
|---|---|---|
| 1000 ms, no repeat | 48, 49, 81 | the three MORPH ASSIGN buttons (A.TOUCH, PEDAL, WHEEL): `(0x02, 0, 0)`, `(0x02, 0, 1)` and `(0x04, 0, 1)` in `arm_keys` notation |
| 500 ms, no repeat | 35 keys, among them 16 (MASTER CLOCK), 17 (TRANSPOSE) | the manual's "roughly half a second" |
| 350 ms, no repeat | 83, 50, 51, 18, 160, 161, 70 | controls not yet named |
| 350 ms, then every 80 ms | 146, 147, 114, 82 | the four Program soft buttons in `arm_keys` notation `(strobe, port, bit)`: 146 = `(0x10, 0, 2)` Program 1, 147 = `(0x10, 0, 3)` Program 2, 114 = `(0x08, 0, 2)` Program 3, 82 = `(0x04, 0, 2)` Program 4 |
| 300 ms, then every 80 ms (defaults) | every other key, among them Shift (36), Page (118), Store (145), Undo (21) | not in the table |

The full row list is `crates/nord-panel/src/behaviour/firmware.rs` `KEY_TIMINGS` (decoded by `arm_key_timing.py rust`); its tests pin the counts (35 / 11 / 3) and the live step values.

## 3. Answers

| Constant | Result |
|---|---|
| `HOLD_MS` 500 | VERIFIED as the firmware hold of 35 of 49 table keys; the Morph Assign buttons hold 1000 ms (`HOLD_MS_MORPH`, now used by `ButtonTracker::for_control`). Program soft buttons hold 350 ms and repeat every 80 ms; not modelled (the Rust panel has no repeat gesture) |
| "Program buttons only act after 40 to 60 ticks" | harness artefact, not a firmware threshold: the debounce word is 0 for every key, and the harness key polarity is inverted (section 4), so in the old sweeps the action ran on the release edge. The scan and poll are main-loop hooks that ran about once per 128 harness ticks |
| Auto-repeat | the four Program soft buttons: held plus first repeat at 350 ms (58 steps, measured), then every 80 ms (13 steps, measured at polls 71 and 84); unlisted keys the same with 300 ms (50 steps); all 45 other table keys have no repeat |
| Release | the up event (slot 1) is immediate: no release threshold, no debounce |
| Shift chord | not a timing: Shift is key 36 (default 300 ms / 80 ms); the modifier word `0xC05846AC` is set on its release and read by the encoder events ([arm-oled-render](arm-oled-render.md) s17.3) |
| Real time of a poll step | `ASSUMED` 6 ms (the divisor implies it). The hook runs after a burst of messages, not from the 1 kHz timer ISR; real cadence needs a hardware clip or the message source of the main loop |
| `DOUBLE_TAP_MS` 300 | stays `ASSUMED`. The firmware double-press word is 300 and is compared as `300 * 24` against the 64-bit tick (`0xC007B204`); with the ISR increment of 1 per 1 ms that would be 7.2 s, so the unit is unresolved and the consumer of event kind 4 was not found |

## 4. Polarity of the harness key matrix (finding)

The harness `arm_keys.KeyMatrix` answers idle `0xFF` and clears the bits of held keys (pressed reads 0; ASSUMED in [arm-oled-render](arm-oled-render.md) s7). The key object says the opposite:

- With the harness polarity a press runs the **key-up** handler (slot 1, countdown cleared) and the release runs the **key-down** path (`0xC007B204`, countdown loaded), so no hold event can fire while the key is down: 12,000 ticks (96 polls) of a held Program 1 produced no held or repeat event.
- The poll skips every column whose stored word is `0xFFFFFFFF` (`0xC0082EF8`, `cmn r3, #1`): the idle stored word is all ones, i.e. the scan reads **0 when idle and 1 when pressed** (stored `~word` has the key bit clear while it is down).
- Run with the polarity inverted (idle 0, held bit 1; script in the reproduce list below) the sequence is coherent: press at poll 0 = key-down event; the held event (kind 3) and the first repeat (kind 2) both at **poll 58**, further repeats at polls 71 and 84; release = key-up event.

Measured sequence for Program 1 (key 146), tick64 in parentheses (harness ticks, not real time):

| Event | Poll step after the press | Firmware value |
|---|---:|---|
| key down (slot 0) | 0 | immediate, no debounce |
| held (kind 3) + first repeat (kind 2) | 58 (7372) | 350 ms / 6 = 58 |
| repeat | 71 (9036) | +13 = 80 ms / 6 |
| repeat | 84 (10700) | +13 |
| key up (slot 1) | at the release edge (poll 86) | immediate |

Tags: the step counts are VERIFIED (run: hooks at `0xC00830C4` for kind 3 and `0xC0082FC8` for kind 2, inlined posts that never call the listener slots when the vtable entry is the default); the active-high polarity is VERIFIED by the code paths above, not by hardware. The harness itself is left unchanged (existing sweeps depend on it); a polarity fix is a follow-up.

Reproduce (local firmware): boot `tools/arm_screens.py boot ... --out state.bin`, restore it in a `Session`, replace `session.keys.port_byte` with a function that returns the held bits (idle 0), run 400 ticks, hold key `(0x10, 0, 2)`, hook `0xC0082EC8` (poll), `0xC007B668` (down), `0xC00830C4` and `0xC0082FC8` (inlined held / repeat) and count polls.

## 5. Pot behaviour: not found (what was tried)

- Strings of the image (`[ -~]{4,}`) for pick-up, catch, snap, jump, fader, ADC: none. Only `Live`, `Edt_Sys_kLiveMode`, `Edt_Sys_kLivePos`, `OrgDrawbarLive` exist.
- Peripherals touched by the host image (`arm_xref.py mmio`): Timer0, SPI0/1, UART1/2, I2C0, GPIO, EMIFA CS2 (panel scan window `0x60000010..0x60000025`) and CS3 (DSP words). There is no ADC: the matrix scan has 98 key bits plus 10 encoder bits and no analogue column, so the faders, drawbars and Synth knobs are not read by the host ARM through this window ([oled-strings-vs-firmware](oled-strings-vs-firmware.md): no UART1 or SPI0 stores in steady state).
- The pot path therefore lives outside the code we can run (a panel controller or the second image), and a "catch" rule cannot be read from the host firmware. `SNAP_TOLERANCE` (2 %) and the pick-up rule stay `ASSUMED`; live-mode and morph interaction also stay manual-based.

## Implications

- `HOLD_MS` is now backed by the firmware table; Morph Assign needs a 1 s hold (behaviour change in the panel, test `morph_buttons_need_a_one_second_hold`).
- Hold on Program soft buttons repeats; the panel could repeat list scrolling, not done.
- OQ13 narrows to: double-tap interval, pot pick-up, LED ladder segment counts (all hardware or panel-MCU facts).

## Follow-ups

- Find the message source that runs the hook rounds (real poll period) to turn steps into ms with evidence.
- Find the consumer of key event kind 4 (double press) and the unit of tick64 in `0xC007B204`.
- Map the remaining table keys (83, 50, 51, 18, 160, 161, 70, ...) to controls with `tools/arm_screens.py sweep` using polarity-correct holds.
