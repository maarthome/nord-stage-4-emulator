---
# ns4-vq5k
title: 'Tools: extract GUI pictures from the UP2 .nkr resource container'
status: completed
type: task
created_at: 2026-11-15T01:30:00Z
updated_at: 2026-11-15T01:30:00Z
---

## Context
The user asked for the UP2 button and panel PNGs. They sit unencrypted in `Ultimate Pianos 2.nkr`.

## Higher Goal
Reference material for an emulator GUI and for reading the UP2 panel layout.

## Acceptance Criteria
- [x] `tools/nkr_extract.py` parses the directory and writes every file
- [x] Real file: 298 files, 149 PNG, all with readable dimensions
- [x] Unit tests (7) on a synthetic container; research note

## Out of Scope
Committing the artwork; reading other libraries; mapping pictures to controls.

## Summary of Changes
- Added `tools/nkr_extract.py`, `tools/tests/test_nkr_extract.py`, `docs/research/nkr-resource-container.md`.
