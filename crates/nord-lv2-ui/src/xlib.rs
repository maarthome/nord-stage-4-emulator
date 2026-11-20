//! Minimal hand-written Xlib bindings (`X11/Xlib.h`, `X11/X.h`): only what the panel uses.
//! Struct layouts are `#[repr(C)]` copies of the header definitions for LP64 Linux.

#![allow(non_snake_case, non_upper_case_globals, dead_code)]

use std::ffi::{c_char, c_int, c_long, c_short, c_uint, c_ulong, c_ushort, c_void};

pub enum Display {}
pub type Window = c_ulong;
pub type Drawable = c_ulong;
pub type Pixmap = c_ulong;
pub type Atom = c_ulong;
pub type Font = c_ulong;
pub type Time = c_ulong;
pub type Bool = c_int;
pub type GC = *mut c_void;

// Event masks (X.h).
pub const ButtonPressMask: c_long = 1 << 2;
pub const ButtonReleaseMask: c_long = 1 << 3;
pub const ButtonMotionMask: c_long = 1 << 13;
pub const ExposureMask: c_long = 1 << 15;
pub const StructureNotifyMask: c_long = 1 << 17;

// Event types (X.h).
pub const BUTTON_PRESS: c_int = 4;
pub const BUTTON_RELEASE: c_int = 5;
pub const MOTION_NOTIFY: c_int = 6;
pub const EXPOSE: c_int = 12;
pub const CLIENT_MESSAGE: c_int = 33;

pub const ShiftMask: c_uint = 1 << 0;
pub const ControlMask: c_uint = 1 << 2;

pub const LineSolid: c_int = 0;
pub const CapRound: c_int = 2;
pub const JoinRound: c_int = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XAnyEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XButtonEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub button: c_uint,
    pub same_screen: Bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XMotionEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub is_hint: c_char,
    pub same_screen: Bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XClientMessageEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub message_type: Atom,
    pub format: c_int,
    pub data: [c_long; 5],
}

