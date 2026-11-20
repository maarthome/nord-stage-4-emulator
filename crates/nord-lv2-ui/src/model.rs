//! Platform-independent UI model: control table, panel geometry, values and mouse interaction.
//!
//! Port indices, ranges and defaults mirror `crates/nord-lv2/bundle/nord-stage4.lv2/nord-stage4.ttl`
//! (checked by a test that reads the TTL), so the GUI never drifts from the plugin.

pub const PORT_COUNT: usize = 20;
pub const WIDTH: i32 = 730;
pub const HEIGHT: i32 = 240;

pub const HEADER_H: i32 = 40;
pub const PANEL_TOP: i32 = 52;
pub const PANEL_BOTTOM: i32 = HEIGHT - 12;
pub const GROUP_LABEL_Y: i32 = 72;
pub const TOGGLE_Y: i32 = 102;
pub const TOGGLE_W: i32 = 52;
pub const TOGGLE_H: i32 = 26;
pub const KNOB_Y: i32 = 162;
pub const KNOB_R: i32 = 26;
pub const VALUE_Y: i32 = 212;

// The value text sits below the knob arc and inside the group panel (checked at compile time).
const _: () = assert!(VALUE_Y < PANEL_BOTTOM && KNOB_Y + KNOB_R < VALUE_Y - 8);

const MARGIN: i32 = 16;
const COL_W: i32 = 80;
const GAP: i32 = 14;
/// Master column: one group gap after Synth C (`layer_x` adds that gap for index 7).
const MASTER_X: i32 = layer_x(7);
const MASTER_W: i32 = 96;

/// Left edge of layer column `i` (0..7): Organ A,B | Piano A,B | Synth A,B,C, with a gap
/// between instrument groups.
pub const fn layer_x(i: i32) -> i32 {
    let gaps =
        if i >= 2 { GAP } else { 0 } + if i >= 4 { GAP } else { 0 } + if i >= 7 { GAP } else { 0 };
    MARGIN + i * COL_W + gaps
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Toggle,
    Knob { min: f32, max: f32 },
}

#[derive(Debug, Clone, Copy)]
pub struct Control {
    pub port: u32,
    pub symbol: &'static str,
    /// Text on the toggle button, or the caption above a knob that has no toggle.
    pub label: &'static str,
    pub kind: Kind,
    pub default: f32,
    /// Centre of the toggle button or knob.
    pub cx: i32,
    pub cy: i32,
}

pub struct Group {
    pub name: &'static str,
    pub x: i32,
    pub w: i32,
}

pub const GROUPS: [Group; 4] = [
    Group {
        name: "ORGAN",
        x: layer_x(0),
        w: 2 * COL_W,
    },
    Group {
        name: "PIANO",
        x: layer_x(2),
        w: 2 * COL_W,
    },
    Group {
        name: "SYNTH",
        x: layer_x(4),
        w: 3 * COL_W,
    },
    Group {
        name: "MASTER",
        x: MASTER_X,
        w: MASTER_W,
    },
];

const LAYER: [(&str, &str); 7] = [
    ("organ_a", "A"),
    ("organ_b", "B"),
    ("piano_a", "A"),
    ("piano_b", "B"),
    ("synth_a", "A"),
    ("synth_b", "B"),
    ("synth_c", "C"),
];
const LAYER_ON_DEFAULT: [bool; 7] = [true, false, true, false, true, false, false];
const ON_SYMBOLS: [&str; 7] = [
    "organ_a_on",
    "organ_b_on",
    "piano_a_on",
    "piano_b_on",
    "synth_a_on",
    "synth_b_on",
    "synth_c_on",
];
const LEVEL_SYMBOLS: [&str; 7] = [
    "organ_a_level",
    "organ_b_level",
    "piano_a_level",
    "piano_b_level",
    "synth_a_level",
    "synth_b_level",
    "synth_c_level",
];

