//! X11 window and drawing of the panel (Linux only). Owns its own X connection, as LV2 UIs do;
//! all calls happen on the host's UI thread (instantiate, idle, cleanup).

use crate::model::{self, Input, Kind, Ui, HEIGHT, WIDTH};
use crate::xlib::*;
use std::ffi::{c_int, c_uint, c_ulong, CString};

// Colours (24-bit TrueColor pixels). ASSUMED: picked by eye for a dark-red stage-piano panel.
const BG: c_ulong = 0xA8151C;
const HEADER: c_ulong = 0x161616;
const GROUP: c_ulong = 0x8A1016;
const GROUP_EDGE: c_ulong = 0x6E0C11;
const TEXT: c_ulong = 0xF4F1EE;
const TEXT_DIM: c_ulong = 0xE3B5B3;
const KNOB: c_ulong = 0x1C1C1C;
const KNOB_EDGE: c_ulong = 0x050505;
const TRACK: c_ulong = 0x5A0A0E;
const ARC: c_ulong = 0xFFD7D2;
const POINTER: c_ulong = 0xFFFFFF;
const LED_ON: c_ulong = 0xFF3B2F;
const LED_OFF: c_ulong = 0x3A0507;
const BUTTON: c_ulong = 0x262626;
const BUTTON_ON: c_ulong = 0x3A3A3A;
const DISPLAY: c_ulong = 0x0B0B0B;

/// Knob sweep: 270° clockwise starting at 7:30 (225° in X11 angle convention, 0° = 3 o'clock).
const SWEEP_START_DEG: f32 = 225.0;
const SWEEP_DEG: f32 = 270.0;

pub struct View {
    dpy: *mut Display,
    pub window: Window,
    pixmap: Pixmap,
    gc: GC,
    font: *mut XFontStruct,
    title_font: *mut XFontStruct,
    wm_delete: Atom,
    pub dirty: bool,
    pub closed: bool,
}

fn load_font(dpy: *mut Display, names: &[&str]) -> *mut XFontStruct {
    for n in names {
        let c = CString::new(*n).unwrap();
        let f = unsafe { XLoadQueryFont(dpy, c.as_ptr()) };
        if !f.is_null() {
            return f;
        }
    }
    std::ptr::null_mut()
}

impl View {
    /// Opens the panel as a child of `parent` (embedded host window) or as a top-level window.
    pub fn open(parent: Option<Window>) -> Option<View> {
        unsafe {
            let dpy = XOpenDisplay(std::ptr::null());
            if dpy.is_null() {
                return None;
            }
            let screen = XDefaultScreen(dpy);
            let parent_win = parent.unwrap_or_else(|| XRootWindow(dpy, screen));
            let window = XCreateSimpleWindow(
                dpy,
                parent_win,
                0,
                0,
                WIDTH as c_uint,
                HEIGHT as c_uint,
                0,
                0,
                BG,
            );
            XSelectInput(
                dpy,
                window,
                ExposureMask
                    | ButtonPressMask
                    | ButtonReleaseMask
                    | ButtonMotionMask
                    | StructureNotifyMask,
            );
            let mut wm_delete = 0;
            if parent.is_none() {
                let title = CString::new("Nord Stage 4 Emulator").unwrap();
                XStoreName(dpy, window, title.as_ptr());
                let name = CString::new("WM_DELETE_WINDOW").unwrap();
                wm_delete = XInternAtom(dpy, name.as_ptr(), 0);
                XSetWMProtocols(dpy, window, &mut wm_delete, 1);
            }
            let depth = XDefaultDepth(dpy, screen) as c_uint;
            let pixmap = XCreatePixmap(dpy, window, WIDTH as c_uint, HEIGHT as c_uint, depth);
            let gc = XCreateGC(dpy, window, 0, std::ptr::null_mut());
            let font = load_font(
                dpy,
                &[
                    "-*-dejavu sans-bold-r-normal-*-12-*-*-*-p-*-iso10646-1",
                    "-*-helvetica-bold-r-normal-*-12-*-*-*-*-*-iso8859-1",
                    "7x13bold",
                    "fixed",
                ],
            );
            let title_font = load_font(
                dpy,
                &[
                    "-*-dejavu sans-bold-r-normal-*-17-*-*-*-p-*-iso10646-1",
                    "-*-helvetica-bold-r-normal-*-18-*-*-*-*-*-iso8859-1",
                    "9x15bold",
                    "fixed",
                ],
            );
            XMapWindow(dpy, window);
            XFlush(dpy);
            Some(View {
                dpy,
                window,
                pixmap,
                gc,
                font,
                title_font,
                wm_delete,
                dirty: true,
                closed: false,
            })
        }
    }

