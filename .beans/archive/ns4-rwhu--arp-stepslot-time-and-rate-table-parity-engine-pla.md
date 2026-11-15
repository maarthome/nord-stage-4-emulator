---
# ns4-rwhu
title: Arp step/slot time and rate table parity (engine played 16th at linear 30..300 BPM)
status: completed
type: task
created_at: 2026-11-14T17:16:04Z
updated_at: 2026-11-14T17:16:04Z
parent: ns4-xc5n
---

## Context

`SynthLayer::step_seconds` plays one step as a 16th note at `arp_rate_bpm` (`15 / D` seconds) and `synth_preset.rs` maps the 7-bit stored rate linearly to 30..300 BPM. The fourth research pass (`docs/research/synth-arp-patterns.md` section 8) found three facts that do not agree with that: the firmware rate table has 128 exponential entries (display BPM `16 .. 1200`, ns4decode lists the same values up to 990 then "Fast1..5"), the free-running clock moves `2^19` phase units per firmware beat with a grid point every `2^18` (`15 / D` s), and the manual (p.36) says one pattern step is a 1/8 note (`30 / D` s) at the displayed BPM. Either a grid point is a slot (then the engine plays every step twice as fast) or it is a step (then the manual wording is loose).

## Higher Goal

Arp / Gate rate and step length equal the hardware (1:1 parity).

## Acceptance Criteria

- [x] Decide step length: the record player is `0xC023B3EC` (the pattern object's clock-listener method), one record per grid point of `2**18` phase units, two records per step: **a step is `30 / D` s (1/8 note), the engine was 2x fast.** Replay: `tools/ns4_arp_player.py gates` (0 differences), end to end on the booted firmware (25.0 ms at dial 127, 66.7 ms at dial 105).
- [x] Replace the linear 30..300 BPM stored-rate mapping by the 128-entry firmware table (`synth/arp_rate.rs`, `SynthLayerDecoded::arp_rate_raw`); the GUI edit clamp widens to 16..1200, the knob / host range stays 20..300.
- [x] Fix `step_seconds` (`30 / D`); arp / gate tests keep their step times by running at 480 BPM, new tests pin 120 BPM = 12000 samples per step and the table goldens.
- [x] Docs: research note section 9, `synth-engine.md`, `cbin-and-presets.md`, D66; OQ28 answered and removed; OQ32 and bean ns4-hwe8 for the gate fraction of `E`.

## Out of Scope

Master Clock sync (already beats-based); pattern content; tie playback.

## Summary of Changes

- `crates/nord-core/src/synth/arp_rate.rs` (new): the firmware table `R[v]` (128 ints, `bank0_CODE@0x2BC3F0`), `arp_bpm_from_raw`, `arp_raw_from_bpm`, `step_seconds_at` = `30 / D`; tests pin the table ends, the duplicate pairs, the round trip and replay-derived goldens (dial 0 / 54 / 127 = 1.875 / 0.25 / 0.025 s per step).
- `SynthLayer::step_seconds` uses it (VERIFIED). `cbin/synth_preset.rs` decodes `arp_bpm` from the table and keeps `arp_rate_raw` (lossless for all 128 values, also the N.A. duplicates); `synth_params_arp.rs` derives the Master Clock subdivision from the raw; the GUI `ArpRate` edit clamps to 16..1200.
- `tools/ns4_arp_player.py` + tests (new): replays the real player on the ARM emulator; `ns4_arp_gate.py` points to it.
- Research note section 9 (player, end-to-end timing, 24 kHz time base, gate / swing / tie / pan / accent), spec, D66, OQ28 removed, OQ22 note, OQ32 + bean ns4-hwe8 (E = 2/3 step on the boot grid).
- `ns4 bench`: no scenario enables the arpeggiator, so the hash is unchanged (checked after the change).
- Not done: `beans archive` (it would also move the other completed beans).
