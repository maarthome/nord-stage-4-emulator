---
# ns4-l9vl
title: 'FM voice wiring: pitch, P words, ARM words, strip a4/a8 probe'
status: completed
type: task
priority: normal
created_at: 2026-11-15T16:44:37Z
updated_at: 2026-11-15T16:47:53Z
---

## Context
D92 left the FM voice words open and the table was claimed type-only.

## Higher Goal
Wire the FM banks 1/2 (types 49..53) into a voice, opt-in, with verified inputs only (OQ22).

## Acceptance Criteria
- [x] FmVoice wires W, p0, P words, ARM words to stages 1/2 with tests
- [x] Probe over all 38 strip offsets read by the apply; type-only claim corrected
- [x] D96, spec 4.5, OQ22, ROADMAP, audio-remaining row 10 updated
- [ ] All AGENTS.md checks pass, PR opened (not merged)

## Out of Scope
Writer of strip +0xA4/+0xA8, generator of the 361 words, default flip, SynthLayer hookup.

## Summary of Changes
See D96.
