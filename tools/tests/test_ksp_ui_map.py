# ruff: noqa: E501  (KSP fixtures keep real script lines)
import csv
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import ksp_ui_map as k  # noqa: E402

SCRIPT = """on init
declare ui_slider $a(0,1000000)
end on
on ui_control($qdil3)
set_engine_par($ENGINE_PAR_SENDLEVEL_1,$qdil3,-1,7,$NI_BUS_OFFSET+$qgbyb)
set_control_par_str(get_ui_id($1ioyu),$CONTROL_PAR_TEXT,"Reverb Amount [A]")
set_control_par_str(get_ui_id($vffwi),$CONTROL_PAR_TEXT,$qdil3*10/62996/10 & "." & ($qdil3*10/62996 mod 10))
end on
on ui_control($plain)
set_control_par(get_ui_id($x),$CONTROL_PAR_HIDE,1)
end on
on ui_control($snj2w)
set_engine_par($ENGINE_PAR_SENDLEVEL_0,$snj2w,-1,7,$NI_BUS_OFFSET+$qgbyb)
set_control_par_str(get_ui_id($1ioyu),$CONTROL_PAR_TEXT,"Delay Amount [B]")
end on
"""


def _saved():
    return b"\x00\x00\x00\x00\x08\x00\x00\x00$qdil3 500000\x07\x00\x00\x00$zzzzz -4\x08\x00\x00\x00$qdil3 9"


def test_handlers_split_and_plain_handlers_skipped():
    hs = k.parse_handlers(SCRIPT)
    assert [v for v, _ in hs] == ["$qdil3", "$plain", "$snj2w"]
    cs = k.build_controls(SCRIPT, {})
    assert [c.var for c in cs] == ["$qdil3", "$snj2w"]


def test_engine_label_and_display_extracted():
    c = k.build_controls(SCRIPT, {})[0]
    assert c.engine_par == "$ENGINE_PAR_SENDLEVEL_1"
    assert c.args == "$qdil3,-1,7,$NI_BUS_OFFSET+$qgbyb"
    assert c.label == "Reverb Amount [A]"
    assert c.display_formula == '$qdil3*10/62996/10 & "." & ($qdil3*10/62996 mod 10)'
    assert k.build_controls(SCRIPT, {})[1].display_formula == ""


def test_saved_values_first_occurrence_wins_and_negative_ok():
    vals = k.parse_saved_values(_saved())
    assert vals == {"$qdil3": 500000, "$zzzzz": -4}
    cs = k.build_controls(SCRIPT, vals)
    assert cs[0].saved_value == 500000 and cs[1].saved_value is None


def test_csv_and_cli(tmp_path, capsys):
    data = SCRIPT.encode("latin1") + _saved()
    src = tmp_path / "dec.bin"
    src.write_bytes(data)
    out = tmp_path / "map.csv"
    assert k.main([str(src), str(len(SCRIPT)), str(out)]) == 0
    assert "2 engine-parameter controls, 1 with saved values" in capsys.readouterr().out
    rows = list(csv.DictReader(out.open(encoding="utf-8")))
    assert rows[0]["label"] == "Reverb Amount [A]" and rows[0]["saved_value"] == "500000"
    assert rows[1]["saved_value"] == ""
