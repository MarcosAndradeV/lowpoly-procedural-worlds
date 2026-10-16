use crate::noise::NoiseGenerator;

/// Preset configurations that define the generation character of the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldPreset {
    Archipelago,
    AlpineHighlands,
    Canyonlands,
    AlienCrags,
}

impl WorldPreset {
    pub fn name(&self) -> &'static str {
        match self {
            WorldPreset::Archipelago => "Continental Archipelago",
            WorldPreset::AlpineHighlands => "Alpine Highlands",
            WorldPreset::Canyonlands => "Canyonlands & Mesas",
            WorldPreset::AlienCrags => "Alien Volcanic Crags",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            WorldPreset::Archipelago => "Scattered islands, sandy bays, rolling meadows and coastal waters",
            WorldPreset::AlpineHighlands => "Towering ridged mountains, glacial summits, and deep valleys",
            WorldPreset::Canyonlands => "Terraced sandstone mesas, arid plateaus, and dry river canyons",
            WorldPreset::AlienCrags => "Jagged volcanic spires, dark basalt formations, and geothermal vents",
        }
    }
}

/// Dynamic world generation parameters and noise samplers.
#[derive(Debug, Clone)]
pub struct WorldConfig {
    pub seed: u64,
    pub preset: WorldPreset,
    pub water_level: f32,
    pub height_scale: f32,
    pub base_freq: f32,
    pub octaves: usize,
    pub terrain_noise: NoiseGenerator,
    pub moisture_noise: NoiseGenerator,
    pub detail_noise: NoiseGenerator,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self::new(4242, WorldPreset::Archipelago)
    }
}

impl WorldConfig {
    pub fn new(seed: u64, preset: WorldPreset) -> Self {
        let terrain_noise = NoiseGenerator::new(seed);
        let moisture_noise = NoiseGenerator::new(seed.wrapping_add(101));
        let detail_noise = NoiseGenerator::new(seed.wrapping_add(555));

        let mut config = Self {
            seed,
            preset,
            water_level: 0.0,
            height_scale: 70.0,
            base_freq: 0.0018,
            octaves: 6,
            terrain_noise,
            moisture_noise,
            detail_noise,
        };
        config.apply_preset_parameters();
        config
    }

    /// Reconfigures parameters to match the active preset.
    fn apply_preset_parameters(&mut self) {
        match self.preset {
            WorldPreset::Archipelago => {
                self.water_level = 0.0;
                self.height_scale = 65.0;
                self.base_freq = 0.0016;
                self.octaves = 5;
            }
            WorldPreset::AlpineHighlands => {
                self.water_level = -10.0;
                self.height_scale = 115.0;
                self.base_freq = 0.0012;
                self.octaves = 6;
            }
            WorldPreset::Canyonlands => {
                self.water_level = -15.0;
                self.height_scale = 85.0;
                self.base_freq = 0.0014;
                self.octaves = 5;
            }
            WorldPreset::AlienCrags => {
                self.water_level = -5.0;
                self.height_scale = 95.0;
                self.base_freq = 0.0022;
                self.octaves = 6;
            }
        }
    }

    /// Sets a new preset and updates the generation parameters.
    pub fn set_preset(&mut self, preset: WorldPreset) {
        self.preset = preset;
        self.apply_preset_parameters();
    }

    /// Randomizes the seed and re-initializes noise generators.
    pub fn randomize_seed(&mut self) {
        // Fast pseudo-random seed step
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.terrain_noise = NoiseGenerator::new(self.seed);
        self.moisture_noise = NoiseGenerator::new(self.seed.wrapping_add(101));
        self.detail_noise = NoiseGenerator::new(self.seed.wrapping_add(555));
    }

    /// Evaluates terrain elevation at coordinates `(x, z)`.
    pub fn get_height(&self, x: f32, z: f32) -> f32 {
        match self.preset {
            WorldPreset::Archipelago => {
                // Organic continent/island shapes using domain warping
                let warp = self.terrain_noise.domain_warp_fbm(x, z, 120.0, self.octaves, self.base_freq);
                // Island mask: base rolling hills with deep ocean trenches and island groups
                let island_mask = (warp * 1.2).sin();
                let detail = self.detail_noise.fbm2d(x, z, 3, self.base_freq * 4.0, 2.0, 0.45);
                (warp * 0.7 + island_mask * 0.3 + detail * 0.15) * self.height_scale
            }
            WorldPreset::AlpineHighlands => {
                // High mountain ridges using ridged multifractal combined with fBm
                let ridges = self.terrain_noise.ridged2d(x, z, self.octaves, self.base_freq, 2.05, 0.52);
                let base_hills = self.terrain_noise.fbm2d(x, z, 3, self.base_freq * 0.6, 2.0, 0.5);
                let detail = self.detail_noise.fbm2d(x, z, 3, self.base_freq * 3.5, 2.0, 0.4);
                // Sharpen peaks with non-linear power curve
                let mountain_shape = ridges.powf(1.4);
                (base_hills * 0.35 + mountain_shape * 0.75 + detail * 0.1) * self.height_scale
            }
            WorldPreset::Canyonlands => {
                // Terraced plateaus: quantize height slightly to form natural rock strata
                let warp = self.terrain_noise.domain_warp_fbm(x, z, 80.0, 4, self.base_freq);
                let cellular = self.terrain_noise.cellular2d(x * self.base_freq * 1.5, z * self.base_freq * 1.5);
                let raw_h = (warp * 0.6 + cellular * 0.4) * self.height_scale;
                // Terracing effect
                let step = 14.0;
                (raw_h / step).floor() * step + ((raw_h % step) / step).powf(3.0) * step
            }
            WorldPreset::AlienCrags => {
                // Jagged, chaotic volcanic spikes and craters
                let ridges = self.terrain_noise.ridged2d(x, z, self.octaves, self.base_freq * 1.4, 2.1, 0.55);
                let warp = self.terrain_noise.domain_warp_fbm(x, z, 150.0, 4, self.base_freq);
                let crater = self.detail_noise.cellular2d(x * 0.003, z * 0.003);
                (ridges * 0.65 + warp * 0.25 - crater * 0.25) * self.height_scale
            }
        }
    }

    /// Evaluates moisture level in `[0.0, 1.0]` at coordinates `(x, z)`.
    pub fn get_moisture(&self, x: f32, z: f32) -> f32 {
        let m = self.moisture_noise.fbm2d(x, z, 4, self.base_freq * 0.7, 2.0, 0.5);
        // Normalize roughly to [0.0, 1.0]
        ((m + 0.9) * 0.55).clamp(0.0, 1.0)
    }

    /// Whether this world is currently configured as volcanic/alien.
    pub fn is_volcanic(&self) -> bool {
        self.preset == WorldPreset::AlienCrags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presets_produce_different_topography() {
        let mut cfg = WorldConfig::new(1234, WorldPreset::Archipelago);
        let h_arch = cfg.get_height(100.0, 100.0);

        cfg.set_preset(WorldPreset::AlpineHighlands);
        let h_alpine = cfg.get_height(100.0, 100.0);

        assert_ne!(h_arch, h_alpine);
    }

    #[test]
    fn test_moisture_in_range() {
        let cfg = WorldConfig::default();
        for x in (0..20).map(|i| i as f32 * 50.0) {
            for z in (0..20).map(|i| i as f32 * 50.0) {
                let m = cfg.get_moisture(x, z);
                assert!((0.0..=1.0).contains(&m), "Moisture out of range: {}", m);
            }
        }
    }
}
