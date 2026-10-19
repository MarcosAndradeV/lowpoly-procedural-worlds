use crate::landmarks::{
    compute_landmark_for_cell, LandmarkSite, LANDMARK_CELL_SIZE,
};
use crate::towns::{
    can_connect_towns, compute_town_for_cell, evaluate_road_segment, SettlementSurface, TownSite,
    TOWN_CELL_SIZE,
};
use lppw_noise::NoiseGenerator;
use std::cell::Cell;

const TOWN_CACHE_DIM: i32 = 16;
const TOWN_CACHE_SIZE: usize = (TOWN_CACHE_DIM * TOWN_CACHE_DIM) as usize;

const LANDMARK_CACHE_DIM: i32 = 16;
const LANDMARK_CACHE_SIZE: usize = (LANDMARK_CACHE_DIM * LANDMARK_CACHE_DIM) as usize;

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
    pub towns_enabled: bool,
    pub landmarks_enabled: bool,
    pub terrain_noise: NoiseGenerator,
    pub moisture_noise: NoiseGenerator,
    pub detail_noise: NoiseGenerator,
    town_cache: Vec<Cell<TownSite>>,
    landmark_cache: Vec<Cell<LandmarkSite>>,
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
            towns_enabled: true,
            landmarks_enabled: true,
            terrain_noise,
            moisture_noise,
            detail_noise,
            town_cache: vec![Cell::new(TownSite::default()); TOWN_CACHE_SIZE],
            landmark_cache: vec![Cell::new(LandmarkSite::default()); LANDMARK_CACHE_SIZE],
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

    /// Collision-free O(1) 2D toroidal cache lookup for the town inside macro-cell `(mx, mz)`.
    #[inline]
    pub fn get_town_in_cell(&self, mx: i32, mz: i32) -> TownSite {
        if !self.towns_enabled {
            return TownSite::default();
        }
        let idx = ((mx.rem_euclid(TOWN_CACHE_DIM) as usize) << 4)
            | (mz.rem_euclid(TOWN_CACHE_DIM) as usize);

        let cached = self.town_cache[idx].get();
        let water_bits = self.water_level.to_bits();
        let preset_id = self.preset as u8;

        if cached.mx == mx
            && cached.mz == mz
            && cached.seed == self.seed
            && cached.preset_id == preset_id
            && cached.water_bits == water_bits
        {
            return cached;
        }

        let computed = compute_town_for_cell(self, mx, mz);
        self.town_cache[idx].set(computed);
        computed
    }

    /// Collision-free O(1) 2D toroidal cache lookup for the landmark inside cell `(lx, lz)`.
    #[inline]
    pub fn get_landmark_in_cell(&self, lx: i32, lz: i32) -> LandmarkSite {
        if !self.landmarks_enabled {
            return LandmarkSite::default();
        }
        let idx = ((lx.rem_euclid(LANDMARK_CACHE_DIM) as usize) << 4)
            | (lz.rem_euclid(LANDMARK_CACHE_DIM) as usize);

        let cached = self.landmark_cache[idx].get();
        let water_bits = self.water_level.to_bits();
        let preset_id = self.preset as u8;

        if cached.lx == lx
            && cached.lz == lz
            && cached.seed == self.seed
            && cached.preset_id == preset_id
            && cached.water_bits == water_bits
        {
            return cached;
        }

        let computed = compute_landmark_for_cell(self, lx, lz);
        self.landmark_cache[idx].set(computed);
        computed
    }

    /// Evaluates whether coordinates `(x, z)` are within the immediate footprint of a landmark.
    #[inline]
    pub fn is_near_landmark(&self, x: f32, z: f32) -> bool {
        if !self.landmarks_enabled {
            return false;
        }
        let lx = (x / LANDMARK_CELL_SIZE).floor() as i32;
        let lz = (z / LANDMARK_CELL_SIZE).floor() as i32;
        let lm = self.get_landmark_in_cell(lx, lz);
        if !lm.valid {
            return false;
        }
        let dx = x - lm.x;
        let dz = z - lm.z;
        if dx.abs() >= lm.blend_radius || dz.abs() >= lm.blend_radius {
            return false;
        }
        (dx * dx + dz * dz) < (lm.blend_radius * 0.95).powi(2)
    }

    /// Evaluates unflattened raw procedural terrain elevation at coordinates `(x, z)`.
    pub fn get_raw_height(&self, x: f32, z: f32) -> f32 {
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
                (base_hills * 0.35 + mountain_shape * 0.75 + detail * 0.1) * self.height_scale * 1.5
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

    /// Evaluates terrain elevation at coordinates `(x, z)` including town and landmark plateau flattening.
    pub fn get_height(&self, x: f32, z: f32) -> f32 {
        let mut h = self.get_raw_height(x, z);

        // 1. Landmark plateau flattening
        if self.landmarks_enabled {
            let lx = (x / LANDMARK_CELL_SIZE).floor() as i32;
            let lz = (z / LANDMARK_CELL_SIZE).floor() as i32;
            let lm = self.get_landmark_in_cell(lx, lz);
            if lm.valid {
                let dx = x - lm.x;
                let dz = z - lm.z;
                if dx.abs() < lm.blend_radius && dz.abs() < lm.blend_radius {
                    let dist_sq = dx * dx + dz * dz;
                    if dist_sq < lm.blend_radius * lm.blend_radius {
                        let dist = dist_sq.sqrt();
                        let target_flat = lm.y;
                        if dist <= lm.radius {
                            h = target_flat;
                        } else {
                            let t = (dist - lm.radius) / (lm.blend_radius - lm.radius);
                            let s = t * t * (3.0 - 2.0 * t);
                            h = target_flat + (h - target_flat) * s;
                        }
                    }
                }
            }
        }

        if !self.towns_enabled {
            return h;
        }

        // 2. Town plateau flattening
        let mx = (x / TOWN_CELL_SIZE).floor() as i32;
        let mz = (z / TOWN_CELL_SIZE).floor() as i32;
        let town = self.get_town_in_cell(mx, mz);
        if !town.valid {
            return h;
        }

        // Fast AABB reject: 97%+ of vertices exit here in 2 float ops
        let dx = x - town.x;
        let dz = z - town.z;
        if dx.abs() >= town.blend_radius || dz.abs() >= town.blend_radius {
            return h;
        }

        let dist_sq = dx * dx + dz * dz;
        if dist_sq >= town.blend_radius * town.blend_radius {
            return h;
        }

        let dist = dist_sq.sqrt();
        let target_flat = (town.y * 0.94 + h * 0.06).max(self.water_level + 4.5);
        if dist <= town.radius {
            target_flat
        } else {
            let t = (dist - town.radius) / (town.blend_radius - town.radius);
            let s = t * t * (3.0 - 2.0 * t);
            target_flat + (h - target_flat) * s
        }
    }

    /// Evaluates whether `(x, z)` lies on a town plaza, village green, cobblestone street, or dirt road.
    pub fn get_settlement_surface(&self, x: f32, z: f32) -> SettlementSurface {
        if !self.towns_enabled {
            return SettlementSurface::None;
        }

        let mx = (x / TOWN_CELL_SIZE).floor() as i32;
        let mz = (z / TOWN_CELL_SIZE).floor() as i32;
        let mut town = self.get_town_in_cell(mx, mz);
        if !town.valid {
            return SettlementSurface::None;
        }

        let dx = x - town.x;
        let dz = z - town.z;
        if dx.abs() < town.radius * 1.05 && dz.abs() < town.radius * 1.05 {
            let dist = (dx * dx + dz * dz).sqrt();
            if dist < town.radius * 0.38
                || (dist > town.radius * 0.58 && dist < town.radius * 0.74)
            {
                return SettlementSurface::TownPlaza;
            } else if dist < town.radius * 0.98 {
                return SettlementSurface::TownGreen;
            }
        }

        // Lazy one-time computation of valid cardinal road connections for this town
        if (town.conn_mask & 0x80) == 0 {
            let mut mask = 0x80_u8;
            let n_e = self.get_town_in_cell(mx + 1, mz);
            let n_w = self.get_town_in_cell(mx - 1, mz);
            let n_s = self.get_town_in_cell(mx, mz + 1);
            let n_n = self.get_town_in_cell(mx, mz - 1);
            if can_connect_towns(self, &town, &n_e) {
                mask |= 1;
            }
            if can_connect_towns(self, &town, &n_w) {
                mask |= 2;
            }
            if can_connect_towns(self, &town, &n_s) {
                mask |= 4;
            }
            if can_connect_towns(self, &town, &n_n) {
                mask |= 8;
            }
            town.conn_mask = mask;
            let idx = ((mx.rem_euclid(TOWN_CACHE_DIM) as usize) << 4)
                | (mz.rem_euclid(TOWN_CACHE_DIM) as usize);
            self.town_cache[idx].set(town);
        }

        let active_roads = town.conn_mask & 0x0F;
        if active_roads == 0 {
            return SettlementSurface::None;
        }

        let dirs: [(u8, i32, i32); 4] = [(1, 1, 0), (2, -1, 0), (4, 0, 1), (8, 0, -1)];
        for &(bit, dmx, dmz) in &dirs {
            if (active_roads & bit) != 0 {
                let n = self.get_town_in_cell(mx + dmx, mz + dmz);
                if let Some((t, dist)) = evaluate_road_segment(&town, &n, x, z)
                    && dist <= 10.5
                {
                    return if !(0.22..=0.78).contains(&t) {
                        SettlementSurface::CobbleRoad
                    } else {
                        SettlementSurface::DirtRoad
                    };
                }
            }
        }

        SettlementSurface::None
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

    #[test]
    fn test_landmark_plateau_flattening_and_proximity() {
        let config = WorldConfig::new(9999, WorldPreset::AlpineHighlands);
        // Find a valid landmark across nearby cells
        let mut found_lm: Option<crate::landmarks::LandmarkSite> = None;
        for lx in -3..=3 {
            for lz in -3..=3 {
                let lm = config.get_landmark_in_cell(lx, lz);
                if lm.valid {
                    found_lm = Some(lm);
                    break;
                }
            }
            if found_lm.is_some() {
                break;
            }
        }

        let lm = found_lm.expect("Expected at least one valid landmark in 7x7 cell neighborhood");
        assert!(config.is_near_landmark(lm.x, lm.z));

        // Center should be flattened exactly to lm.y
        let h_center = config.get_height(lm.x, lm.z);
        assert!((h_center - lm.y).abs() < 1e-3);

        // Outside blend radius should not be considered near landmark
        let far_x = lm.x + lm.blend_radius * 2.5;
        let far_z = lm.z + lm.blend_radius * 2.5;
        assert!(!config.is_near_landmark(far_x, far_z));
    }
}
