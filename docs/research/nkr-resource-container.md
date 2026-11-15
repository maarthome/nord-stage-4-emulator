# Kontakt `.nkr` resource container (UP2 GUI pictures)

Checked: 2026-11-15
Sources: `Ultimate Pianos 2.nkr` (42,674,951 bytes, user's own library, local only under `.reference/`, git-ignored); `tools/nkr_extract.py`; `tools/tests/test_nkr_extract.py`.
Question: can the GUI pictures (buttons, sliders, backgrounds) of the UP2 library be extracted without Kontakt?

## Layout (VERIFIED on this file)

Not encrypted. A directory of nodes comes first, then the data blocks.

| Item | Layout |
|---|---|
| Node | `u16 size, u32 offset, u16 type, UTF-16 name, u16 0`; `size = 2 + 4 + 2 + 2*chars + 2` |
| Type 1 | folder: `Resources`, `pictures`, `scripts`, `data`, `ir_samples`, `performance_view` |
| Type 4 | file: `offset` is its data block |
| Data block | 22-byte header (`FA 05 E9 2A`, `u16 0x0111`, 4 zero bytes, `FF 00 00 00`, `u32 size` at +14, 4 zero bytes) then `size` payload bytes; the next block starts at `offset + 22 + size` |

Result of `uv run python tools/nkr_extract.py <nkr> <out_dir>`: 298 files = 149 PNG + 149 `.txt` sidecars (one per picture: alpha channel, animation count, horizontal / vertical animation, resize flags, fixed margins). Every PNG parses (IHDR width and height in `manifest.csv`). The folders `scripts`, `data`, `ir_samples` and `performance_view` are empty in this file, so the reverb impulse responses are not here (they are in the sample `.nkx`, not read). `uv run pytest tools/tests/test_nkr_extract.py`: 7 passed.

PNG groups by file-name prefix: 92 buttons `bt_*`, 30 sliders `sl_*`, 17 keyboard `kb_*`, 6 backgrounds `bg_*`, 2 fonts `font_*`, 1 label `lb_*`, 1 `icon`. 145 of the sidecars declare more than one animation frame, so a button PNG is a strip of states (off, on, hover) stacked vertically or horizontally per the `.txt`.

`up2_bg_bottom.PNG` (633 x 399) is the panel background with flat color-keyed rectangles where controls sit: the layout of the panel, not a finished screenshot.

## Not verified / not done

- Only this one `.nkr` was read; other libraries may use other types or headers (ASSUMED).
- Folder to file membership (the 6-byte child headers after the folder nodes) is not parsed; names are flat.
- Which control each picture belongs to is in the KSP script (`set_control_par_str(..., $CONTROL_PAR_PICTURE, "up2_bt_...")`, 552 `up2_` references), not mapped here.
- The pictures are the library author's own artwork and stay local; they are not committed to the repo.

## Implications

A GUI skin or a control-by-control mock-up of the UP2 panel can be built from these strips, using `ksp_ui_map.py` for control positions and labels.

## Follow-ups

- Map control to picture and position from the script (`set_control_par`, `move_control`).
- Locate the reverb IRs in the `.nkx` (`ir_samples` is empty here).
