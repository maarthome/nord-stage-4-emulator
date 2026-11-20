//! Program loading on the host's worker thread (LV2 Worker, `lv2/worker/worker.h`).
//!
//! The audio thread never reads files or builds engines. It schedules a [`Request::load`]; the
//! worker reads the `.ns4p`, builds a complete engine with the program applied and responds with
//! it; the audio thread swaps it in and schedules [`Request::free`] for the old engine.
//! ABI layouts VERIFIED against LV2 1.18 `worker.h` (field order and types).

use crate::instance::build_engine;
use nord_core::cbin::{CbinFile, ProgramDecoded};
use nord_core::engine::Stage4Engine;
use std::ffi::{c_void, CStr};
use std::path::PathBuf;
use std::sync::Arc;

pub const SCHEDULE_URI: &CStr = c"http://lv2plug.in/ns/ext/worker#schedule";
pub const INTERFACE_URI: &CStr = c"http://lv2plug.in/ns/ext/worker#interface";

pub const STATUS_SUCCESS: u32 = 0;
pub const STATUS_ERR_UNKNOWN: u32 = 1;

pub type RespondFn =
    unsafe extern "C" fn(handle: *mut c_void, size: u32, data: *const c_void) -> u32;

/// `LV2_Worker_Schedule`.
#[repr(C)]
pub struct Schedule {
    pub handle: *mut c_void,
    pub schedule_work:
        Option<unsafe extern "C" fn(handle: *mut c_void, size: u32, data: *const c_void) -> u32>,
}

/// `LV2_Worker_Interface`.
#[repr(C)]
pub struct Interface {
    pub work: unsafe extern "C" fn(
        instance: *mut c_void,
        respond: RespondFn,
        handle: *mut c_void,
        size: u32,
        data: *const c_void,
    ) -> u32,
    pub work_response:
        unsafe extern "C" fn(instance: *mut c_void, size: u32, body: *const c_void) -> u32,
    pub end_run: Option<unsafe extern "C" fn(instance: *mut c_void) -> u32>,
}

/// Read-only data the worker needs, shared with it by `Arc` pointer inside each request so the
/// worker never touches the plugin instance the audio thread is using.
pub struct Shared {
    pub programs: Vec<PathBuf>,
}

const LOAD: u32 = 1;
const FREE: u32 = 2;

/// A job for the worker, copied byte-for-byte by the host.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request {
    kind: u32,
    pub program: u32,
    pub gain: f32,
    pub sample_rate: f32,
    pub wave: u32,
    /// `Arc<Shared>` from `Arc::into_raw` (LOAD) or `Box<Stage4Engine>` from `Box::into_raw` (FREE).
    ptr: usize,
}

/// The worker's answer: the built engine (`Box::into_raw`, 0 when the load failed).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Response {
    pub program: u32,
    pub engine: usize,
}

/// Bytes of a `#[repr(C)]` plain-data value, for `schedule_work` / `respond`.
pub fn as_bytes<T: Copy>(v: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((v as *const T).cast::<u8>(), std::mem::size_of::<T>()) }
}

/// Reads a plain-data value back; `None` when the size does not match.
pub fn from_bytes<T: Copy>(b: &[u8]) -> Option<T> {
    (b.len() == std::mem::size_of::<T>())
        .then(|| unsafe { std::ptr::read_unaligned(b.as_ptr().cast::<T>()) })
}

impl Request {
    /// Not real-time safe only in the `Arc` clone's atomic increment (lock-free).
    pub fn load(
        shared: &Arc<Shared>,
        program: u32,
        gain: f32,
        sample_rate: f32,
        wave: bool,
    ) -> Self {
        Self {
            kind: LOAD,
            program,
            gain,
            sample_rate,
            wave: u32::from(wave),
            ptr: Arc::into_raw(Arc::clone(shared)) as usize,
        }
    }

    pub fn free(engine: Box<Stage4Engine>) -> Self {
        Self {
            kind: FREE,
            program: 0,
            gain: 0.0,
            sample_rate: 0.0,
            wave: 0,
            ptr: Box::into_raw(engine) as usize,
        }
    }

    /// Takes back ownership of the request's pointer when the host could not schedule it.
    /// Leaks nothing it should not: a dropped `Arc` clone, or the engine handed back.
    ///
    /// # Safety
    /// `self` must come from [`Request::load`] / [`Request::free`] and not have been run.
    pub unsafe fn reclaim(self) -> Option<Box<Stage4Engine>> {
        match self.kind {
            LOAD => {
                drop(Arc::from_raw(self.ptr as *const Shared));
                None
            }
            FREE => Some(Box::from_raw(self.ptr as *mut Stage4Engine)),
            _ => None,
        }
    }
}

