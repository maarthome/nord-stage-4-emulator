# nord-lv2 — LV2 instrument (Linux, Raspberry Pi 5)

An LV2 build of the Nord Stage 4 Emulator engine (`nord-core`) for Linux hosts such as Carla,
Ardour, jalv, mod-host and Zynthian, including aarch64 / Raspberry Pi 5 on Debian trixie.

It is a thin wrapper. The LV2 C ABI is declared by hand, so it needs no nih-plug, egui, X11 or LV2
headers, and the only dependency is `nord-core`.

## Build and install

```bash
crates/nord-lv2/install-lv2.sh
```

This builds `target/release/libnord_lv2.so` and copies it, with the TTL files from
`bundle/nord-stage4.lv2/`, to `~/.lv2/nord-stage4.lv2/`. Set `LV2_DEST` to install somewhere
else, for example `LV2_DEST=/usr/local/lib/lv2` (that path needs sudo).

Quick test with jalv (`sudo apt install jalv lilv-utils`):

```bash
lv2info 'https://github.com/ymatrix1/nord-stage-4-emulator#lv2'
jalv 'https://github.com/ymatrix1/nord-stage-4-emulator#lv2'
```

## Ports

| Index | Symbol | What |
|---|---|---|
| 0 | `midi_in` | MIDI in. Channel 1 plays the layers that are switched on; CCs follow the Nord Stage 4 CC map (`nord_core::midi::cc`) |
| 1–2 | `out_l`, `out_r` | Main outputs 1-2 |
| 3–4 | `aux_l`, `aux_r` | Outputs 3-4 (optional; silent unless the engine routes something there) |
| 5 | `master` | Master level, −60 to +6 dB (−60 = silent) |
| 6–12 | `*_on` | Layer on/off (Scene I): Organ A, Organ B, Piano A, Piano B, Synth A, B, C |
| 13–19 | `*_level` | Layer levels 0–1, same order |

The defaults match a fresh engine: Organ A, Piano A and Synth A on, all levels at 0.8. A control
port is written to the engine only when its value changes, so MIDI CCs that move the same state
(layer levels, Scene II) keep working between knob moves.

## Real-time behaviour

`run()` never allocates, locks or does I/O. MIDI events are applied at their exact frame offset,
and blocks of any size are rendered in 256-frame chunks through `process_block_multi`.
`activate()` rebuilds the engine.

## Not yet included

- No custom GUI: hosts show their generic control UI.
- No program / preset (`.ns4p`) loading or LV2 state yet; sounds start from engine defaults.
- No host tempo (`time:Position`) yet; the master clock runs at its default BPM.
- The WAVE wavetables are firmware and are not included (see the main README).
