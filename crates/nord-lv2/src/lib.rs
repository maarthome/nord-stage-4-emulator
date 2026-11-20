//! LV2 instrument plugin for the Nord Stage 4 Emulator (Linux, including aarch64 / Raspberry Pi 5).
//!
//! A thin C-ABI shell around [`instance::Instance`]: MIDI in through an `atom:Sequence` port,
//! Main 1-2 and Aux 3-4 stereo outputs, and plain control ports (master level, layer on/off,
//! layer levels) so any host (Carla, Ardour, jalv, mod-host, Zynthian) shows a generic UI.
//! No GUI, no nih-plug: the only dependency is `nord-core`.

pub mod atom;
pub mod ffi;
pub mod instance;
pub mod ports;

use ffi::{Lv2Descriptor, Lv2Feature, Lv2Handle, Lv2UridMap};
use instance::Instance;
use ports::{Controls, CONTROL_COUNT};
use std::ffi::{c_char, c_void, CStr};

/// Plugin URI; must equal the subject in `bundle/nord-stage4.lv2/*.ttl` (checked by a test).
pub const PLUGIN_URI: &CStr = c"https://github.com/ymatrix1/nord-stage-4-emulator#lv2";

/// Raw port pointers as connected by the host.
struct Ports {
    midi: *const u8,
    audio: [*mut f32; 4],
    controls: [*const f32; CONTROL_COUNT],
}

struct Plugin {
    inst: Instance,
    ports: Ports,
}

unsafe extern "C" fn instantiate(
    _descriptor: *const Lv2Descriptor,
    sample_rate: f64,
    _bundle_path: *const c_char,
    features: *const *const Lv2Feature,
) -> Lv2Handle {
    let map: *const Lv2UridMap = ffi::find_feature(features, ffi::URID_MAP_URI).cast();
    // urid:map is a required feature in the TTL; without it MIDI events cannot be recognised.
    let Some(midi_urid) = ffi::map_uri(map, ffi::MIDI_EVENT_URI) else {
        return std::ptr::null_mut();
    };
    let plugin = Box::new(Plugin {
        inst: Instance::new(sample_rate as f32, midi_urid),
        ports: Ports {
            midi: std::ptr::null(),
            audio: [std::ptr::null_mut(); 4],
            controls: [std::ptr::null(); CONTROL_COUNT],
        },
    });
    Box::into_raw(plugin).cast()
}

unsafe extern "C" fn connect_port(instance: Lv2Handle, port: u32, data: *mut c_void) {
    let Some(p) = instance.cast::<Plugin>().as_mut() else {
        return;
    };
    match port {
        ports::MIDI_IN => p.ports.midi = data.cast(),
        1..=4 => p.ports.audio[(port - 1) as usize] = data.cast(),
        idx if (ports::MASTER_DB..ports::PORT_COUNT).contains(&idx) => {
            p.ports.controls[(port - ports::MASTER_DB) as usize] = data.cast()
        }
        _ => {}
    }
}

unsafe extern "C" fn activate(instance: Lv2Handle) {
    if let Some(p) = instance.cast::<Plugin>().as_mut() {
        p.inst.reset();
    }
}

unsafe extern "C" fn run(instance: Lv2Handle, sample_count: u32) {
    let Some(p) = instance.cast::<Plugin>().as_mut() else {
        return;
    };
    let n = sample_count as usize;

    let mut raw = [None; CONTROL_COUNT];
    for (r, &ptr) in raw.iter_mut().zip(p.ports.controls.iter()) {
        *r = ptr.as_ref().copied();
    }
    p.inst.apply_controls(&Controls::from_raw(raw));

    // The sequence header says how many bytes are valid (the port buffer may be larger).
    let midi = if p.ports.midi.is_null() {
        None
    } else {
        let header = std::slice::from_raw_parts(p.ports.midi, 8);
        atom::sequence_len(header).map(|len| std::slice::from_raw_parts(p.ports.midi, len))
    };

    let [a, b, c, d] = p.ports.audio;
    let mut outs = [
        out_slice(a, n),
        out_slice(b, n),
        out_slice(c, n),
        out_slice(d, n),
    ];
    p.inst.run(midi, &mut outs, n);
}

/// # Safety
/// `ptr` must be NULL or valid for `n` writable `f32`s for `'a`.
unsafe fn out_slice<'a>(ptr: *mut f32, n: usize) -> Option<&'a mut [f32]> {
    (!ptr.is_null()).then(|| std::slice::from_raw_parts_mut(ptr, n))
}

unsafe extern "C" fn cleanup(instance: Lv2Handle) {
    if !instance.is_null() {
        drop(Box::from_raw(instance.cast::<Plugin>()));
    }
}

