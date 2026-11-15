#!/usr/bin/env python3
"""FX default parameters of the firmware against the emulator's defaults.

The ARM edit-parameter table (`docs/research/arm-edit-params.csv`, column `default`) holds the
value every `Edt_*` id has before a program sets it. This tool maps the FX ids (Mod 1, Mod 2, Amp
and EQ, Comp, Delay, Reverb, Leslie) to the emulator's fields, converts the firmware default to
the emulator unit with the verified laws, and compares it with the default in
`LayerFxChain::new` (`fx/mod.rs`), `FxDecoded::default` (`cbin/fx_preset.rs`) and the organ
(`organ/mod.rs`).

    python tools/fx_defaults.py            # write the CSV and the Rust table
    python tools/fx_defaults.py --check    # fail when either is stale

Evidence and limits: `docs/research/fx-defaults.md`.
"""

from __future__ import annotations

import argparse
import csv
import math
import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PARAMS = ROOT / "docs/research/arm-edit-params.csv"
DATA = ROOT / "docs/research/data/fx-defaults.csv"
RUST = ROOT / "crates/nord-core/src/fx/fw_defaults_table.rs"
FX = ROOT / "crates/nord-core/src/fx"
FIELDS = "block,edt_name,fw_raw,unit,fw_value,source,emu_field,emu_default,status,note".split(",")
#: One UI count of a 0..127 knob: two values closer than this are the same position.
COUNT = 1.0 / 127.0
#: Relative tolerance of a frequency or time that the display rounds (0.5 %).
REL = 0.005


@dataclass(frozen=True)
class Row:
    """One FX id: how the firmware default converts and which emulator field it is compared to."""

    block: str
    edt: str
    unit: str  # frac | bool | enum | db | eq_hz | delay_ms | skip
    source: str  # chain | preset | organ
    field: str
    note: str = ""
    apply: bool = False  # written by `LayerFxChain::apply_firmware_defaults`


ROWS = (
    Row(
        "mod1", "Edt_Effects_kFx1Enable", "bool", "chain", "mod1_type", "Off is the disabled state"
    ),
    Row("mod1", "Edt_Effects_kFx1Type", "enum", "preset", "mod1_type_raw"),
    Row("mod1", "Edt_Effects_kFx1Amt", "frac", "chain", "mod1_amount", apply=True),
    Row("mod1", "Edt_Effects_kFx1Amt", "frac", "preset", "mod1_amount"),
    Row("mod1", "Edt_Effects_kFx1RateClkDiv", "frac", "preset", "mod1_rate"),
    Row("mod1", "Edt_Effects_kFx1RateClkDiv", "skip", "chain", "mod1_rate_hz", "rate law per type"),
    Row("mod1", "Edt_Effects_kFx1ClkEnable", "bool", "preset", "mod1_clock_sync"),
    Row(
        "mod2", "Edt_Effects_kFx2Enable", "bool", "chain", "mod2_type", "Off is the disabled state"
    ),
    Row("mod2", "Edt_Effects_kFx2Type", "enum", "preset", "mod2_type_raw"),
    Row("mod2", "Edt_Effects_kFx2Amt", "frac", "chain", "mod2_amount", apply=True),
    Row("mod2", "Edt_Effects_kFx2Amt", "frac", "preset", "mod2_amount"),
    Row("mod2", "Edt_Effects_kFx2Rate", "frac", "preset", "mod2_rate"),
    Row("mod2", "Edt_Effects_kFx2Rate", "skip", "chain", "mod2_rate_hz", "rate law per type"),
    Row(
        "amp", "Edt_Effects_kAmpEnable", "bool", "chain", "amp_cabinet", "Off is the disabled state"
    ),
    Row("amp", "Edt_Effects_kAmpTypeNew", "enum", "preset", "amp_type_new"),
    Row("eq", "Edt_Effects_kAmpTreble", "db", "chain", "eq_treble_db"),
    Row("eq", "Edt_Effects_kAmpMid", "db", "chain", "eq_mid_db"),
    Row("eq", "Edt_Effects_kAmpBass", "db", "chain", "eq_bass_db"),
    Row("eq", "Edt_Effects_kAmpMidFreq", "eq_hz", "chain", "eq_mid_freq_hz", apply=True),
    Row("eq", "Edt_Effects_kAmpMidFreq", "frac", "preset", "eq_mid_freq_norm"),
    Row("drive", "Edt_Effects_kAmpDrive", "frac", "chain", "drive_amount"),
    Row("drive", "Edt_Effects_kAmpDrive", "frac", "preset", "amp_drive"),
    Row("comp", "Edt_Effects_kCmpEnable", "bool", "chain", "comp_enabled"),
    Row("comp", "Edt_Effects_kCmpAmt", "frac", "chain", "comp_amount", apply=True),
    Row("comp", "Edt_Effects_kCmpAmt", "frac", "preset", "comp_amount"),
    Row("comp", "Edt_Effects_kCmpResponse", "bool", "chain", "comp_fast"),
    Row("delay", "Edt_Effects_kDlyEnable", "bool", "chain", "delay_enabled"),
    Row("delay", "Edt_Effects_kDlyClkEnable", "bool", "chain", "delay_clock_sync"),
    Row("delay", "Edt_Effects_kDlyRateClkDiv", "delay_ms", "chain", "delay_time_ms", apply=True),
    Row("delay", "Edt_Effects_kDlyRateClkDiv", "frac14", "preset", "delay_rate_norm"),
    Row("delay", "Edt_Effects_kDlyMix", "frac", "chain", "delay_mix", apply=True),
    Row("delay", "Edt_Effects_kDlyMix", "frac", "preset", "delay_mix"),
    Row("delay", "Edt_Effects_kDlyMode", "bool", "chain", "delay_analog"),
    Row("delay", "Edt_Effects_kDlyFbkPingPong", "bool", "chain", "delay_ping_pong"),
    Row("delay", "Edt_Effects_kDlyFbkAmt", "frac", "chain", "delay_feedback", apply=True),
    Row("delay", "Edt_Effects_kDlyFbkAmt", "frac", "preset", "delay_feedback"),
    Row("delay", "Edt_Effects_kDlyFbFxType", "enum", "preset", "delay_fb_fx_type"),
    Row("reverb", "Edt_Effects_kRvbEnable", "bool", "chain", "reverb_model", "no enable flag"),
    Row("reverb", "Edt_Effects_kRvbAmt", "frac", "chain", "reverb_mix", apply=True),
    Row("reverb", "Edt_Effects_kRvbAmt", "frac", "preset", "reverb_amount"),
    Row("reverb", "Edt_Effects_kRvbColor", "enum", "preset", "reverb_color"),
    Row("reverb", "Edt_Effects_kRvbType", "rvb_type", "chain", "reverb_model"),
    Row("rotary", "Edt_OrgCom_kEffectsLeslieEnable", "bool", "organ", "rotary_enabled"),
    Row("rotary", "Edt_OrgCom_kEffectsLeslieDrv", "frac", "organ", "rotary_drive"),
)


