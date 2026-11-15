---
# ns4-9tpa
title: Keybed mouse velocity from hit position
status: completed
type: task
created_at: 2026-11-15T12:07:03Z
updated_at: 2026-11-15T12:07:03Z
---

## Context
D76 fixed mouse velocity at 88; hardware Felt Upright played near 43.

## Higher Goal
Audio 1:1 with hardware.

## Acceptance Criteria
- [x] Velocity from hit position, 20..127
- [x] Tests, D84, GUI spec

## Out of Scope
nord-core velocity curves.

## Summary of Changes
New keybed_velocity.rs, keybed.rs uses it, D84, spec.
