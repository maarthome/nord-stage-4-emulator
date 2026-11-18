//! The arpeggiator switch of the shared firmware options (`--legacy-arp`, D166).
//!
//! The ARM pattern player walks UP/DOWN over the whole octave pool and ends an Arp / Poly note at
//! 2/3 of the step (`docs/research/arp-fw-vs-native.md`, `docs/research/arm-tick-timers.md` s6).
//! Both are the default; `on = false` restores the engine walk and the note held to the next step.

use crate::engine::Stage4Engine;

/// Selects the firmware arpeggiator walk and gate (`true`) or the legacy engine arpeggiator
/// (`false`) on the three synth layers. Plain field writes, safe between audio blocks.
pub fn apply_arp_firmware(engine: &mut Stage4Engine, on: bool) {
    for s in [
        &mut engine.synth_a,
        &mut engine.synth_b,
        &mut engine.synth_c,
    ] {
        s.arp_firmware_walk = on;
        s.arp_firmware_gate = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_opts::FirmwareOptions;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn apply_switches_walk_and_gate_on_all_three_synth_layers() {
        let mut e = Stage4Engine::new(48_000.0);
        assert!(e.synth_a.arp_firmware_walk && e.synth_b.arp_firmware_gate);
        apply_arp_firmware(&mut e, false);
        for s in [&e.synth_a, &e.synth_b, &e.synth_c] {
            assert!(!s.arp_firmware_walk && !s.arp_firmware_gate);
        }
        apply_arp_firmware(&mut e, true);
        assert!(e.synth_c.arp_firmware_walk && e.synth_c.arp_firmware_gate);
    }

    #[test]
    fn firmware_arp_is_the_default_and_legacy_arp_opts_out() {
        assert!(FirmwareOptions::default().arp_firmware);
        assert!(FirmwareOptions::firmware_all().arp_firmware);
        assert!(!FirmwareOptions::legacy_all().arp_firmware);
        assert!(!FirmwareOptions::default().summary().contains("legacy-arp"));
        let o = FirmwareOptions::from_args(&args(&["--legacy-arp"])).unwrap();
        assert!(!o.arp_firmware && o.summary().contains("legacy-arp"));
        // the older spelling stays accepted
        let o = FirmwareOptions::from_args(&args(&["--arp-fw-walk"])).unwrap();
        assert!(o.arp_firmware);
        let mut e = Stage4Engine::new(48_000.0);
        FirmwareOptions::from_args(&args(&["--legacy-arp"]))
            .unwrap()
            .apply_to(&mut e);
        assert!(!e.synth_a.arp_firmware_walk && !e.synth_c.arp_firmware_gate);
        FirmwareOptions::default().apply_to(&mut e);
        assert!(e.synth_a.arp_firmware_walk && e.synth_c.arp_firmware_gate);
    }
}
