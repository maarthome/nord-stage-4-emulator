---
# ns4-hwe8
title: Arp gate lengths on the firmware slot grid (E = 2/3 step, plain arp staccato)
status: todo
type: task
created_at: 2026-11-15T01:17:10Z
updated_at: 2026-11-15T01:17:10Z
parent: ns4-xc5n
---

## Context

The fifth research pass on the arp (docs/research/synth-arp-patterns.md section 9, OQ32) showed that the firmware plays the note-off of an E step at the second grid point of its step; the pattern objects boot with the grid 0xC02BC1A0 whose second point is at 2/3 of a step, so E lasts 2/3 of a step and Pattern off (all-zero words, every step E) is staccato. The engine keeps E = 4/8 of a step and holds the plain arpeggio note to the next step.

## Higher Goal

Arp / Gate note lengths equal the hardware (1:1 parity).

## Acceptance Criteria

- [ ] Decide the meaning of the pattern object flag [obj+0xC] (boot value 1, no writer found): find a writer or a hardware recording of an arp note against its step.
- [ ] Model E as 2/3 of a step and the plain arpeggio as E steps (gate lengths are stored as eighths: GUI dial 0..8, host parameters, pattern words), with goldens from tools/ns4_arp_player.py events.
- [ ] Update synth-engine.md, the gate tags and OQ32.

## Out of Scope

Step length and rate table (done in ns4-rwhu); Master Clock sync.

## Summary of Changes

(open)
