---
# ns4-zgmb
title: 'Tools: UP2 reference oracle (NCW decoder, WAV catalog, NKI script/zone parser)'
status: in-progress
type: task
created_at: 2026-11-14T21:09:02Z
updated_at: 2026-11-14T21:09:02Z
---

## Context
The user owns an Ultimate Pianos 2 Kontakt library and exported (from Kontakt edit mode) its samples as `.ncw`/WAV and a re-saved `.nki`. These are local reference data (`.reference/`, git-ignored) for validating piano, reverb and chorus behaviour.

## Higher Goal
1:1 audio parity for the Nord Stage piano engine via measured reference data.

## Acceptance Criteria
- [x] `tools/ncw.py` decodes NCW to PCM and a CLI converts folders to WAV
- [x] `tools/up2_oracle.py` builds a metrics catalog (peak, decay, centroid, tail) from exported WAVs
- [x] `tools/nki_lz.py` decompresses the `.nki` preset stream and extracts the KSP script
- [x] `tools/ksp_ui_map.py` maps `on ui_control` handlers to engine parameters with saved values
- [x] `tools/nki_zones.py` parses zones and the sample file list
- [x] Unit tests with synthetic fixtures for every tool
- [x] Validated on the real export (zone frames equal WAV frames 26,405/26,405; names match 26,405/26,405)
- [x] `docs/research/ncw-format.md` and `docs/research/nki-lz-stream.md`
- [ ] Group records (envelopes, filters, effect chains with values, IR references) parsed (names, volume, modulator inventory done in ns4-ckiq; the rest tracked there)
- [x] Script velocity/release formulas compared against the WAV catalog: not possible, samples are peak-normalized per file (documented in nki-lz-stream.md follow-ups)

## Out of Scope
Decrypting `.nkx` or protected libraries; redistributing samples or derived third-party audio; chorus/phaser/flanger parameter values (set outside `on ui_control`).

## Summary of Changes
- Added `tools/ncw.py`, `tools/up2_oracle.py`, `tools/nki_lz.py`, `tools/ksp_ui_map.py`, `tools/nki_zones.py` with tests (31 new tests).
- Documented the NCW layout and the `.nki` LZ stream (two extensions beyond the `ni-file` notes: 8192 far-offset escape, `0xFF` length continuation), the UI control map and zone validation numbers.
- Remaining criteria stay open for a follow-up bean.
