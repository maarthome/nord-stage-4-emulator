//! Goldens of the FM stages: the Python equation models (equal to the listing interpreter and to
//! `sharc-org`, `tools/synth_fm_golden.py`) against the f32 port.

use super::{fm_stage1, fm_stage2, fm_table, FmInputs, FmState};
use crate::synth::tests_fw_fm_golden::{Case, CASES};

fn f(w: u32) -> f32 {
    f32::from_bits(w)
}

fn pair(w: [u32; 2]) -> [f32; 2] {
    [f(w[0]), f(w[1])]
}

fn inputs(c: &Case) -> FmInputs {
    FmInputs {
        p0: pair(c.p0),
        ratio: [pair(c.ratio[0]), pair(c.ratio[1])],
        inc_scale: c.inc_scale,
        pos: c.pos,
        x: pair(c.x),
        arm: c.arm.map(f),
        weights: c.weights.map(f),
        gain: pair(c.gain),
    }
}

/// Largest `|port - model|` over the eight output words of a case.
fn max_err(c: &Case) -> f32 {
    let table = fm_table(c.ty).expect("table");
    let mut st = FmState { acc: c.acc };
    let out = match c.stage {
        1 => fm_stage1(&mut st, &inputs(c), table),
        _ => fm_stage2(&mut st, &inputs(c), table),
    };
    let mut worst = 0.0f32;
    for (o, want) in out.iter().zip(&c.out) {
        for l in 0..2 {
            worst = worst.max((o[l] - f(want[l])).abs());
        }
    }
    worst
}

#[test]
fn both_stages_match_the_python_model_goldens() {
    // f32 against 40-bit registers: every case within 1e-3 of full scale (the phase words differ
    // by a few hundred least significant bits at most).
    let mut worst = 0.0f32;
    for c in &CASES {
        let e = max_err(c);
        assert!(e < 1e-3, "stage {} type {}: max error {e}", c.stage, c.ty);
        worst = worst.max(e);
    }
    assert!(worst >= 0.0);
}

#[test]
fn goldens_cover_both_stages_and_all_five_types() {
    for stage in [1, 2] {
        for ty in 49..=53 {
            assert!(
                CASES.iter().any(|c| c.stage == stage && c.ty == ty),
                "{stage} {ty}"
            );
        }
    }
}

#[test]
fn the_goldens_are_not_trivially_silent() {
    // The committed ARM words of some types leave a stage's gain word at 0 (silent output); the
    // random-word cases carry signal for every stage.
    for stage in [1, 2] {
        let loud = CASES
            .iter()
            .filter(|c| c.stage == stage && c.out.iter().flatten().any(|w| f(*w).abs() > 1e-3))
            .count();
        assert!(loud >= 3, "stage {stage}: {loud} loud cases");
    }
}
