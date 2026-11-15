# Piano transcription of the reference videos: real velocities and pedal for the comparisons

- **Checked:** 2026-11-15
- **Sources (local, never committed):** Part A demo video (`_zksT69H30c`, 41 min), the six DSoundman course videos and the six Audible Genius clips under `.reference/`, the factory backup `Nord Stage 4 Factory Restore RevK v1.66.ns4b`; code `tools/transcribe_clip.py`, `tools/midi_events.py`, `tools/clip_flags.py`, `tools/emu_events.py`, `tools/emu_env.py`, `tools/transcribe_agree.py`; results [emu_vs_hw_piano_transcribed.json](data/emu_vs_hw_piano_transcribed.json); earlier work [piano-note-velocity](piano-note-velocity.md), [piano-felt-upright-soft](piano-felt-upright-soft.md), [smp-slew](smp-slew.md), [piano-receiver-scale](piano-receiver-scale.md), [dsoundman-course-organ-piano](dsoundman-course-organ-piano.md).
- **Question:** The comparisons replayed the notes at one uniform velocity (88) and without the sustain pedal. What do real per-note velocities and pedal change in the Velvet Grand, Felt Upright, White Grand and Pearl Upright comparisons, and is the Felt Upright velocity-to-timbre mapping at fault?

Tags follow `.agents/rules/provenance-in-code.md`. The transcriber output is an **estimate** (section 5). Only numbers are committed: note, velocity, pedal and per-second flag files; no audio.

## 1. Method and tool versions

