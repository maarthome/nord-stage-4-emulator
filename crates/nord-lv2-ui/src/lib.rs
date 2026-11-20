//! X11 GUI for the `nord-lv2` plugin (LV2 `ui:X11UI`, `libnord_lv2_ui.so`).
//!
//! Embeds into the host's window when it passes `ui:parent`, otherwise opens a top-level window.
//! Hosts drive it through `ui:idleInterface` (jalv, Carla, Ardour, Zynthian's UI host). Control
//! values travel as plain `float` port writes; the panel model lives in [`model`].

pub mod model;
#[cfg(target_os = "linux")]
mod view;
#[cfg(target_os = "linux")]
mod xlib;

/// UI URI; must equal the `ui:ui` object in the bundle's `manifest.ttl` (checked by a test).
pub const UI_URI: &std::ffi::CStr = c"https://github.com/ymatrix1/nord-stage-4-emulator#ui";

#[cfg(target_os = "linux")]
mod lv2ui {
    use crate::model::{Ui, HEIGHT, WIDTH};
    use crate::view::View;
    use crate::xlib::Window;
    use std::ffi::{c_char, c_int, c_void, CStr};

    pub type Handle = *mut c_void;
    pub type WriteFn = unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void);

    pub const PARENT_URI: &CStr = c"http://lv2plug.in/ns/extensions/ui#parent";
    pub const RESIZE_URI: &CStr = c"http://lv2plug.in/ns/extensions/ui#resize";
    pub const IDLE_URI: &CStr = c"http://lv2plug.in/ns/extensions/ui#idleInterface";

    /// `LV2_Feature`.
    #[repr(C)]
    pub struct Feature {
        pub uri: *const c_char,
        pub data: *mut c_void,
    }

    /// `LV2UI_Resize`.
    #[repr(C)]
    pub struct Resize {
        pub handle: *mut c_void,
        pub ui_resize: Option<unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int>,
    }

    /// `LV2UI_Idle_Interface`.
    #[repr(C)]
    pub struct IdleInterface {
        pub idle: unsafe extern "C" fn(Handle) -> c_int,
    }

    /// `LV2UI_Descriptor`.
    #[repr(C)]
    pub struct Descriptor {
        pub uri: *const c_char,
        pub instantiate: unsafe extern "C" fn(
            *const Descriptor,
            *const c_char,
            *const c_char,
            WriteFn,
            *mut c_void,
            *mut *mut c_void,
            *const *const Feature,
        ) -> Handle,
        pub cleanup: unsafe extern "C" fn(Handle),
        pub port_event: unsafe extern "C" fn(Handle, u32, u32, u32, *const c_void),
        pub extension_data: unsafe extern "C" fn(*const c_char) -> *const c_void,
    }
    unsafe impl Sync for Descriptor {}

    pub struct Instance {
        pub view: View,
        pub ui: Ui,
        pub write: WriteFn,
        pub controller: *mut c_void,
    }

    /// Data of the feature `uri` in the host's NULL-terminated array, or NULL.
    ///
    /// # Safety
    /// `features` must be NULL or a valid NULL-terminated `LV2_Feature*` array.
    pub unsafe fn feature(features: *const *const Feature, uri: &CStr) -> Option<*mut c_void> {
        if features.is_null() {
            return None;
        }
        let mut i = 0;
        loop {
            let f = *features.add(i);
            if f.is_null() {
                return None;
            }
            if !(*f).uri.is_null() && CStr::from_ptr((*f).uri) == uri {
                return Some((*f).data);
            }
            i += 1;
        }
    }

    unsafe extern "C" fn instantiate(
        _d: *const Descriptor,
        _plugin_uri: *const c_char,
        _bundle: *const c_char,
        write: WriteFn,
        controller: *mut c_void,
        widget: *mut *mut c_void,
        features: *const *const Feature,
    ) -> Handle {
        // ui:parent carries the X11 Window id itself in the pointer value.
        let parent = feature(features, PARENT_URI).map(|p| p as Window);
        let Some(view) = View::open(parent) else {
            return std::ptr::null_mut();
        };
        if !widget.is_null() {
            *widget = view.window as *mut c_void;
        }
        if let Some(r) = feature(features, RESIZE_URI).and_then(|p| p.cast::<Resize>().as_ref()) {
            if let Some(f) = r.ui_resize {
                f(r.handle, WIDTH, HEIGHT);
            }
        }
        let inst = Box::new(Instance {
            view,
            ui: Ui::default(),
            write,
            controller,
        });
        Box::into_raw(inst).cast()
    }

    unsafe extern "C" fn cleanup(h: Handle) {
        if !h.is_null() {
            drop(Box::from_raw(h.cast::<Instance>()));
        }
    }

    /// Host → UI: control port values (format 0 = plain float).
    unsafe extern "C" fn port_event(
        h: Handle,
        port: u32,
        size: u32,
        format: u32,
        buf: *const c_void,
    ) {
        let Some(inst) = h.cast::<Instance>().as_mut() else {
            return;
        };
        if format == 0 && size == 4 && !buf.is_null() && inst.ui.set_port(port, *buf.cast::<f32>())
        {
            inst.view.dirty = true;
        }
    }

    unsafe extern "C" fn idle(h: Handle) -> c_int {
        let Some(inst) = h.cast::<Instance>().as_mut() else {
            return 1;
        };
        let Instance {
            view,
            ui,
            write,
            controller,
        } = inst;
        view.pump(|input| match ui.input(input) {
            Some((port, v)) => {
                write(*controller, port, 4, 0, (&v as *const f32).cast());
                true
            }
            None => false,
        });
        if view.dirty {
            view.draw(ui);
        }
        // Non-zero tells the host the (top-level) window was closed.
        c_int::from(view.closed)
    }

    pub static IDLE: IdleInterface = IdleInterface { idle };

    unsafe extern "C" fn extension_data(uri: *const c_char) -> *const c_void {
        if !uri.is_null() && CStr::from_ptr(uri) == IDLE_URI {
            (&IDLE as *const IdleInterface).cast()
        } else {
            std::ptr::null()
        }
    }

    pub static DESCRIPTOR: Descriptor = Descriptor {
        uri: crate::UI_URI.as_ptr(),
        instantiate,
        cleanup,
        port_event,
        extension_data,
    };

    /// LV2 UI entry point.
    #[no_mangle]
    pub extern "C" fn lv2ui_descriptor(index: u32) -> *const Descriptor {
        if index == 0 {
            &DESCRIPTOR
        } else {
            std::ptr::null()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::Mutex;

        static WRITES: Mutex<Vec<(u32, f32)>> = Mutex::new(Vec::new());

        unsafe extern "C" fn record(
            _c: *mut c_void,
            port: u32,
            size: u32,
            fmt: u32,
            buf: *const c_void,
        ) {
            assert_eq!((size, fmt), (4, 0));
            WRITES.lock().unwrap().push((port, *buf.cast::<f32>()));
        }

        #[test]
        fn descriptor_and_extension_data() {
            let d = unsafe { &*lv2ui_descriptor(0) };
            assert!(lv2ui_descriptor(1).is_null());
            assert_eq!(unsafe { CStr::from_ptr(d.uri) }, crate::UI_URI);
            let idle = unsafe { (d.extension_data)(IDLE_URI.as_ptr()) };
            assert_eq!(idle, (&IDLE as *const IdleInterface).cast());
            assert!(unsafe { (d.extension_data)(PARENT_URI.as_ptr()) }.is_null());
        }

        /// Full host round trip on a real X server (`xvfb-run`); returns early without one.
        #[test]
        fn instantiate_port_event_idle_cleanup() {
            let d = unsafe { &*lv2ui_descriptor(0) };
            let mut widget: *mut c_void = std::ptr::null_mut();
            let feats: [*const Feature; 1] = [std::ptr::null()];
            let h = unsafe {
                (d.instantiate)(
                    d,
                    std::ptr::null(),
                    std::ptr::null(),
                    record,
                    std::ptr::null_mut(),
                    &mut widget,
                    feats.as_ptr(),
                )
            };
            if h.is_null() {
                assert!(std::env::var_os("DISPLAY").is_none());
                return;
            }
            assert!(!widget.is_null());
            unsafe {
                let v = 0.25f32;
                (d.port_event)(h, 13, 4, 0, (&v as *const f32).cast());
                let inst = &mut *h.cast::<Instance>();
                assert_eq!(inst.ui.values[13], 0.25);
                assert!(inst.view.dirty);
                assert_eq!((IDLE.idle)(h), 0);
                assert!(!inst.view.dirty);
                // a click on Organ B goes back to the host through write_function
                let c = inst.ui.controls[2];
                let change = inst.ui.input(crate::model::Input::Press {
                    x: c.cx,
                    y: c.cy,
                    button: 1,
                    fine: false,
                });
                let (port, val) = change.unwrap();
                (inst.write)(inst.controller, port, 4, 0, (&val as *const f32).cast());
                (d.cleanup)(h);
            }
            assert_eq!(WRITES.lock().unwrap().last(), Some(&(7, 1.0)));
        }
    }
}

#[cfg(test)]
mod tests {
    const MANIFEST: &str = include_str!("../../nord-lv2/bundle/nord-stage4.lv2/manifest.ttl");

    #[test]
    fn manifest_declares_this_ui() {
        let uri = super::UI_URI.to_str().unwrap();
        assert!(MANIFEST.contains(&format!("ui:ui <{uri}>")));
        assert!(MANIFEST.contains(&format!("<{uri}>\n    a ui:X11UI")));
        assert!(MANIFEST.contains("ui:binary <libnord_lv2_ui.so>"));
        assert!(MANIFEST.contains("lv2:extensionData ui:idleInterface"));
    }
}