def read_params() -> dict[str, dict[str, str]]:
    with PARAMS.open(encoding="utf-8", newline="") as fh:
        return {r["name"]: r for r in csv.DictReader(fh)}


def number_list(path: Path, pattern: str) -> list[float]:
    m = re.search(pattern + r"\s*=\s*\[(.*?)\];", path.read_text(encoding="utf-8"), re.S)
    if m is None:
        raise ValueError(f"{path.name}: {pattern} not found")
    return [float(x) for x in re.findall(r"-?\d+(?:\.\d+)?(?:e-?\d+)?", m.group(1))]


def delay_ms(v: int, table: list[int]) -> float:
    """Fractional delay index (= ms) of the `kDlyRateClkDiv` value `v` (`delay_index`)."""
    i = len(table) - 1
    while i != 28 and v > table[i - 46]:
        i -= 46
    while v > table[i]:
        i -= 1
    if v == table[i]:
        return float(i)
    return i + (table[i] - v) / (table[i] - table[i + 1])


def literal(text: str) -> float | str:
    """A Rust literal of a default: a number, an arithmetic of numbers, a bool or a path."""
    t = text.strip().rstrip(",").split("//")[0].strip()
    if t in ("true", "false"):
        return 1.0 if t == "true" else 0.0
    num = t.replace("_", "")
    if re.fullmatch(r"[-0-9.e+*/ ()]+", num):
        return float(eval(num, {"__builtins__": {}}))  # noqa: S307 - digits and operators only
    return t


def struct_defaults(path: Path, anchor: str, back: bool = False) -> dict[str, float | str]:
    """`field: literal,` pairs of the `Self { .. }` literal after (or around) `anchor`."""
    text = path.read_text(encoding="utf-8")
    at = text.index(anchor)
    start = text.rfind("Self {", 0, at) if back else text.index("Self {", at)
    body = text[start : text.index("\n        }\n", start)]
    lets = {
        m.group(1): float(m.group(2))
        for m in re.finditer(r"let (\w+) = ([0-9.]+);", text[: text.index(anchor) + 400])
    }
    out: dict[str, float | str] = {}
    for m in re.finditer(r"^\s+(\w+): ([^\n]+?),?\s*(?://.*)?$", body, re.M):
        v = literal(m.group(2))
        out[m.group(1)] = lets.get(str(v), v) if isinstance(v, str) else v
    return out


def emulator_defaults() -> dict[str, dict[str, float | str]]:
    return {
        "chain": struct_defaults(FX / "mod.rs", "pub fn new(sample_rate: f32)"),
        "preset": struct_defaults(
            ROOT / "crates/nord-core/src/cbin/fx_preset.rs", "impl Default for FxDecoded"
        ),
        "organ": struct_defaults(
            ROOT / "crates/nord-core/src/organ/mod.rs", "rotary_enabled: true,", back=True
        ),
    }


