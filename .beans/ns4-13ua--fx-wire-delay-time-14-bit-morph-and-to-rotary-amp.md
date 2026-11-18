---
# ns4-13ua
title: 'FX: wire delay-time (14-bit) morph and To Rotary amp routing'
status: todo
type: task
created_at: 2026-11-12T01:46:47Z
updated_at: 2026-11-12T01:46:47Z
parent: ns4-trkb
---

## Context
delay_rate morph and Amp mode To Rotary are decoded but not applied.

## Acceptance Criteria
- [ ] delay_rate morph in FxMorphSet with unit test
- [ ] To Rotary routes amp output into organ rotary or documented as unsupported

## Out of Scope
Firmware table work.

## Progress 2026-11-18
The firmware ports for these FX exist (opt-in `FirmwareFx`). The ranked-distance evidence for flipping them (D163, `docs/research/program-ranking-fx.md`): `FirmwareFx::PARITY` takes the 384-program mean score from 5.35 to 1.66; default flip proposed, owner decision pending.
