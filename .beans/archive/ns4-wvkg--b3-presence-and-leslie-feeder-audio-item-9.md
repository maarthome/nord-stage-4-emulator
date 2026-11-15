---
# ns4-wvkg
title: B3 presence and Leslie feeder (audio item 9)
status: completed
type: task
created_at: 2026-11-15T15:21:52Z
updated_at: 2026-11-15T15:21:52Z
---

## Context
Audio item 9: B3 presence and Leslie level open; feeder of the Leslie processor untraced.

## Higher Goal
Audio 1:1 with hardware.

## Acceptance Criteria
- [x] Trace the Leslie feeder (organ-preamp-search s6.1)
- [x] Replay the B3 clip with a realistic note set (tools/note_offsets.py)
- [x] Docs, D94, OQ21, ROADMAP, audio-remaining updated; no default flipped (D22)

## Out of Scope
DMA ring model, reverb, synth rack.

## Summary of Changes
New tools/note_offsets.py + tests; docs only otherwise.