    /// Drains pending X events, passing mouse input to `on_input`; returns after the queue is
    /// empty. `on_input` returns true when the panel must be redrawn.
    pub fn pump(&mut self, mut on_input: impl FnMut(Input) -> bool) {
        unsafe {
            while XPending(self.dpy) > 0 {
                let mut ev: XEvent = std::mem::zeroed();
                XNextEvent(self.dpy, &mut ev);
                let input = match ev.type_ {
                    BUTTON_PRESS => Some(Input::Press {
                        x: ev.button.x,
                        y: ev.button.y,
                        button: ev.button.button,
                        fine: ev.button.state & (ShiftMask | ControlMask) != 0,
                    }),
                    BUTTON_RELEASE => Some(Input::Release),
                    MOTION_NOTIFY => Some(Input::Motion {
                        x: ev.motion.x,
                        y: ev.motion.y,
                        fine: ev.motion.state & (ShiftMask | ControlMask) != 0,
                    }),
                    EXPOSE => {
                        self.dirty = true;
                        None
                    }
                    CLIENT_MESSAGE
                        if self.wm_delete != 0 && ev.client.data[0] as Atom == self.wm_delete =>
                    {
                        self.closed = true;
                        None
                    }
                    _ => None,
                };
                if let Some(i) = input {
                    self.dirty |= on_input(i);
                }
            }
        }
    }

    fn color(&self, c: c_ulong) {
        unsafe { XSetForeground(self.dpy, self.gc, c) };
    }

    fn rect(&self, x: i32, y: i32, w: i32, h: i32, c: c_ulong) {
        self.color(c);
        unsafe {
            XFillRectangle(
                self.dpy,
                self.pixmap,
                self.gc,
                x,
                y,
                w as c_uint,
                h as c_uint,
            )
        };
    }

    fn outline(&self, x: i32, y: i32, w: i32, h: i32, c: c_ulong) {
        self.color(c);
        self.line_width(1);
        unsafe {
            XDrawRectangle(
                self.dpy,
                self.pixmap,
                self.gc,
                x,
                y,
                (w - 1) as c_uint,
                (h - 1) as c_uint,
            )
        };
    }

    fn line_width(&self, w: u32) {
        unsafe { XSetLineAttributes(self.dpy, self.gc, w, LineSolid, CapRound, JoinRound) };
    }

    fn circle(&self, cx: i32, cy: i32, r: i32, c: c_ulong) {
        self.color(c);
        let d = (2 * r) as c_uint;
        unsafe {
            XFillArc(
                self.dpy,
                self.pixmap,
                self.gc,
                cx - r,
                cy - r,
                d,
                d,
                0,
                360 * 64,
            )
        };
    }

    fn arc(
        &self,
        cx: i32,
        cy: i32,
        r: i32,
        (start_deg, extent_deg): (f32, f32),
        width: u32,
        c: c_ulong,
    ) {
        self.color(c);
        self.line_width(width);
        let d = (2 * r) as c_uint;
        let (a1, a2) = ((start_deg * 64.0) as c_int, (extent_deg * 64.0) as c_int);
        unsafe { XDrawArc(self.dpy, self.pixmap, self.gc, cx - r, cy - r, d, d, a1, a2) };
    }

    /// Pixel width of `s` (ASCII) in `font`; 6 px per character when no font loaded.
    fn text_width(&self, font: *mut XFontStruct, s: &str) -> i32 {
        if font.is_null() {
            6 * s.len() as i32
        } else {
            unsafe { XTextWidth(font, s.as_ptr().cast(), s.len() as c_int) }
        }
    }

