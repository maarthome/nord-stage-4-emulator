//! LV2 instrument plugin for the Nord Stage 4 Emulator (Linux, including aarch64 / Raspberry Pi 5).
//!
//! A thin C-ABI shell around [`instance::Instance`]: MIDI in through an `atom:Sequence` port,
//! Main 1-2 and Aux 3-4 stereo outputs, control ports (master level, layer on/off, layer levels,
//! program select) and output ports with the engine's real state. Programs (`.ns4p`) load on the
//! host's worker thread (`worker.rs`); WAVE tables come from the user's Nord OS file (`wave.rs`).
//! The GUI is the separate `nord-lv2-ui` crate. The only dependency is `nord-core`.

pub mod atom;
pub mod ffi;
pub mod instance;
pub mod library;
pub mod ports;
pub mod wave;
pub mod worker;

use ffi::{Lv2Descriptor, Lv2Feature, Lv2Handle, Lv2UridMap};
use instance::Instance;
use ports::{Controls, CONTROL_COUNT, LAYERS};
use std::ffi::{c_char, c_void, CStr};
use std::sync::Arc;
use worker::{Request, Response, Schedule, Shared};

/// Plugin URI; must equal the subject in `bundle/nord-stage4.lv2/*.ttl` (checked by a test).
pub const PLUGIN_URI: &CStr = c"https://github.com/ymatrix1/nord-stage-4-emulator#lv2";

/// Output control ports: program, then 7 layer on/off, then 7 layer levels.
const STATE_OUTS: usize = 1 + 2 * LAYERS;

/// Raw port pointers as connected by the host.
struct Ports {
    midi: *const u8,
    audio: [*mut f32; 4],
    controls: [*const f32; CONTROL_COUNT],
    program: *const f32,
    state: [*mut f32; STATE_OUTS],
}

struct Plugin {
    inst: Instance,
    ports: Ports,
    shared: Arc<Shared>,
    schedule: *const Schedule,
}