def convert(row: Row, p: dict[str, str], eq_hz: list[float], table: list[int]) -> float:
    d, lo, hi = float(p["default"]), float(p["min"]), float(p["max"])
    if row.unit == "frac":
        return d / 127.0
    if row.unit == "frac14":
        return d / 16383.0
    if row.unit == "db":
        return d * 0.25
    if row.unit == "eq_hz":
        return eq_hz[int(d)]
    if row.unit == "delay_ms":
        return delay_ms(int(d), table)
    if row.unit == "rvb_type":
        return d
    return d if hi >= lo else math.nan


def status(row: Row, fw: float, emu: float | str | None) -> tuple[str, str]:
    if row.unit == "skip":
        return "not-compared", row.note
    if emu is None:
        return "not-compared", "no emulator field"
    if isinstance(emu, str):
        return "representation", row.note or f"emulator value `{emu}`"
    if row.unit in ("frac", "frac14"):
        ok = abs(fw - emu) <= COUNT
    elif row.unit in ("eq_hz", "delay_ms"):
        ok = abs(fw - emu) <= REL * max(abs(fw), abs(emu))
    else:
        ok = abs(fw - emu) < 1e-9
    return ("match" if ok else "differs"), row.note


def build() -> list[dict[str, str]]:
    params = read_params()
    emu = emulator_defaults()
    eq_hz = number_list(FX / "tables.rs", r"EQ_FREQ_HZ: \[f32; 128\]")
    table = [
        int(x)
        for x in number_list(
            FX / "fw_delay_table.rs", r"FW_DELAY_TABLE: \[i16; FW_DELAY_TABLE_LEN\]"
        )
    ]
    out = []
    for row in ROWS:
        p = params[row.edt]
        fw = convert(row, p, eq_hz, table)
        e = emu[row.source].get(row.field)
        st, note = status(row, fw, e)
        if row.edt.endswith("kRvbType") or row.field == "reverb_model":
            st, note = (
                ("match", "Stage")
                if (row.unit == "rvb_type" and fw == 6 and e == "ReverbModel::Stage")
                else (st, note)
            )
        out.append(
            {
                "block": row.block,
                "edt_name": row.edt,
                "fw_raw": p["default"],
                "unit": row.unit,
                "fw_value": f"{fw:.6g}",
                "source": row.source,
                "emu_field": row.field,
                "emu_default": f"{e:.6g}" if isinstance(e, float) else str(e),
                "status": st,
                "note": "; ".join(
                    x
                    for x in (
                        note,
                        "applied by apply_firmware_defaults"
                        if row.apply and st == "differs"
                        else "",
                    )
                    if x
                ),
            }
        )
    return out


def csv_text(rows: list[dict[str, str]]) -> str:
    import io

    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=FIELDS, lineterminator="\n")
    w.writeheader()
    w.writerows(rows)
    return buf.getvalue()


def rust_text(rows: list[dict[str, str]]) -> str:
    """The firmware defaults of the `chain` fields `apply_firmware_defaults` writes."""
    applied = {r.field for r in ROWS if r.apply and r.source == "chain"}
    sel = [r for r in rows if r["source"] == "chain" and r["emu_field"] in applied]
    out = [
        "//! The firmware defaults of the FX chain fields `apply_firmware_defaults` writes.",
        "//!",
        "//! GENERATED by `python tools/fx_defaults.py`; do not edit. VERIFIED: the `default`",
        "//! column of `docs/research/arm-edit-params.csv` (ARM edit table) converted with the",
        "//! verified laws (knob fraction `v / 127`, delay index table, mid frequency table);",
        "//! `docs/research/data/fx-defaults.csv` lists every compared id.",
        "",
        "/// `(field, firmware default in the emulator unit)`.",
        f"pub(super) const FW_DEFAULTS: [(&str, f32); {len(sel)}] = [",
    ]
    out += [f'    ("{r["emu_field"]}", {float(r["fw_value"])!r}),' for r in sel]
    out += [
        "];",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    use super::FW_DEFAULTS;",
        "",
        "    #[test]",
        "    fn the_table_has_finite_values_and_unique_fields() {",
        "        for (i, (name, v)) in FW_DEFAULTS.iter().enumerate() {",
        '            assert!(v.is_finite() && *v >= 0.0, "{name}");',
        '            assert!(FW_DEFAULTS[..i].iter().all(|(n, _)| n != name), "{name}");',
        "        }",
        "    }",
        "}",
        "",
    ]
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--check", action="store_true")
    ns = ap.parse_args(argv)
    rows = build()
    produced = {DATA: csv_text(rows), RUST: rust_text(rows)}
    stale = []
    for path, text in produced.items():
        if ns.check:
            if not path.exists() or path.read_text(encoding="utf-8").replace("\r\n", "\n") != text:
                stale.append(path.name)
        else:
            path.write_text(text, encoding="utf-8", newline="\n")
    if stale:
        print("stale:", ", ".join(stale))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
