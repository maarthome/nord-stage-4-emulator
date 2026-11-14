# Kontakt .nki preset stream (LZ77) and the UP2 KSP script

Checked: 2026-11-14
Sources: user-saved `Ultimate Pianos 2.1.1 Yuri Total.nki` (Kontakt 8.4.0.0, not library-protected, `.reference/`, git-ignored); the `ni-file` project's notes and `deflate.rs`/`cb.rs`/`offset.rs` (github.com/gitter-badger/ni-file); `tools/nki_lz.py`; `tools/tests/test_nki_lz.py`.
Question: can the instrument data (script, groups, effects) of a saved `.nki` be read without opening Kontakt?

## Container

`hsin` blocks: `u64 length, "hsin", 16-byte checksum, u64 data length, data, u32 children, u32 type id, embedded blocks`. The first block length equals the file size. In this file the compressed preset stream starts at offset 2490 (control byte of the literal run holding `on init`) and ends at the footer `hsin` block (offset 1003466).

## Stream tokens

| Control byte | Meaning |
|---|---|
| `0x00..0x1F` | literal run of `1 + (c & 0x1F)` bytes |
| `0x20..0xDF` | copy, length 3..8 (`3 + ((c - 0x20) >> 5)`), offset `((c & 0x1F) << 8) + next + 1` |
| `0xE0..0xFF` | copy, length `9 + extension bytes` (a `0xFF` byte means another extension byte follows), offset as above |
| offset == 8192 | two more bytes (big endian) follow; real offset = `8192 +` that value |

The first rule set is from the `ni-file` notes. The 8192 escape and the `0xFF` length continuation are not in those notes; both were found because the decode went wrong at exactly those tokens, and they fixed it (script text turned from garbage into clean KSP).

## Evidence (VERIFIED)

- `uv run python tools/nki_lz.py <nki> out.bin --script out.ksp` decodes 12,194,606 bytes and consumes the stream exactly to the footer (decoder raises if it does not).
- The leading 180,766 bytes are one run of clean ASCII (KSP script); `on init` ... `on persistence_changed`.
- `uv run pytest tools/tests/test_nki_lz.py`: 8 passed (token classes, `ni-file` vectors, overlapping copy, `0xFF` continuation, far-offset escape, end-of-stream check).

## What the script shows (UP2, Yuri Total)

Handlers: `on init`, `on note`, `on release`, `on controller`, `on persistence_changed`, 151 `on ui_control`.

| Item | Finding |
|---|---|
| Velocity | `on note` remaps `$EVENT_VELOCITY` per slot: negative mode `v - (127-v)*v/T[k]`, positive `v + (127-v)*v/T[k]`, mode 9 forces 127, and `v > 63` becomes `32 + v/2` for some slot types; the table `T` (`%awued`) is in the instrument data. |
| Release samples | `on release` plays a release layer at volume `v_rel = v0 * 10^(-96/60 * x)`, `x = ((t - 20) / T)^p` with `t` the held time in ms, `T = T_lo * (T_hi/T_lo)^((note-21)/87)`, `p = %sgof3[..] / 100`; event volume -4500 mdB. |
| Pedal | `on controller` fades `MARK_5..16` groups on CC64 < 64 and rebuilds release notes for keys no longer held. |
| Effects | engine parameters used: choral (chorus), phasis (phaser), flair (flanger), compressor (threshold, decay, shape), 3-band EQ (gain1..3, freq2), reverb delay (RDL time, ping-pong, feedback), insert output gain, send levels 0 and 1. UI labels `Reverb Amount [A..D]`, `FX A Rate [C]`. |

Numeric values of those parameters live in the instrument tables after the script (not yet parsed).

## UI control map (tools/ksp_ui_map.py)

`on ui_control($v)` handlers pair a control variable with the engine parameter it drives, a label and a display formula. Saved control values are `$name value` pairs stored after the script. `uv run python tools/ksp_ui_map.py nki_decoded.bin 180766 ui_map.csv` found 24 engine-parameter controls, all 24 with a saved value (`uv run pytest tools/tests/test_ksp_ui_map.py`: 4 passed).

| Engine parameter | Controls | Saved values (Kontakt units) |
|---|---|---|
| `SENDLEVEL_1` (label "Reverb Amount" A..D) | 4 | 182688, 318372, 182688, 318372 |
| `SENDLEVEL_0` ("Delay Amount" A..D) | 4 | 0, 0, 0, 629961 |
| `RDL_TIME` / `RDL_PINGPONG` / `RDL_FEEDBACK` (reverb delay) | 3 | 160, 1, 339743 |
| `ATTACK`, `RELEASE` ("Original") | 2 | 0, 0 |
| `EFFECT_BYPASS`, `THRESHOLD`, `COMP_DECAY`, `SHAPE`, `GAIN1..3`, `FREQ2` (per-bus compressor, EQ, effect bypass) | 11 | see CSV |

