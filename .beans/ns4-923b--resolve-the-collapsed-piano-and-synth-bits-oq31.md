---
# ns4-923b
title: Resolve the collapsed Piano and Synth bits (OQ31)
status: completed
type: task
created_at: 2026-11-15T00:26:19Z
updated_at: 2026-11-15T00:26:19Z
---

## Context

OQ31: `ns4 preset-check` reported 4 Piano presets (payload bits 174 and 176) and 136 Synth presets (17 bit clusters in bytes 194-321 of the 493 B block) whose decode + pack differs from the file bits. The bit-merging encoder hid it on STORE, the meaning of the bits was open. Programs additionally collapsed the Organ layer B percussion flags (181 of 384 programs).

## Higher Goal

Every decoded field round-trips its own raw value, so the base merge (`to_file_like`, `to_cbin_file_like`) is a safety net, not a crutch, and no stored bit is a mystery.

## Acceptance Criteria

- [x] Name every collapsed cluster (firmware field registry `Edt_*`, `arm-edit-params.csv`, manual, factory data).
- [x] Add a decoded field or an explicit raw field for each; apply in the engine where the behaviour is known.
- [x] `ns4 preset-check <backup>` reports 0 problems and 0 lossy presets, and a decode + pack of all 384 programs reproduces their bits without the merge.
- [x] `ns4 preset-check <backup> --bits` lists the collapsed runs (empty now) for future regressions.
- [x] Unit tests on synthetic bits (no firmware or backup committed).
- [x] Spec section 5.8, OQ31 resolved, decision D64.

## Out of Scope

Engine models for LP+HP, Clav rocker EQ, LFO amount / Filter Env amount morphs, layer B percussion. The nord-gui panel files. Hardware acceptance of a restored backup (OQ26 header category stays open).

## Summary of Changes

- Synth: `Edt_Syn_kFltType` 3 = LP+HP (45 + 28 + 28 layers), env time and cutoff steps that the firmware display tables repeat (`SynthLayerRaw`, `hinted_idx`), morph deltas of LFO Rate / LFO Amount / Filter Env Amount (`lfo_rate_morph`, `lfo_amount_morph`, `filter_env_amount_morph`).
- Piano: `Edt_Pno_kEq` raw 6 and 7 (Clav Treble+Brilliant, Soft+Treble+Brilliant) as `timbre_raw`.
- Organ: layer B percussion flags (`percussion_*_b`), found by the new program decode + pack check.
- `ns4 preset-check --bits`, `preset_bits.rs`; the program check now compares decode + pack with the file.
