use crate::towns::TOWN_CELL_SIZE;
use crate::world::WorldConfig;
use lppw_noise::hash2d;

/// Grid dimension for macro landmark cells in world units.
pub const LANDMARK_CELL_SIZE: f32 = 850.0;

/// Types of rare, memorable points of interest across the procedural world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandmarkType {
    /// Concentric circle of ancient weathered megaliths, trilithons, and a runic central altar.
    Stonehenge,
    /// Tall coastal promontory beacon tower with observation balcony, glowing lantern room, and wooden pier.
    Lighthouse,
    /// Solitary high-altitude spire with corbelled balconies and a levitating magical crystal orb.
    WizardTower,
    /// Mythical massive ancient giant tree with sprawling buttress roots and lush multi-tiered canopy.
    WorldTree,
    /// Stepped stone pyramid / desert ziggurat with sandstone colonnade and flame braziers.
    AncientTemple,
}

impl LandmarkType {
    pub fn name(&self) -> &'static str {
        match self {
            LandmarkType::Stonehenge => "Ancient Megalith Circle",
            LandmarkType::Lighthouse => "Coastal Beacon Lighthouse",
            LandmarkType::WizardTower => "Solitary Wizard Spire",
            LandmarkType::WorldTree => "Colossal World Tree",
            LandmarkType::AncientTemple => "Forgotten Sandstone Temple",
        }
    }
}

/// Metadata and spatial bounds for a procedural landmark within cell `(lx, lz)`.
#[derive(Debug, Clone, Copy)]
pub struct LandmarkSite {
    pub lx: i32,
    pub lz: i32,
    pub seed: u64,
    pub preset_id: u8,
    pub water_bits: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub landmark_type: LandmarkType,
    pub radius: f32,
    pub blend_radius: f32,
    pub hash: u32,
    pub valid: bool,
}

impl Default for LandmarkSite {
    fn default() -> Self {
        Self {
            lx: i32::MIN,
            lz: i32::MIN,
            seed: 0,
            preset_id: u8::MAX,
            water_bits: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            landmark_type: LandmarkType::Stonehenge,
            radius: 24.0,
            blend_radius: 54.0,
            hash: 0,
            valid: false,
        }
    }
}

/// Evaluates deterministic landmark placement for macro-cell `(lx, lz)`.
pub fn compute_landmark_for_cell(config: &WorldConfig, lx: i32, lz: i32) -> LandmarkSite {
    let cell_hash = hash2d(lx, lz, config.seed ^ 0x9E3779B9);

    // ~65% of macro cells spawn a landmark so the horizon has visual anchors without clutter
    if (cell_hash & 0xFF) > 165 {
        return LandmarkSite {
            lx,
            lz,
            seed: config.seed,
            preset_id: config.preset as u8,
            water_bits: config.water_level.to_bits(),
            valid: false,
            ..Default::default()
        };
    }

    let cell_x0 = (lx as f32) * LANDMARK_CELL_SIZE;
    let cell_z0 = (lz as f32) * LANDMARK_CELL_SIZE;

    let margin = 160.0;
    let inner_span = LANDMARK_CELL_SIZE - margin * 2.0;

    let u = ((cell_hash & 0xFF) as f32) / 255.0;
    let v = (((cell_hash >> 8) & 0xFF) as f32) / 255.0;

    let cand_x = cell_x0 + margin + u * inner_span;
    let cand_z = cell_z0 + margin + v * inner_span;

    // Check proximity to any town in this region to prevent overlap
    let tm_x = (cand_x / TOWN_CELL_SIZE).floor() as i32;
    let tm_z = (cand_z / TOWN_CELL_SIZE).floor() as i32;
    let town = config.get_town_in_cell(tm_x, tm_z);
    if town.valid {
        let dist_to_town = ((cand_x - town.x).powi(2) + (cand_z - town.z).powi(2)).sqrt();
        if dist_to_town < town.blend_radius + 90.0 {
            return LandmarkSite {
                lx,
                lz,
                seed: config.seed,
                preset_id: config.preset as u8,
                water_bits: config.water_level.to_bits(),
                valid: false,
                ..Default::default()
            };
        }
    }

    let raw_y = config.get_raw_height(cand_x, cand_z);
    let water_y = config.water_level;

    // Deep ocean underwater cells cannot support surface landmarks
    if raw_y < water_y - 2.5 {
        return LandmarkSite {
            lx,
            lz,
            seed: config.seed,
            preset_id: config.preset as u8,
            water_bits: config.water_level.to_bits(),
            valid: false,
            ..Default::default()
        };
    }

    let rel_y = raw_y - water_y;
    let (ltype, radius, blend_radius) = if rel_y < 12.0 {
        // Coastal low ground: Lighthouse with stone foundation pier
        (LandmarkType::Lighthouse, 22.0, 48.0)
    } else if rel_y > 50.0 || config.is_volcanic() {
        // Mountain peak or volcanic crag: Solitary Wizard Spire
        (LandmarkType::WizardTower, 26.0, 56.0)
    } else if config.preset == crate::world::WorldPreset::Canyonlands {
        // Desert canyon plateau: Ancient Sandstone Temple
        (LandmarkType::AncientTemple, 30.0, 64.0)
    } else if (cell_hash & 1) == 0 {
        // Forest / Plains / Highland: Ancient Stonehenge Circle
        (LandmarkType::Stonehenge, 28.0, 60.0)
    } else {
        // Forest / Plains / Highland: Colossal World Tree
        (LandmarkType::WorldTree, 32.0, 68.0)
    };

    let final_y = raw_y.max(water_y + 0.8);

    LandmarkSite {
        lx,
        lz,
        seed: config.seed,
        preset_id: config.preset as u8,
        water_bits: config.water_level.to_bits(),
        x: cand_x,
        y: final_y,
        z: cand_z,
        landmark_type: ltype,
        radius,
        blend_radius,
        hash: cell_hash,
        valid: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{WorldConfig, WorldPreset};

    #[test]
    fn test_landmark_evaluation_is_deterministic() {
        let config = WorldConfig::new(4242, WorldPreset::AlpineHighlands);
        let l1 = compute_landmark_for_cell(&config, 1, 2);
        let l2 = compute_landmark_for_cell(&config, 1, 2);
        assert_eq!(l1.valid, l2.valid);
        assert_eq!(l1.x, l2.x);
        assert_eq!(l1.y, l2.y);
        assert_eq!(l1.z, l2.z);
        assert_eq!(l1.landmark_type, l2.landmark_type);
    }
}
