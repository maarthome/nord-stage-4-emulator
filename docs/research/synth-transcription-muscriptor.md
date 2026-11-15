# MuScriptor as a second transcriber for synth clips

- **Checked:** 2026-11-15
- **Sources:** [MuScriptor paper](https://huggingface.co/papers/2607.08168) (arXiv 2607.08168, Kyutai / Mirelo AI / IRCAM), code [github.com/muscriptor/muscriptor](https://github.com/muscriptor/muscriptor) (MIT), weights [MuScriptor/muscriptor-small](https://huggingface.co/MuScriptor/muscriptor-small) / `-medium` / `-large` (CC BY-NC 4.0, gated: the HF account must accept the terms). Local, never committed: Part A demo video `_zksT69H30c`, the clips of `docs/research/data/emu_vs_hw_segments.json` `synth_horizon` (290 s, 30 s), `synth_leaky` (1275 s, 35 s) and `pianopad_grand` (1171 s, 30 s). Code `tools/muscriptor_events.py`, `tools/transcribe_agree.py`, `tools/video_ref.py notes`; results [muscriptor_agreement.json](data/muscriptor_agreement.json); earlier work [piano-transcription](piano-transcription.md).
- **Question:** Does MuScriptor recover the notes of the synth demo clips (Horizon Whl, Leaky Stabs II, Grand Pad) better than Transkun (about 40 % recall on piano, worse on synths), does it give per-note velocity, and can its output drive `emu_vs_hw.py --events-from`?

Tags follow `.agents/rules/provenance-in-code.md`. All scores are against the video-confirmed notes, which bound agreement and are not ground truth (see [piano-transcription](piano-transcription.md) section 5).

## 1. Method

| Item | Value |
|---|---|
| Install | isolated venv outside the repo (`C:\tmp\ns4-muscriptor`, Python 3.12, `uv pip install --torch-backend=cu124 muscriptor`), RTX 4070 SUPER; weights fetched from the gated HF repos with the user's existing HF login (small 0.41 GB, medium 1.23 GB; large 5.47 GB, added after the user approved it) |
| Licence | code MIT; weights **CC BY-NC 4.0**: fine for this non-commercial research use, weights stay out of git, results are derived numbers only |
| Audio | `ffmpeg` 44.1 kHz mono clip per segment |
| Run | `muscriptor transcribe --model {small,medium} -f json` (note start/end events with `instrument`), then `python tools/muscriptor_events.py convert ... --offset <at_s> --drop-instruments drums` |
| Truth | `video_ref.py notes` confirmed notes: Horizon 172, Leaky 288, Grand Pad 146 |
| Score | `tools/transcribe_agree.py` (same MIDI number, onset within tolerance, greedy one-to-one); Transkun is the committed stream `piano_events__zksT69H30c.json` sliced to the clip |

## 2. Results (VERIFIED, run once per configuration)

Recall / precision lower bound at +-150 ms (exact MIDI number); the +-80 ms and pitch-class numbers are in `data/muscriptor_agreement.json`. MuScriptor rows drop the `drums` label.

| Clip | Transkun 2.0 | MuScriptor small | MuScriptor medium | MuScriptor large | medium + `--instruments synth_lead,synth_pad,electric_bass` |
|---|---|---|---|---|---|
| Horizon Whl (172 confirmed) | 0.20 / 0.12 (275 notes) | 0.13 / 0.08 (291) | 0.19 / 0.09 (351) | 0.12 / 0.06 (326) | 0.14 / 0.08 |
| Leaky Stabs II (288) | 0.14 / 0.25 (162) | 0.17 / 0.30 (166) | 0.23 / 0.32 (203) | **0.24 / 0.24** (291) | 0.23 / 0.28 |
| Grand Pad (146) | 0.25 / 0.28 (127) | 0.24 / 0.27 (128) | 0.21 / 0.30 (105) | 0.25 / 0.31 (117) | 0.25 / 0.26 |

At +-80 ms recall: Transkun 0.145 / 0.115 / 0.178, medium 0.116 / 0.163 / 0.164 (Horizon / Leaky / Grand Pad).

- **Recall stays low for every model (0.13 to 0.25).** MuScriptor medium is on par with Transkun, better on Leaky (+9 points), worse on Grand Pad, equal on Horizon. It is not the hoped-for step up; the small model is slightly worse than medium and the **large model (1.4B) is no better than medium** (worse on Horizon, equal on Leaky and Grand Pad), so size does not help on synth timbres.
- **Same-pitch onset bias** against the video key-down (median over matched pairs): Transkun +46 / +23 / +34 ms, MuScriptor medium +15 / +57 / +9 ms. Both are late, as in the piano clips; neither is clean enough to retime events.
- **Instrument labels are guesses** (Horizon: electric_bass + string_ensemble, Leaky: distorted_electric_guitar + electric_bass, Grand Pad: acoustic_piano), so they do not identify the engine. The small model puts **1,476 `drums` notes** on Leaky (noise bursts): drop `drums` (precision 0.02 with them, 0.30 without). An `--instruments` hint changes counts, not recall in a consistent direction.
- **No velocity.** The model card states velocity and dynamics are not produced, the JSON events hold only pitch, start, end and instrument, and the MIDI output has a fixed velocity. No ordinal levels either. A sustain pedal is not produced.

## 3. Replay

`tools/muscriptor_events.py` writes the `{"events": [{midi, onset_s, offset_s, instrument}], "pedal": []}` document (media time via `--offset`, `--drop-instruments`, `--keep-instruments`, optional `--velocity N` stamp). It carries no `velocity`, so `render-patch` plays the notes at the variant velocity (`parse_note_events` default) and `note_env` reports no slope. Use it as `python tools/emu_vs_hw.py <spec> --events-from <stream.json>` (the path form already exists; `tools/emu_events.py` slices it per segment, test `test_stream_slices_for_emu_vs_hw`). To keep real dynamics, merge Transkun velocities by nearest onset, not done here.

## Implications

- MuScriptor does not replace Transkun for the synth clips: recall of both is 0.13 to 0.25 at +-150 ms. Use the two as independent opinions and trust only notes both find (not measured here).
- For a velocity-bearing replay Transkun stays the only source; MuScriptor can only supply extra onsets.
- `drums` output is noise on synth material; always drop it.

## Follow-ups

- Union / intersection of Transkun and MuScriptor notes against the video-confirmed set (does the intersection reach useful precision?).
- Aria-AMT / ByteDance piano transcription for the piano clips.
- Emulator-known events (a rendered MIDI with known notes) would give a true ground truth; none exists for the demo performance.
