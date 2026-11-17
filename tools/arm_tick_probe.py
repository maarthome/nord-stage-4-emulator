#!/usr/bin/env python3
"""The real ARM timer and the periodic tasks of the host firmware, measured in the harness (D130).

Boots the full machine to the main loop (`arm_steady.boot_to_main_loop`) with write hooks on the
timer block, then reads the firmware's own scheduler tables and runs ticks to count calls:

* `timers`  every write the firmware makes to Timer0 (`0x01C20000`): TGCR, TCR, PRD12, PRD34, the
            interrupt attach calls (`0xC005012C`) and the timer set-up calls (`0xC005019C`), turned
            into Hz with the 24 MHz input clock the firmware divides by (`0x016E3600`);
* `wheel`   the 512 slot tick wheel (one slot function per timer interrupt, `0xC0052AA4`) and the
            software timer list it serves (17 periodic entries `{period, due, object, function}`,
            period in timer ticks);
* `keydelay` the key-down DSP message groups (organ, note 60) with the interrupt injected every
            200,000 / 100,000 / 50,000 / 25,000 instructions: are the 3 / 17 / 47 "tick" delays of
            `keydown-organ-keys.csv` counted in timer interrupts or in instructions?
* `fxwatch` load a program (`--program`, `--eeprom`) and log which FX DSP words the ARM writes
            periodically over `--ticks` timer interrupts (is the PUMP Master Clock counter driven by
            the ARM?); `pump_free_run_hz` reproduces the 6.11 Hz free-run figure;
* `flagscan` static scan of the arpeggiator code for stores to `[rX, #0xC]` (the pattern object
            flag that selects the 2/3 grid, OQ32);
* `run`     `--ticks` ticks with the real timer interrupt (as `arm_steady.run_ticking`): calls per
            virtual second of every periodic function, the arpeggiator clock `0xC01E6368` and the
            pattern player `0xC023B3EC`, and the DSP words each of them writes.

    uv run --with unicorn --with capstone python tools/arm_tick_probe.py all \
        target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin \
        --out docs/research/data/arm-tick-timers.json

Only addresses, periods and counts are written (no firmware bytes). Evidence, table and the engine
comparison: `docs/research/arm-tick-timers.md`.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path

TIMER0 = 0x01C20000
TIMER_REGS = {0x10: "TIM12", 0x14: "TIM34", 0x18: "PRD12", 0x1C: "PRD34", 0x20: "TCR", 0x24: "TGCR"}
# VERIFIED (disassembly of the set-up routine `0xC005019C`): the period register is
# `0x016E3600 / hz` = 24,000,000 / hz, so the input clock the firmware assumes is 24 MHz. That the
# board feeds the timer 24 MHz (AM1705 OSCIN crystal) is ASSUMED, not measured; the firmware's own
# default tempo (panel 120 BPM equals 0.25 s per firmware beat at 24,000 units per second,
# `synth-arp-patterns.md` s9.3) is the consistency check.
TIMER_CLOCK_HZ = 24_000_000
TICK_COUNTER = 0x80001DC0  # 64-bit, `+1` per timer interrupt (handler `0x800007D8`)
IRQ_TABLE = 0xC055F674
WHEEL_THUNK = 0xC00A03F4  # `bl 0xC0052AA4` in the main loop: the wheel dispatcher call site
WHEEL_DISPATCH = 0xC0052AA4
WHEEL_OWNER_REG = 5  # `[r5 + 0x688]` is the wheel object in the main loop
WHEEL_OFFSET = 0x688
SLOT_TABLE = 0x30  # wheel object: index `+0`, mask `+4`, slot functions from `+0x30`
PERIODIC_CELL = 0xC0583110  # cell holding the periodic list object (serviced by `0x8000030C`)
ONESHOT_CELL = 0xC0584304  # cell holding the one-shot stack object (serviced by `0x800002AC`)
ENTRY_STRIDE = (
    0x28  # periodic entry: id +0, period +8, due +0x10, object +0x18, fn +0x1C, mode +0x20
)
ARP_CLOCK = 0xC01E6368
ARP_PLAYER = 0xC023B3EC
DEFAULT_ENTRY = 0x800002A8  # `bx lr`: the empty wheel slot


def prd_hz(prd: int, clock_hz: int = TIMER_CLOCK_HZ) -> float:
    """Interrupt or wrap rate of a timer half programmed with period register `prd`."""
    return clock_hz / prd if prd else 0.0


def decode_tgcr(value: int) -> dict[str, int | str]:
    """Fields of the global control register (AM1705 timer: TIMMODE `3:2`, release bits `1:0`)."""
    modes = {0: "64-bit", 1: "dual 32-bit unchained", 2: "watchdog", 3: "dual 32-bit chained"}
    return {
        "timmode": modes[(value >> 2) & 3],
        "tim12_released": value & 1,
        "tim34_released": (value >> 1) & 1,
        "prescale34": (value >> 8) & 0xF,
    }


def decode_tcr(value: int) -> dict[str, str]:
    """Enable modes of the control register (ENAMODE12 bits `7:6`, ENAMODE34 bits `23:22`)."""
    names = {0: "disabled", 1: "one-shot", 2: "continuous", 3: "continuous (reload)"}
    return {"enamode12": names[(value >> 6) & 3], "enamode34": names[(value >> 22) & 3]}


def timer_summary(writes: list[tuple[int, int, int]]) -> dict[str, object]:
    """Final timer configuration from `(pc, offset, value)` register writes (last write wins)."""
    last: dict[int, int] = {}
    for _pc, offset, value in writes:
        last[offset] = value
    out: dict[str, object] = {}
    for half, prd_off in (("TIM12", 0x18), ("TIM34", 0x1C)):
        prd = last.get(prd_off, 0)
        out[half] = {"prd": prd, "hz": prd_hz(prd), "period_us": 1e6 / prd_hz(prd) if prd else 0}
    out["tgcr"] = decode_tgcr(last.get(0x24, 0))
    out["tcr"] = decode_tcr(last.get(0x20, 0))
    return out


def wheel_rates(slots: list[int], tick_hz: float) -> list[dict[str, object]]:
    """Per slot function: slot count, gaps and call rate when one slot runs per tick."""
    by_fn: dict[int, list[int]] = defaultdict(list)
    for i, fn in enumerate(slots):
        by_fn[fn].append(i)
    rows = []
    for fn, idx in by_fn.items():
        gaps = sorted({b - a for a, b in zip(idx, idx[1:], strict=False)})
        rows.append(
            {
                "fn": fn,
                "slots": len(idx),
                "gaps": gaps,
                "hz": tick_hz * len(idx) / len(slots),
                "idle": fn == DEFAULT_ENTRY,
            }
        )
    return sorted(rows, key=lambda r: (r["idle"], -float(r["hz"])))  # type: ignore[arg-type]


def periodic_rows(words: list[list[int]], tick_hz: float) -> list[dict[str, object]]:
    """Rows from the 10-word dumps of the periodic list entries (`ENTRY_STRIDE` bytes each).

    Words: `[0]` id, `[2]` period (timer ticks, low word), `[6]` object, `[7]` function, `[8]` mode.
    """
    rows = []
    for w in words:
        period = w[2]
        rows.append(
            {
                "id": w[0],
                "fn": w[7],
                "object": w[6],
                "mode": w[8],
                "period_ticks": period,
                "hz": tick_hz / period if period else 0.0,
                "period_ms": 1e3 * period / tick_hz,
            }
        )
    return rows


def calls_per_second(calls: int, ticks: int, tick_hz: float) -> float:
    """Calls per virtual second in a run of `ticks` timer interrupts."""
    return calls * tick_hz / ticks if ticks else 0.0


def harness_scale(insns_per_tick: int, tick_hz: float, arm_mhz: float = 300.0) -> dict[str, float]:
    """What the harness tick (`arm_steady.INSNS_PER_TICK`) means against the real tick rate.

    At `tick_hz` the real interval holds `arm_mhz * 1e6 / tick_hz` ARM clocks (about one
    instruction per clock at best); the harness runs `insns_per_tick` per interrupt.
    """
    real = arm_mhz * 1e6 / tick_hz
    return {"real_insns_per_tick": real, "harness_over_real": insns_per_tick / real}


def message_ticks(rows: list[tuple[int, int]]) -> list[int]:
    """Tick indices that carried DSP messages, from `(tick, message count)` rows."""
    return sorted({t for t, n in rows if n})


# EMPIRICAL (arm_edit_capture of `kFx1Rate`, type 10 and 11, 44 values, D100): the PUMP phase
# increment `c[-32]` at the sample values `v`; linear between them (`fw_mod1_rest::eval`).
PUMP_V = (0, 1, 2, 15, 31, 47, 63, 79, 95, 111, 127)
PUMP_INC = (0x3200, 0x7200, 0xB200, 0x3F200, 0x7F200, 0xBF200, 0xFF200, 0x13F200, 0x17F200,
            0x1BF200, 0x1FF200)  # fmt: skip
PUMP_CAP_TYPE10 = 0x7FFFFFFF  # `c[-22]`, type 10 (type 11: 0x7F000000)
PUMP_START = -(1 << 31)  # `0x80000000`, the phase after a Master Clock reset
SY_FRAME_HZ = 43_402.34375  # `dsp-frame-rate.md`


def pump_increment(v: float) -> float:
    """PUMP phase increment per call at rate value `v` (0..127), linear between the samples."""
    v = min(max(v, 0.0), 127.0)
    k = max(i for i, x in enumerate(PUMP_V) if x <= v)
    k = min(k, len(PUMP_V) - 2)
    t = (v - PUMP_V[k]) / (PUMP_V[k + 1] - PUMP_V[k])
    return PUMP_INC[k] + (PUMP_INC[k + 1] - PUMP_INC[k]) * t


def pump_free_run_hz(v: float, cap: int = PUMP_CAP_TYPE10, frames_per_call: int = 2) -> float:
    """Free-running PUMP cycle rate: calls from `0x80000000` to the cap at `inc` per call.

    The routine `0x122D2C` adds `c[-32]` per call and caps at `c[-22]` (signed min); a reset to
    `0x80000000` happens when the Master Clock counter changes, so with a running clock the cycle
    is the beat (`bpm / 60` Hz) whenever the free-run cycle is longer than a beat.
    """
    calls = (cap - PUMP_START) / pump_increment(v)
    return SY_FRAME_HZ / frames_per_call / calls


def spacing_stats(ticks: list[int]) -> dict[str, float]:
    """Count and mean / min / max gap of the tick indices at which one DSP word was written."""
    gaps = [b - a for a, b in zip(ticks, ticks[1:], strict=False)]
    if not gaps:
        return {"count": float(len(ticks)), "mean_gap": 0.0, "min_gap": 0.0, "max_gap": 0.0}
    return {
        "count": float(len(ticks)),
        "mean_gap": sum(gaps) / len(gaps),
        "min_gap": float(min(gaps)),
        "max_gap": float(max(gaps)),
    }


def find_stores(
    code: bytes, base: int, lo: int, hi: int, offset: int, reg_filter: int | None = None
) -> list[tuple[int, str]]:
    """ARM `str`/`strb` immediate-offset stores with `[rn, #offset]` in `[lo, hi)` of the image.

    Pure decoder for the A32 single data transfer form (`cond 01 0 P U 0 W 0 Rn Rt imm12`, store):
    returns `(address, "str rt, [rn, #offset]")`. Pre-indexed, positive offset only, no writeback.
    Used for the arpeggiator pattern object flag `[obj + 0xC]` (OQ32).
    """
    out: list[tuple[int, str]] = []
    for addr in range(lo, hi, 4):
        word = int.from_bytes(code[addr - base : addr - base + 4], "little")
        # bits 27:26 = 01 (single data transfer), I = 0 (immediate), P = 1, U = 1, W = 0, L = 0
        if (word >> 26) & 3 != 1 or (word >> 25) & 1 or not (word >> 24) & 1:
            continue
        if not (word >> 23) & 1 or (word >> 21) & 1 or (word >> 20) & 1:
            continue
        if word & 0xFFF != offset:
            continue
        rn, rt, byte = (word >> 16) & 0xF, (word >> 12) & 0xF, (word >> 22) & 1
        if reg_filter is not None and rn != reg_filter:
            continue
        out.append((addr, f"str{'b' if byte else ''} r{rt}, [r{rn}, #{offset:#x}]"))
    return out


def delay_model(per_tick_insns: list[int], first_ticks: list[int], tol: float = 0.25) -> str:
    """Is a measured delay counted in timer interrupts or in ARM instructions?

    The same event is timed with the interrupt injected every `per_tick_insns[i]` instructions and
    arrives at tick `first_ticks[i]`. Constant ticks: timer driven; constant `ticks * insns`:
    instruction driven; otherwise `mixed`.
    """
    ticks = [float(t) for t in first_ticks]
    insns = [t * p for t, p in zip(ticks, per_tick_insns, strict=True)]

    def flat(v: list[float]) -> bool:
        return max(v) - min(v) <= tol * max(max(v), 1.0)

    if flat(ticks):
        return "timer"
    return "instructions" if flat(insns) else "mixed"


# ----------------------------------------------------------------------------------------------
# Unicorn runs (need the local firmware; not covered by the unit tests)
# ----------------------------------------------------------------------------------------------


def _boot(code: Path, sram: Path):  # pragma: no cover - needs unicorn and the firmware
    import arm_boot_full as bf

    return bf.prepare(code, sram)


def probe_timers(emu) -> dict[str, object]:  # pragma: no cover - needs unicorn and the firmware
    """Boot to the main loop with hooks; returns the writes, the attach and set-up calls."""
    import arm_steady as ast
    from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE

    writes: list[tuple[int, int, int]] = []
    reads: dict[int, int] = defaultdict(int)
    attach: list[dict[str, int]] = []
    setup: list[dict[str, int]] = []

    def on_write(_uc, _acc, addr, _size, value, _user):  # noqa: ANN001
        writes.append((emu.reg(15), addr - TIMER0, value & 0xFFFFFFFF))

    def on_read(_uc, _acc, addr, _size, _value, _user):  # noqa: ANN001
        if addr - TIMER0 in (0x10, 0x14):
            reads[emu.reg(15)] += 1

    def on_attach(_uc, _addr, _size, _user):  # noqa: ANN001
        dev = emu.reg(0)
        attach.append({"handler": emu.reg(1), "channel": emu.reg(2), "irq": emu.read32(dev + 4)})

    def on_setup(_uc, _addr, _size, _user):  # noqa: ANN001
        setup.append({"half": emu.reg(1), "freq_hz": emu.reg(2), "caller": emu.reg(14) - 4})

    emu.uc.hook_add(UC_HOOK_MEM_WRITE, on_write, begin=TIMER0, end=TIMER0 + 0xFFF)
    emu.uc.hook_add(UC_HOOK_MEM_READ, on_read, begin=TIMER0, end=TIMER0 + 0xFFF)
    emu.uc.hook_add(UC_HOOK_CODE, on_attach, begin=0xC005012C, end=0xC005012C)
    emu.uc.hook_add(UC_HOOK_CODE, on_setup, begin=0xC005019C, end=0xC005019C)
    pc, n, err = ast.boot_to_main_loop(emu)
    top_reads = sorted(reads.items(), key=lambda kv: -kv[1])[:5]
    return {
        "pc": pc,
        "instructions": n,
        "error": err,
        "writes": writes,
        "attach": attach,
        "setup": setup,
        "counter_reads": [{"pc": p, "reads": c} for p, c in top_reads],
        "summary": timer_summary(writes),
        "irq_handler": emu.read32(IRQ_TABLE + 4 * 22),
    }


def dump_wheel(emu) -> dict[str, object]:  # pragma: no cover - needs unicorn and the firmware
    """Read the tick wheel and the two software timer lists from the main loop state."""
    r = emu.read32
    wheel = r(emu.reg(WHEEL_OWNER_REG) + WHEEL_OFFSET)
    mask = r(wheel + 4)
    slots = [r(wheel + SLOT_TABLE + 4 * i) for i in range(mask + 1)]
    plist = r(PERIODIC_CELL)
    count = r(plist)
    entries = [[r(plist + 8 + ENTRY_STRIDE * i + 4 * k) for k in range(10)] for i in range(count)]
    return {
        "wheel_object": wheel,
        "slots": slots,
        "periodic_object": plist,
        "periodic_entries": entries,
        "oneshot_count": r(r(ONESHOT_CELL)),
    }


def run_ticks(emu, fns: dict[int, str], ticks: int, settle: int):  # pragma: no cover
    """Run `settle` then `ticks` timer interrupts; count calls and DSP words of `fns`."""
    import arm_emu as ae
    import arm_steady as ast
    from unicorn import UC_HOOK_CODE

    emu.install_dsp_ports()
    ast.enable_real_timer(emu)
    calls: dict[tuple[int, int], int] = defaultdict(int)
    words: dict[int, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    stack: list[tuple[int, int]] = []  # (function, port write count at entry)
    active = {"on": False}

    def on_entry(_uc, addr, _size, _user):  # noqa: ANN001
        if not active["on"]:
            return
        calls[(addr, emu.reg(0))] += 1
        stack.append((addr, len(emu.port_writes)))
        lr = emu.reg(14)
        handle = []

        def on_exit(uc2, _a, _s, _u):  # noqa: ANN001
            if stack:
                fn, start = stack.pop()
                for m in ae.group_port_writes(emu.port_writes[start:], emu.pad):
                    words[fn][f"{m.chip} core {m.core} dm {m.dm_word:#x}"] += 1
            uc2.hook_del(handle[0])

        handle.append(emu.uc.hook_add(UC_HOOK_CODE, on_exit, begin=lr & ~1, end=lr & ~1))

    for fn in fns:
        emu.uc.hook_add(UC_HOOK_CODE, on_entry, begin=fn, end=fn)
    ast.run_ticking(emu, settle * ast.INSNS_PER_TICK)
    emu.clear_messages()
    active["on"] = True
    t0 = emu.read32(TICK_COUNTER)
    ast.run_ticking(emu, ticks * ast.INSNS_PER_TICK)
    t1 = emu.read32(TICK_COUNTER)
    active["on"] = False
    return calls, words, t1 - t0


def key_delays(  # pragma: no cover - needs unicorn and the firmware
    code: Path,
    sram: Path,
    cache: Path,
    per_ticks: list[int],
    note: int,
    budget: int,
    model: int = 1,
) -> dict[str, object]:
    """Time the organ key-down DSP message groups (organ A, `arm_zone_capture`) per spacing."""
    import arm_keydown as kd
    import arm_steady as ast
    import arm_zone_capture as zc

    out: dict[str, object] = {"note": note, "model": model, "runs": []}
    for per in per_ticks:
        kd.TICK_INSNS = per  # `Rig.tick` runs one interrupt per `per` instructions
        setup = zc.prepare(code, sram, cache)
        setup.keep({0})
        zc.install_organ_model(setup, model, 0)
        setup.freeze()
        setup.reset()
        setup.rig.send(kd.note_on(note, 100))
        rows: list[tuple[int, int]] = []
        detail: list[list[object]] = []
        for n in range(budget // per):
            setup.emu.clear_messages()
            ast.run_ticking(setup.emu, per, per_tick=per)
            rows.append((n, len(setup.emu.messages)))
            if setup.emu.messages:
                detail.append(
                    [n, [f"{m.chip}{m.core} {m.dm_word:#x}" for m in setup.emu.messages[:4]]]
                )
        out["runs"].append(  # type: ignore[union-attr]
            {"per_tick": per, "message_ticks": message_ticks(rows), "detail": detail}
        )
    groups = [r["message_ticks"] for r in out["runs"]]  # type: ignore[union-attr]
    first = [g[0] for g in groups if g]  # type: ignore[index]
    out["model_first_group"] = (
        delay_model(per_ticks, first) if len(first) == len(groups) else "none"
    )
    return out


def fx_watch(  # pragma: no cover - needs unicorn and the firmware
    code: Path, sram: Path, eeprom: Path, program: Path, ticks: int, settle: int
) -> dict[str, object]:
    """Load `program`, run `ticks` timer interrupts and log the periodic FX DSP words."""
    import arm_keydown as kd
    import arm_program_load as apl
    import arm_screens as sc
    import arm_steady as ast

    session = sc.Session(code, sram)
    session.load_eeproms(apl.build_image(eeprom.read_bytes(), program.read_bytes(), "p"))
    kd.install_board_present(session.emu)
    session.boot()
    session.emu.install_dsp_ports()
    ast.run_ticking(session.emu, settle * ast.INSNS_PER_TICK)
    by_word: dict[str, list[int]] = defaultdict(list)
    for n in range(ticks):
        session.emu.clear_messages()
        ast.run_ticking(session.emu, ast.INSNS_PER_TICK, per_tick=ast.INSNS_PER_TICK)
        for m in session.emu.messages:
            if m.chip == "FX":
                by_word[f"core {m.core} dm {m.dm_word:#x}"].append(n)
    return {
        "program": program.stem,
        "ticks": ticks,
        "words": {k: spacing_stats(v) for k, v in sorted(by_word.items())},
    }


# VERIFIED (D130, `flagscan`): the arpeggiator code ranges scanned for stores to `[rX, #0xC]`.
ARP_SCAN_RANGES = ((0xC023B000, 0xC0243000), (0xC011A000, 0xC0120000), (0xC00FD000, 0xC00FE000))


def flag_scan(code: Path) -> list[dict[str, object]]:  # pragma: no cover - needs the firmware
    """Every `str [rX, #0xC]` in the arpeggiator ranges (pattern object flag, OQ32)."""
    data = code.read_bytes()
    rows = []
    for lo, hi in ARP_SCAN_RANGES:
        for addr, text in find_stores(data, 0xC0000000, lo, hi, 0xC):
            if "[r13," not in text:  # stack spills are not object fields
                rows.append({"address": addr, "insn": text})
    return rows


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument(
        "command", choices=["timers", "wheel", "run", "all", "keydelay", "fxwatch", "flagscan"]
    )
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path)
    ap.add_argument("--ticks", type=int, default=6000)
    ap.add_argument("--settle", type=int, default=800)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--cache", type=Path, default=Path("target/keydown.snap"))
    ap.add_argument("--per-ticks", type=int, nargs="+", default=[200_000, 100_000, 50_000, 25_000])
    ap.add_argument("--note", type=int, default=60)
    ap.add_argument("--model", type=int, default=1, help="organ model (0 B3, 1 Vox)")
    ap.add_argument("--eeprom", type=Path, default=Path("target/oled/eeprom.bin"))
    ap.add_argument("--program", type=Path)
    ns = ap.parse_args(argv)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    if ns.command == "flagscan":
        print(json.dumps(flag_scan(ns.image), indent=1))
        return 0
    if ns.command == "fxwatch":
        res = fx_watch(ns.image, ns.sram, ns.eeprom, ns.program, ns.ticks, ns.settle)
        text = json.dumps(res, indent=1)
        if ns.out:
            ns.out.write_text(text)
        print(text)
        return 0
    if ns.command == "keydelay":
        res = key_delays(ns.image, ns.sram, ns.cache, ns.per_ticks, ns.note, 20_000_000, ns.model)
        text = json.dumps(res, indent=1)
        if ns.out:
            ns.out.write_text(text)
        print(text)
        return 0
    emu = _boot(ns.image, ns.sram)
    result: dict[str, object] = {"timers": probe_timers(emu)}
    tick_hz = float(result["timers"]["summary"]["TIM34"]["hz"])  # type: ignore[index]
    wheel = dump_wheel(emu)
    result["tick_hz"] = tick_hz
    result["wheel"] = {
        "slots_total": len(wheel["slots"]),  # type: ignore[arg-type]
        "rates": wheel_rates(wheel["slots"], tick_hz),  # type: ignore[arg-type]
    }
    result["periodic"] = periodic_rows(wheel["periodic_entries"], tick_hz)  # type: ignore[arg-type]
    result["oneshot_pending"] = wheel["oneshot_count"]
    if ns.command in ("run", "all"):
        fns = {int(p["fn"]): "periodic" for p in result["periodic"]}  # type: ignore[union-attr]
        fns[ARP_CLOCK] = "arp clock"
        fns[ARP_PLAYER] = "arp player"
        calls, words, dt = run_ticks(emu, fns, ns.ticks, ns.settle)
        result["run"] = {
            "ticks_injected": ns.ticks,
            "counter_delta": dt,
            "calls": [
                {
                    "fn": fn,
                    "object": obj,
                    "calls": c,
                    "hz": calls_per_second(c, ns.ticks, tick_hz),
                }
                for (fn, obj), c in sorted(calls.items())
            ],
            "dsp_messages": {hex(k): dict(v) for k, v in words.items()},
        }
    text = json.dumps(result, indent=1, default=lambda o: hex(o) if isinstance(o, int) else str(o))
    if ns.out:
        ns.out.write_text(text)
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
