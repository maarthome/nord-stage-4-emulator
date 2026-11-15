---
# ns4-g4lw
title: Verify ASSUMED constants from the firmware
status: completed
type: task
created_at: 2026-11-15T01:19:19Z
updated_at: 2026-11-15T01:30:00Z
---

## Context

291 `ASSUMED` mentions in `crates/nord-core/src`. The ARM firmware holds parameter laws and choice lists as constants and display strings; the ARM harness and static disassembly read them without hardware.

## Higher Goal

1:1 audio parity: replace guesses by firmware facts, tag every constant `VERIFIED` / `EMPIRICAL` / `ASSUMED` with source and reproduce command.

## Acceptance Criteria

- [x] Audit the audible `ASSUMED` constants and record what was verified and what was tried ([assumed-constants-audit](../docs/research/assumed-constants-audit.md)).
- [x] Rotary Horn / Rotor Speed and Acceleration choices from the ARM handlers (`organ/rotor_choices.rs`, `tools/arm_leslie_settings.py`).
- [x] Pitch Stick Range and Vibrato Delay lists from firmware strings (`tools/ns4_fw_choices.py`).
- [x] Tests for every changed constant; checks of `AGENTS.md` pass after the last edit.

## Out of Scope

Glide time law (DSP reader not located), Electric timbre gains, FX Mod 1 rate law (OQ21), velocity curve selector.

## Summary of Changes

See [assumed-constants-audit](../docs/research/assumed-constants-audit.md) and D65.
