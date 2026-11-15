//! Shift / Hold function legends of the Master and Organ bays: the name printed below the control
//! a function hangs off (or beside it), with an optional state LED.

use super::ctl::{led_small, mark, mark_dim, LEGEND_LED_GAP};
use super::prelude::*;

/// Distance (px) from the button bottom to the top of its Shift legend: with the legend LED
/// (radius 1.4) beside the text it keeps 2.5 px clear of the bezel (audit rule `B`).
const LEGEND_DROP: f32 = 2.3;

/// Distance (px) from the right edge of a button to a legend printed beside it.
const RIGHT_LEGEND_GAP: f32 = 7.0;

/// Shift / Hold function legend printed below `base` (the control it hangs off), with an
/// optional state LED. `dim` greys it out and shows the missing-feature tooltip.
pub(super) fn shift_legend(
    ui: &mut Ui,
    base: Rect,
    id: &str,
    text: &str,
    lit: Option<bool>,
    dim: Option<&str>,
) {
    shift_legend_at(ui, base, id, text, lit, dim, false);
}

/// Like [`shift_legend`], but printed to the right of `base` when `right` (as the real panel
/// prints "SOLO" beside the ON button of a layer section: the box title sits right under it).
pub(super) fn shift_legend_at(
    ui: &mut Ui,
    base: Rect,
    id: &str,
    text: &str,
    lit: Option<bool>,
    dim: Option<&str>,
    right: bool,
) {
    let col = if dim.is_some() {
        dim_legend(Color32::WHITE)
    } else {
        Color32::WHITE
    };
    let p = ui.painter().clone();
    let drawn = if right {
        let pos = pos2(
            base.right() + RIGHT_LEGEND_GAP + if lit.is_some() { LEGEND_LED_GAP } else { 0.0 },
            base.center().y,
        );
        label(&p, pos, text, 5.5, col, Align2::LEFT_CENTER)
    } else {
        let pos = pos2(base.center().x, base.bottom() + LEGEND_DROP);
        label(&p, pos, text, 5.5, col, Align2::CENTER_TOP)
    };
    if let Some(l) = lit {
        // State LED before the text, as printed on the instrument ("o SUSTPED"); keeps it away
        // from a group frame that ends at the right of the legend.
        let x = drawn.left() - LEGEND_LED_GAP;
        led_small(
            &p,
            pos2(x, drawn.center().y),
            l,
            LED_AMBER,
            LED_AMBER_DIM,
            1.4,
        );
    }
    let hit = if right {
        drawn.expand2(vec2(LEGEND_LED_GAP, 2.0))
    } else {
        Rect::from_min_size(pos2(base.center().x - 12.0, base.bottom()), vec2(24.0, 8.0))
    };
    let resp = ui.interact(hit, ui.id().with((id, "legend")), Sense::hover());
    mark(ui, id);
    match dim {
        Some(t) => {
            mark_dim(ui, id);
            resp.on_hover_text(t);
        }
        None => {
            resp.on_hover_text(format!("Shift / hold: {text}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::epaint::Shape;

    fn text_rect(right: bool) -> Rect {
        let btn = Rect::from_min_size(pos2(100.0, 100.0), vec2(37.0, 22.0));
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                shift_legend_at(ui, btn, "t", "SOLO", None, None, right);
            });
        });
        out.shapes
            .iter()
            .find_map(|c| match &c.shape {
                Shape::Text(t) => Some(t.galley.rect.translate(t.pos.to_vec2())),
                _ => None,
            })
            .expect("a legend text")
    }

    #[test]
    fn right_legend_sits_right_of_the_button_and_the_default_one_under_it() {
        let btn = Rect::from_min_size(pos2(100.0, 100.0), vec2(37.0, 22.0));
        let r = text_rect(true);
        assert!(r.left() >= btn.right() + RIGHT_LEGEND_GAP - 0.5, "{r:?}");
        assert!((r.center().y - btn.center().y).abs() < 1.0, "{r:?}");
        let u = text_rect(false);
        assert!(u.top() >= btn.bottom(), "{u:?}");
        assert!((u.center().x - btn.center().x).abs() < 1.0, "{u:?}");
    }
}