pub const MASTER_PORT: u32 = 5;
pub const MASTER_MIN_DB: f32 = -60.0;
const LAYER_ON_FIRST: u32 = 6;
const LAYER_LEVEL_FIRST: u32 = 13;

/// All 15 controls: master, 7 toggles, 7 level knobs.
pub fn controls() -> [Control; 15] {
    let mut out = [Control {
        port: MASTER_PORT,
        symbol: "master",
        label: "LEVEL",
        kind: Kind::Knob {
            min: MASTER_MIN_DB,
            max: 6.0,
        },
        default: 0.0,
        cx: MASTER_X + MASTER_W / 2,
        cy: KNOB_Y,
    }; 15];
    for i in 0..7 {
        let cx = layer_x(i as i32) + COL_W / 2;
        out[1 + i] = Control {
            port: LAYER_ON_FIRST + i as u32,
            symbol: ON_SYMBOLS[i],
            label: LAYER[i].1,
            kind: Kind::Toggle,
            default: if LAYER_ON_DEFAULT[i] { 1.0 } else { 0.0 },
            cx,
            cy: TOGGLE_Y,
        };
        out[8 + i] = Control {
            port: LAYER_LEVEL_FIRST + i as u32,
            symbol: LEVEL_SYMBOLS[i],
            label: LAYER[i].0,
            kind: Kind::Knob { min: 0.0, max: 1.0 },
            default: 0.8,
            cx,
            cy: KNOB_Y,
        };
    }
    out
}

/// Text under a knob: levels on the panel's 0-10 scale, master in dB.
pub fn format_value(c: &Control, v: f32) -> String {
    match c.kind {
        Kind::Knob { .. } if c.port == MASTER_PORT => {
            if v <= MASTER_MIN_DB {
                "-inf dB".to_string()
            } else {
                format!("{v:.1} dB")
            }
        }
        Kind::Knob { min, max } => format!("{:.1}", 10.0 * (v - min) / (max - min)),
        Kind::Toggle => if v > 0.5 { "ON" } else { "OFF" }.to_string(),
    }
}

/// Knob position 0..1 for `v`.
pub fn knob_norm(kind: Kind, v: f32) -> f32 {
    match kind {
        Kind::Knob { min, max } => ((v - min) / (max - min)).clamp(0.0, 1.0),
        Kind::Toggle => v.clamp(0.0, 1.0),
    }
}

/// Mouse input in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    /// Button 1 = left, 3 = right (reset to default), 4/5 = wheel up/down.
    Press {
        x: i32,
        y: i32,
        button: u32,
        fine: bool,
    },
    Motion {
        x: i32,
        y: i32,
        fine: bool,
    },
    Release,
}

/// Pixels of vertical drag for a full knob sweep (×10 with the fine modifier, Shift).
pub const DRAG_PX: f32 = 200.0;
const WHEEL_STEP: f32 = 0.02;

struct Drag {
    ctrl: usize,
    start_y: i32,
    start_norm: f32,
}

pub struct Ui {
    pub controls: [Control; 15],
    pub values: [f32; PORT_COUNT],
    drag: Option<Drag>,
}

impl Default for Ui {
    fn default() -> Self {
        let controls = controls();
        let mut values = [0.0; PORT_COUNT];
        for c in &controls {
            values[c.port as usize] = c.default;
        }
        Self {
            controls,
            values,
            drag: None,
        }
    }
}

impl Ui {
    pub fn value(&self, c: &Control) -> f32 {
        self.values[c.port as usize]
    }

    /// Host → UI value update. Returns true when the panel must be redrawn.
    pub fn set_port(&mut self, port: u32, v: f32) -> bool {
        match self.values.get_mut(port as usize) {
            Some(slot) if *slot != v => {
                *slot = v;
                true
            }
            _ => false,
        }
    }

