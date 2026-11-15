# The SY pitch table entry `e`: who fills `DM(0x9354A + index)` (negative result, 2026-11-15)

- **Checked:** 2026-11-15
- **Sources (local, never committed):** OS v1.66 `bank0_CODE.bin` / `bank0_SRAM.bin` in the ARM unicorn harness (`target/keydown.snap`), the five DSP images cut from `bank0_CODE@0xC044667C..0xC05263A8` (SY1SY2 listing `target/oracle/sy2.lst`, SMP image `dsp_image_4_SMP.bin` through `ns4_dsp_raw.py` and `adi_dis.py`), the boot log `target/bulk/boot_log.jsonl` ([arm-bulk-tables](arm-bulk-tables.md)), the sample set `Carina Accordion 1_PS 4.1.nsmp4` from the local backup, new `tools/arm_sy_slot_probe.py` (`tools/tests/test_arm_sy_slot_probe.py`). Follows [synth-sample-kernel-rule](synth-sample-kernel-rule.md) sections 12 to 14 (D61, D65) and [synth-p-writer](synth-p-writer.md) (the method: find the table driven writer, then run it).
- **Question:** Which ARM routine (or other agent) writes the 16-bit packed float entry `e` that the SY audio ISR step 3 reads at `DM(0x9354A + index)`, so that `--sample-pitch fw` can be rebuilt on the written values and flipped to the default (D22)?

Tags follow `.agents/rules/provenance-in-code.md`.

## 1. Short answer

| Question | Result | Tag |
|---|---|---|
| Is there an ARM writer of `e` in the harness? | **No.** Over sample load (`0xC01D710C`), bind, two ticks and a SAMPLES note-on (voice update `0xC01E00AC` with the queue pump and the DSP status bit, `play_note`, 19 SY messages) no ARM message addresses `0x93500..0x935A0`; the boot log (290 SY, 1,540 SMP messages) has none either | VERIFIED (run, `tools/arm_sy_slot_probe.py`) |
| Is the entry in the DSP image? | **No.** The SY image block stream sets `DM(0x93580) = 0x80008000` (block at `bank0_CODE@0xC0501670`) and zero fill for `0x93581..0x9358D`; `0x9354A..0x9357F` are zero (flat image `target/oracle/sy.bin`, 54 words read back) | VERIFIED (image dump) |
| Does any ARM code or table name the window? | **No.** The 32-bit literals `0x93540..0x93591` occur in `bank0_CODE` only inside the DSP image streams (`0xC0501670..0xC05017B8` SY, `0xC051E28C..0xC051E35C` SMP); no instruction or rodata pointer outside the DSP images carries one. The port tables `0xC0547F78`, `0xC05482F8` and `0xC0548D78` entry 13 (`0x92196`) are the only SY targets (`arm-chan-tables.csv`, target 6: 97 rows) | VERIFIED (byte scan, table dump) |
| Does the SMP DSP write it? | No evidence. The SMP image has its own records at `0x93542`, `0x9354A`, `0x93578`, `0x93580` (pointer pairs `0x9404E, 0xE3A00`, a different table), and its program listing (`adi_dis.py` over short words `0x1C0300 .. 0x1C2800`, 3,618 instructions, incl. the ISRs at `0x1C0EE0`, `0x1C03FD`, `0x1C0A71`, `0x1C058C`) holds no operand in `0x93500..0x935A0` | VERIFIED (listing scan); SMP to SY link not checked (no SMP ISR run) |
| Is a table of semitone entries stored in the ARM image? | **No.** A scan of `bank0_CODE` for runs of 16-bit values whose unpacked ratio (`unpack_ratio`) steps by `2^(1/12)` (halfword stride 1 and 2, up and down, 5 or more steps) finds only two 32-bit integer tables with a geometric ratio of 1.06 (84 and 78 entries at `0xC02B9EC8` and `0xC0441134`, values 521 .. 63348 and 900 .. 64355, purpose not identified, not packed floats) and short accidental runs; none is a 16-bit packed float table | VERIFIED (scan) |
| What is new about the index? | The index is **not** the DSP reset value `0x36`: the ARM writes pair block word 0 itself (section 2). It is a per-layer slot that the ARM copies from a two-word slot object; in the harness the object holds `0, -1` and nothing writes it during load, bind or note-on | VERIFIED (run, listing) |
| Verdict | **Writer not found.** The default of `--sample-pitch` stays off; no engine change, no re-baseline (D89) | open |