/// Reads and decodes program `n` (1-based) of the library.
pub fn read_program(shared: &Shared, n: u32) -> Result<ProgramDecoded, String> {
    let path = n
        .checked_sub(1)
        .and_then(|i| shared.programs.get(i as usize))
        .ok_or_else(|| format!("no program {n} ({} in the folder)", shared.programs.len()))?;
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file = CbinFile::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    ProgramDecoded::from_cbin_file(&file).map_err(|e| format!("{}: {e}", path.display()))
}

/// Runs one job on the worker thread. LOAD returns the response to send; FREE drops the engine.
///
/// # Safety
/// `req` must come from [`Request::load`] / [`Request::free`] and run at most once.
pub unsafe fn run_job(req: Request) -> Option<Response> {
    match req.kind {
        LOAD => {
            let shared = Arc::from_raw(req.ptr as *const Shared);
            let prog = match req.program {
                0 => Ok(None),
                n => read_program(&shared, n).map(Some),
            };
            let engine = match prog {
                Ok(p) => Box::into_raw(build_engine(
                    req.sample_rate,
                    p.as_ref(),
                    req.gain,
                    req.wave != 0,
                )) as usize,
                Err(e) => {
                    eprintln!("nord-lv2: program not loaded: {e}");
                    0
                }
            };
            Some(Response {
                program: req.program,
                engine,
            })
        }
        FREE => {
            drop(Box::from_raw(req.ptr as *mut Stage4Engine));
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_bytes_round_trip() {
        let shared = Arc::new(Shared { programs: vec![] });
        let r = Request::load(&shared, 7, 0.5, 48_000.0, true);
        assert_eq!(Arc::strong_count(&shared), 2);
        let back: Request = from_bytes(as_bytes(&r)).unwrap();
        assert_eq!(back, r);
        assert_eq!(from_bytes::<Request>(&as_bytes(&r)[1..]), None);
        unsafe { back.reclaim() };
        assert_eq!(Arc::strong_count(&shared), 1);
    }

    #[test]
    fn load_default_missing_and_broken_programs() {
        let dir = std::env::temp_dir().join(format!("nord-lv2-worker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let broken = dir.join("broken.ns4p");
        std::fs::write(&broken, b"not a CBIN file").unwrap();
        let shared = Arc::new(Shared {
            programs: vec![broken],
        });

        // program 0: a default engine
        let resp = unsafe { run_job(Request::load(&shared, 0, 1.0, 48_000.0, false)) }.unwrap();
        assert_eq!(resp.program, 0);
        assert_ne!(resp.engine, 0);
        let e = unsafe { Box::from_raw(resp.engine as *mut Stage4Engine) };
        assert_eq!(e.sample_rate, 48_000.0);
        assert!(unsafe { run_job(Request::free(e)) }.is_none());

        // a broken file and a number past the list both answer "no engine"
        for n in [1, 2] {
            let resp = unsafe { run_job(Request::load(&shared, n, 1.0, 48_000.0, false)) }.unwrap();
            assert_eq!((resp.program, resp.engine), (n, 0));
        }
        assert_eq!(
            Arc::strong_count(&shared),
            1,
            "every request released its Arc"
        );
        assert!(read_program(&shared, 0).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_real_program_applies_it_and_scales_levels() {
        let dir = std::env::temp_dir().join(format!("nord-lv2-worker-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = ProgramDecoded::default();
        p.global.tempo_bpm = 133.0;
        let path = dir.join("Tempo 133.ns4p");
        let bytes = p.to_cbin_file(0, 0).unwrap().to_bytes();
        std::fs::write(&path, &bytes).unwrap();
        let shared = Arc::new(Shared {
            programs: vec![path],
        });

        // what the file decodes to, applied at unity gain
        let decoded = ProgramDecoded::from_cbin_file(&CbinFile::parse(&bytes).unwrap()).unwrap();
        let reference = build_engine(48_000.0, Some(&decoded), 1.0, false);
        let resp = unsafe { run_job(Request::load(&shared, 1, 0.5, 48_000.0, false)) }.unwrap();
        assert_eq!(resp.program, 1);
        let e = unsafe { Box::from_raw(resp.engine as *mut Stage4Engine) };
        assert_eq!(e.master_clock.bpm, 133.0);
        assert_eq!(e.scene.scene_1_mask, reference.scene.scene_1_mask);
        for (a, b) in e.layer_levels.iter().zip(reference.layer_levels.iter()) {
            assert!((a.base - 0.5 * b.base).abs() < 1e-6);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
