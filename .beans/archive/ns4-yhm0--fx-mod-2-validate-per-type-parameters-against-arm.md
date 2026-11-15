---
# ns4-yhm0
title: 'FX Mod 2: validate per-type parameters against ARM and SHARC (D90)'
status: completed
type: task
created_at: 2026-11-15T13:19:25Z
updated_at: 2026-11-15T13:19:25Z
---

## Context
Mod 2 stages read kFx2Amt/kFx2Rate words from 11 samples; shared words assumed A R / R(64).

## Higher Goal
Audio 1:1 with hardware; CHOR precedent (fx-chorus-hw-clip).

## Acceptance Criteria
- [x] Dense ARM capture of every Fx2 word (44 values)
- [x] Joint law for words both ids write
- [x] CHOR 4/5, VIBE closed forms pinned at all 44 values
- [x] ENS finding recorded, not wired
- [x] D90, OQ21, ROADMAP, spec, research note

## Out of Scope
Reverb types, Grand Pad synth FX rack, default flips.

## Summary of Changes
See docs/research/fx-mod2-validation.md.
