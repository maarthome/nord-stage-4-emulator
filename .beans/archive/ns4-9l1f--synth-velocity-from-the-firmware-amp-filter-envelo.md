---
# ns4-9l1f
title: 'Synth velocity from the firmware: amp, filter envelope, factory census'
status: completed
type: task
priority: normal
created_at: 2026-11-15T18:07:19Z
updated_at: 2026-11-15T18:21:52Z
---

## Context
The listening check showed synth velocity is not captured right: the synth renders use uniform velocity 100 and the engine's amp velocity law is ASSUMED.

## Higher Goal
Audio 1:1: the same synth program and MIDI velocity must give the hardware level and filter response.

## Acceptance Criteria
- [x] Read from the ORG core 2 pass how velocity enters the amp and the filter envelope (strip words 53 and 19, key byte g).
- [x] Derived-number CSVs with generator and --check (tools/synth_velocity_law.py, synth_velocity_census bin).
- [x] Opt-in SynthLayer::firmware_velocity (--firmware-synth-velocity) with tests (monotonic, endpoints, oracle match).
- [x] Render Horizon Whl, Leaky Stabs II, Grand Pad and report which programs are sensitive.
- [x] Specs, D104, OQ, ROADMAP.
- [ ] Default flip: stays off (D22), needs end to end verification.

## Out of Scope
Oscillator envelope velocity, GUI switch, default flip.

## Summary of Changes
- ORG core 2 oracle: amp velocity gain V at DM(0xC10B4) (bit blend of strip words 53 and the key byte g, -6 s dB to +2 s dB) and filter-envelope factor G/128 at DM(0xC2723); 768 of 768 words within 1 ulp.
- Code: synth/fw_velocity.rs, SynthLayer::firmware_velocity, --firmware-synth-velocity, program_sweep --synth-velocity, tools/synth_velocity_law.py (oracle, --check), nord-tools synth_velocity_census (--out, --check).
- Data: docs/research/data/synth-velocity-law.csv, synth-program-velocity.csv. Docs: research/synth-velocity.md, synth-engine spec, D104, OQ22, ROADMAP, audio-remaining.
- Default stays off (D22). Open: multiply of V in the voice, file bits of the three fields, kOscEnvVel, GUI switch, hardware clip.
