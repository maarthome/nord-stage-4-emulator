//! CPU cost of the firmware organ stages (B3 Bass, Farfisa): seconds of CPU per second of audio.
//!
//! `cargo run --release -p nord-core --example organ_fw_bench` (research s17, TD-15). One chord of
//! three keys is held for 4 s of 48 kHz audio; the figure is `wall time / audio time`.

use std::time::Instant;

use nord_core::organ::{OrganLayer, OrganModel, ToneWheelSource};

fn layer(bass: bool) -> OrganLayer {
    let mut o = OrganLayer::new(48_000.0);
    o.rotary_enabled = false;
    if bass {
        o.model = OrganModel::B3;
        o.b3_bass_mode = true;
        o.tonewheel_source = ToneWheelSource::B3Wheels;
        o.set_b3_bass_processor(true);
    } else {
        o.model = OrganModel::Farfisa;
        o.tonewheel_source = ToneWheelSource::FarfisaWheels;
        o.set_farfisa_processor(true);
    }
    for k in [60, 64, 67] {
        o.note_on(k);
    }
    o
}

fn main() {
    let frames = 4 * 48_000;
    for (name, bass) in [("b3 bass", true), ("farfisa", false)] {
        let mut o = layer(bass);
        let t = Instant::now();
        let mut peak = 0.0f32;
        for _ in 0..frames {
            peak = peak.max(o.render_frame()[0].abs());
        }
        let cpu = t.elapsed().as_secs_f64() / 4.0;
        println!("{name}: {cpu:.3} s CPU per second of audio (peak {peak:.4})");
    }
}