    /// Draws `s` with its baseline centred on (cx, baseline).
    fn text_centered(&self, font: *mut XFontStruct, s: &str, cx: i32, baseline: i32, c: c_ulong) {
        let w = self.text_width(font, s);
        self.text_left(font, s, cx - w / 2, baseline, c);
    }

    fn text_left(&self, font: *mut XFontStruct, s: &str, x: i32, baseline: i32, c: c_ulong) {
        self.color(c);
        unsafe {
            if !font.is_null() {
                XSetFont(self.dpy, self.gc, (*font).fid);
            }
            XDrawString(
                self.dpy,
                self.pixmap,
                self.gc,
                x,
                baseline,
                s.as_ptr().cast(),
                s.len() as c_int,
            );
        }
    }

    fn knob(&self, c: &model::Control, v: f32) {
        let r = model::KNOB_R;
        let n = model::knob_norm(c.kind, v);
        // track and value arc just outside the knob body
        self.arc(c.cx, c.cy, r + 6, (SWEEP_START_DEG, -SWEEP_DEG), 4, TRACK);
        if n > 0.0 {
            self.arc(c.cx, c.cy, r + 6, (SWEEP_START_DEG, -SWEEP_DEG * n), 4, ARC);
        }
        self.circle(c.cx, c.cy, r, KNOB_EDGE);
        self.circle(c.cx, c.cy, r - 2, KNOB);
        let a = (SWEEP_START_DEG - SWEEP_DEG * n).to_radians();
        let (s, co) = (a.sin(), a.cos());
        let (x0, y0) = (c.cx + (co * 6.0) as i32, c.cy - (s * 6.0) as i32);
        let (x1, y1) = (
            c.cx + (co * (r - 5) as f32) as i32,
            c.cy - (s * (r - 5) as f32) as i32,
        );
        self.color(POINTER);
        self.line_width(3);
        unsafe { XDrawLine(self.dpy, self.pixmap, self.gc, x0, y0, x1, y1) };
        let text = model::format_value(c, v);
        self.text_centered(self.font, &text, c.cx, model::VALUE_Y, TEXT);
    }

    fn toggle(&self, c: &model::Control, on: bool) {
        let (w, h) = (model::TOGGLE_W, model::TOGGLE_H);
        let (x, y) = (c.cx - w / 2, c.cy - h / 2);
        self.rect(x, y, w, h, if on { BUTTON_ON } else { BUTTON });
        self.outline(x, y, w, h, KNOB_EDGE);
        // LED in the left part of the button, layer letter on the right
        self.rect(x + 7, c.cy - 4, 12, 8, if on { LED_ON } else { LED_OFF });
        self.text_centered(
            self.font,
            c.label,
            x + 34,
            c.cy + 5,
            if on { TEXT } else { TEXT_DIM },
        );
    }

    /// `<` [ 012  Program name ] `>` in the header.
    fn program_selector(&self, ui: &Ui) {
        let (y, h, bw) = (model::PROG_Y, model::PROG_H, model::PROG_BTN_W);
        for (x, glyph) in [(model::PROG_PREV_X, "<"), (model::PROG_NEXT_X, ">")] {
            self.rect(x, y, bw, h, BUTTON);
            self.outline(x, y, bw, h, 0x3C3C3C);
            self.text_centered(self.font, glyph, x + bw / 2, y + 17, TEXT);
        }
        let (bx, w) = (model::PROG_BOX_X, model::PROG_BOX_W);
        self.rect(bx, y, w, h, DISPLAY);
        self.outline(bx, y, w, h, 0x3C3C3C);
        let label = ui.program_label();
        let (num, name) = label.split_at(label.len().min(3));
        self.text_left(self.font, num, bx + 8, y + 17, LED_ON);
        let name_x = bx + 8 + self.text_width(self.font, num);
        let name = self.fit(name, bx + w - 8 - name_x);
        self.text_left(self.font, &name, name_x, y + 17, TEXT);
    }