## 2. The index chain (new, listing and run)

| Step | Where | What | Tag |
|---|---|---|---|
| DSP reads | SY ISR step 3, `0x1C0692 .. 0x1C06AF` | `r13 = DM(i7 - 0x20)` (word 0 of the pair block, `i7 = 0x921BE` for pair 0); `r4 = fext r13 by 0x400` is the index, `r1 = fext r13 by 0x410` is the field spec; `m5 = r4` and, **in the other SIMD lane**, `m6 = s4`; then `r12 = DM(m5, i5)`, `r12 = fext r12 by r1`, `funpack`. So the pair block word 0 of the even address (`0x9219E`) feeds lane x and the odd one (`0x9219F`) feeds lane y; the earlier "28 pairs" reading of section 13 is the same loop, two voices per pass | VERIFIED (listing) |
| DSP default | voice reset `0x1C0794` | `DM(rec + 0x16 - 0x20) = 0x36 \| 0x4000000`: index 54, spec `0x0400` (low half); `DM(0x93580)` holds `0x80008000`, the entry unpacks to a ratio the step clip rejects. This is a parking value, not a played one | VERIFIED (listing, run) |
| ARM overwrite | `0xC01E2B08` (voice object types dispatched at `0xC01E2AE8`: `[obj + 0x24] \| 0x4000000`), `0xC01E2C6C` / `0xC01E2D08` (`[obj + 0x28] \| 0x4100000`), store at `0xC01E2CC8` after `bl 0xC006CA48` (FIFO lock) | the ARM sends word 0 of the pair block (address `[table + 4]` of `0xC05482F8`, `0xC01E2C90`) as slot index OR spec; in the harness the first send is `0x9219E <- 0xFFFFFFFF` (the slot fields are still `-1`) | VERIFIED (run, pc `0xC01E2CC8`) |
| Slot source | voice update `0xC01E09D8`, `0xC01E09F0`: `[obj + 0x24] = [sp + 0x7C]`, `[obj + 0x28] = [sp + 0x78]`, set at `0xC01E22E8 / 0xC01E22EC` from the object `0x9062F0F0` (`[+0]`, `[+4]`) of the handle table entry `[sp + 0x154]` | after the voice update `[obj + 0x24] = 0`, `[obj + 0x28] = -1`: slot 0 for lane x, none for lane y, i.e. `word0 = 0x04000000`, entry `DM(0x9354A)` low half | VERIFIED (run: memory watches at `0x90670254`, `0x90670258`, `0x9062F0F0`) |
| Slot allocator | none seen | the slot object holds `0, -1` from the settled snapshot and no store hits it during sample load, bind, two ticks or the note-on (only the copy into the voice object at `0xC01D66C0`, the voice reset) | VERIFIED (run, watch `0x9062F0F0..F7`) |

Consequence for section 14 of [synth-sample-kernel-rule](synth-sample-kernel-rule.md): "the voice index stays `0x36` in all 28 pairs" holds for the DSP-side reset only. On the hardware the ARM replaces it by the slot of the layer; for the first SAMPLES voice it is **0**, so the DSP reads `DM(0x9354A)` (lane x low half), which is zero in the image: the ratio is 0 and the step is 0 until something fills it.

## 3. What was ruled out

