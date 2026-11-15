# Engine layer routing: section enables, layer zones and the B3 clip

**Checked:** 2026-11-13 (updated 2026-11-14: the zone code table is VERIFIED, [zone-key-ranges](zone-key-ranges.md); the empty layer enable rule is VERIFIED by running the firmware program load, section 7; `FirmwareRouting` is the default, section 6, [D68](../DECISIONS.md))
**Sources:** `docs/research/keydown-zones.md`, `docs/research/emulator-vs-hardware-audio.md`, the B3 finding of PR #194 (`feat/organ-output-level`, `organ-output-level.md`, not merged when this was written), the 384 factory programs of `Nord Stage 4 Factory Restore RevK v1.66.ns4b` (local, git-ignored), `target/fw_unpacked/bank0_{CODE,SRAM}.bin` (local, v1.66), `tools/arm_program_routing.py`, `tools/arm_program_load.py` (new, section 7), `tools/arm_store.py`, `tools/arm_zones.py`, `tools/arm_edit_capture.py`, `tools/emu_vs_hw.py`, `ns4 render-patch --firmware-routing` (new). Only addresses, offsets, derived values and counts are recorded; no firmware bytes.
**Question:** The B3 clip (`B3 Chords n Solo`) sounds organ layer B in the engine but not on the hardware. Which fields of the program decide which layers sound and which keys they accept, what does the firmware do with them, and does an engine that follows them close the gap?

Tags: **VERIFIED** = reproduced by a command, **EMPIRICAL** = fitted to factory data or recordings, **ASSUMED** = reading.

## 1. Answer in short

| Question | Answer | Tag |
|---|---|---|
| Is layer B of `B3 Chords n Solo` silent because it belongs to section II? | **No.** Section I is organ only (`section_enable_i = 0b100`) with both layers on (organ `enable_i = 0b11`), section II is empty. On the ARM the zone enable byte of both organ layers is 1 in scene I (section 2). Layer B is silent on the hardware because of its **keyboard zone**: the layer blocks store `Edt_Org_kKbZone` = 0 for A and 6 for B, and the Mid split point (65) is the only active one, so A plays the keys below 65 and B the keys from 65 | VERIFIED (ARM zone bytes), EMPIRICAL (zone codes, section 3) |
| Which fields decide which layers sound? | `section_enable_i / ii` (global bits `284..290`), per-section layer enables `enable_i / ii` (organ and piano 2 bits at `9` / `12`, synth 3 bits at `10` / `15`), the active scene. Zone byte of layer `i` = section enable AND layer-mask bit `i` | VERIFIED (ARM, 30 rows, 0 mismatches) |
| Which fields decide which keys a layer accepts? | `split_enabled` and the three point flags `split_zone_enabled` (bits `8`, `9..11`; the firmware ids `Edt_Com_kSplitEnabled / Low / Mid / High`), the point positions and crossfade widths, and a **4-bit zone code per layer** (`kKbZone`: organ / piano layers `+8`, synth layers `+58`). `octave` is the 4-bit field after it and does not enter the range | VERIFIED (field names and order; code meaning and key range since 2026-11-14, [zone-key-ranges](zone-key-ranges.md)) |
| Does the engine now route like that? | Yes: `FirmwareRouting::On`, the default of `FirmwareOptions` since D73 (`--legacy-routing` / `--legacy-all` give the old behaviour); the raw `Stage4Engine::new` keeps `Off`, `render_hash` unchanged | built, tested |
| Does it close the B3 clip gap? | see section 5 | EMPIRICAL |

## 2. ARM check: the zone enable byte follows the program's section / layer enables

`tools/arm_program_routing.py` decodes scene I and II of a `.ns4p` (section enable bit and layer mask of the organ, piano and synth sections), writes `(enable, layer mask, focus 0)` into the section table `[[0xC055B144]] + 0x288` (view and scene A), lets the layer enable handlers (`Edt_OrgCom_kLayerEnableOrg_I` `0xAE`, ..., [keydown-zones](keydown-zones.md) section 2.2) run for every layer, and reads the zone byte back.

First the truth table with synthetic views (one dispatch per layer, layers of one section):

