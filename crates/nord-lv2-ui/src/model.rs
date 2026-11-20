//! Platform-independent UI state and mouse interaction: values, drag, program selector and the
//! sync from the plugin's state outputs. Geometry and the control table are in [`crate::layout`].

pub use crate::layout::*;

/// Idle calls (~30 per second) during which plugin output for a control the user just changed is
/// ignored, so the old value does not snap a knob back while the plugin catches up.
const SUPPRESS_IDLES: u8 = 10;

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

/// Which part of the program selector is under the mouse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgHit {
    Prev,
    Name,
    Next,
}

struct Drag {
    ctrl: usize,
    start_y: i32,
    start_norm: f32,
}

pub struct Ui {
    pub controls: [Control; 15],
    pub values: [f32; PORT_COUNT],
    /// Display names of the programs, program N = entry N-1 (same order as the plugin).
    pub programs: Vec<String>,
    drag: Option<Drag>,
    suppress: [u8; PORT_COUNT],
}

impl Default for Ui {
    fn default() -> Self {
        Self::with_programs(Vec::new())
    }
}

impl Ui {
    pub fn with_programs(programs: Vec<String>) -> Self {
        let controls = controls();
        let mut values = [0.0; PORT_COUNT];
        for c in &controls {
            values[c.port as usize] = c.default;
        }
        Self {
            controls,
            values,
            programs,
            drag: None,
            suppress: [0; PORT_COUNT],
        }
    }

    /// Program now playing, as reported by the plugin.
    pub fn program(&self) -> u32 {
        self.values[PROGRAM_OUT_PORT as usize].max(0.0).round() as u32
    }

    /// Text of the program box.
    pub fn program_label(&self) -> String {
        match self.program() {
            0 if self.programs.is_empty() => {
                "000  Engine default (no .ns4p files found)".to_string()
            }
            0 => format!("000  Engine default   ({} programs)", self.programs.len()),
            n => match self.programs.get(n as usize - 1) {
                Some(name) => format!("{n:03}  {name}"),
                None => format!("{n:03}  (not in the program folder)"),
            },
        }
    }

    pub fn prog_hit(&self, x: i32, y: i32) -> Option<ProgHit> {
        if !(PROG_Y..PROG_Y + PROG_H).contains(&y) {
            return None;
        }
        let inside = |x0: i32, w: i32| (x0..x0 + w).contains(&x);
        if inside(PROG_PREV_X, PROG_BTN_W) {
            Some(ProgHit::Prev)
        } else if inside(PROG_NEXT_X, PROG_BTN_W) {
            Some(ProgHit::Next)
        } else if inside(PROG_BOX_X, PROG_BOX_W) {
            Some(ProgHit::Name)
        } else {
            None
        }
    }

    /// Plugin → UI value of any port (inputs echoed by the host, and outputs). Returns whether to
    /// redraw, and an input write that brings the plugin's input port in line with its real state
    /// (after a program load changed layers), so the next click starts from what is shown.
    pub fn host_value(&mut self, port: u32, v: f32) -> (bool, Option<(u32, f32)>) {
        let out_layers = (6 + OUT_OFFSET)..(20 + OUT_OFFSET);
        if out_layers.contains(&port) {
            let input = port - OUT_OFFSET;
            let dragging = self
                .drag
                .as_ref()
                .is_some_and(|d| self.controls[d.ctrl].port == input);
            if dragging || self.suppress[input as usize] > 0 {
                return (false, None);
            }
            let changed = self.set_port(input, v);
            return (changed, changed.then_some((input, v)));
        }
        if self.suppress.get(port as usize).is_some_and(|&s| s > 0) {
            return (false, None);
        }
        (self.set_port(port, v), None)
    }