Reverb, delay and chorus engine parameters (`CHORAL_*`, `PHASIS_*`, `FLAIR_*`) outside `on ui_control` are set from procedures and are not covered by this map. Send-level display text is `v*10/62996/10`; the unit behind that scale is unconfirmed. Saved values are the saved state of the instrument, not necessarily the factory default.

## Zones and sample file list (tools/nki_zones.py)

The decoded stream holds 26,615 zone records after the script (226 bytes, 462 of 254 bytes) and a sample file list of 27,427 entries. Fields (offsets from the 12-byte signature `FF*7 00 01 00 00 00`): zone id @44, velocity low/high @64/@66, key low/high @68/@70, root key @80, file index @100 (name = list entry `index - 7`, EMPIRICAL base), sample rate @108, frame count @113 (unaligned).

| Check (`nki_zones.py ... --catalog catalog.csv`) | Result |
|---|---|
| Zone root key equals note in file name and velocity high equals velocity in name | 26,405 of 26,405 parseable names match, 0 differ (210 names carry no velocity) |
| Zone frame count equals measured WAV frame count (Kontakt WAV export) | 26,405 equal, 0 different |
| Velocity layers per set and root tile 1..127 without gap or overlap | 4,116 of 4,154 stacks; the 38 others contain duplicate zones with the same range (stacked layers) |
| Key ranges of the lowest velocity layer tile the keyboard per set | 299 of 302 sets; 3 sets miss one key (`Concert Grand Ambient DN`, `Studio Grand 1 DN` at 90, `Amber Upright DN` at 92) |

No group id exists inside the zone record (the only other varying bytes are 2-value flags), so group membership, per-group envelopes, filters and effect chains are not parsed yet.

## Groups (tools/nki_groups.py)

181 group records sit between the script and the zone array, each starting with `u32 n, UTF-16 name` (`astoria .grand #dn`) and about 4850 bytes long. Suffixes: `#dn` 28, `#up` 84, `#xx` 28, `#ko` 41. Each record holds length-prefixed ASCII identifiers: parameters (`volume`, `pan`, `pitch`, `frequency`, `intensity`, `filterCutoff`, `ahdsr_attack`, `ahdsr_release`) and modulators (`ENV_AHDSR_VOLUME`, `LFO_SAW_TREMOLO`, `LFO_SAW_AUTOPAN`, `LFO_TRI_PITCH`, `ENV_AHDSR_PITCH`, `VEL_VOLUME`, `KP_VOLUME`, `KSP_*`, `KP_*`).

| Finding | Evidence |
|---|---|
| Group volume is the 3rd `volume` identifier's float | EMPIRICAL: the only occurrence that varies across groups; 0.780274 for all 28 `#dn`, 28 `#xx` and 84 `#up` groups, 0.840287 for all 41 `#ko` groups (release groups differ by a fixed amount). The dB meaning of the normalized value is not confirmed (ASSUMED: 0.78 is 0 dB). |
| Same modulator set everywhere | 187 each of `ENV_AHDSR_VOLUME`, `LFO_SAW_TREMOLO`, `LFO_SAW_AUTOPAN`, `LFO_TRI_PITCH`, `ENV_AHDSR_PITCH`, `VEL_VOLUME`, `KP_VOLUME`, `KSP_VOLUME`, `KSP_CUTOFF` across the 181 groups (a few groups carry extra blocks); per-group differences were not compared. |
| Zone to group membership | Not resolved. Zones are stored in group-like runs and a greedy suffix match (`#dn`/`#up`/`#xx`/`#ko` against `DN`/`UP`/`KO` sample names) lines up only the first 107 groups (grands and uprights), but electric pianos, digital sets and pads do not follow the group order, and the group names differ from the sample set names for some sets. No group id exists inside the 226-byte zone record. |

Envelope times, LFO rates, modulation intensities and per-group effect values are stored as floats after these identifiers but are not decoded yet.

## Not verified / not done

- Group envelope times, LFO rates, modulation intensities, effect chains and values, `ir_samples` convolution references, and zone-to-group membership are not decoded. Zones, the file list, group names, group volume and modulator inventory are (see above).
- Kontakt versions other than 8.4 and protected libraries: untested.
- Compression runs ~12x (1 MB to 12 MB); decode time ~1 s.

## Implications

Velocity shaping and release-sample behaviour for the UP2 oracle can be taken from the script instead of screenshots. The effect parameter values still need the tables or Kontakt screenshots.

## Follow-ups

- Parse the instrument tables after the script (groups, zones, per-group envelopes, effect chains, `%awued`-style arrays).
- Confirm the formulas above by comparing against the WAV catalog (`tools/up2_oracle.py`).