| view (enable, mask `CBA`) | organ A B | piano A B | synth A B C |
|---|---|---|---|
| (1, 001) | 1 0 | 1 0 | 1 0 0 |
| (1, 010) | 0 1 | 0 1 | 0 1 0 |
| (1, 011) | 1 1 | 1 1 | 1 1 0 |
| (1, 100) | 0 0 | 0 0 | 0 0 1 |
| (1, 111) | 1 1 | 1 1 | 1 1 1 |
| (0, 011) | 0 0 | 0 0 | 0 0 0 |
| (0, 111) | 0 0 | 0 0 | 0 0 0 |

The dispatched value of the layer enable id (0..3) changes nothing: the byte depends only on the section view. So **zone byte of layer `i` = section enable AND bit `i` of the layer mask** (VERIFIED, run).

Then decoded factory programs, both scenes, three sections each (`python tools/arm_program_routing.py ... <programs>`): `B3 Chords n Solo`, `B3 Jazz Split`, `B3 Old Forest`, `Astoria Grand`, `12dB Sweep`: 30 section rows, **0 mismatches** between the decode (`scene_1_mask` / `scene_2_mask` rule of the engine) and the ARM zone bytes. For `B3 Chords n Solo` scene I the organ zones read `[1, 1]` (A and B on), scene II everything 0. Section 7 closes the gap that stood here: the writer that fills the section table from the program is now executed (the firmware recalls the stored program, `tools/arm_program_load.py`), so the file bit to table field mapping (`SectionEnable_I` `[Org, Pno, Syn]`, `Edt_*Com_kLayerEnable*_I`) is run, not read by name, and it exposed the empty-mask rule.

## 3. The zone code of a layer

Each layer block starts `tag (8 bits)`, then the firmware field-name table of the layer buffers lists `KbZone, OctaveShift, ...` ([synth-0x2f-ns4decode-crosscheck](synth-0x2f-ns4decode-crosscheck.md): `Edt_Syn_kKbZone` at `+58`, 4 bits, the octave right after it at `+62`). The organ and piano layers have the same chain with the octave at `+12` (the decoded octave field), so `kKbZone` is the 4 bits at `+8` (VERIFIED order, EMPIRICAL offset). `Edt_*_kKbZone` has range `0..11`, default 5 (`arm-edit-params.csv`).

Values over the 384 factory programs (count of layers):

| Layer | codes found |
|---|---|
| Organ A | 0: 10, 5: 234, 6: 5, 9: 133, 10: 2 |
| Organ B | 0: 7, 5: 238, 6: 6, 9: 133 |
| Piano A / B | 5: 251 / 249, 9: 126 / 132, 0, 4, 6 rare |
| Synth A / B / C | 5: 235 / 238 / 236, 9: 115 / 114 / 119, 0: 9 / 14 / 10, 1, 2, 3, 4, 6, 10 rare |

Only 5 and 9 occur on most layers (programs without a split); the others occur in programs whose `split_enabled` is set (48 programs). In the 2-zone programs (only the Mid point flag set) the pairs are always `0` / `6`: `B3 Chords n Solo` A 0 B 6, `B3 Jazz Split` A 0 B 6, `B3 Bassline` A 0 B 6, `B3 Praise Split` A 6 B 0, `B3 Praise Bass` A 6 B 0, `Quintet Bass Spl` synth A 6 B 0, `Funk Hit Split` synth A 6 B 0. Reading `0` = the lower region and `6` = the upper region fits the program names (the bass part of a bass split is `0`) and the hardware clip. Codes `1`, `2`, `3` appear as single zones of 3 and 4 zone splits (`Choral Pad`, `Silver Rush`, `Guitar Concerto`), `4` as another lower half (`Night Gaunts Whl`, `Horizon Whl`: synth A 4, B and C 6).

**Superseded 2026-11-14.** The first reading (`0..3` = LED zone 1..4, `4` = zones 1-2, `6` = zones 3-4, `7` = zones 1-3, `8` = zones 2-4) was EMPIRICAL and partly wrong: only `0`, `5` and the Mid-only `6` agree with the firmware. The ARM code behind the setters `0xC00C843C` / `0xC00C83FC` was found (they are called by `0xC010ABA4` through the layer vtable, [zone-key-ranges](zone-key-ranges.md) section 2) and the code to range map (a byte table at `0xC02CB230` plus the bound functions `0xC01B0DC4` / `0xC01B0C40`) is now VERIFIED: with four zones `0` = zone 1, `1` = zone 2, `2` = zones 1-2, `3` = zones 2-3, `4` = zones 1-3, `5` = all, `6` = zones 2-4, `7` = zones 2-3, `8` = zones 3-4, `9` = zone 3, `10` = zone 4, `11` = zone 4; fewer active points join the zones next to an inactive point. 384 of 384 factory programs match the ARM zone objects.

