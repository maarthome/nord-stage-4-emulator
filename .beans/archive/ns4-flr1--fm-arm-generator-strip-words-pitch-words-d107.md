---
# ns4-flr1
title: FM ARM generator, strip words, pitch words (D107)
status: completed
type: task
priority: normal
created_at: 2026-11-15T19:55:47Z
updated_at: 2026-11-15T20:11:45Z
---

**Context**
FM audio-remaining row 10: strip +0xA4/+0xA8 writer, 361-word generator, DM(0xC2716) writer, position word, operator ratios.

**Higher Goal**
Close FM with ARM and SHARC evidence only; wire verified words into FmVoice opt-in.

**Acceptance Criteria**
- [x] Generator ported (Python + Rust), 30/30 variants bit exact
- [x] kOscWave handler writer of +0xA4/+0xA8 found, FmVoice::select
- [x] DM(0xC2716) DSP writer, pitch octave correction (MIDI 60, f/F_SY)
- [x] No hardware FM clip exists, stated
- [x] select CSV landed, tests, gates, PR

**Out of Scope**
Default flip into SynthLayer (D22); level vs hardware clip.

**Summary of Changes**
See PR.
