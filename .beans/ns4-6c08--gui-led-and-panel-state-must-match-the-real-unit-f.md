---
# ns4-6c08
title: 'GUI: LED and panel state must match the real unit for the same program (demo-video check)'
status: in-progress
type: task
created_at: 2026-11-15T05:53:23Z
updated_at: 2026-11-15T05:53:23Z
---

## Context
The same program (`A:14 B3 Chords n Solo`) was loaded on the real Stage 4 (frame at 3:56 of the demo video `NEW NORD STAGE 4 - PROGRAMAS PRESETS DEMO (PART A)`) and in the emulator. The state-driven LEDs differed: the ladders showed layer A's drawbars, the percussion LEDs were the defaults, the zone LEDs were all lit, the sliders followed the program. `apply_program` mirrored only layer A and a few Organ fields.

## Higher Goal
With the same program loaded, every LED and control that depends on state shows what the real panel shows (interoperability and parity, `docs/specs/visual-style-and-panel.md` 4.2a).

## Acceptance Criteria
- [x] A program can be loaded by panel location for screenshots: `nord-stage4 --backup X.ns4b --program A:14 --screenshot ...`, `tools/gui_shots.py --backup X --program A:14`
- [x] Real vs decoded vs ours table for A:14 and 7 more programs (A:13, A:22, A:24, A:34, A:38, A:77, A:54 partly) from video frames
- [x] Organ: per-layer drawbars, percussion, model, vibrato, octave, rotary mirrored; ladders and percussion show the focused layer; load focuses the highest enabled layer (Organ)
- [x] Percussion played per layer (engine), program export writes both layers
- [x] Sliders are physical (a load does not move them), Preset-mode edits write the focused layer
- [x] KB ZONE LEDs from the stored zone code and the split state, dark without a split
- [x] ON/OFF LED of the focused layer blinks while several layers are on (Organ, Piano, Synth)
- [x] Piano: layer extras and shared buttons mirrored; ACOUSTICS LEDs only on acoustic types
- [x] Unit tests per mismatch; legibility audit unchanged (0, empty allow-list); no `Clavia` in the UI
- [ ] Piano TIMBRE `Off` vs SOFT, UNISON / DYN COMP LEDs, SUSTPED / PSTICK per layer, the stored focus bits (open, see the spec)

## Out of Scope
LED spacing, rotary layout, model LED order (branch `fix/led-spacing-rotary-order`), layout overlay tooling (`feat/layout-overlay-validation`). Sound comparison of layers with different percussion sets.

## Summary of Changes
- `headless_load.rs`, `main.rs`, `tools/gui_shots.py`: `--program A:14` with `--backup`.
- `panel/organ_layer.rs` (new), `ui_state/organ.rs`, `organ_drawbars.rs`, `bay_organ.rs`, `organ_strip.rs`, `organ_preset.rs`, `kb_zone.rs`: per-layer Organ state and mirror.
- `panel/piano_mirror.rs` (new), `piano_logic.rs`, `bay_piano_select.rs`, `bay_piano.rs`, `bay_synth/strip.rs`: Piano mirror, ACOUSTICS gating, layer LED blink.
- `nord-core`: `engine::zone_leds_for`, per-layer percussion in `preset_load.rs` and `program_export.rs`; `OrganEdit::{Drawbars, Percussion}`.
- Docs: `visual-style-and-panel.md` 4.2a, `organ-engine.md` 3.3.0, `OPEN-QUESTIONS.md` OQ31.
