---
# ns4-yp85
title: Apply the program Touch velocity curves (Piano/Organ/Synth) from the firmware
status: todo
type: task
created_at: 2026-11-15T04:15:47Z
updated_at: 2026-11-15T04:15:47Z
---

## Context
The emulator passes the velocity of a note almost unchanged to the engine (MIDI controller velocity as sent; mouse clicks use a fixed 88). The Nord Stage 4 programs store a per-section velocity response (for example `Touch: Medium` on Piano A:13 Felt Upright; Soft / Medium / Hard and the Organ and Synth equivalents). That response is part of the sound and applies to any note source, including an external USB MIDI controller. No model of these Touch curves was found in the engine. The keybed profile curve of the hardware (`[manager+0x134]`, profile types 1 and 2, raw key time 34..416 to velocity byte) only matters for the Nord's own keybed, not for external MIDI, and is out of scope.

## Higher Goal
Closer audio parity: the same program and the same MIDI velocity must give the same level and timbre as the hardware, which includes the Touch setting. Also needed to interpret the Felt Upright and Velvet Grand clip comparisons (uniform velocity 88 is an estimate).

## Acceptance Criteria
- [ ] Find in the ARM firmware where the per-program Touch setting (Piano, Organ, Synth sections) is read and how the velocity is mapped (curve tables, edit ids, which core applies it).
- [ ] Verify whether the curve applies to incoming MIDI velocity as well as to the keybed (run the real note-on path in the harness for the three Touch values).
- [ ] Implement the verified curves in the engine, opt-in first (FirmwareOptions switch and a GUI Firmware bar control), flip per D22 only after comparing Felt Upright and Velvet Grand with the Transkun velocities.
- [ ] Unit tests with oracle/ARM goldens, docs (piano-engine spec, synth-engine spec, organ spec, OQ23 velocity item, ROADMAP).

## Out of Scope
The Nord keybed raw-timing to velocity profile curve (hardware keybed only). MIDI port selection (separate task).
