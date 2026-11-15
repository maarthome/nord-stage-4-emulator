---
# ns4-g6i0
title: 'ENS tap registers: init and loader kernels checked (D109)'
status: completed
type: task
created_at: 2026-11-15T19:41:49Z
updated_at: 2026-11-15T19:41:49Z
---

## Context
D100 left ENS B4/L4/B7/L7 ASSUMED.

## Higher Goal
Verified ARM/SHARC words for every Mod type.

## Acceptance Criteria
- [x] ENS init decoded (sets B7/L7 only)
- [x] Loader kernels zero L4; no code writes L4/B4
- [x] Missing run named in OQ21a
- [x] D109, ROADMAP, audio-remaining row 6

## Out of Scope
The DSP-side run with the ARM module install; default flips.

## Summary of Changes
See docs/research/fx-mod1-validation.md s3b.
