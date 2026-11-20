//! Hand-written LV2 C ABI (`lv2/core/lv2.h`, `lv2/urid/urid.h`).
//!
//! Only the handful of types the plugin needs. Layouts are `#[repr(C)]` copies of the headers
//! (VERIFIED against LV2 1.18 `lv2.h` / `urid.h`: field order and types).

use std::ffi::{c_char, c_void, CStr};

pub type Lv2Handle = *mut c_void;
pub type Lv2Urid = u32;

pub const URID_MAP_URI: &CStr = c"http://lv2plug.in/ns/ext/urid#map";
pub const MIDI_EVENT_URI: &CStr = c"http://lv2plug.in/ns/ext/midi#MidiEvent";

/// `LV2_Feature`.
#[repr(C)]
pub struct Lv2Feature {
    pub uri: *const c_char,
    pub data: *mut c_void,
}

/// `LV2_URID_Map`.
#[repr(C)]
pub struct Lv2UridMap {
    pub handle: *mut c_void,
    pub map: Option<unsafe extern "C" fn(handle: *mut c_void, uri: *const c_char) -> Lv2Urid>,
}

/// `LV2_Descriptor`.
#[repr(C)]
pub struct Lv2Descriptor {
    pub uri: *const c_char,
    pub instantiate: unsafe extern "C" fn(
        descriptor: *const Lv2Descriptor,
        sample_rate: f64,
        bundle_path: *const c_char,
        features: *const *const Lv2Feature,
    ) -> Lv2Handle,
    pub connect_port: unsafe extern "C" fn(instance: Lv2Handle, port: u32, data: *mut c_void),
    pub activate: Option<unsafe extern "C" fn(instance: Lv2Handle)>,
    pub run: unsafe extern "C" fn(instance: Lv2Handle, sample_count: u32),
    pub deactivate: Option<unsafe extern "C" fn(instance: Lv2Handle)>,
    pub cleanup: unsafe extern "C" fn(instance: Lv2Handle),
    pub extension_data: Option<unsafe extern "C" fn(uri: *const c_char) -> *const c_void>,
}

// The descriptor is immutable static data; the raw pointers only point at 'static C strings.
unsafe impl Sync for Lv2Descriptor {}

/// Finds a feature by URI in the host's NULL-terminated feature array.
///
/// # Safety
/// `features` must be NULL or a valid NULL-terminated array of valid `LV2_Feature` pointers.
pub unsafe fn find_feature(features: *const *const Lv2Feature, uri: &CStr) -> *mut c_void {
    if features.is_null() {
        return std::ptr::null_mut();
    }
    let mut i = 0;
    loop {
        let f = *features.add(i);
        if f.is_null() {
            return std::ptr::null_mut();
        }
        if !(*f).uri.is_null() && CStr::from_ptr((*f).uri) == uri {
            return (*f).data;
        }
        i += 1;
    }
}

/// Maps `uri` through the host's URID map, or `None` when the map is missing.
///
/// # Safety
/// `map` must be NULL or point at a valid `LV2_URID_Map`.
pub unsafe fn map_uri(map: *const Lv2UridMap, uri: &CStr) -> Option<Lv2Urid> {
    let map = map.as_ref()?;
    let f = map.map?;
    Some(f(map.handle, uri.as_ptr()))
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn fake_map(_h: *mut c_void, uri: *const c_char) -> Lv2Urid {
        CStr::from_ptr(uri).to_bytes().len() as Lv2Urid
    }

    #[test]
    fn find_feature_walks_null_terminated_array() {
        let mut map = Lv2UridMap {
            handle: std::ptr::null_mut(),
            map: Some(fake_map),
        };
        let other = Lv2Feature {
            uri: c"http://example.org/other".as_ptr(),
            data: std::ptr::null_mut(),
        };
        let urid = Lv2Feature {
            uri: URID_MAP_URI.as_ptr(),
            data: (&mut map as *mut Lv2UridMap).cast(),
        };
        let list: [*const Lv2Feature; 3] = [&other, &urid, std::ptr::null()];
        unsafe {
            let found = find_feature(list.as_ptr(), URID_MAP_URI);
            assert_eq!(found, (&mut map as *mut Lv2UridMap).cast());
            assert!(find_feature(list.as_ptr(), c"urn:missing").is_null());
            assert!(find_feature(std::ptr::null(), URID_MAP_URI).is_null());
            let id = map_uri(found.cast(), MIDI_EVENT_URI).unwrap();
            assert_eq!(id as usize, MIDI_EVENT_URI.to_bytes().len());
            assert_eq!(map_uri(std::ptr::null(), MIDI_EVENT_URI), None);
        }
    }
}
