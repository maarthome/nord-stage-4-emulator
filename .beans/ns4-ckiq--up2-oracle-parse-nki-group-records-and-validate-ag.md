---
# ns4-ckiq
title: 'UP2 oracle: parse NKI group records and validate against WAV catalog'
status: in-progress
type: task
created_at: 2026-11-14T23:47:48Z
updated_at: 2026-11-14T23:47:48Z
---

## Context
Follow-up of ns4-zgmb (merged, PR 346). Zones/file list/script are parsed; group records (names, envelopes, filters, effects with values, IR references) are not.

## Higher Goal
1:1 audio parity for the Nord Stage piano/FX engine via measured reference data.

## Acceptance Criteria
- [x] Locate and document group records in the decoded .nki stream (`tools/nki_groups.py`)
- [ ] Zone to group membership (not resolved: no group id in the zone record; order matches only for the first 107 groups)
- [x] Group volume read and sanity-checked (0.780274 for dn/up/xx, 0.840287 for all 41 ko groups); envelope/LFO values not decoded
- [x] Unit tests with synthetic fixtures; docs/research updated

## Out of Scope
Protected libraries; redistributing third-party data.

## Summary of Changes
- Added `tools/nki_groups.py` (181 groups: name, volume, modulator inventory) with 5 tests; documented the group layout, the volume rule and the unresolved zone-to-group mapping in `docs/research/nki-lz-stream.md`.
