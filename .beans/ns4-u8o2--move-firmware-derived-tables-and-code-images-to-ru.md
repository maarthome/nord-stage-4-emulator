---
# ns4-u8o2
title: Move firmware-derived tables and code images to runtime loading
status: todo
type: task
created_at: 2026-11-17T04:06:20Z
updated_at: 2026-11-17T04:06:20Z
parent: ns4-trkb
---

**Context**
D128 classifies tracked firmware-derived data (docs/research/firmware-derived-data-inventory.md). Classes B (code images) and C (large tables) are still committed.

**Higher Goal**
Repository holds only our own work; firmware data is read from the user OS update file at runtime.

**Acceptance Criteria**
- [ ] Loader for core DSP images via FirmwareContainer::extract_segment
- [ ] Follow-ups 1 to 4 of the inventory migrated, tests use synthetic fixtures
- [ ] Default render without an image has a documented fallback

**Out of Scope**
History rewrite (owner decision).

**Summary of Changes**
(pending)