    /// Index of the control under (x, y).
    pub fn hit(&self, x: i32, y: i32) -> Option<usize> {
        self.controls.iter().position(|c| match c.kind {
            Kind::Toggle => (x - c.cx).abs() <= TOGGLE_W / 2 && (y - c.cy).abs() <= TOGGLE_H / 2,
            Kind::Knob { .. } => {
                let (dx, dy) = (x - c.cx, y - c.cy);
                dx * dx + dy * dy <= (KNOB_R + 8) * (KNOB_R + 8)
            }
        })
    }

    /// Handles one input event. Returns `(port, new value)` when a control changed; the caller
    /// sends it to the host and redraws.
    pub fn input(&mut self, ev: Input) -> Option<(u32, f32)> {
        match ev {
            Input::Press { x, y, button, fine } => {
                let i = self.hit(x, y)?;
                let c = self.controls[i];
                let cur = self.value(&c);
                let new = match (c.kind, button) {
                    (_, 3) => c.default,
                    (Kind::Toggle, 1) => {
                        if cur > 0.5 {
                            0.0
                        } else {
                            1.0
                        }
                    }
                    (Kind::Knob { .. }, 1) => {
                        self.drag = Some(Drag {
                            ctrl: i,
                            start_y: y,
                            start_norm: knob_norm(c.kind, cur),
                        });
                        return None;
                    }
                    (Kind::Knob { .. }, 4 | 5) => {
                        let step = if fine { WHEEL_STEP / 10.0 } else { WHEEL_STEP };
                        let dir = if button == 4 { 1.0 } else { -1.0 };
                        denorm(c.kind, knob_norm(c.kind, cur) + dir * step)
                    }
                    _ => return None,
                };
                self.commit(c.port, new)
            }
            Input::Motion { y, fine, .. } => {
                let d = self.drag.as_ref()?;
                let c = self.controls[d.ctrl];
                let px = if fine { DRAG_PX * 10.0 } else { DRAG_PX };
                let norm = d.start_norm + (d.start_y - y) as f32 / px;
                self.commit(c.port, denorm(c.kind, norm))
            }
            Input::Release => {
                self.drag = None;
                None
            }
        }
    }

    fn commit(&mut self, port: u32, v: f32) -> Option<(u32, f32)> {
        self.set_port(port, v).then_some((port, v))
    }
}

