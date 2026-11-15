import os
import sys

import pytest
from PIL import Image

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import layout_overlay as lo  # noqa: E402

RECT = {"x": 10.0, "y": 20.0, "w": 100.0, "h": 50.0}


def test_fit_axis_exact_and_errors():
    a, b = lo.fit_axis([(0, 5), (10, 25)])
    assert (a, b) == pytest.approx((2.0, 5.0))
    with pytest.raises(ValueError):
        lo.fit_axis([(1, 1)])
    with pytest.raises(ValueError):
        lo.fit_axis([(1, 1), (1, 2)])


def test_mapping_round_trip_and_anisotropy():
    m = lo.Mapping(lo.outline_anchors((0, 0, 200, 100), RECT))
    assert m.to_mm(0, 0) == pytest.approx((10.0, 20.0))
    assert m.to_mm(200, 100) == pytest.approx((110.0, 70.0))
    assert m.to_px(*m.to_mm(37, 41)) == pytest.approx((37, 41))
    assert m.anisotropy() == pytest.approx(1.0)
    tall = lo.Mapping(lo.outline_anchors((0, 0, 200, 50), RECT))
    assert tall.anisotropy() == pytest.approx(2.0)


def test_parse_anchors():
    assert lo.parse_anchors("1,2,3,4; 5,6,7,8") == [(1, 2, 3, 4), (5, 6, 7, 8)]
    with pytest.raises(ValueError):
        lo.parse_anchors("1,2,3")


def test_offsets_use_gui_rect_when_present_and_flag_over_threshold():
    m = lo.Mapping(lo.outline_anchors((0, 0, 100, 50), RECT))  # 1 mm per px
    ctl = [
        {
            "id": "a.b",
            "rect": {"x": 10, "y": 20, "w": 2, "h": 2},
            "gui": {"x": 20, "y": 20, "w": 2, "h": 2},
        },
        {"id": "a.c", "rect": {"x": 10, "y": 20, "w": 2, "h": 2}},
    ]
    photo = {"a.b": (11.0, 1.0), "a.c": (1.0, 1.0), "zzz": (0, 0)}
    rows = {r[0]: r for r in lo.compute_offsets(ctl, photo, m, "gui")}
    assert rows["a.b"][3] == pytest.approx(0.0)  # gui centre (21, 21) is the real one
    assert rows["a.c"][3] == pytest.approx(0.0)
    layout_rows = {r[0]: r for r in lo.compute_offsets(ctl, photo, m, "layout")}
    assert layout_rows["a.b"][1] == pytest.approx(-10.0)
    text = lo.format_report(list(layout_rows.values()))
    assert "a.b" in text and " !" in text and "2 controls, 1 over" in text


def test_load_photo_anchors(tmp_path):
    p = tmp_path / "a.csv"
    p.write_text("id,photo_x,photo_y\nx.y,1.5,2\n", encoding="utf-8")
    assert lo.load_photo_anchors(str(p)) == {"x.y": (1.5, 2.0)}


def test_committed_anchor_csvs_parse_and_match_the_inventory_naming():
    for sec in ("organ", "master"):
        rows = lo.load_photo_anchors(os.path.join(lo.DATA_DIR, f"photo_anchors_{sec}.csv"))
        assert rows
        assert all(0 < x < 4000 and 0 < y < 4000 for x, y in rows.values())


def test_draw_overlay_and_side_by_side_sizes():
    img = Image.new("RGB", (200, 100), (0, 0, 0))
    m = lo.Mapping(lo.outline_anchors((0, 0, 200, 100), RECT))
    ctl = [{"id": "s.k", "rect": {"x": 20, "y": 30, "w": 10, "h": 10}, "estimated": True}]
    out = lo.draw_overlay(img, ctl, {"s.k": (50.0, 50.0)}, m, "layout", [{"rect": RECT}])
    assert out.size == (200, 100) and out.getpixel((50, 50)) != (0, 0, 0)
    both = lo.side_by_side(out, Image.new("RGB", (50, 50)))
    assert both.height == 50
    assert both.width == 100 + 50 + 10


def test_build_mapping_uses_outline_or_anchors():
    layout = {"sections": {"organ": RECT}}
    m2 = lo.build_mapping(layout, "organ", "0,0,0,0;10,10,5,5")
    assert m2.ax == pytest.approx(0.5)
    with pytest.raises(SystemExit):
        lo.build_mapping(layout, "effects", "")
    m3 = lo.Mapping(lo.outline_anchors((250.0, 12.0, 1948.0, 1168.0), RECT))
    assert m3.to_mm(250, 12) == pytest.approx((10.0, 20.0))


def test_find_crops_dir_walks_up_and_honours_env(tmp_path, monkeypatch):
    monkeypatch.delenv("NS4_CROPS", raising=False)
    (tmp_path / ".reference" / "crops").mkdir(parents=True)
    deep = tmp_path / "a" / "b"
    deep.mkdir(parents=True)
    assert lo.find_crops_dir(str(deep)) == str(tmp_path / ".reference" / "crops")
    monkeypatch.setenv("NS4_CROPS", "/x")
    assert lo.find_crops_dir(str(deep)) == "/x"


def test_topdown_crops_map_isotropically_onto_their_section():
    """`program`, `keybed`, `wheels` use the straight-on HA88 render: one mm/px on both axes."""
    rects = {
        "program": {"x": 535.367, "y": 0.0, "w": 130.038, "h": 179.147},
        "keybed": {"x": 0.0, "y": 179.147, "w": 1290.0, "h": 159.78},
        "wheels": {"x": 0.0, "y": 0.0, "w": 240.708, "h": 179.147},
    }
    for name, rect in rects.items():
        file, _sec, outline, _secs, window = lo.CROPS[name]
        assert file.startswith("../") and window is not None
        m = lo.Mapping(lo.outline_anchors(outline, rect))
        assert abs(m.anisotropy() - 1.0) < 0.01, name
        assert abs(m.ax - 0.1195) < 0.001, name


def test_organ_and_piano_crops_map_isotropically_onto_the_render():
    """`organ` / `piano` / `master` use the crop registration onto the HA88 render (`ISO`)."""
    for name in ("organ", "piano"):
        m = lo.build_mapping({"sections": {}}, name, "")
        assert m.anisotropy() == pytest.approx(1.0)
        assert abs(m.ax - 0.1195) < 0.0015, name
    assert lo.SECTION_CROP["master"] == "organ"
    # The section top edge (render y 691 px = 25.0 mm) lands on the same mm row in both crops.
    organ = lo.Mapping.isotropic(*lo.ISO["organ"])
    piano = lo.Mapping.isotropic(*lo.ISO["piano"])
    assert organ.to_mm(0, 12)[1] == pytest.approx(25.0, abs=0.3)
    assert piano.to_mm(0, 17)[1] == pytest.approx(25.0, abs=0.3)
    # Synth: same plate top (was 26.0 mm, 8.4 px lower); program / keybed / wheels are render crops.
    synth = lo.Mapping.isotropic(*lo.ISO["synth"])
    assert synth.to_mm(0, 12)[1] == pytest.approx(25.0, abs=0.05)
    assert synth.anisotropy() == pytest.approx(1.0)
    for name in ("program", "keybed", "wheels"):
        assert lo.Mapping.isotropic(*lo.ISO[name]).to_mm(400, 482) == pytest.approx((0.0, 0.0))
    px = organ.to_px(*organ.to_mm(655.0, 895.0))
    assert px == pytest.approx((655.0, 895.0))