| Item | Value |
|---|---|
| Transcriber | [Transkun](https://github.com/yujia-yan/transkun) 2.0.1 (MIT), pretrained `2.0.pt`; notes with onset, offset, velocity (1..127) and sustain pedal CC 64 |
| Environment | isolated venv outside the repo (`C:\tmp\ns4-transcribe\venv`), Python 3.12.11, torch 2.6.0+cu124, RTX 4070 SUPER (CUDA); transkun installed with `--no-deps` plus its dependencies, **except `ncls`** (no wheel, needs MSVC; transkun imports it only in its training data module `transkun/Data.py`, never on the inference path) |
| Audio | `ffmpeg` 8.0: 44.1 kHz mono 16-bit PCM of the whole soundtrack; Transkun segments it internally (41 min in about 5 min on the GPU) |
| MIDI to events | `tools/midi_events.py` (standard library SMF parser: tempo map, running status, velocity-0 note-off) writes `{"events": [{midi, onset_s, offset_s, velocity, snr_db}], "pedal": [{t, value}]}`; `ns4 render-patch --events` now also reads the optional top-level `pedal` list as CC 64 (`render.rs`, test `test_parse_note_events_pedal`) |
| Replay | `python tools/emu_vs_hw.py --events-from transkun` slices the stream to each segment (times re-based to 0, pedal state carried in) and renders every variant; a variant with its own `velocity` (`vel-88`) drops the per-note velocities (uniform, pedal kept) |
| Envelope metrics | `tools/emu_env.py`: attack, hold (peak to -6 dB), release (-20 dB after key-up) medians and the level-versus-velocity slope (dB per velocity unit) of hardware and emulator |
| Not installed | `basic-pitch` (the Audible Genius clips hold at most 5 transcribed notes, nothing to compare) and any other model |

Outputs (all in `docs/research/data/`): `piano_events_<video id>.json` (full-length stream in media time, `program_ranges` for Part A) and `piano_flags_<video id>.csv` (one row per second: program, class, RMS, notes, audio support, `piano_like`). Part A is `piano_events__zksT69H30c.json` (14,991 notes, 1,010 pedal events). Slice any window with `python tools/transcribe_clip.py slice <stream> --at S --duration D --out file.json`, then `ns4 render-patch ... --events file.json`.

| Video | Notes | Pedal events |
|---|---|---|
| Part A demo `_zksT69H30c` | 14,991 | 1,010 |
| DSoundman: Soft Piano `NVMtX0epXog` / Soft Grand `0-upeS5SzbE` / Grand + String `Rq7N88wLKRw` / Piano + Synth Pad `hR31DQrxfT4` / Beginners `9OQdETpuIE8` / Preset Library `OWQHkuJNDQ4` | 2,027 / 1,439 / 1,369 / 1,949 / 2,286 / 754 | 428 / 270 / 288 / 460 / 254 / 190 |
| DSoundman drawbars `SuilmCpKeps` (organ: junk) | 498 | 32 |
| Audible Genius (`3zlrgiT9nIQ`, `qde8OVkL6RI`, `AdtsvmaqaVY`, `0NMvKWDc-Lo`, `s0OhmMpbscU`, `MjkV3qz8Gqk`; synth sounds) | 2, 5, 0, 5, 0, 1 | 0 to 2 |

The DSoundman `.mp4` of the synth LFO lesson was not transcribed (no piano).

## 2. Which stretches are piano (flags)

Transkun reports notes for any audio, so organ, synth and speech passages give plausible junk. Two cheap flags (`tools/clip_flags.py`):

1. **Program class** (ASSUMED: from the program name only, not from the decoded engine mix; `Grand Pad` and `Bright Felt` may carry pads): piano / ep / organ / other, per program range of `video_part_a_programs.json`. Part A: 2,462 s labelled, 324 s piano-class, 308 s ep, 806 s organ, 1,018 s other.
2. **Audio support** `snr_db` per note: the spectral peak at the note's fundamental (+-40 cents) over the median magnitude in `[f0 / 1.5, 2 f0]`, 250 ms after the onset (EMPIRICAL threshold 8 dB). A window is `piano_like` when the class is piano and at least half of its notes are supported (a video without a program list uses the support alone).

Piano-class ranges in Part A (`program_ranges`; piano-like seconds equal the range length except where noted):

| Program | Video range (s) | Notes | Piano-like s |
|---|---|---|---|
| A:13 Felt Upright | 146 to 234 | 402 | 88 |
| A:21 White Grand | 426 to 462 | 228 | 36 |
| A:23 Pearl Upright | 500 to 544 | 252 | 44 |
| A:41 Bright Imperial | 1128 to 1138 | 55 | 10 |
| A:43 Grand Pad | 1168 to 1204 | 141 | 36 |
| A:51 Bright Felt | 1406 to 1428 and 1684 to 1700 | 117 + 80 | 22 + 16 |
| A:61 Velvet Grand | 1706 to 1739 | 220 | 33 |
| A:62 Shimmer Upright | 1739 to 1778 | 74 | 39 |

The audio-support flag does not tell organ from piano when the pitch is right (an organ chord is "supported"), so the class is the real separator; treat the class as a hint and listen before trusting a stretch.

## 3. Velocities of the played notes (the transcriber estimate)

| Clip | Notes | Velocity p10 / median / p90 |
|---|---|---|
| Velvet Grand 1708 to 1733 s | 188 | 31 / 50 / 68 |
| Felt Upright 152 to 160 s (the passage the user asked about) | 38 | 16 / 36 / 56 |
| Felt Upright 150 to 180 s | 144 | 24 / 43 / 73 |
| White Grand 430 to 460 s | 212 | 43 / 65 / 93 |
| Pearl Upright 505 to 535 s | 182 | 35 / 51 / 79 |

The performers play **soft**: a uniform 88 (and the previous GUI 100) is far above the median of every clip except White Grand.

## 4. Comparisons at the transcribed velocities (VERIFIED, run; ns4 built from this branch)

Deltas are emulator minus hardware: band deltas relative to each signal's total (dB), RMS absolute (dB, includes the unknown recording gain and the YouTube loudness); the 8 to 20 kHz band is the YouTube low-pass and is not scored. `default` and the other unlabelled rows use the per-note velocities and the pedal; `vel-88` replays the same notes at a uniform 88 (pedal kept).

### 4.1 Velvet Grand (`piano_dry`, 188 notes, 25 s)

| Variant | low 120-400 | mid 0.4-1.5k | pres 1.5-3k | brill 3-8k | RMS | attack hw / emu ms | hold hw / emu ms | slope hw / emu dB per vel |
|---|---|---|---|---|---|---|---|---|
| default, transcribed | +0.7 | +0.6 | +4.5 | +8.0 | -1.6 | 64 / 54 | 1.6 / 1.7 | 0.127 / 0.128 |
| `--firmware-piano-chain` | +0.7 | +0.6 | +4.5 | +8.0 | +1.0 | 64 / 54 | | |
| `--piano-slew-fw` | +1.1 | -2.1 | +2.9 | -1.0 | -7.8 | 64 / 121 | | 0.127 / -0.010 |
| chain + slew | +1.1 | -2.1 | +2.9 | -1.0 | **-5.2** | 64 / 121 | | |
| `--legacy-piano-level` | +0.1 | -0.1 | +2.4 | +3.7 | +3.4 | 64 / 50 | | 0.127 / 0.054 |
| uniform 88, default | -0.3 | -0.7 | +3.1 | +2.6 | +5.9 | 64 / 50 | | |
| uniform 88, chain | -0.3 | -0.7 | +3.1 | +2.6 | +8.5 | | | |
| uniform 88, chain + slew | +0.3 | -2.2 | +3.2 | +1.7 | -3.0 | 64 / 117 | | |

Reading:

- **The +9.7 dB default excess was a velocity artefact.** Earlier numbers were at a uniform velocity 100 (default +9.7, chain +12.3, [piano-receiver-scale](piano-receiver-scale.md) s5). At the transcribed velocities (median 50) the default is **-1.6 dB** (chain +1.0), inside the 3 dB target of D22, and at uniform 88 it is +5.9 dB. The level gap of the old comparisons was mostly the replay velocity, not the engine. Caveat: Transkun's velocity is calibrated on concert-grand recordings, so this agreement is partly a statement about its scale (section 5).
- **The emulator level law matches the hardware per-note law.** The least-squares slope of each note's 300 ms peak level on its velocity is 0.127 dB per velocity unit on the hardware and 0.128 on the default emulator (White Grand 0.114 / 0.116, Pearl 0.080 / 0.082, Felt 0.133 / 0.159). The velocities come from the hardware audio, so a wrong emulator law would show; the opt-in `--legacy-piano-level` (0.054) and `--piano-slew-fw` (about 0) do not follow it.
- **Spectrum:** low and mid within 0.7 dB; presence +4.5 and brilliance +8.0: the loud notes of the default render are brighter than the recording (the 3 to 8 kHz band of the hardware recording is also reduced by the codec), while uniform 88 gives +2.6. Brightness versus per-note velocity is therefore the open item, not the level.
- **Chain + slew** lands at **-5.2 dB** RMS (was -2.2 dB at uniform 88; -3.0 here at uniform 88) with a **121 ms attack against 64 ms** on the hardware; it stays opt-in. Per-note velocities do not rescue it (the slew levels do not follow the velocity: slope -0.01).

### 4.2 Felt Upright (`piano_felt_tr`, 144 notes, 150 to 180 s)

| Variant | low | mid | pres | brill | RMS | attack hw / emu ms | release hw / emu ms |
|---|---|---|---|---|---|---|---|
| default, transcribed | +1.1 | +0.6 | +2.1 | +1.6 | -3.8 | 56 / 50 | 961 / 304 |
| chain | +1.1 | +0.6 | +2.1 | +1.6 | -1.1 | | |
| chain + slew | +2.2 | +2.0 | -3.5 | -8.0 | -9.2 | 56 / 99 | 961 / 431 |
| uniform 88, default | +1.5 | -0.2 | +0.4 | -4.8 | **+4.9** | 56 / 56 | |
| uniform 88, chain + slew | +1.5 | +1.2 | -0.3 | -5.9 | -5.9 | 56 / 101 | |

**The hardware is softer because the notes are played soft, not because of the velocity-to-timbre mapping.** At the transcribed velocities (median 43; 36 in the 152 to 160 s passage) the emulator matches the hardware spectrum within 2.1 dB in every scored band and the level within 3.8 dB (emulator quieter). The same notes at the uniform velocity 88 that the GUI keybed now plays (D76) are **4.9 dB louder** than the hardware (against -3.8 dB at the played velocities, an 8.7 dB difference between the two replays), and the uniform replay misses the dynamics (a flat velocity has none of the soft notes that make the passage sound gentle). So the user's perception that the hardware sounds softer is explained by the performance velocity (median about 43) against a GUI that plays 88; the sampler's velocity layers and the "Soft" timbre (Off in this program) are not at fault. A GUI mouse velocity near 45 would sound like the video. The measured release (about 0.3 s against 1.0 s on the hardware) is the pedal and room of the recording, still open (OQ20).

### 4.3 White Grand and Pearl Upright

| Segment, variant | low | mid | pres | brill | RMS | attack hw / emu ms |
|---|---|---|---|---|---|---|
| White Grand (A:21, 212 notes) default | -1.1 | -1.8 | -1.0 | +0.0 | -1.7 | 58 / 43 |
| White Grand chain + slew | +2.5 | -0.2 | +0.4 | -3.6 | -9.2 | 58 / 114 |
| White Grand uniform 88 | -0.8 | -2.9 | +0.2 | -2.1 | +1.3 | 58 / 47 |
| Pearl Upright (A:23, 182 notes) default | -1.4 | 0.0 | +0.7 | -0.1 | -2.5 | 58 / 43 |
| Pearl Upright chain + slew | -0.8 | -1.0 | -4.9 | -10.1 | -6.8 | 58 / 93 |
| Pearl Upright uniform 88 | -0.4 | -0.9 | -1.1 | -1.6 | +4.7 | 58 / 42 |

Four independent clips agree: default at the played velocities is within -1.7 to -3.8 dB RMS (Velvet -1.6, White -1.7, Pearl -2.5, Felt -3.8) and within 2 dB in every scored band on three of them; chain + slew is 5 to 9 dB low with an attack twice as slow, in every clip. The opt-in firmware paths do not flip.

### 4.4 White Grand of the DSoundman course (`NVMtX0epXog`, 113 to 140 s)

`docs/research/data/emu_vs_hw_dsoundman_piano.json`, program `A:21 White Grand` with only the piano section enabled (ASSUMED stand-in for the course's Layer Init; the program FX stay on, the course had none). Speech mixes into the hardware 3 to 8 kHz band.

| Variant | low | mid | pres | brill | RMS | attack hw / emu ms |
|---|---|---|---|---|---|---|
| default, transcribed | -0.7 | -3.3 | -5.6 | -14.3 | -11.7 | 33 / 42 |
| chain + slew | -2.5 | -2.1 | -9.3 | -19.0 | -18.6 | 33 / 110 |
| uniform 88 | -0.3 | -4.0 | -5.4 | -15.5 | -8.1 | 33 / 42 |

The brilliance deficit is speech and codec noise in the recording (the piano alone has under -44 dB above 4 kHz, [dsoundman-course-organ-piano](dsoundman-course-organ-piano.md)); the absolute RMS is not comparable (this video's recording gain and the program's FX differ from the earlier -9.5 dB convention of that note), so no level conclusion is drawn from this clip. Clips with no usable notes: the Audible Genius clips (synth sounds, 0 to 5 transcribed notes) and the DSoundman drawbar clip (organ, junk); the other piano clips (`0-upeS5SzbE` Soft Grand XL, `Rq7N88wLKRw`, `hR31DQrxfT4`) are transcribed and available for slicing, but their sounds are not in the factory backup or carry layered sounds, so no render was compared.

## 5. Accuracy of the transcription and caveats

Agreement with the video-confirmed notes (`tools/video_ref.py notes`, notes whose key is seen going down; `tools/transcribe_agree.py`), same MIDI number and onset within the tolerance:

| Clip | Confirmed notes | Recall +-80 ms | Recall +-150 ms | Pitch-class recall +-150 ms | Precision (lower bound) +-150 ms |
|---|---|---|---|---|---|
| Velvet Grand | 169 | 0.25 | 0.38 | 0.52 | 0.34 |
| Felt Upright | 143 | 0.25 | 0.41 | 0.50 | 0.40 |
| White Grand | 197 | 0.26 | 0.42 | 0.49 | 0.39 |

- The Transkun onset is **late by a median of 50 to 70 ms** against the video key-down on same-pitch pairs (0.052, 0.070, 0.065 s), so the +-80 ms column undercounts; recall is **about 40 %** of the video-confirmed notes (about 50 % by pitch class) and precision against the confirmed notes at least 34 to 40 % (a lower bound: the video confirms only the visible part of the keyboard). The video detector is itself not ground truth (it was built from the same image analysis), so these numbers bound the agreement, they do not measure Transkun's error.
- Velocities are an **estimate**: Transkun predicts a loudness-like velocity trained on concert-grand recordings, not the Nord's MIDI velocity, and the hardware recording is lossy, compressed and reverberant. Treat them as ordinal (soft versus loud) and as a median of about 45 to 65 for these clips, not as exact values; the agreement of the level slopes (0.127 / 0.128) shows the two scales are consistent, not that they are equal.
- Overlapping notes, the pedal and layered programs (pads under the piano) produce spurious and merged notes; organ, synth and speech passages give junk (flags, section 2).
- A second transcriber (ByteDance `piano_transcription_inference`) was not run (not approved); it is the planned second opinion.

## Implications

- Comparisons of piano programs should replay the transcribed velocities and pedal (`--events-from transkun`); uniform 88 was a poor model of a soft performance, and the old +9.7 dB default excess was a replay-velocity artefact. The firmware paths (`--firmware-piano-chain`, `--piano-slew-fw`) still do not flip: slew costs 5 to 9 dB and doubles the attack time at every velocity.
- The Felt Upright "much softer on the hardware" is the played velocity (median about 43): no change to the velocity layers, timbre or level law is indicated. The keybed velocity 88 of D76 is still above the video's performance; see D79.

## Follow-ups

- Velocity-to-brightness at loud notes (presence +4.5, brilliance +8.0 on Velvet Grand): needs a hardware recording at known velocities (the single-note recording request of [piano-felt-upright-soft](piano-felt-upright-soft.md)).
- Run the planned second transcriber on the same clips and compare the velocity series.
- Part A programs other than the five compared (Bright Imperial, Grand Pad, Bright Felt, Shimmer Upright) can be sliced from the committed stream.