/// `XEvent` is a union padded to 24 longs.
#[repr(C)]
pub union XEvent {
    pub type_: c_int,
    pub any: XAnyEvent,
    pub button: XButtonEvent,
    pub motion: XMotionEvent,
    pub client: XClientMessageEvent,
    pub pad: [c_long; 24],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XCharStruct {
    pub lbearing: c_short,
    pub rbearing: c_short,
    pub width: c_short,
    pub ascent: c_short,
    pub descent: c_short,
    pub attributes: c_ushort,
}

#[repr(C)]
pub struct XFontStruct {
    pub ext_data: *mut c_void,
    pub fid: Font,
    pub direction: c_uint,
    pub min_char_or_byte2: c_uint,
    pub max_char_or_byte2: c_uint,
    pub min_byte1: c_uint,
    pub max_byte1: c_uint,
    pub all_chars_exist: Bool,
    pub default_char: c_uint,
    pub n_properties: c_int,
    pub properties: *mut c_void,
    pub min_bounds: XCharStruct,
    pub max_bounds: XCharStruct,
    pub per_char: *mut XCharStruct,
    pub ascent: c_int,
    pub descent: c_int,
}

#[link(name = "X11")]
extern "C" {
    pub fn XOpenDisplay(name: *const c_char) -> *mut Display;
    pub fn XCloseDisplay(d: *mut Display) -> c_int;
    pub fn XDefaultScreen(d: *mut Display) -> c_int;
    pub fn XRootWindow(d: *mut Display, screen: c_int) -> Window;
    pub fn XDefaultDepth(d: *mut Display, screen: c_int) -> c_int;
    pub fn XCreateSimpleWindow(
        d: *mut Display,
        parent: Window,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
        border_width: c_uint,
        border: c_ulong,
        background: c_ulong,
    ) -> Window;
    pub fn XDestroyWindow(d: *mut Display, w: Window) -> c_int;
    pub fn XSelectInput(d: *mut Display, w: Window, mask: c_long) -> c_int;
    pub fn XMapWindow(d: *mut Display, w: Window) -> c_int;
    pub fn XStoreName(d: *mut Display, w: Window, name: *const c_char) -> c_int;
    pub fn XInternAtom(d: *mut Display, name: *const c_char, only_if_exists: Bool) -> Atom;
    pub fn XSetWMProtocols(d: *mut Display, w: Window, protocols: *mut Atom, count: c_int)
        -> c_int;
    pub fn XCreateGC(d: *mut Display, dr: Drawable, mask: c_ulong, values: *mut c_void) -> GC;
    pub fn XFreeGC(d: *mut Display, gc: GC) -> c_int;
    pub fn XSetForeground(d: *mut Display, gc: GC, pixel: c_ulong) -> c_int;
    pub fn XSetLineAttributes(
        d: *mut Display,
        gc: GC,
        width: c_uint,
        style: c_int,
        cap: c_int,
        join: c_int,
    ) -> c_int;
    pub fn XSetFont(d: *mut Display, gc: GC, font: Font) -> c_int;
    pub fn XLoadQueryFont(d: *mut Display, name: *const c_char) -> *mut XFontStruct;
    pub fn XFreeFont(d: *mut Display, font: *mut XFontStruct) -> c_int;
    pub fn XTextWidth(font: *mut XFontStruct, s: *const c_char, len: c_int) -> c_int;
    pub fn XFillRectangle(
        d: *mut Display,
        dr: Drawable,
        gc: GC,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
    ) -> c_int;
    pub fn XDrawRectangle(
        d: *mut Display,
        dr: Drawable,
        gc: GC,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
    ) -> c_int;
    pub fn XFillArc(
        d: *mut Display,
        dr: Drawable,
        gc: GC,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
        a1: c_int,
        a2: c_int,
    ) -> c_int;
    pub fn XDrawArc(
        d: *mut Display,
        dr: Drawable,
        gc: GC,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
        a1: c_int,
        a2: c_int,
    ) -> c_int;
    pub fn XDrawLine(
        d: *mut Display,
        dr: Drawable,
        gc: GC,
        x1: c_int,
        y1: c_int,
        x2: c_int,
        y2: c_int,
    ) -> c_int;
    pub fn XDrawString(
        d: *mut Display,
        dr: Drawable,
        gc: GC,
        x: c_int,
        y: c_int,
        s: *const c_char,
        len: c_int,
    ) -> c_int;
    pub fn XCreatePixmap(
        d: *mut Display,
        dr: Drawable,
        w: c_uint,
        h: c_uint,
        depth: c_uint,
    ) -> Pixmap;
    pub fn XFreePixmap(d: *mut Display, p: Pixmap) -> c_int;
    pub fn XCopyArea(
        d: *mut Display,
        src: Drawable,
        dst: Drawable,
        gc: GC,
        sx: c_int,
        sy: c_int,
        w: c_uint,
        h: c_uint,
        dx: c_int,
        dy: c_int,
    ) -> c_int;
    pub fn XPending(d: *mut Display) -> c_int;
    pub fn XNextEvent(d: *mut Display, ev: *mut XEvent) -> c_int;
    pub fn XFlush(d: *mut Display) -> c_int;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// Offsets of the fields read from events, per the LP64 Xlib.h layout.
    #[test]
    fn event_layouts_match_xlib_lp64() {
        assert_eq!(size_of::<XEvent>(), 24 * size_of::<c_long>());
        assert_eq!(offset_of!(XButtonEvent, x), 64);
        assert_eq!(offset_of!(XButtonEvent, state), 80);
        assert_eq!(offset_of!(XButtonEvent, button), 84);
        assert_eq!(offset_of!(XMotionEvent, state), 80);
        assert_eq!(offset_of!(XClientMessageEvent, data), 56);
        assert_eq!(offset_of!(XFontStruct, fid), 8);
        assert_eq!(offset_of!(XFontStruct, ascent), 88);
    }
}
