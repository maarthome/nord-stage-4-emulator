//! Option groups of the program ranking (`program_rank --ablate`, `docs/research/program-ranking.md`):
//! the fields by which `--firmware-all --firmware-routing` differs from the engine default, split in
//! eight groups that can be put back to the default one at a time or in combinations.
//!
//! The six FX groups are the ones that explain the ranked distance of the worst programs, so
//! [`fx_subsets`] lists every non-empty combination of them (63) for the combined ablation.

use nord_core::render_opts::FirmwareOptions;

/// Puts the fields of one group of the first options back to the values of the second.
pub type Revert = fn(&mut FirmwareOptions, &FirmwareOptions);

/// Number of groups.
pub const GROUP_COUNT: usize = 8;

/// Index of the first FX group in [`GROUPS`] (`fx_delay`); the six FX groups follow it.
pub const FIRST_FX: usize = 2;

/// Number of FX groups.
pub const FX_COUNT: usize = 6;

/// Name and revert function of every group. Organ tone wheels and the synth filter engine come
/// first, then the FX groups.
pub const GROUPS: [(&str, Revert); GROUP_COUNT] = [
    ("organ_tone", |o, d| o.tone_source = d.tone_source),
    ("synth_filter", |o, d| o.filter_engine = d.filter_engine),
    ("fx_delay", |o, d| {
        o.fx.delay_laws = d.fx.delay_laws;
        o.fx.delay_time = d.fx.delay_time;
    }),
    ("fx_compressor", |o, d| o.fx.compressor = d.fx.compressor),
    ("fx_reverb", |o, d| {
        o.fx.reverb_mix = d.fx.reverb_mix;
        o.fx.reverb = d.fx.reverb;
    }),
    ("fx_eq", |o, d| o.fx.eq = d.fx.eq),
    ("fx_mod1", |o, d| o.fx.mod1 = d.fx.mod1),
    ("fx_mod2", |o, d| o.fx.mod2 = d.fx.mod2),
];

/// The firmware-derived options the ranking compares against the default (the oracle-verified
/// model): `--firmware-all` plus `--firmware-routing`.
pub fn firmware_options() -> FirmwareOptions {
    FirmwareOptions {
        firmware_routing: true,
        ..FirmwareOptions::firmware_all()
    }
}

/// `firmware` with the groups in `set` (indices into [`GROUPS`]) put back to the default.
pub fn reverted_set(firmware: &FirmwareOptions, set: &[usize]) -> FirmwareOptions {
    let mut o = *firmware;
    for &g in set {
        GROUPS[g].1(&mut o, &FirmwareOptions::default());
    }
    o
}

/// `firmware` with one group put back to the default.
pub fn reverted(firmware: &FirmwareOptions, group: usize) -> FirmwareOptions {
    reverted_set(firmware, &[group])
}

/// Key of a combination: the group names joined by `+` in [`GROUPS`] order.
pub fn subset_key(set: &[usize]) -> String {
    let mut s = set.to_vec();
    s.sort_unstable();
    s.iter().map(|&g| GROUPS[g].0).collect::<Vec<_>>().join("+")
}

/// Every non-empty combination of the FX groups (indices into [`GROUPS`]), by mask 1..2^6.
pub fn fx_subsets() -> Vec<Vec<usize>> {
    (1u32..(1 << FX_COUNT))
        .map(|mask| {
            (0..FX_COUNT)
                .filter(|b| mask & (1 << b) != 0)
                .map(|b| FIRST_FX + b)
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_cover_every_difference() {
        let firmware = firmware_options();
        let all: Vec<usize> = (0..GROUP_COUNT).collect();
        assert_eq!(
            reverted_set(&firmware, &all),
            FirmwareOptions::default(),
            "a group is missing"
        );
        assert_ne!(firmware, FirmwareOptions::default());
        for g in 0..GROUP_COUNT {
            assert_ne!(
                reverted(&firmware, g),
                firmware,
                "group {g} changes nothing"
            );
        }
    }

    #[test]
    fn fx_groups_are_the_six_after_the_engine_groups() {
        assert!(GROUPS[..FIRST_FX]
            .iter()
            .all(|(n, _)| !n.starts_with("fx_")));
        assert!(GROUPS[FIRST_FX..].iter().all(|(n, _)| n.starts_with("fx_")));
        assert_eq!(GROUP_COUNT - FIRST_FX, FX_COUNT);
    }

    #[test]
    fn fx_subsets_are_all_63_distinct_combinations() {
        let s = fx_subsets();
        assert_eq!(s.len(), 63);
        let keys: std::collections::HashSet<String> = s.iter().map(|x| subset_key(x)).collect();
        assert_eq!(keys.len(), 63);
        assert_eq!(s.iter().filter(|x| x.len() == 1).count(), 6);
        assert_eq!(s.iter().filter(|x| x.len() == 2).count(), 15);
        assert_eq!(s.last().map(Vec::len), Some(6));
    }

    #[test]
    fn reverting_all_fx_groups_leaves_only_engine_groups_different() {
        let firmware = firmware_options();
        let all_fx: Vec<usize> = (FIRST_FX..GROUP_COUNT).collect();
        let o = reverted_set(&firmware, &all_fx);
        assert_eq!(o.fx, FirmwareOptions::default().fx);
        assert_ne!(o.tone_source, FirmwareOptions::default().tone_source);
    }

    #[test]
    fn subset_key_sorts_and_joins_names() {
        assert_eq!(subset_key(&[7, 2]), "fx_delay+fx_mod2");
        assert_eq!(subset_key(&[4]), "fx_reverb");
    }
}