| Candidate | Evidence | Result |
|---|---|---|
| DSP store (SY) | no store into `0x9354A..0x9358F` in the listing (section 14 of the kernel-rule note); the new check of the stage A states and the ISR loop shows only stores through the pair block pointer (`dm(0xffffffe4, i3)` etc.) | none |
| ARM opcode 0 message | none of the 19 note-on messages and none of the 290 boot messages | none |
| ARM opcode 1 to 15 message (handler call, SPI DMA opcode 9 `0x1C03AA`) | the ARM never sends `w1 >> 28 != 0` in the boot log or the note-on; the SPI master routine `0xC0066E64` (62 callers, including `0xC01E26F8`, `0xC01E296C`, `0xC01E30F4` in the type 4 event) is a request / answer transfer: the callers pass a DM address, a count and a stack buffer for the **answer** (DSP to ARM) | none seen; opcode 9 not driven by the ARM in any run |
| Pitch sender | the three copies `0xC01E8CCC`, `0xC01E8FCC`, `0xC01E92D4` write only the pitch word (`[table + 4] + 0x20`, `0x921A6`) and read the voice fields; word 8 is the key independent zone word (section 13) | not the entry |
| Table in rodata | no semitone table of packed floats (section 1) | none |

## 4. What stays open

| Item | Why it stays open |
|---|---|
| The writer of `e` | not an ARM message, not a DSP store, not in the SMP code. What the harness cannot run: (1) the ARM state after the DSP answers the prepare message (the status bit is set by hand, [synth-sample-kernel-rule](synth-sample-kernel-rule.md) s12), (2) the slot allocator that would give the slot object a real index, probably tied to the SAMPLES bank activation of the whole synth section (the object keeps its snapshot values `0, -1` in every run here), (3) an SPI DMA from the ARM or the SMP into the SY (opcode 9), (4) a DSP ISR other than the audio ISR (the vector sweep of `sharc-ref` stops at vector 0) |
| What `e` holds | still `word_law(key) / 0.05` only as a model (D65); the index chain says the table is per slot, not per key, so the value may also be a per-zone or per-note entry written together with the slot |

Reproduce:

```bash
uv run --with unicorn python tools/arm_sy_slot_probe.py target/fw_unpacked/bank0_CODE.bin \
    target/fw_unpacked/bank0_SRAM.bin --cache target/keydown.snap \
    --ns4b ".reference/Nord Stage 4 Factory Restore RevK v1.66.ns4b" \
    --member "Samp Lib/Accordion_Harm/Carina Accordion 1_PS 4.1.nsmp4"
uv run pytest tools/tests/test_arm_sy_slot_probe.py
```

The byte scans are plain: the 32-bit literals `0x93540..0x93591` in `bank0_CODE`, and 16-bit runs through `unpack_ratio` of `docs/research/sy-smp-audio.md` section 4 / `sinc::step_join`.

## 5. Follow-up checks (2026-11-15, review suggestions, each verified here)

