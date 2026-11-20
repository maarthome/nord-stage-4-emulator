//! LV2 port map and control values. Indices must match `bundle/nord-stage4.lv2/nord-stage4.ttl`
//! (checked by a test that reads the TTL).

pub const MIDI_IN: u32 = 0;
/// Main 1-2 and Aux 3-4 audio outputs, in engine output order.
pub const AUDIO_OUT: [u32; 4] = [1, 2, 3, 4];
pub const MASTER_DB: u32 = 5;
/// First of the 7 layer on/off toggles (Organ A, Organ B, Piano A, Piano B, Synth A, B, C).
pub const LAYER_ON_FIRST: u32 = 6;
/// First of the 7 layer level controls, same layer order.
pub const LAYER_LEVEL_FIRST: u32 = 13;
pub const PORT_COUNT: u32 = 20;

pub const LAYERS: usize = 7;
pub const CONTROL_COUNT: usize = 1 + 2 * LAYERS;

/// Engine defaults: Scene I has Organ A, Piano A and Synth A on (`LayerSceneState::default`),
/// every layer level is 0.8 (`Stage4Engine::new`). The TTL defaults repeat these values.
pub const DEFAULT_LAYER_ON: [bool; LAYERS] = [true, false, true, false, true, false, false];
pub const DEFAULT_LAYER_LEVEL: f32 = 0.8;
pub const MASTER_DB_MIN: f32 = -60.0;
pub const MASTER_DB_MAX: f32 = 6.0;

/// A snapshot of all control ports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    pub master_db: f32,
    pub layer_on: [bool; LAYERS],
    pub layer_level: [f32; LAYERS],
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            master_db: 0.0,
            layer_on: DEFAULT_LAYER_ON,
            layer_level: [DEFAULT_LAYER_LEVEL; LAYERS],
        }
    }
}

impl Controls {
    /// Builds controls from raw port values in port order (`MASTER_DB` first). `None` (an
    /// unconnected port) keeps the default. Values are clamped to the TTL ranges.
    pub fn from_raw(raw: [Option<f32>; CONTROL_COUNT]) -> Self {
        let d = Self::default();
        let mut c = d;
        c.master_db = raw[0]
            .unwrap_or(d.master_db)
            .clamp(MASTER_DB_MIN, MASTER_DB_MAX);
        for i in 0..LAYERS {
            c.layer_on[i] = raw[1 + i].map_or(d.layer_on[i], |v| v > 0.5);
            c.layer_level[i] = raw[1 + LAYERS + i]
                .unwrap_or(d.layer_level[i])
                .clamp(0.0, 1.0);
        }
        c
    }

    /// Master gain as a linear factor; the bottom of the range is treated as silence.
    pub fn master_gain(&self) -> f32 {
        if self.master_db <= MASTER_DB_MIN {
            0.0
        } else {
            10f32.powf(self.master_db / 20.0)
        }
    }

    /// Scene I layer mask, bit `i` = layer `i` (same bit order as `LayerSceneState`).
    pub fn layer_mask(&self) -> u8 {
        self.layer_on
            .iter()
            .enumerate()
            .fold(0, |m, (i, &on)| if on { m | (1 << i) } else { m })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_indices_are_contiguous_and_complete() {
        assert_eq!(MASTER_DB as usize, 1 + AUDIO_OUT.len());
        assert_eq!(LAYER_LEVEL_FIRST, LAYER_ON_FIRST + LAYERS as u32);
        assert_eq!(PORT_COUNT, LAYER_LEVEL_FIRST + LAYERS as u32);
        assert_eq!(PORT_COUNT as usize, 1 + AUDIO_OUT.len() + CONTROL_COUNT);
    }

    #[test]
    fn defaults_match_engine_defaults() {
        let c = Controls::default();
        assert_eq!(
            c.layer_mask(),
            nord_core::morph::LayerSceneState::default().scene_1_mask
        );
        assert_eq!(c.master_gain(), 1.0);
        let e = nord_core::engine::Stage4Engine::new(48_000.0);
        assert!(e.layer_levels.iter().all(|l| l.base == DEFAULT_LAYER_LEVEL));
    }

    #[test]
    fn from_raw_clamps_and_falls_back() {
        let mut raw = [None; CONTROL_COUNT];
        raw[0] = Some(-100.0);
        raw[1 + 1] = Some(1.0); // Organ B on
        raw[1] = Some(0.0); // Organ A off
        raw[1 + LAYERS] = Some(3.0);
        let c = Controls::from_raw(raw);
        assert_eq!(c.master_db, MASTER_DB_MIN);
        assert_eq!(c.master_gain(), 0.0);
        assert!(!c.layer_on[0] && c.layer_on[1] && c.layer_on[2]);
        assert_eq!(c.layer_level[0], 1.0);
        assert_eq!(c.layer_level[1], DEFAULT_LAYER_LEVEL);
        assert_eq!(c.layer_mask(), 0b0001_0110);
        assert!(
            (Controls::from_raw([Some(-6.0); CONTROL_COUNT]).master_gain() - 0.501).abs() < 1e-3
        );
    }
}
