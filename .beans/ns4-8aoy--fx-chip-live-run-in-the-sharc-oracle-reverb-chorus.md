---
# ns4-8aoy
title: FX chip live run in the SHARC oracle (reverb, chorus)
status: in-progress
type: task
priority: normal
created_at: 2026-11-17T03:05:44Z
updated_at: 2026-11-17T03:05:53Z
---

**Context**: FX routines were checked one at a time by direct call, never as the chain on live audio.

**Higher Goal**: audio 1:1 with hardware via the SHARC oracle; FX checked on live input against the native models.

**Acceptance Criteria**
- [x] sharc-fxlive runs the FX Core1/Core2 pass on a stereo signal and collects output, tap and entry registers
- [x] native counterpart fx_live_native and comparison tools/fx_oracle_live.py with tests
- [x] chain transfer verified (0.1866, lag 0, -157.6 dB)
- [ ] reverb and chorus oracle response equal to the native ports (blocked, OQ35)
- [ ] PNO lane WAV as FX input

**Out of Scope**: synth, organ, piano models, JIT/AOT, default flips.

**Summary of Changes**: see docs/research/fx-oracle-live.md and D123.
