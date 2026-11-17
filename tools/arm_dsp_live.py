#!/usr/bin/env python3
r"""Run the booted ARM firmware against ONE live emulated DSP chip (`sharc-live`, D138).

The ARM harness (`arm_screens.Session`, the full machine of `arm_dsp_capture`) boots with one
factory program in the Store record. Every ARM -> DSP message is grouped as the firmware stores it
(`arm_dsp_link.PortGrouper` on the real EMIFA ports during the boot, on the fake ports after it);
messages for the chosen chip are sent to the `sharc-live` process, the others are only counted.
After the boot the DSP clock runs against the ARM timer tick (24 kHz, `arm_dsp_link.Clock`): after
every ARM tick the owed passes run. Phases: `load` (program recall), `idle`, `key` (one note held),
`off` (released). Numbers only on stdout (JSON); the sent messages go to `--log` as a
`sharc-sys --msg-script` file so a replay of the same stream can be compared (`--replay`).

    C=target/fw_unpacked
    uv run --with unicorn python tools/arm_dsp_live.py $C/bank0_CODE.bin $C/bank0_SRAM.bin \
        --eeprom target/oled/eeprom.bin \
        --program "factory_programs/Program/Bank A/B3 Chords n Solo.ns4p" \
        --dsp-image target/PNO/core1_sram.bin --live target/release/sharc-live \
        --probe 0x1256AD --probe 0x121E2B --log live.script --out live.json

The ARM side keeps the harness conventions: 200,000 instructions per timer tick (16 x the hardware
budget, `arm-tick-timers.md` s5), `arm_sy_voice.pump` after every tick, `DspAck` set.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

import arm_dsp_link as dl
import arm_dsp_return as rt

CHIPS = {"FX": 0, "PNO": 1, "ORG": 2, "SY": 3, "SMP": 4}
LOAD_TICKS, IDLE_TICKS, KEY_TICKS, OFF_TICKS = 800, 100, 1500, 300
KEY, VELOCITY = 60, 100
DM_WORDS = 0x100000  # data word space of one core window (`0x28000000 + 4 w`, w < 2^20)
BOOT_CHUNK = 50_000_000  # as `Session.boot`


def progress(text: str) -> None:
    """Progress line on stderr (stdout carries the JSON result only)."""
    print(f"[{time.strftime('%H:%M:%S')}] {text}", file=sys.stderr, flush=True)


def snapshot(link: dl.Link, exe_t0: float) -> dict[str, object]:
    """Cumulative numbers of `link.stats` (JSON friendly)."""
    s = link.stats
    return {
        "messages_sent": s.messages,
        "distinct_words": len(s.words),
        "passes": link.pass_index,
        "idle_passes": s.idle,
        "halted": s.halted,
        "lane_peak": s.peak,
        "lane_rms": s.rms,
        "probe_hits": {f"{pc:#x}": n for pc, n in sorted(s.hits.items())},
        "probe_first_pass": {f"{pc:#x}": n for pc, n in sorted(s.first_hit.items())},
        "errors": s.errors[:3],
        "wall_s": round(time.perf_counter() - exe_t0, 1),
    }


def phase_delta(before: dict[str, object], after: dict[str, object]) -> dict[str, object]:
    """Difference of two cumulative snapshots for the additive numbers."""
    keys = ("messages_sent", "distinct_words", "passes", "idle_passes")
    out = {k: after[k] - before[k] for k in keys}  # type: ignore[operator]
    hits_a, hits_b = after["probe_hits"], before["probe_hits"]
    out["probe_hits"] = {k: v - hits_b.get(k, 0) for k, v in hits_a.items()}  # type: ignore[union-attr]
    return out


class Feeder:
    """Moves the new port stores of the emulator through the grouper to the link."""

    def __init__(self, emu, link: dl.Link, chip: str, core: int, observer=None) -> None:  # noqa: ANN001
        import arm_emu as ae

        self.emu, self.link, self.want = emu, link, (CHIPS[chip], core)
        self.observer = observer  # called with every grouped message (the return path shadow)
        self.ae = ae
        self.grouper = dl.PortGrouper(
            dl.port_table(ae.PORT2_BASE, ae.PORT1_BASE, ae.COMM_COUNT, emu.pad)
        )
        self.seen: dict[str, int] = {}
        self.skipped = 0

    def feed(self) -> None:
        writes = list(self.emu.port_writes)
        self.emu.port_writes.clear()
        for path, comm, w1, w2, pc in self.grouper.feed(writes):
            m = self.ae.Message(path, comm, w1, w2, pc)
            if self.observer:
                self.observer(m)
            key = f"{m.chip}{m.core}p{path}"
            self.seen[key] = self.seen.get(key, 0) + 1
            if path == 2 and (comm, m.core) == self.want:
                if 0 <= m.dm_word < DM_WORDS:
                    self.link.send(m.dm_word, w2)
                else:  # not a DSP data word (address outside the core window)
                    self.skipped += 1


def return_observer(source):  # noqa: ANN001, ANN201
    """Feeder callback that mirrors every grouped ARM message into the return source."""
    if source is None:
        return None
    return lambda m: source.observe(m.comm, m.core, m.path, m.word1, m.word2)


def return_report(source: rt.ReturnSource, log: rt.ReadLog) -> dict[str, object]:
    """Numbers of the return path: reads per chip and address, answering layers."""
    reads = {
        f"{rt.CHIPS[c]}:{core}:{dm:#x}": n for (c, core, dm), n in sorted(source.reads.items())
    }
    return {
        "reads": reads,
        "answered_by": dict(source.layer),
        "calls": len(log.calls),
        "busy_voices": rt.VOICES - bin(source.sy.idle).count("1"),
    }


def boot_live(session, feeder: Feeder) -> tuple[int, int]:  # noqa: ANN001  # pragma: no cover
    """`Session.boot` with the messages fed after every chunk; returns `(pc, instructions)`."""
    import arm_boot as ab
    import arm_emu as ae
    import arm_panel as ap
    import arm_steady as st

    emu = session.emu
    emu.set_reg(13, ae.STACK_TOP - 0x1000)
    emu.set_reg(14, ae.SENTINEL)
    pc, total = ab.APP_THUNK, 0
    while total < 1_500_000_000:
        pc, ran, err = st.run_until(emu, st.MAIN_LOOP, BOOT_CHUNK, pc=pc)
        total += ran
        feeder.feed()
        progress(
            f"boot {total // 1_000_000} M insns pc={pc:#x} to chip {feeder.link.stats.messages}"
        )
        if pc == st.MAIN_LOOP or err:
            break
        if ap.in_wait_loop(pc):
            st.inject_irq(emu)
    st.enable_real_timer(emu)
    return pc, total


def run_ticks(rig, feeder: Feeder, clock: dl.Clock, ticks: int, insns: int) -> None:  # noqa: ANN001  # pragma: no cover
    """ARM tick by tick: instructions, queue pump, messages to the chip, then the owed passes."""
    import arm_steady as ast
    import arm_sy_voice as sv

    for n in range(ticks):
        if n % 200 == 0:
            progress(f"tick {n}/{ticks} passes {feeder.link.pass_index}")
        ast.run_ticking(rig.emu, insns, per_tick=insns)
        sv.pump(rig.emu)
        feeder.feed()
        owed = clock.tick()
        if owed:
            feeder.link.run(owed)


def drive(ns, link: dl.Link) -> dict[str, object]:  # noqa: ANN001  # pragma: no cover
    """The whole experiment; returns the result record."""
    import arm_dsp_capture as cap
    import arm_keydown as kd
    import arm_program_load as apl
    import arm_screens as sc
    from arm_bulk_tables import real_port_capture

    t0 = time.perf_counter()
    session = sc.Session(ns.image, ns.sram)
    program = ns.program.read_bytes()
    session.load_eeproms(apl.build_image(ns.eeprom.read_bytes(), program, ns.program.stem[:23]))
    kd.install_board_present(session.emu)
    real_port_capture(session.emu)
    source = None
    if ns.return_path:  # D147: the DSP answers the ARM reads (live chip, shadow, images)
        source = rt.ReturnSource(images=rt.load_images(ns.images, {1: "PNO", 2: "ORG"}))
        source.live[(CHIPS[ns.chip], ns.core)] = link.dump
    feeder = Feeder(session.emu, link, ns.chip, ns.core, return_observer(source))
    pc, insns = boot_live(session, feeder)
    result: dict[str, object] = {"boot_pc": f"{pc:#x}", "boot_insns": insns}
    result["boot"] = snapshot(link, t0)
    rig = kd.Rig(session.emu, kd.Uart2().install(session.emu))
    session.emu.install_dsp_ports()
    read_log = rt.ReadLog()
    if source:
        rt.install(session.emu, source, read_log)
    else:
        cap.install_dsp_ack(session.emu)
    feeder.grouper.pending.clear()
    clock, phases, prev = dl.Clock(), {}, snapshot(link, t0)
    plan = [("load", LOAD_TICKS, None), ("idle", IDLE_TICKS, None),
            ("key", ns.key_ticks, lambda: rig.send(kd.note_on(ns.key, VELOCITY))),
            ("off", OFF_TICKS, lambda: rig.send(kd.note_off(ns.key)))]  # fmt: skip
    for name, ticks, action in plan:
        if action:
            action()
        run_ticks(rig, feeder, clock, ticks, ns.insns_per_tick)
        now = snapshot(link, t0)
        phases[name] = {"ticks": ticks, **phase_delta(prev, now)}
        prev = now
    result.update(
        phases=phases,
        final=prev,
        arm_messages=dict(sorted(feeder.seen.items())),
        skipped_out_of_window=feeder.skipped,
    )
    result["ticks"], result["clock_passes"] = clock.ticks, clock.passes
    if source:
        result["return_path"] = return_report(source, read_log)
    return result


def replay(ns, passes: int) -> str:  # pragma: no cover - runs sharc-sys
    """`sharc-sys` replay of the log written by the live run (same stream, no ARM)."""
    cmd = [str(ns.sys_exe), str(ns.dsp_image), "--pno", "--epdma", "--warmup", "0",
           "--passes", str(passes), "--msg-script", str(ns.log), "--lane", "2:0"]  # fmt: skip
    return subprocess.run(cmd, capture_output=True, text=True, check=False).stdout  # noqa: S603


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the local firmware
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path)
    ap.add_argument("--eeprom", type=Path, required=True)
    ap.add_argument("--program", type=Path, required=True)
    ap.add_argument("--dsp-image", type=Path, required=True)
    ap.add_argument("--live", type=Path, required=True, help="sharc-live executable")
    ap.add_argument("--sys-exe", type=Path, help="sharc-sys executable: replay the log afterwards")
    ap.add_argument("--chip", default="PNO", choices=sorted(CHIPS))
    ap.add_argument("--core", type=int, default=1)
    ap.add_argument("--probe", action="append", default=[])
    ap.add_argument("--flags", nargs="*", default=["--pno", "--epdma", "--lane", "2:0"])
    ap.add_argument("--key", type=int, default=KEY)
    ap.add_argument("--key-ticks", type=int, default=KEY_TICKS)
    ap.add_argument("--insns-per-tick", type=int, default=200_000)
    ap.add_argument(
        "--return-path",
        action="store_true",
        help="answer the ARM's SPI reads from the chip, the ARM shadow and the images (D147)",
    )
    ap.add_argument(
        "--images", type=Path, help="dir with PNO/ ORG/ core<N>_sram.bin for the return path"
    )
    ap.add_argument("--log", type=Path, help="write the sent messages as a msg-script")
    ap.add_argument("--out", type=Path)
    ns = ap.parse_args(argv)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    flags = [*ns.flags, *[x for p in ns.probe for x in ("--probe", p)]]
    link = dl.Link.spawn(str(ns.live), str(ns.dsp_image), *flags)
    try:
        result = drive(ns, link)
        if ns.log:
            with ns.log.open("w", encoding="utf-8", newline="\n") as fh:
                fh.write(dl.script_text(link.log))
    finally:
        link.close()
    if ns.sys_exe and ns.log:
        result["replay_sharc_sys"] = replay(ns, link.pass_index).strip().splitlines()
    text = json.dumps(result, indent=1)
    if ns.out:
        ns.out.write_text(text, encoding="utf-8")
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