## 4. Engine

`engine/routing.rs`: `FirmwareRouting::{Off, On}`, `ProgramRouting` (exact scene masks, legacy masks, codes, point flags), `zone_leds(code)` (firmware table since 2026-11-14), `engine/zone_table.rs` (VERIFIED key range model). `apply_program` records the routing facts of the program; `set_firmware_routing` installs the legacy or the exact scene masks, so the switch can be set before or after the program is applied. `handle_midi` asks `keybed_gain(layer, note)`: `Off` the old `KeybedSplitRouter::note_gain`, `On` the region weights of section 6 of [morph-and-zones](../specs/morph-and-zones.md). The decode reads the seven codes into `GlobalProgramDecoded::kb_zone` (read-only, lossless pack). Units: 13 new tests (code table, Mid-only halves, joined zones with an inactive Low point, crossfade unity sum, no split, scene II, mode swap, before / after order, direct layer channel, `kb_zone` decode, flag parsing).

Layers sound only in their zone (bit-identical self-consistency check: the render of the whole program with `--firmware-routing` equals the legacy render of the same program with the layers outside the played zone switched off by editing `layer_*_enabled`; SHA-256 prefixes of the WAVs, `ns4 render-patch --notes N`, velocity 100):

| Program (codes) | Note | Sounding layers | Routing render = render with the others disabled |
|---|---|---|---|
| `B3 Chords n Solo` (A 0, B 6, Mid 65) | 44 | A | identical (`5452c4bfdd4f`; whole program, both layers: `093a1d27fb94`) |
| | 84 | B | identical (`c09d4949c092`; both layers: `725951ffdfaf`) |
| `B3 Praise Split` (A 6, B 0, Mid 60) | 44 | B | identical (`ea1b7e2f44fa`) |
| | 84 | A | identical (`4017618e85db`) |
| `Funk Hit Split` (synth A 6, B 0, Mid 65) | 44 | B | identical (`b8c774adb00d`) |
| | 84 | A | identical (`86b63ea59fd0`) |

This proved the engine implements the rule (the table of that day was EMPIRICAL); the rule is now the firmware's ([zone-key-ranges](zone-key-ranges.md)). The split keys of these programs moved with the decode fix (`B3 Chords n Solo` Mid setting 4 is key 60, `B3 Praise Split` setting 3 is key 53); the hash prefixes above are those of the old table.

## 5. Clip comparison with the routing on

`tools/emu_vs_hw.py --confirmed-only` (same clips, notes and 100 velocity as [emulator-vs-hardware-audio](emulator-vs-hardware-audio.md); variants `routing` = `--firmware-routing`, `b3wheels+routing` = `--tone-source b3wheels --firmware-routing`; new rows in `emu_vs_hw_segments.json`, raw rows in [emu_vs_hw_routing.json](data/emu_vs_hw_routing.json)). Emulator minus hardware, dB re RMS (RMS in dB):

| Segment | Variant | sub | low | mid | pres | brill | RMS | score |
|---|---|---|---|---|---|---|---|---|
| organ_b3 | default | +17.5 | -0.2 | +0.3 | -5.7 | +0.5 | +14.9 | 4.83 |
| organ_b3 | routing | **-1.2** | -0.4 | +3.5 | -11.0 | -9.0 | +11.5 | 5.01 |
| organ_b3 | b3wheels | +16.5 | +1.2 | -1.1 | -6.7 | -2.2 | +8.0 | 5.54 |
| organ_b3 | **b3wheels+routing** | **-1.6** | +1.1 | +2.2 | -12.5 | -4.7 | **+3.8** | **4.42** |
| organ_vox / organ_pipe / piano_dry / synth_lead / fx_amp | routing | identical to default (every row, 13 columns) | | | | | | |

Level and crest of the B3 clip (mono mix RMS, peak over both channels, share of samples at full scale; hardware RMS -21.4 dBFS, peak -2.9 dBFS):