impl Plugin {
    /// Hands `req` to the worker. When the host refuses, the request's pointer is taken back; an
    /// engine to free is then leaked rather than dropped on the audio thread.
    unsafe fn schedule(&self, req: Request) -> bool {
        let ok = self
            .schedule
            .as_ref()
            .and_then(|s| s.schedule_work.map(|f| (s.handle, f)))
            .is_some_and(|(h, f)| {
                let b = worker::as_bytes(&req);
                f(h, b.len() as u32, b.as_ptr().cast()) == worker::STATUS_SUCCESS
            });
        if !ok {
            if let Some(engine) = req.reclaim() {
                std::mem::forget(engine);
            }
        }
        ok
    }
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
    let programs = library::program_dir()
        .map(|d| library::list_programs(&d))
        .unwrap_or_default();
    let plugin = Box::new(Plugin {
        inst: Instance::new(sample_rate as f32, midi_urid, wave::ensure_loaded()),
        ports: Ports {
            midi: std::ptr::null(),
            audio: [std::ptr::null_mut(); 4],
            controls: [std::ptr::null(); CONTROL_COUNT],
            program: std::ptr::null(),
            state: [std::ptr::null_mut(); STATE_OUTS],
        },
        shared: Arc::new(Shared { programs }),
        schedule: ffi::find_feature(features, worker::SCHEDULE_URI).cast(),
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
        ports::PROGRAM => p.ports.program = data.cast(),
        idx if (ports::MASTER_DB..ports::PROGRAM).contains(&idx) => {
            p.ports.controls[(idx - ports::MASTER_DB) as usize] = data.cast()
        }
        idx if (ports::PROGRAM_OUT..ports::PORT_COUNT).contains(&idx) => {
            p.ports.state[(idx - ports::PROGRAM_OUT) as usize] = data.cast()
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

    // Program port moved or MIDI Program Change: build the engine on the worker thread.
    if let Some(prog) = p
        .inst
        .take_program_request(p.ports.program.as_ref().copied())
    {
        let req = Request::load(
            &p.shared,
            prog,
            p.inst.gain(),
            p.inst.sample_rate(),
            p.inst.wave(),
        );
        p.schedule(req);
    }

    let s = p.inst.state();
    let mut values = [0.0f32; STATE_OUTS];
    values[0] = s.program as f32;
    for i in 0..LAYERS {
        values[1 + i] = if s.layer_on[i] { 1.0 } else { 0.0 };
        values[1 + LAYERS + i] = s.layer_level[i];
    }
    for (&ptr, v) in p.ports.state.iter().zip(values) {
        if let Some(out) = ptr.as_mut() {
            *out = v;
        }
    }
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

/// Worker thread: runs a job and responds with a built engine. Never touches the instance.
unsafe extern "C" fn work(
    _instance: Lv2Handle,
    respond: worker::RespondFn,
    handle: *mut c_void,
    size: u32,
    data: *const c_void,
) -> u32 {
    if data.is_null() {
        return worker::STATUS_ERR_UNKNOWN;
    }
    let bytes = std::slice::from_raw_parts(data.cast::<u8>(), size as usize);
    let Some(req) = worker::from_bytes::<Request>(bytes) else {
        return worker::STATUS_ERR_UNKNOWN;
    };
    if let Some(resp) = worker::run_job(req) {
        let b = worker::as_bytes(&resp);
        if respond(handle, b.len() as u32, b.as_ptr().cast()) != worker::STATUS_SUCCESS
            && resp.engine != 0
        {
            drop(Box::from_raw(
                resp.engine as *mut nord_core::engine::Stage4Engine,
            ));
        }
    }
    worker::STATUS_SUCCESS
}

/// Audio thread, after `run`: swap in the built engine, send the old one back to be freed.
unsafe extern "C" fn work_response(instance: Lv2Handle, size: u32, body: *const c_void) -> u32 {
    let Some(p) = instance.cast::<Plugin>().as_mut() else {
        return worker::STATUS_ERR_UNKNOWN;
    };
    if body.is_null() {
        return worker::STATUS_ERR_UNKNOWN;
    }
    let bytes = std::slice::from_raw_parts(body.cast::<u8>(), size as usize);
    let Some(resp) = worker::from_bytes::<Response>(bytes) else {
        return worker::STATUS_ERR_UNKNOWN;
    };
    if resp.engine != 0 {
        let new = Box::from_raw(resp.engine as *mut nord_core::engine::Stage4Engine);
        let old = p.inst.swap_engine(new, resp.program);
        p.schedule(Request::free(old));
    }
    worker::STATUS_SUCCESS
}

static WORKER: worker::Interface = worker::Interface {
    work,
    work_response,
    end_run: None,
};

unsafe extern "C" fn extension_data(uri: *const c_char) -> *const c_void {
    if !uri.is_null() && CStr::from_ptr(uri) == worker::INTERFACE_URI {
        (&WORKER as *const worker::Interface).cast()
    } else {
        std::ptr::null()
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
    extension_data: Some(extension_data),
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

    static JOBS: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    static RESPONSES: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

    unsafe extern "C" fn fake_schedule(_h: *mut c_void, size: u32, data: *const c_void) -> u32 {
        let b = std::slice::from_raw_parts(data.cast::<u8>(), size as usize).to_vec();
        JOBS.lock().unwrap().push(b);
        worker::STATUS_SUCCESS
    }

    unsafe extern "C" fn fake_respond(_h: *mut c_void, size: u32, data: *const c_void) -> u32 {
        let b = std::slice::from_raw_parts(data.cast::<u8>(), size as usize).to_vec();
        RESPONSES.lock().unwrap().push(b);
        worker::STATUS_SUCCESS
    }

    /// Host side of the worker: run queued jobs, then deliver responses to the audio thread.
    unsafe fn drain_worker(h: Lv2Handle) {
        let wi = &*desc_ext_worker();
        loop {
            let jobs: Vec<Vec<u8>> = std::mem::take(&mut *JOBS.lock().unwrap());
            if jobs.is_empty() {
                break;
            }
            for j in jobs {
                assert_eq!(
                    (wi.work)(
                        h,
                        fake_respond,
                        std::ptr::null_mut(),
                        j.len() as u32,
                        j.as_ptr().cast()
                    ),
                    0
                );
            }
            let resps: Vec<Vec<u8>> = std::mem::take(&mut *RESPONSES.lock().unwrap());
            for r in resps {
                assert_eq!((wi.work_response)(h, r.len() as u32, r.as_ptr().cast()), 0);
            }
        }
    }

    fn desc_ext_worker() -> *const worker::Interface {
        let d = unsafe { &*lv2_descriptor(0) };
        unsafe { (d.extension_data.unwrap())(worker::INTERFACE_URI.as_ptr()) }.cast()
    }

    #[test]
    fn c_abi_worker_loads_program_and_reports_state() {
        use nord_core::cbin::ProgramDecoded;
        let dir = std::env::temp_dir().join(format!("nord-lv2-cabi-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Bank A")).unwrap();
        let mut prog = ProgramDecoded::default();
        prog.global.tempo_bpm = 101.0;
        let bytes = prog.to_cbin_file(0, 0).unwrap().to_bytes();
        std::fs::write(dir.join("Bank A").join("First.ns4p"), &bytes).unwrap();
        std::fs::write(dir.join("Second.ns4p"), &bytes).unwrap();
        // Only this test instantiates with a program folder; the other C-ABI test ignores programs.
        std::env::set_var(library::PROGRAM_DIR_ENV, &dir);

        let desc = unsafe { &*lv2_descriptor(0) };
        assert!(!desc_ext_worker().is_null());
        let mut map = Lv2UridMap {
            handle: std::ptr::null_mut(),
            map: Some(test_map),
        };
        let mut sched = Schedule {
            handle: std::ptr::null_mut(),
            schedule_work: Some(fake_schedule),
        };
        let f_map = Lv2Feature {
            uri: ffi::URID_MAP_URI.as_ptr(),
            data: (&mut map as *mut Lv2UridMap).cast(),
        };
        let f_sched = Lv2Feature {
            uri: worker::SCHEDULE_URI.as_ptr(),
            data: (&mut sched as *mut Schedule).cast(),
        };
        let feats: [*const Lv2Feature; 3] = [&f_map, &f_sched, std::ptr::null()];
        let h = unsafe { (desc.instantiate)(desc, 48_000.0, std::ptr::null(), feats.as_ptr()) };
        assert!(!h.is_null());

        const N: usize = 128;
        let mut out = [[0.0f32; N]; 2];
        let mut program_cell = 2.0f32;
        let program: *mut f32 = &mut program_cell; // the host's port buffer
        let mut state = [-1.0f32; STATE_OUTS];
        let mut seq = build_sequence(1, &[]);
        unsafe {
            (desc.connect_port)(h, ports::AUDIO_OUT[0], out[0].as_mut_ptr().cast());
            (desc.connect_port)(h, ports::AUDIO_OUT[1], out[1].as_mut_ptr().cast());
            (desc.connect_port)(h, ports::MIDI_IN, seq.as_mut_ptr().cast());
            (desc.connect_port)(h, ports::PROGRAM, program.cast());
            for (i, s) in state.iter_mut().enumerate() {
                (desc.connect_port)(h, ports::PROGRAM_OUT + i as u32, (s as *mut f32).cast());
            }
            (desc.activate.unwrap())(h);
            (desc.run)(h, N as u32);
            assert_eq!(state[0], 0.0, "nothing loaded yet");
            drain_worker(h);
            (desc.run)(h, N as u32);
            assert_eq!(state[0], 2.0, "program 2 (Second.ns4p) playing");
            let inst = &(*h.cast::<Plugin>()).inst;
            assert_eq!(inst.engine.master_clock.bpm, 101.0);
            let mask = inst.engine.scene.scene_1_mask;
            for i in 0..LAYERS {
                assert_eq!(
                    state[1 + i] == 1.0,
                    mask & (1 << i) != 0,
                    "layer {i} on/off output"
                );
            }
            // past the end of the list: the load fails and program 2 keeps playing
            *program = 9.0;
            (desc.run)(h, N as u32);
            drain_worker(h);
            (desc.run)(h, N as u32);
            assert_eq!(state[0], 2.0);
            (desc.cleanup)(h);
        }
        std::env::remove_var(library::PROGRAM_DIR_ENV);
        std::fs::remove_dir_all(&dir).unwrap();
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