| Check | Result | Tag |
|---|---|---|
| (1) xrefs of `0x9062F0F0` and what allocates it | `0x9062F0F0` is a heap address; no literal `0x9062F0F0` / `0x9062F0F4` in `bank0_CODE`. It is object 0 of a **pool of 56 slot objects** (`0x1C` bytes each, `0x9062F0F0 .. 0x9062F7D0`, list at `0xC0563664`) built at boot by `0xC01ED7FC..0xC01ED838`: `[+0] = 0`, `[+4] = -1`, the other five words 0 (stores at `0xC01ED81C`, `0xC01ED820`). 56 = the SY voice count. Users of the list literal `0xC0563664`: `0xC01E0FB4` (voice update), `0xC01E23CC` (`0xC01E23D0`, a per-event walk that calls `0xC01D69A4` / `0xC01D662C`, not an allocator), `0xC01E41B8`, `0xC01ECD74` (not read yet) | VERIFIED (run, byte scan, listing) |
| (1) trigger of `go` (`0x1C084C`) | the only ARM literals of the four stage A states are the ones listed in [sy-smp-audio](sy-smp-audio.md) s3; no ARM store of `0x1C084C` in any run here (the message list of the note-on ends with `0x92432 <- 0xBF800000`) | VERIFIED (run); trigger open |
| (2) bulk records spanning the window | the SY stream (`dsp_image_3_SY1SY2.bin`, 569 blocks) has **zero fill `0x93045 .. 0x93580` (1,339 words, tag 1)**, data `0x93580` (1 word), zero fill `0x93581 .. 0x9358E`, data `0x9358E .. 0x935D2` (68 words). So the table `0x9354A..0x9357F` is explicitly zeroed by the loader and must be filled at run time. Literals in `0x934A0..0x935A0` outside the DSP streams: none; `movw` / `movt` pairs with `#0x9` high half and a low half in `0x3400..0x35A0`: none | VERIFIED (block parse, byte scan) |
| (3) cold boot, write watch | `arm_bulk_tables.capture_boot` from the cold image (not `keydown.snap`), 96,111 port messages: **0** SY or SMP messages in `0x93500..0x935A0`; write hooks on all 56 pool objects over boot and 12 settle windows: **112 writes = the 56 constructor stores `0` and `-1`**, nothing else. So the allocator does not run in boot or in the settled main loop; it needs a user action not driven here (a SAMPLES layer activation beyond `bind_layer`, a program recall, or the real ring consumer) | VERIFIED (run) |
| (4) SY ISRs, opcode handlers 1 to 15 under `sharc-ref --watch` | **not run** (cost); the note-on message list has no opcode 1 to 15 word, so no handler is reached from the ARM side; an ISR other than the audio ISR writing the table is still possible | open |
| (5) other `.nsmp4` members with `[zone + 0x6C] == 1` | **not run**; the kernel-rule note section 12 captured one member only | open |

### Two observations of `0xC01E24F0`

[synth-sample-kernel-rule](synth-sample-kernel-rule.md) section 14 says the type 4 event `0xC01E24F0` sends no SY message **during the sample bind** (`load_sample`, `EDIT_SMP_ID`, `EDIT_MODE`, `bind_layer`, two ticks): at that point no note is on and the event is not in the queue. Section 2 above (and follow-up 2 of that note) describe the **note-on**: the key event queues the type 4 event first and the voice update (slot 1) second, `pump` delivers them in that order, so the event sends the voice reset (`0x92414 <- 0x1C0794`) and word 0 (`0x9219E <- 0xFFFFFFFF`) while the slot fields of the voice object are still `-1`; only then does the voice update copy slot `0` and `-1` from the pool object into `[obj + 0x24]`, `[obj + 0x28]` (`0xC01E09D8`, `0xC01E09F0`). Same code, different harness state (no note, then a queued note); the earlier statement is about bind, this one about note-on. Whether the hardware sends word 0 a second time after the voice update (value `0x04000000`) is not shown: the pump delivers each event once.

## Implications

- `--sample-pitch fw` stays opt-in (D89); `step_join::table_entry_for_key` remains a model. No default, golden or `render_hash` moves.
- Any future writer must be searched where the harness is blind: the slot allocator (who gives `0x9062F0F0 + 0` a non-zero slot and fills the table), the "go" state after the prepare answer, an SPI DMA into the SY.
- A slot index chain exists, so the entry is indexed per slot, not per key: if a writer is found, check whether `e` carries the zone rate ratio and the key law separately.

## Follow-ups

1. Read the two unread users of the pool list `0xC0563664` (`0xC01E41B8`, `0xC01ECD74`) and the activation that gives a pool object a slot; the cold-boot watch (section 5) shows boot and the settled loop never write it.
2. Drive an SMP / SY ISR set in `sharc-ref` (vector sweep beyond vector 0) with the full message list and a table fill poked at `0x9354A`, to see whether the zero entry really mutes the voice.
3. Run opcode 9 (`0x1C03AA`) with a harness SPI model if the ARM ever sends it.