    /// Call once per idle; ends the suppression window of recent user writes.
    pub fn tick(&mut self) {
        for s in self.suppress.iter_mut() {
            *s = s.saturating_sub(1);
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
                if let Some(h) = self.prog_hit(x, y) {
                    return self.program_input(h, button);
                }
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
        let changed = self.set_port(port, v);
        if changed {
            self.suppress[port as usize] = SUPPRESS_IDLES;
        }
        changed.then_some((port, v))
    }

    /// `<` / `>` step, wheel over the name steps, right-click on the name goes to 0.
    fn program_input(&mut self, h: ProgHit, button: u32) -> Option<(u32, f32)> {
        let cur = self.program() as i64;
        let last = (self.programs.len() as i64).min(PROGRAM_MAX as i64);
        let next = match (h, button) {
            (ProgHit::Prev, 1) | (ProgHit::Name, 5) => cur - 1,
            (ProgHit::Next, 1) | (ProgHit::Name, 4) => cur + 1,
            (ProgHit::Name, 3) => 0,
            _ => return None,
        };
        let next = next.clamp(0, last);
        if next == cur {
            return None;
        }
        // Always send: the program input may already hold `next` (e.g. after a Program Change).
        let v = next as f32;
        self.values[PROGRAM_PORT as usize] = v;
        self.suppress[PROGRAM_PORT as usize] = SUPPRESS_IDLES;
        Some((PROGRAM_PORT, v))
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
    fn host_updates() {
        let mut ui = Ui::default();
        assert!(ui.set_port(13, 0.5));
        assert!(!ui.set_port(13, 0.5));
        assert!(!ui.set_port(99, 1.0));
    }

    #[test]
    fn program_selector_steps_clamps_and_labels() {
        let mut ui = Ui::with_programs(vec!["Organ".into(), "Piano".into()]);
        let at = |h: ProgHit| match h {
            ProgHit::Prev => (PROG_PREV_X + 3, PROG_Y + 3),
            ProgHit::Name => (PROG_BOX_X + 10, PROG_Y + 3),
            ProgHit::Next => (PROG_NEXT_X + 3, PROG_Y + 3),
        };
        let press = |h, button| {
            let (x, y) = at(h);
            Input::Press {
                x,
                y,
                button,
                fine: false,
            }
        };
        assert_eq!(ui.prog_hit(at(ProgHit::Name).0, 50), None);
        assert_eq!(ui.program_label(), "000  Engine default   (2 programs)");
        assert_eq!(ui.input(press(ProgHit::Prev, 1)), None, "already at 0");
        assert_eq!(ui.input(press(ProgHit::Next, 1)), Some((PROGRAM_PORT, 1.0)));
        assert_eq!(
            ui.program(),
            0,
            "display follows the plugin's output, not the click"
        );
        ui.host_value(PROGRAM_OUT_PORT, 2.0);
        assert_eq!(ui.program_label(), "002  Piano");
        assert_eq!(ui.input(press(ProgHit::Next, 1)), None, "last program");
        assert_eq!(ui.input(press(ProgHit::Name, 5)), Some((PROGRAM_PORT, 1.0)));
        assert_eq!(ui.input(press(ProgHit::Name, 3)), Some((PROGRAM_PORT, 0.0)));
        ui.host_value(PROGRAM_OUT_PORT, 7.0);
        assert_eq!(ui.program_label(), "007  (not in the program folder)");
        assert_eq!(
            Ui::default().program_label(),
            "000  Engine default (no .ns4p files found)"
        );
    }

    #[test]
    fn plugin_state_outputs_sync_display_and_inputs() {
        let mut ui = Ui::default();
        // a program switched Organ B on: show it and align the input port
        assert_eq!(ui.host_value(7 + OUT_OFFSET, 1.0), (true, Some((7, 1.0))));
        assert_eq!(ui.host_value(7 + OUT_OFFSET, 1.0), (false, None));
        assert_eq!(ui.values[7], 1.0);
        // while the user turns a knob, stale output does not snap it back
        let k = ui.controls[8];
        ui.input(Input::Press {
            x: k.cx,
            y: k.cy,
            button: 1,
            fine: false,
        });
        ui.input(Input::Motion {
            x: k.cx,
            y: k.cy - 40,
            fine: false,
        });
        assert_eq!(
            ui.host_value(13 + OUT_OFFSET, 0.8),
            (false, None),
            "dragging"
        );
        ui.input(Input::Release);
        assert_eq!(
            ui.host_value(13 + OUT_OFFSET, 0.8),
            (false, None),
            "just written"
        );
        for _ in 0..SUPPRESS_IDLES {
            ui.tick();
        }
        assert_eq!(ui.host_value(13 + OUT_OFFSET, 0.5), (true, Some((13, 0.5))));
        // plain input echoes from the host only update the display
        assert_eq!(ui.host_value(5, -3.0), (true, None));
    }
}
