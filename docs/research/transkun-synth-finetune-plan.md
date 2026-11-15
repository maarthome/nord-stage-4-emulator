# Transkun adapter for synth clips: feasibility and plan (LoRA vs LoKr vs full)

- **Checked:** 2026-11-15
- **Sources (local, never committed):** installed Transkun 2.0.1 in `C:\tmp\ns4-transcribe\venv` (`transkun/ModelTransformer.py`, `LayersTransformer.py`, `Data.py`, `train.py`, `pretrained/2.0.conf`, `pretrained/2.0.pt`, `transkun-2.0.1.dist-info/LICENSE`, `METADATA`), upstream [github.com/Yujia-Yan/Skipping-The-Frame-Level](https://github.com/Yujia-Yan/Skipping-The-Frame-Level) (named in the package metadata; **not cloned**, the installed wheel is the source), ISMIR 2024 paper arXiv 2404.09466; `target/release/ns4.exe` (`render-patch`), `.reference/` factory backup `Nord Stage 4 Factory Restore RevK v1.66.ns4b`; earlier work [piano-transcription](piano-transcription.md), [synth-transcription-muscriptor](synth-transcription-muscriptor.md), [emu_vs_hw_segments.json](data/emu_vs_hw_segments.json); probe scripts in `C:\tmp\ns4-ft-probe` (`inspect_model.py`, `bench_step.py`, `lora_count.py`, `peft_check.py`, listed in section 9).
- **Question:** Can a small adapter on Transkun, trained on deterministic emulator renders of synth programs, raise note recall and give per-note velocity on hardware synth clips (Horizon Whl, Leaky Stabs II, Grand Pad), fits the RTX 4070 SUPER (12 GB), and which of LoRA, LoKr and full fine-tuning should be tried first?

Tags: **VERIFIED** = run in this session (command and number given); **ASSUMED** = not checked, with what would confirm it. Phase 0 trains nothing.

## 1. Result in five lines

1. Transkun 2.0 is a 13.6 M parameter model (VERIFIED), small enough that **even a full fine-tune fits** the GPU (peak 6.7 GiB at 4 x 16 s, VERIFIED). The adapter choice is about forgetting and file separation, not memory.
2. Its attention uses raw `nn.Parameter` q/k/v matrices, not `nn.Linear` (VERIFIED, 36 tensors, 2.36 M params). Stock PEFT `target_modules` reaches only the other 50 Linear and 5 Conv2d layers (6.9 M weights); q/k/v needs `target_parameters` (VERIFIED with PEFT 0.21.3) or a 20-line wrapper.
3. The velocity output is a 128-way categorical per detected note, trained by log-likelihood (VERIFIED in `log_prob`). That is the head to adapt; velocity is only scored on notes the CRF finds, so recall and velocity are two separate problems.
4. Rendering is not the bottleneck: the heaviest tested program (Horizon Whl) renders at 4.9x real time on one core and about 90x aggregate with 32 workers (VERIFIED). Training eats about 60 s of audio per second (bf16, VERIFIED on dummy audio).
5. **Recommendation:** start with **LoRA rank 16** on all Linear and Conv2d layers plus q/k/v (via wrapper), full training of the velocity and refined onset/offset heads; run **LoKr as the second arm with the same data and seed**; hold full fine-tune back unless both plateau (section 6). PEFT supports both on every layer except `ConvTranspose1d`, and raw q/k/v via `target_parameters` (VERIFIED, section 4).

## 2. Model facts (VERIFIED, `inspect_model.py`, `lora_count.py`)

| Item | Value |
|---|---|
| Parameters | 13,613,711 (backbone 12,626,688; scorer 131,841; velocityPredictor 459,392; refinedOFPredictor 395,780) |
| Input | 44.1 kHz audio, mono mix, per-segment gain normalisation (`processFramesBatch` subtracts mean, divides by std), 229-band log mel (`f_min` 30, `f_max` 8000), window 4096, hop 1024, 5 extra windows |
| Segment | 16 s with 8 s hop in `2.0.conf` (the README says the shipped weight was trained at 10 / 20 s; inference code uses the conf values) |
| Backbone | 5 Conv2d (`inputConv`, `downConv.1/5/9/13`) + 1 ConvTranspose1d (`upConv1dSkip`) + 6 encoder layers with factorised attention over frequency (F) and time (T), 8 heads, `baseSize` 64, FFN factor 4, RMSNorm and GroupNorm |
| Leaf modules | `Linear` 50 (9.17 M weights), `Conv2d` 5 (1.55 M), `ConvTranspose1d` 1 (0.52 M), `GroupNorm` 4, `RMSNorm` 24 (no params) |
| Raw attention params | `backbone.encoderLayers.N.mhaBlock{F,T}.module.{q,k,v}_proj_weight` (`MultiHeadAttentionKernel`, `LayersTransformer.py:146-148`), 36 tensors, 2,359,296 weights; the output projection `out_proj` is a Linear |
| Output | pitches `[-64, -67] + 21..108` (88 keys plus two pedal symbols: -64 sustain (CC 64) and -67 soft pedal (CC 67), `Data.py` `supportedCC`). Per pitch a **neural semi-CRF over intervals** (`ScaledInnerProductIntervalScorer`, 131,841 params) scores (onset, offset) frame pairs; there are no separate framewise onset/offset heads |
| Per-note heads | `velocityPredictor` (Linear 768 to 512, GELU, dropout, Linear 512 to 128; input is `[ctx_on, ctx_off, ctx_on * ctx_off]`) and `refinedOFPredictor` (768 to 512 to 4: sub-frame onset/offset position as ContinuousBernoulli plus "has onset" / "has offset" Bernoulli logits) |
| Velocity read-out | expectation of the 128-way softmax (`computeStats`); trained with `log_softmax` gather on the integer velocity |
| Loss | negative of `log P(intervals) + log P(velocity) + log P(refined offsets) + log P(presence)` per segment, `loss = -logp.sum(-1).mean()` (`train.py:187`); CRF normaliser `computeLogZ` |
| Dropout | scorer 0.1, velocity 0.1, refined 0.1, context 0.0 |

The decoded transcription (`transcribeFrames`, `transcribe.py`) keeps the CRF path and then asks the two heads for velocity and sub-frame times of that path. A synthetic-trained adapter therefore changes (a) which intervals the CRF finds (needs backbone and scorer adaptation) and (b) the velocity read-out (heads, plus the context features).

### 2.1 Training code and data format (VERIFIED by reading, not run)

| Item | Value |
|---|---|
| Script | `python -m transkun.train <ckpt> --nProcess N --datasetPath <dir> --datasetMetaFile_train train.pickle --datasetMetaFile_val val.pickle --modelConf <conf> [--augment --noiseFolder --irFolder]`; AdaBelief (`torch_optimizer`) with OneCycleLR (5 % warm-up), `max_lr` 2e-4, weight decay 1e-4, `nIter` 180000, adaptive gradient clipping at the 0.8 quantile, `loss/50` before backward (gradient accumulation by 50 sub-batches per step in the loop) |
| Dataset | `DatasetMaestro`: a pickle of dicts (`duration`, `audio_filename`, `notes` list of `Note(start, end, pitch, velocity, hasOnset, hasOffset)` with pedal as negative pitches); audio read from `.wav` by `readAudioSlice` (scipy `mmap`, integer PCM scaled by the dtype max, so write int16 WAV) |
| Slicing | `DatasetMaestroIterator`: chunks of `segmentSizeInSecond` hopping `segmentHopSizeInSecond` with random dither; `collate_fn_batching` stacks audio |
| Note rules | per pitch, notes must not overlap (`validateNotes` asserts `start >= previous end`); notes quantised to 1024-sample frames collide are merged and warn |
| Gotcha | `createIndexEvents` imports `ncls` (no Windows wheel, not installed). The dataset index path needs it; **inference does not**. Plan: write our own small iterator producing `{"notes", "audioSlices"}` batches and call `model.log_prob` directly (this is what `bench_step.py` does) |
| Not installed | `peft`, `lycoris-lora`, `pip` inside the venv (it was built with uv) |

## 3. Licence (VERIFIED for the code, ASSUMED for the weights)

- `LICENSE` in the wheel: **MIT**, copyright Yujia Yan 2021. Fine-tuning, modifying and redistributing the code is permitted with the notice.
- The pretrained `2.0.pt` has no separate licence file in the wheel. The README says only that it is the shipped checkpoint. **ASSUMED:** MIT applies to the weights as part of the package. The checkpoint was trained on MAESTRO (CC BY-NC-SA 4.0 audio and MIDI) per the training instructions; whether that restricts the weights is unsettled (ASSUMED: non-commercial research use is safe).
- **Decision for this repo:** adapters and any derived weights stay local (`C:\tmp`), never committed, consistent with the repo rule; only scripts, configs, numbers and docs go in git. Non-commercial research use only. Confirm with the upstream author before any redistribution.

## 4. PEFT / LyCORIS support (VERIFIED, `peft_check.py`)

Environment: new venv `C:/tmp/ns4-ft-venv` (Python 3.12, `uv`), reusing torch 2.6.0+cu124 and transkun from the transcription venv through a `.pth`; installed `peft` 0.21.3, `lycoris-lora`, `accelerate`, `transformers`, `toml`, `einops`. The existing venvs are untouched. Gotcha: `uv pip install peft` pulls a second torch (2.15.0) into the new venv; it was uninstalled so the `.pth` torch wins. `lycoris-lora` 4.1.3 fails to import on torch 2.6 (custom op schema); 3.2.0.post2 imports. LyCORIS itself is not needed: PEFT ships LoKr.

One forward, backward and optimizer-free step on a dummy batch (8 s audio, 15 notes), base weights loaded from `2.0.pt`:

| Config | Result | Trainable | Tensors with grad |
|---|---|---|---|
| `LoraConfig(r=16, alpha=32)` on Linear + Conv2d (regex below) + `modules_to_save` heads | works | 1,574,900 | 50 |
| `LoKrConfig(r=8, alpha=16)` same targets and heads | works | 892,468 (adapters about 37 k, heads 855 k) | 50 |
| `target_parameters` on the 36 raw q/k/v tensors (+ `inputConv` LoRA) | **works** in PEFT 0.21.3 | 296,800 (q/k/v LoRA r=16 = 294,912, matches the arithmetic below) | 37 |
| LoRA on `ConvTranspose1d` (`upConv1dSkip`) | **not supported** (`ValueError`) | n/a | n/a |
| regex that also matches `downConv.0/2/3/...` | fails: `ConstantPad2d` / `ReLU`-type modules are not adaptable; list only `downConv.(1|5|9|13)` | n/a | n/a |

- Raw q/k/v need **no wrapper**: `target_parameters` works, so the 20-line wrapper is only a fallback. Their LoKr support is unchecked.
- With the default reentrant activation checkpoint, a forward whose inputs do not require grad gives "does not require grad". An adapter on `backbone.inputConv` (or `use_reentrant=False`) avoids it; the real configs include `inputConv`, so it does not bite. Keep in mind for a q/k/v-only arm.
- `ConvTranspose1d` (0.52 M) stays frozen or fully trained.
- LoRA, LoKr and LoHa for Conv2d and Linear are in PEFT itself (`LoKrConfig`), LyCORIS is optional.

**Config used in the check (regex VERIFIED):**

```python
LoraConfig(r=16, lora_alpha=32, lora_dropout=0.05, bias="none",
           target_modules=r"backbone\.(encoderLayers\.\d+\.(mhaBlock|fnnBlock)[FT]\.module(\.(q|k|v)|\.out_proj|\.[03])|inputConv|downConv\.\d+)|scorer\.map\.0",
           modules_to_save=["velocityPredictor", "refinedOFPredictor"])
LoKrConfig(r=8, alpha=16, decompose_both=False, decompose_factor=-1, rank_dropout=0.0,
           module_dropout=0.0, target_modules=<same regex>,
           modules_to_save=["velocityPredictor", "refinedOFPredictor"])
```

Trainable weights by rank (VERIFIED arithmetic, `lora_count.py`, LoRA `r * (in + out)` per layer, Conv2d `r * (in*k*k + out)`):

| r | Backbone + scorer Linear (46) | Heads Linear (4) | Conv2d (5) | q/k/v (36) | Total |
|---|---|---|---|---|---|
| 8 | 351,792 | 29,728 | 58,800 | 147,456 | 587,776 (4.3 % of 13.6 M) |
| 16 | 703,584 | 59,456 | 117,600 | 294,912 | 1,175,552 (8.6 %) |
| 32 | 1,407,168 | 118,912 | 235,200 | 589,824 | 2,351,104 (17.3 %) |

`modules_to_save` copies the two heads in full (855,172 weights) on top; they are small enough to train fully, which is how `velocityPredictor` can learn a new loudness mapping.

## 5. GPU feasibility (VERIFIED, `bench_step.py`)

Setup: RTX 4070 SUPER 12,282 MiB (1.9 GiB already used by the desktop), torch 2.6.0+cu124, **full fine-tune of all 13.6 M parameters with AdamW** (the upstream script uses AdaBelief, same state size: two moments) (the worst case for memory), dummy Gaussian audio and random note lists (loss values are meaningless). `model.train()` turns on activation checkpointing automatically (`Backbone.forward`: `useGradientCheckpoint or self.training`), so "gradient checkpointing on" is the default. Step time is the mean of steps 2 to 4.

| Segment | Batch | Precision | Peak allocated | Step time | Audio per second of training |
|---|---|---|---|---|---|
| 16 s | 1 | fp32 | 1.75 GiB | 0.52 s | 31 s |
| 16 s | 2 | fp32 | 3.50 GiB | 0.72 s | 44 s |
| 16 s | 4 | fp32 | 6.69 GiB | 1.42 s | 45 s |
| 16 s | 2 | bf16 autocast | 2.57 GiB | 0.75 s | 43 s |
| 16 s | 4 | bf16 autocast | 5.51 GiB | 1.06 s | 60 s |
| 8 s | 4 | bf16 autocast | 2.15 GiB | 0.55 s | 58 s |
| 8 s | 8 | bf16 autocast | 3.78 GiB | 0.85 s | 75 s |

- Everything fits in 12 GiB with large margin; a LoRA / LoKr run stores far fewer gradients and optimizer states than this full run, so these numbers are an **upper bound** for the adapters (activations dominate).
- bf16 saves about 20 % memory and 25 % time at batch 4. Its effect on the semi-CRF log-partition (`computeLogZ`) is **not checked**: ASSUMED stable, check by comparing the loss of one real batch in fp32 and bf16 (should agree within about 0.5 %); otherwise train in fp32 (the cost is small).
- Throughput: one pass over 50 h of audio at 16 s segments with 8 s hop (each second seen twice) costs about 100 h / 60 per second = **about 1.7 h** at batch 4 bf16 (ASSUMED linear scaling; real data adds dataloader time). 50 h dataset, 10 epochs is a night of training.
- No GPU unlock needed (the card is not the limit).

## 6. Data rate (VERIFIED, `ns4 render-patch`)

Command: `ns4 render-patch <program.ns4p> --events ev60.json --velocity 100 --wav out.wav` on a random 350-note, 65.3 s phrase (stereo float32, 44.1 kHz), 32 logical cores. `target/release/ns4.exe` of the main checkout built before this session; binary freshness vs `main` not checked.

| Program (engine) | Wall for 65.3 s audio | Real-time factor | Note |
|---|---|---|---|
| Smooth Lead (backup load included) | 2.6 s | 25x | light synth |
| Leaky Stabs II (backup load included) | 14.9 s | 4.4x | |
| Grand Pad (backup load included) | 15.4 s | 4.2x | piano + synth |
| Horizon Whl, from `--backup` | 22.7 s | 2.9x | includes about 9 s to read the 3 GB `.ns4b` |
| Horizon Whl, extracted `.ns4p` (`ns4 backup <b> --extract`) | 13.3 s | 4.9x | heaviest tested |
| Horizon Whl, 8 s phrase | 1.5 s | 5.3x | start-up included |

| Parallel workers (Horizon, extracted, 65.3 s each) | Wall | Aggregate real-time factor |
|---|---|---|
| 8 | 15 s | 35x |
| 16 | 17 s | 61x |
| 32 | 23 s | 91x |

Conclusions:
- Extract each program once (`ns4 backup <backup> --extract <entry> <file>`) and render from the file: the backup load costs about 9 s per process.
- **About 90 h of audio per hour of rendering** for the heaviest program, much more for light ones; a 50 h dataset costs under 40 minutes of CPU. Disk: stereo float32 is 0.35 MB/s; store 16-bit mono 44.1 kHz WAV (0.088 MB/s, 16 GB for 50 h) because Transkun reads integer PCM.
- **Dataset target:** start with **20 h** (about 1,100 programs x 65 s phrases is 20 h; many more phrase variants per program are cheaper than more programs), scale to 50 h only if held-out F1 is still rising. Programs matter more than hours: the factory bank has the synth, organ and piano programs; hold out whole programs for validation.

## 7. Plan, phases and commands

**Phase 1: data (CPU, parallel, one session).** Extract programs; generate note sequences (random phrases, chords, arpeggios, repeats, realistic lengths; velocity 1..127 skewed to soft like the real clips, median about 45 per [piano-transcription](piano-transcription.md) section 3; no same-pitch overlaps; sustain pedal events for piano); render with `ns4 render-patch --events`; write int16 mono 44.1 kHz WAV plus a note-list pickle in `DatasetMaestro` format.
Augmentations (ASSUMED useful, ablate): random gain (Transkun already normalises gain), light EQ tilt, codec round trip (video audio is lossy), noise floor, 0 to 2 s of room-like reverb on dry programs. Provenance: a script `tools/ns4_synth_dataset.py` with tests, outputs under `C:\tmp\ns4-ft-data`, never committed.

```bash
python tools/ns4_synth_dataset.py extract --backup ".reference/Nord Stage 4 Factory Restore RevK v1.66.ns4b" --out C:/tmp/ns4-ft-data/programs
python tools/ns4_synth_dataset.py render --programs C:/tmp/ns4-ft-data/programs --hours 20 --workers 24 --seed 1 --out C:/tmp/ns4-ft-data
```

**Phase 2: training (GPU).** Arms, same data, same seed, same step budget (about 20k steps at batch 4, 16 s segments, `max_lr` 2e-4 for adapters, 2e-5 for any full fine-tune, early stopping on held-out programs):

| Arm | Trainable | Notes |
|---|---|---|
| A: LoRA r=16 | adapters (1.18 M) + heads (0.86 M) | first choice |
| B: LoKr r=8 | adapters + heads | same targets; fewer weights per layer, Kronecker structure may fit the conv layers better |
| C: full fine-tune | 13.6 M | only if A and B plateau |

Layer names in the regex are VERIFIED (`named_modules`); the q/k/v entries exist only after the wrapper. Mix per batch: 70 % synth renders, 30 % piano and organ renders from the same pipeline (forgetting control). The piano path keeps using the base weights: the adapter is a separate `adapter_model.safetensors` loaded only for synth clips, so the base `2.0.pt` stays bit-identical (check by SHA-256 before and after).

**Phase 3: evaluation protocol.**

| # | Check | Metric | Pass |
|---|---|---|---|
| 1 | Held-out **programs** (not phrases) rendered by the emulator, same pipeline | note F1 at onset +-50 ms (mir_eval, exact pitch, no offset), velocity MAE, velocity Pearson r, over the detected-and-matched notes | F1 well above base Transkun and not collapsing on unseen programs; velocity MAE under 12 (ASSUMED threshold, base Transkun is the reference) |
| 2 | Piano regression: base vs base + adapter disabled | transcriptions of the committed piano clips identical; with adapter **enabled**, F1 and velocity vs base on emulator piano renders | disabled = bit-identical output; enabled may differ (report it) |
| 3 | Hardware synth clips (`synth_horizon`, `synth_leaky`, `pianopad_grand`; no ground truth) | recall / precision at +-150 ms against the video-confirmed notes (`tools/transcribe_agree.py`, Transkun baseline 0.20 / 0.14 / 0.25 recall, [synth-transcription-muscriptor](synth-transcription-muscriptor.md) section 2) | recall above 0.40 on at least two of three clips |
| 4 | Replay score: `tools/emu_vs_hw.py --events-from <adapter stream>` against **uniform velocity 100** and against **base Transkun** | band deltas, RMS delta, per-note level correlation (`tools/note_env.py` slope) | adapter stream beats both on the mean absolute band delta |
| 5 | Listening WAVs for the user | same folder layout as `C:\tmp\listen-2026-11-15` | user judgement |

Checks 1 and 2 are cheap and automated; check 3 is the true target and is noisy (video-confirmed notes bound agreement, not truth, as in the earlier notes); check 4 is the metric the project cares about.

**Decision criteria between arms** (after arms A and B at the same step budget):
- Pick the arm with the higher held-out F1 at 50 ms plus lower velocity MAE; break ties by hardware recall (check 3), then by smaller adapter.
- If A and B differ by under 1 F1 point and 1 velocity unit, take **LoRA** (the PEFT default, fewer moving parts).
- If both stay below the check 1 threshold or held-out loss plateaus well above the training loss, run arm C. Expect forgetting: only acceptable because the base file stays separate.
- If held-out emulator F1 is high but hardware recall (check 3) does not move, stop: the problem is the emulator-to-hardware domain gap (ASSUMED the main risk), not capacity. Next step is more augmentation, not a larger adapter.

## 8. Risks

| Risk | Mitigation |
|---|---|
| Emulator-to-hardware domain gap (unverified synth voicing, filters, FX); the model learns the emulator, not the keyboard | augmentation; hold out programs; **check 3 on real clips** is the gate |
| Same synthetic "velocity" for hardware: the emulator velocity law is itself an estimate; a velocity head trained on it predicts emulator-velocity | velocity MAE only means something against the emulator; on hardware use the replay level match (check 4), not an absolute velocity claim |
| Reverb tails and overlapping notes cap recall | include reverb programs and long releases; report recall by program class |
| Pedal symbols never appear on synth clips | train with pedal events only on piano renders; ignore pedal on synth in evaluation |
| `ncls` missing, `Data.py` indexing | own iterator (section 2.1) |
| Raw q/k/v with LoKr untested; reentrant checkpoint needs grad-requiring input | LoRA via `target_parameters`; keep `inputConv` adapted |
| bf16 and the CRF normaliser | compare fp32 vs bf16 loss on one batch before using bf16 |
| Weight licence uncertainty (MAESTRO lineage) | local only, non-commercial, no redistribution (section 3) |
| Scope creep: more tools than the project needs | the stop rule in section 7 |

## 9. Reproduce (probe scripts, local only)

`C:\tmp\ns4-ft-probe\inspect_model.py` (params, layer census), `lora_count.py` (adapter parameter arithmetic), `peft_check.py` (section 4, run with `C:/tmp/ns4-ft-venv\Scripts\python.exe -I`), `bench_step.py` (dummy-batch step: builds `Note` lists with no same-pitch overlap, calls `model.log_prob`, backward, `AdamW.step`, reports `torch.cuda.max_memory_allocated`). Run with `C:\tmp\ns4-transcribe\venv\Scripts\python.exe -I <script>`. Render timing: `ns4 render-patch` as in section 6, workers via `xargs -P`. These are throw-away measurement scripts and are not committed; the numbers above are the record.

## Implications

- Adapter training is practical on the existing GPU in hours, with rendering a small share of the budget.
- The first real engineering step is an own dataset iterator (small, testable); PEFT configs already run.
- Success is decided on hardware clips, not on held-out emulator renders.

## Follow-ups

- [x] Confirm PEFT LoRA / LoKr on Linear, Conv2d and raw q/k/v (section 4).
- [ ] Phase 1: `tools/ns4_synth_dataset.py` (extract, phrase generator, render, write dataset) with tests; 20 h.
- [ ] fp32 vs bf16 loss comparison on one real batch.
- [ ] Phase 2 and 3 as above; record a decision (next free D) with the chosen arm and the hardware-clip scores.
- [ ] Confirm the checkpoint licence with the upstream author if weights are ever to leave the machine.
