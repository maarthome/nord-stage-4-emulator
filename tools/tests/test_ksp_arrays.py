import csv
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import ksp_arrays as ka  # noqa: E402

SCRIPT = """on init
declare %rate[4] := (0, 100, 200,
300)
declare %path[10] := (47, 76, 105, 98, 114, 97, 114, 121, 47, 120)
declare %cont[4] := (1, 2, ...
3, 4)
declare %amt[2] := (5, 6)
end on
function fx
set_engine_par($ENGINE_PAR_CHORAL_RATE,%rate[%idx[$b]],-1,3,$NI_BUS_OFFSET+$b)
set_engine_par($ENGINE_PAR_CHORAL_MIX,%amt[$b]*46000,-1,3,$NI_BUS_OFFSET+$b)
set_engine_par($ENGINE_PAR_PHASIS_RATE,%rate[1],-1,3,0)
end function
"""


def test_parse_arrays_multiline_and_text():
    arrs = {a.name: a for a in ka.parse_arrays(SCRIPT)}
    assert arrs["rate"].values == [0, 100, 200, 300] and arrs["rate"].size == 4
    assert arrs["path"].is_text and arrs["path"].text == "/Library/x"
    assert not arrs["rate"].is_text and arrs["rate"].text == ""
    assert not arrs["amt"].is_text  # too short to be a string
    assert arrs["cont"].values == [1, 2, 3, 4]


def test_engine_par_uses_maps_arrays_to_parameters():
    uses = ka.engine_par_uses(SCRIPT)
    assert uses["rate"] == ["ENGINE_PAR_CHORAL_RATE", "ENGINE_PAR_PHASIS_RATE"]
    assert uses["amt"] == ["ENGINE_PAR_CHORAL_MIX"]
    assert uses["idx"] == ["ENGINE_PAR_CHORAL_RATE"]


def test_bad_value_list_is_skipped():
    assert ka.parse_arrays("declare %bad[2] := (1, x)\n") == []


def test_csv_and_cli(tmp_path, capsys):
    src = tmp_path / "s.ksp"
    src.write_text(SCRIPT, encoding="utf-8")
    out = tmp_path / "a.csv"
    assert ka.main([str(src), str(out)]) == 0
    assert "4 arrays, 1 text, 2 feed engine params" in capsys.readouterr().out
    rows = {r["name"]: r for r in csv.DictReader(out.open(encoding="utf-8"))}
    assert rows["rate"]["values"] == "0 100 200 300"
    assert rows["rate"]["engine_par"] == "ENGINE_PAR_CHORAL_RATE ENGINE_PAR_PHASIS_RATE"
    assert rows["path"]["text"] == "/Library/x"