| Variant | RMS dBFS | crest dB | full scale |
|---|---|---|---|
| hardware | -21.4 | 18.6 | 0 % |
| default | -5.9 | 5.9 | 15.4 % |
| routing | -9.5 | 9.5 | 0.72 % |
| b3wheels | -12.8 | 12.8 | 0.30 % |
| **b3wheels+routing** | **-17.2** | **17.2** | **0 %** |

What it says:

- The sub excess is the layer split. Of the 101 confirmed notes 97 lie below 65 (39..62, mostly 43..47) and only 4 at or above (66, 79, 79, 91), so on the hardware the clip is layer A (zone 0, octave +2) alone; layer B (16' bar at 8, octave 0) belongs to the keys from 65. With the routing the sub falls from +16.5 / +17.5 to -1.6 / -1.2 dB, the RMS gap from +8.0 to +3.8 dB (b3wheels) and the crest from 12.8 to 17.2 dB (hardware 18.6), the full-scale share from 0.3 % to 0. This is the PR #194 result (layer B off: sub -1.4, RMS +3.6, crest within 0.5 dB with its own crest measure) reached by the program's own fields instead of by switching a layer off.
- The cause named in PR #194 ("layer B belongs to section II, which is off") is not what the firmware does (section 2); the zone code is. The note "notes below 52 dropped gives mid within 0.6 dB" is the same thing seen from the other side: the low notes are the part that only layer A plays, at octave +2 and 8 0 8 0 2 bars.
- Presence and brilliance stay dark (-12.5 / -4.7 dB with `b3wheels`, -11.0 / -9.0 with the Legacy source): this is the open tone / rotary part, not routing. The spectrum score improves for `b3wheels` (5.54 to 4.42, better than the Legacy default 4.83) but not for the Legacy source (4.83 to 5.01), because removing layer B takes away the only energy that was filling presence and brilliance there.
- The other five segments are single-layer or unsplit sections: the routing leaves them identical (the legacy decode already gave the same layers there), so nothing regressed.

Note (2026-11-14): the table used Mid = 65 (old 10-entry decode table). The firmware key for the stored setting is 60, so notes 60..64 of the clip belong to layer B, not A; the clip rows above were not re-measured (the video is local-only). The sub / crest / full-scale conclusions hold for the 97 notes below 60.

## 6. Decision (D22)

| Criterion | Status |
|---|---|
| (1) algorithm verified | the rule (enable AND mask) is VERIFIED on the ARM, the loader's empty-mask rule is VERIFIED (section 7), the key range and the crossfade curves are VERIFIED ([zone-key-ranges](zone-key-ranges.md)) |
| (2) mapping verified, not assumed | **met**: the code to key range map equals the zone objects of 384 / 384 factory programs, and the file layer enables reach the zone bytes through the executed program load (section 7: 20 programs recalled by the firmware, scene I, 0 mismatches against the decode plus the rule) |
| (3) re-baseline `render_hash` | the golden segments render with the raw engine and no program, so `routing.loaded` is false and the hash is unchanged (`golden_render` test passes unchanged). The program sweep is re-baselined: changed programs listed in [D68](../DECISIONS.md) |

`FirmwareOptions::default()` and `firmware_all()` now have `firmware_routing = true`; `legacy_all()` and `--legacy-routing` turn it off; the GUI Firmware bar reads `FirmwareOptions::default`, so the Firmware routing box starts checked (status Verified). The raw engine default (`Stage4Engine::new`) stays `Off`, so a program applied to a bare engine keeps the legacy path until the options are applied. Sweep with the new default: 384 of 384 render, panic 0, non-finite 0, **silent 0** in both modes; clipping >= 0.1 % in 13 renders (17 before); 56 of 384 default renders change against the committed sweep of origin/main (RMS or peak moves by more than 1e-4: the split programs and the section-scenes with an empty layer mask; largest `Farfisa II` +17.9 dB, `LoFi Piano` -13.5, `Piano Soft Orch` +10.3, `Wormhole Organ` -10.2, `Rising Sun` +5.9, `B3 Old Forest` -5.8, `Pizz Quintet Spl` -5.7, `B3 Dual Samba` -5.4) (`docs/research/program-coverage.md`, [program-sweep.json](data/program-sweep.json))

## 7. Empty layer enables (OQ25, closed 2026-11-14 by the real program load)

Question: 9 programs decode to a silent scene I under `--firmware-routing`. Is the layer enable stored elsewhere, or does a loader rule make them sound?

**Answer: the loader rule.** The firmware's view refresh `0xC00F8A50` reads the scene table of every section and stores the view mask as `mask = (scene mask == 0) ? 1 : scene mask` (`cmp r2, #0; moveq r2, #1` at `0xC00F8AB4..0xC00F8AB8`). A stored layer enable of 0 is therefore read as **layer A** in every section whose enable bit is set, whether or not another section sounds. The zone byte is still `section enable AND mask bit` (section 2), so Vox Like Me and the other eight programs sound layer A on the hardware.

How it was run (`tools/arm_program_load.py`, new): the `.ns4p` is written into the Store record of the EEPROM image of `tools/arm_store.py` (record body = `file[0x2C:]`, 824 bytes, CRC-32 over the body, name outside the CRC; derived image under `target/oled/`), the full machine of `arm_screens.Session` boots with it (about 4 minutes of CPU) and the firmware recalls the program at `A:11` itself (the home screen shows the program name). Nothing is written into the section table by the harness. 20 distinct programs were loaded this way: the 9 silent ones plus `Tines Layer` and `J3 Grand Layers` with a scratch probe that did the same Store patch and the same reads, and 12 with the committed tool (`B3 Chords n Solo` as control, `Windhoek B3 II`, `24 Frames J3`, `Pno Brass Punch`, `Piano Soft Orch`, `Rising Sun`, `Steam Piano`, `Piano Str Pad`, `Felt Upright`, `Vox Like Me`, `Tines Layer`, `Farfisa II`); the tool compares view and zone bytes of scene I with the decode plus the rule and found 0 mismatches.

| Finding | Evidence | Tag |
|---|---|---|
| The recall fills the section table `[[0xC055B144]] + 0x288`: per section `view (enable, mask, focus)` at `+0`, scene A at `+0xC`, scene B at `+0x18`. Write watch over the boot: the scene tables are written by the bit-field unpacker `0xC024DD34` (called from `0xC00F8A74` with `setup + 0x284`, stores at `0xC024DEE8..0xC024DEFC`), the view by `0xC00F8AA8..0xC00F8AC8` | run, write watch, B3 Chords n Solo | VERIFIED |
| View mask rule `mask = scene mask or 1`, per section, scene I (`[setup + 0x314] != 1`) and scene II (offset `+0xC`) share the loop | disassembly `0xC00F8A50..0xC00F8B38`; run for scene I | VERIFIED (scene I run, scene II disassembly) |
| The 9 programs after the real load: organ (Vox x5, Farfisa II) view `(1, 1)` with scene A `(1, 0)`; `Burning Lead` synth view `(1, 1)`; `LoFi Piano` piano view `(1, 1)`; `70s CP Strings` piano `(1, 1)` and synth `(1, 1)`. Zone bytes: Vox Like Me, Vox Rising Sun, Vox Inna-Gadda, Vox Im Down, Vox DC5, Farfisa II organ A = 1; Burning Lead synth A = 1; LoFi Piano piano A = 1; 70s CP Strings piano A and synth A = 1 | `tools/arm_program_load.py` on each program | VERIFIED (run) |
| Sections with mask 0 next to a sounding section: `Tines Layer` piano view `(1, 1)` zone piano A = 1 beside synth A and B; `Piano Soft Orch`, `Rising Sun` the same; `J3 Grand Layers` and `24 Frames J3` organ A = 1 beside piano and synth; the view mask is 1 in all of them | run on the five scene I cases (`Tines Layer`, `Piano Soft Orch`, `Rising Sun`, `J3 Grand Layers`, `24 Frames J3`); the other beside-sounding cases are scene II and were not recalled in scene II. Control `B3 Chords n Solo`: view `(1, 3)`, zones `[1, 1]`, focus 1 | VERIFIED (run, scene I) |
| Reach of the rule: 29 section-scenes of 26 factory programs have an enabled section with mask 0 (15 in scene I, 14 in scene II; 15 of them sit next to a sounding section, the rest are the 10 section-scenes of the 9 silent programs and scene II cases); the decode of every program reproduces the view mask the rule gives (`arm_program_load.expected_view`, tests) | decode | VERIFIED (decode of the rule, run on 20 programs) |

So the previous engine rule (layer A only when the whole scene would be silent, ASSUMED) was too narrow: the firmware adds layer A to **every** enabled section with an empty mask. Engine: `routing::default_layer_a(section_enable, mask)` (organ bits 0..1, piano 2..3, synth 4..6; a section without enable bit stays 0). Programs affected: the 9 above, plus the section-scenes beside a sounding section, e.g. scene I `Tines Layer`, `Piano Soft Orch`, `Rising Sun` (piano), `J3 Grand Layers`, `24 Frames J3` (organ) and scene II `Piano Str Pad`, `Steam Piano`, `Pno Brass Punch`, `Oo Pad Whl`, `Melody Pearls` (piano / synth). Tests: `routing::tests::enabled_section_without_layers_plays_layer_a`, `empty_section_next_to_a_sounding_one_still_plays_layer_a`, `section_with_a_sounding_layer_keeps_its_mask`, `tests_firmware_routing::empty_section_next_to_a_sounding_one_plays_layer_a`, `tools/tests/test_arm_program_load.py`.

Not run: scene II through the real load (a scene switch was not driven; the loop is the same with the offset `+0xC`), and the load of programs whose sample sets are missing (the home screen shows `Sample not found` for `Piano Soft Orch` and `Piano Str Pad`: the section table is filled before the sound load, the zone bytes were read after).

## Implications

- The B3 clip gap that looked like an output stage problem is the layer zone: with the routing the sub, the crest and the clipping match the hardware and the RMS is within 4 dB. `B3Wheels` (+ tail) with `--firmware-routing` is now the closest B3 path; the remaining B3 gap is presence / brilliance (-12.5 / -4.7 dB) and the level law `L` (OQ19).
- Factory programs that split (48 of 384) now sound their layers only in their zone with `--firmware-routing`; programs that do not split are unchanged.
- The legacy empty-mask fallback (`apply_program`) hid programs whose scene masks are all zero; with the routing on, every enabled section with no layer on plays layer A, as the firmware loader does (section 7, VERIFIED); a scene with no enabled section stays silent.

## Follow-ups

1. ~~Find the writer of the zone key range from `kKbZone`~~ done 2026-11-14: [zone-key-ranges](zone-key-ranges.md).
2. ~~Run the program load path of the firmware in the harness~~ done 2026-11-14 (section 7). Remaining: drive a scene switch through the real UI path so scene II is executed too, and load a program with a split to read the zone objects from the recalled state.
3. Octave range limit of a zone, Live-mode scene focus, `Edt_Sys_kMIDIOrgSectionEnable` (MIDI section enables); the virtual channel map writer is found (OQ23, [keydown-zones](keydown-zones.md) s2.3), the engine does not model the menu channel assignment yet.
4. A second hardware clip of a split program (the codes are now read from the firmware; the clip would confirm the end-to-end level of the crossfade). Re-run section 5 with the corrected Mid key 60.

## Reproduce

```bash
# OQ25: the real program load (boot 4 min per program; the Store image comes from arm_store.py)
C=target/fw_unpacked
uv run --with unicorn python tools/arm_screens.py boot $C/bank0_CODE.bin $C/bank0_SRAM.bin --out target/oled/state.bin
uv run --with unicorn python tools/arm_store.py store $C/bank0_CODE.bin $C/bank0_SRAM.bin --state target/oled/state.bin --out target/oled/eeprom.bin
uv run --with unicorn python tools/arm_program_load.py $C/bank0_CODE.bin $C/bank0_SRAM.bin --eeprom target/oled/eeprom.bin "factory_programs/Program/Bank A/Vox Like Me.ns4p"
cargo build --release -p nord-core --bin ns4
uv run --with unicorn python tools/arm_program_routing.py target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin --cache target/keydown.snap "factory_programs/Program/Bank A/B3 Chords n Solo.ns4p" "factory_programs/Program/Bank A/B3 Jazz Split.ns4p" "factory_programs/Program/Bank A/B3 Old Forest.ns4p" "factory_programs/Program/Bank A/Astoria Grand.ns4p" "factory_programs/Program/Bank A/12dB Sweep.ns4p"
uv run python tools/emu_vs_hw.py --video <part A webm> --backup <factory .ns4b> --workdir <scratch> --confirmed-only --only organ_b3
ns4 render-patch "Program/Bank A/B3 Chords n Solo.ns4p" --backup <factory .ns4b> --notes 44 --wav out.wav   # routing is the default; --legacy-routing for the old path
```