static DESCRIPTOR: Lv2Descriptor = Lv2Descriptor {
    uri: PLUGIN_URI.as_ptr(),
    instantiate,
    connect_port,
    activate: Some(activate),
    run,
    deactivate: None,
    cleanup,
    extension_data: None,
};

/// LV2 entry point.
#[no_mangle]
pub extern "C" fn lv2_descriptor(index: u32) -> *const Lv2Descriptor {
    if index == 0 {
        &DESCRIPTOR
    } else {
        std::ptr::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atom::tests::build_sequence;
    use std::collections::HashMap;
    use std::sync::Mutex;

    const MANIFEST: &str = include_str!("../bundle/nord-stage4.lv2/manifest.ttl");
    const PLUGIN_TTL: &str = include_str!("../bundle/nord-stage4.lv2/nord-stage4.ttl");

    #[test]
    fn ttl_matches_uri_binary_and_port_map() {
        let uri = PLUGIN_URI.to_str().unwrap();
        assert!(MANIFEST.contains(&format!("<{uri}>")));
        assert!(PLUGIN_TTL.contains(&format!("<{uri}>")));
        assert!(MANIFEST.contains("<libnord_lv2.so>"));
        for i in 0..ports::PORT_COUNT {
            assert!(
                PLUGIN_TTL.contains(&format!("lv2:index {i} ;")),
                "port {i} missing from TTL"
            );
        }
        assert!(!PLUGIN_TTL.contains(&format!("lv2:index {} ;", ports::PORT_COUNT)));
        assert!(PLUGIN_TTL.contains("lv2:requiredFeature urid:map"));
    }

    static URIDS: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);

    unsafe extern "C" fn test_map(_h: *mut c_void, uri: *const c_char) -> u32 {
        let s = CStr::from_ptr(uri).to_string_lossy().into_owned();
        let mut g = URIDS.lock().unwrap();
        let m = g.get_or_insert_with(HashMap::new);
        let next = m.len() as u32 + 1;
        *m.entry(s).or_insert(next)
    }

    #[test]
    fn c_abi_lifecycle_plays_a_note() {
        let desc = unsafe { &*lv2_descriptor(0) };
        assert!(lv2_descriptor(1).is_null());
        assert_eq!(unsafe { CStr::from_ptr(desc.uri) }, PLUGIN_URI);

        // Without urid:map instantiation must fail cleanly.
        let none: [*const Lv2Feature; 1] = [std::ptr::null()];
        let h = unsafe { (desc.instantiate)(desc, 48_000.0, std::ptr::null(), none.as_ptr()) };
        assert!(h.is_null());

        let mut map = Lv2UridMap {
            handle: std::ptr::null_mut(),
            map: Some(test_map),
        };
        let feat = Lv2Feature {
            uri: ffi::URID_MAP_URI.as_ptr(),
            data: (&mut map as *mut Lv2UridMap).cast(),
        };
        let feats: [*const Lv2Feature; 2] = [&feat, std::ptr::null()];
        let h = unsafe { (desc.instantiate)(desc, 48_000.0, std::ptr::null(), feats.as_ptr()) };
        assert!(!h.is_null());
        let midi_urid = unsafe { test_map(std::ptr::null_mut(), ffi::MIDI_EVENT_URI.as_ptr()) };

        const N: usize = 512;
        let mut outs = [[0.0f32; N]; 4];
        let mut master = 0.0f32;
        let mut seq = build_sequence(1, &[(10, midi_urid, &[0x90, 57, 120])]);
        seq.resize(seq.len() + 256, 0); // port capacity beyond the valid size
        unsafe {
            for (i, o) in outs.iter_mut().enumerate() {
                (desc.connect_port)(h, ports::AUDIO_OUT[i], o.as_mut_ptr().cast());
            }
            (desc.connect_port)(h, ports::MIDI_IN, seq.as_mut_ptr().cast());
            (desc.connect_port)(h, ports::MASTER_DB, (&mut master as *mut f32).cast());
            (desc.connect_port)(h, 999, std::ptr::null_mut()); // ignored
            (desc.activate.unwrap())(h);
            let mut loud = 0.0f32;
            for k in 0..16 {
                if k == 1 {
                    seq = build_sequence(1, &[]);
                    (desc.connect_port)(h, ports::MIDI_IN, seq.as_mut_ptr().cast());
                }
                (desc.run)(h, N as u32);
                loud = outs[..2]
                    .iter()
                    .flat_map(|o| o.iter())
                    .fold(loud, |m, v| m.max(v.abs()));
            }
            (desc.cleanup)(h);
            assert!(loud > 0.0, "note through the C ABI produced no audio");
        }
    }
}