fn denorm(kind: Kind, n: f32) -> f32 {
    let n = n.clamp(0.0, 1.0);
    match kind {
        Kind::Knob { min, max } => min + n * (max - min),
        Kind::Toggle => n.round(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TTL: &str = include_str!("../../nord-lv2/bundle/nord-stage4.lv2/nord-stage4.ttl");

    /// The `[ ... ]` port block of the TTL that declares `symbol`.
    fn port_block(symbol: &str) -> &'static str {
        let at = TTL
            .find(&format!("lv2:symbol \"{symbol}\""))
            .unwrap_or_else(|| panic!("{symbol}"));
        let start = TTL[..at].rfind('[').unwrap();
        let end = at + TTL[at..].find(']').unwrap();
        &TTL[start..end]
    }

    fn number_after(block: &str, key: &str) -> f32 {
        let at = block
            .find(key)
            .unwrap_or_else(|| panic!("{key} in {block}"))
            + key.len();
        block[at..]
            .trim_start()
            .split(|c: char| c == ';' || c.is_whitespace())
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn controls_match_plugin_ttl() {
        for c in controls() {
            let b = port_block(c.symbol);
            assert_eq!(number_after(b, "lv2:index") as u32, c.port, "{}", c.symbol);
            assert_eq!(number_after(b, "lv2:default"), c.default, "{}", c.symbol);
            match c.kind {
                Kind::Knob { min, max } => {
                    assert_eq!(number_after(b, "lv2:minimum"), min, "{}", c.symbol);
                    assert_eq!(number_after(b, "lv2:maximum"), max, "{}", c.symbol);
                }
                Kind::Toggle => assert!(b.contains("lv2:toggled"), "{}", c.symbol),
            }
        }
    }

    #[test]
    fn layout_fits_and_controls_do_not_overlap() {
        let cs = controls();
        let last = &GROUPS[3];
        assert_eq!(last.x + last.w + MARGIN, WIDTH);
        for g in GROUPS.windows(2) {
            assert!(
                g[0].x + g[0].w < g[1].x,
                "{} overlaps {}",
                g[0].name,
                g[1].name
            );
        }
        let ui = Ui::default();
        for (i, c) in cs.iter().enumerate() {
            assert_eq!(ui.hit(c.cx, c.cy), Some(i), "{}", c.symbol);
            assert!(c.cx - KNOB_R > 0 && c.cx + KNOB_R < WIDTH);
        }
        assert_eq!(ui.hit(2, 2), None);
    }

    #[test]
    fn toggle_click_and_reset() {
        let mut ui = Ui::default();
        let organ_b = ui.controls[2];
        assert_eq!(organ_b.symbol, "organ_b_on");
        let press = |button| Input::Press {
            x: organ_b.cx,
            y: organ_b.cy,
            button,
            fine: false,
        };
        assert_eq!(ui.input(press(1)), Some((7, 1.0)));
        assert_eq!(ui.input(press(1)), Some((7, 0.0)));
        assert_eq!(ui.input(press(3)), None, "already at default");
    }

    #[test]
    fn knob_drag_wheel_fine_and_clamp() {
        let mut ui = Ui::default();
        let k = ui.controls[8]; // organ_a_level, default 0.8
        assert_eq!(
            ui.input(Input::Press {
                x: k.cx,
                y: k.cy,
                button: 1,
                fine: false
            }),
            None
        );
        let up = ui
            .input(Input::Motion {
                x: k.cx,
                y: k.cy - 20,
                fine: false,
            })
            .unwrap();
        assert_eq!(up.0, 13);
        assert!((up.1 - 0.9).abs() < 1e-6);
        let top = ui
            .input(Input::Motion {
                x: k.cx,
                y: k.cy - 500,
                fine: false,
            })
            .unwrap();
        assert_eq!(top.1, 1.0);
        ui.input(Input::Release);
        assert_eq!(
            ui.input(Input::Motion {
                x: 0,
                y: 0,
                fine: false
            }),
            None,
            "no drag after release"
        );

        let wheel = ui
            .input(Input::Press {
                x: k.cx,
                y: k.cy,
                button: 5,
                fine: true,
            })
            .unwrap();
        assert!((wheel.1 - 0.998).abs() < 1e-6);
        assert_eq!(
            ui.input(Input::Press {
                x: k.cx,
                y: k.cy,
                button: 3,
                fine: false
            }),
            Some((13, 0.8))
        );

        let m = ui.controls[0];
        let (port, v) = ui
            .input(Input::Press {
                x: m.cx,
                y: m.cy,
                button: 4,
                fine: false,
            })
            .unwrap();
        assert_eq!(port, MASTER_PORT);
        assert!((v - (-60.0 + (60.0 / 66.0 + 0.02) * 66.0)).abs() < 1e-4);
    }

    #[test]
    fn host_updates_and_formatting() {
        let mut ui = Ui::default();
        assert!(ui.set_port(13, 0.5));
        assert!(!ui.set_port(13, 0.5));
        assert!(!ui.set_port(99, 1.0));
        let cs = controls();
        assert_eq!(format_value(&cs[8], 0.5), "5.0");
        assert_eq!(format_value(&cs[0], -60.0), "-inf dB");
        assert_eq!(format_value(&cs[0], -6.04), "-6.0 dB");
        assert_eq!(format_value(&cs[1], 1.0), "ON");
        assert_eq!(knob_norm(cs[0].kind, 6.0), 1.0);
    }
}
