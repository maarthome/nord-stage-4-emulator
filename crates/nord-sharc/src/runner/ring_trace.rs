//! Data-access trace of one pass over an address window (`sharc-ring`, D126): which program
//! counter loads or stores which words of a ring buffer. Uses the core's own load and store
//! watches, so nothing in the interpreter changes. Numbers only (pc, address, count).
use super::pass::PassConfig;
use super::system::lean_pass;
use crate::core::Core;
use std::collections::BTreeMap;

/// Accesses of one pass inside the window, grouped by program counter.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PassTrace {
    /// `pc -> addresses` loaded in the window (in order).
    pub reads: BTreeMap<u32, Vec<u32>>,
    /// `pc -> addresses` stored in the window (in order).
    pub writes: BTreeMap<u32, Vec<u32>>,
    /// Steps of the pass and whether it returned to the poll loop.
    pub steps: u64,
    pub idle: bool,
}

/// Runs one pass with the watches on `lo..lo + len` and returns the grouped accesses.
pub fn trace_pass(core: &mut Core, cfg: &PassConfig, lo: u32, len: u32) -> PassTrace {
    (core.rwatch, core.rwatch_len, core.rwatch_on) = (lo, len, true);
    (core.watch, core.watch_len, core.watch_limit) = (lo, len, 1 << 22);
    core.rwatch_hits.clear();
    core.watch_hits.clear();
    core.watch_addrs.clear();
    let (steps, idle) = lean_pass(core, cfg);
    let mut t = PassTrace {
        steps,
        idle,
        ..PassTrace::default()
    };
    for &(pc, a) in &core.rwatch_hits {
        t.reads.entry(pc).or_default().push(a);
    }
    for (h, &a) in core.watch_hits.iter().zip(&core.watch_addrs) {
        t.writes.entry(h.1).or_default().push(a);
    }
    core.rwatch_on = false;
    core.watch = u32::MAX;
    t
}

/// `addrs` as `lo-hi` runs of consecutive addresses (`xN` when a run repeats a word).
pub fn runs(addrs: &[u32]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < addrs.len() {
        let mut j = i;
        while j + 1 < addrs.len() && addrs[j + 1] == addrs[j] + 1 {
            j += 1;
        }
        out.push(if i == j {
            format!("{:x}", addrs[i])
        } else {
            format!("{:x}-{:x}", addrs[i], addrs[j])
        });
        i = j + 1;
    }
    out.join(",")
}

impl PassTrace {
    /// One line per program counter and direction.
    pub fn text(&self) -> String {
        let mut s = format!("steps {} idle {}\n", self.steps, self.idle);
        for (tag, m) in [("R", &self.reads), ("W", &self.writes)] {
            for (pc, a) in m {
                s += &format!("  {tag} pc {pc:06x} n {:4} {}\n", a.len(), runs(a));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::super::pass::tests::clearing_core;
    use super::*;

    #[test]
    fn runs_group_consecutive_addresses() {
        assert_eq!(runs(&[1, 2, 3, 7, 9, 10]), "1-3,7,9-a");
        assert_eq!(runs(&[]), "");
    }

    #[test]
    fn trace_pass_restores_the_watches_and_groups_by_pc() {
        let mut c = clearing_core();
        let cfg = PassConfig {
            trigger: 0xB_00E2,
            idle_pc: 0x12_0002,
            max_steps: 10,
            irq: None,
        };
        let t = trace_pass(&mut c, &cfg, 0xB_0000, 0x10);
        assert!(!c.rwatch_on && c.watch == u32::MAX);
        assert!(t.steps >= 1);
        assert!(t.text().starts_with("steps "));
        // the harness trigger write at 0xB00E2 is outside the window: no stores recorded
        assert!(t.writes.is_empty());
    }
}