    /// `s` shortened with ".." so it is at most `max_px` wide (ASCII; other chars dropped).
    fn fit(&self, s: &str, max_px: i32) -> String {
        let mut t: String = s
            .chars()
            .filter(|c| c.is_ascii() && !c.is_ascii_control())
            .collect();
        if self.text_width(self.font, &t) <= max_px {
            return t;
        }
        while !t.is_empty() && self.text_width(self.font, &format!("{t}..")) > max_px {
            t.pop();
        }
        t + ".."
    }

    /// Paints the whole panel off-screen, then copies it to the window.
    pub fn draw(&mut self, ui: &Ui) {
        self.rect(0, 0, WIDTH, HEIGHT, BG);
        self.rect(0, 0, WIDTH, model::HEADER_H, HEADER);
        self.text_left(self.title_font, "NORD STAGE 4", 16, 26, TEXT);
        self.text_left(self.font, "EMULATOR  -  LV2", 160, 26, LED_ON);
        self.program_selector(ui);
        for g in &model::GROUPS {
            let h = model::PANEL_BOTTOM - model::PANEL_TOP;
            self.rect(g.x, model::PANEL_TOP, g.w, h, GROUP);
            self.outline(g.x, model::PANEL_TOP, g.w, h, GROUP_EDGE);
            self.text_centered(
                self.font,
                g.name,
                g.x + g.w / 2,
                model::GROUP_LABEL_Y,
                TEXT_DIM,
            );
        }
        for c in &ui.controls {
            let v = ui.value(c);
            match c.kind {
                Kind::Toggle => self.toggle(c, v > 0.5),
                Kind::Knob { .. } => {
                    if c.port == model::MASTER_PORT {
                        self.text_centered(self.font, c.label, c.cx, c.cy - 58, TEXT);
                    }
                    self.knob(c, v)
                }
            }
        }
        unsafe {
            XCopyArea(
                self.dpy,
                self.pixmap,
                self.window,
                self.gc,
                0,
                0,
                WIDTH as c_uint,
                HEIGHT as c_uint,
                0,
                0,
            );
            XFlush(self.dpy);
        }
        self.dirty = false;
    }
}

impl Drop for View {
    fn drop(&mut self) {
        unsafe {
            for f in [self.font, self.title_font] {
                if !f.is_null() {
                    XFreeFont(self.dpy, f);
                }
            }
            XFreeGC(self.dpy, self.gc);
            XFreePixmap(self.dpy, self.pixmap);
            XDestroyWindow(self.dpy, self.window);
            XCloseDisplay(self.dpy);
        }
    }
}

/// Serialises tests that open X connections: Xlib is not thread-safe across connections without
/// `XInitThreads`, which a plugin cannot call (the host owns the process).
#[cfg(test)]
pub(crate) static X_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    /// Opens, draws, feeds input and closes against a real X server when one is available
    /// (`xvfb-run cargo test -p nord-lv2-ui`); without `DISPLAY` it only checks `open` fails cleanly.
    #[test]
    fn open_draw_and_close() {
        let _x = X_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut view) = View::open(None) else {
            assert!(
                std::env::var_os("DISPLAY").is_none(),
                "DISPLAY set but X connection failed"
            );
            return;
        };
        let mut ui = Ui::with_programs(vec![
            "A very long program name that cannot fit in the box".into()
        ]);
        ui.host_value(model::PROGRAM_OUT_PORT, 1.0);
        view.draw(&ui);
        let fitted = view.fit(&ui.programs[0], 120);
        assert!(fitted.ends_with("..") && view.text_width(view.font, &fitted) <= 120);
        assert_eq!(view.fit("Short", 500), "Short");
        let k = ui.controls[8];
        ui.input(Input::Press {
            x: k.cx,
            y: k.cy,
            button: 1,
            fine: false,
        });
        assert!(ui
            .input(Input::Motion {
                x: k.cx,
                y: k.cy - 40,
                fine: false
            })
            .is_some());
        view.pump(|_| false);
        view.draw(&ui);
        assert!(!view.dirty && !view.closed);
        assert_ne!(view.window, 0);
    }
}
