---
# ns4-yw1c
title: 'Tools: UP2 oracle effect and curve tables from the KSP script'
status: in-progress
type: task
created_at: 2026-11-15T00:40:00Z
updated_at: 2026-11-15T00:40:00Z
---

## Context
The UP2 script holds chorus, phaser, flanger, EQ and curve tables as literal arrays. They are the only numeric effect settings available without Kontakt.

## Higher Goal
Compare Nord Stage 4 effects with the Kontakt effect (D60, OQ21) using known settings.

## Acceptance Criteria
- [x] `tools/ksp_arrays.py` extracts every literal array and links it to `ENGINE_PAR_*`
- [x] Real script: 48 arrays, 11 feed engine parameters
- [x] Unit tests; research note updated
- [ ] Unit mapping of Kontakt `CHORAL_RATE` / `FLAIR_RATE` / `PHASIS_RATE` to Hz (needs a Kontakt measurement)
- [ ] Which knob positions the hardware clip of D60 used

## Out of Scope
Redistributing the tables as data files in the repo; running Kontakt.

## Summary of Changes
- Added `tools/ksp_arrays.py` and tests; documented the tables in `docs/research/nki-lz-stream.md`.
