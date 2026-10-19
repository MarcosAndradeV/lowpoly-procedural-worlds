use crate::world::WorldConfig;
pub use lppw_noise::hash2d;
use std::f32::consts::PI;

/// Size of each macro-cell in world units (50 * 18.0 terrain cells).
/// Larger macro-cells ensure towns are spaced naturally across the landscape (1-2 in view).
pub const TOWN_CELL_SIZE: f32 = 900.0 * 2.0;

/// Surface modification classification for towns and roads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementSurface {
    None,
    TownPlaza,
    TownGreen,
    CobbleRoad,
    DirtRoad,
}

/// Deterministic metadata for a town within a macro-cell `(mx, mz)`.
#[derive(Debug, Clone, Copy)]
pub struct TownSite {
    pub mx: i32,
    pub mz: i32,
    pub seed: u64,
    pub preset_id: u8,
    pub water_bits: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub radius: f32,
    pub blend_radius: f32,
    pub hash: u32,
    pub conn_mask: u8,
    pub valid: bool,
}

impl Default for TownSite {
    fn default() -> Self {
        Self {
            mx: i32::MIN,
            mz: i32::MIN,
            seed: 0,
            preset_id: u8::MAX,
            water_bits: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            radius: 56.0,
            blend_radius: 112.0,
            hash: 0,
            conn_mask: 0,
            valid: false,
        }
    }
}

/// Evaluates deterministic town placement for macro-cell `(mx, mz)`.
pub fn compute_town_for_cell(config: &WorldConfig, mx: i32, mz: i32) -> TownSite {
    let cell_hash = hash2d(mx, mz, config.seed ^ 0x5F3759DF);
    let is_starter_cell = mx == 0 && mz == 0;

    // Only ~55% of non-starter macro-cells attempt to spawn a town so the world feels organic and uncrowded
    if !is_starter_cell && (cell_hash & 0xFF) > 140 {
        return TownSite {
            mx,
            mz,
            seed: config.seed,
            preset_id: config.preset as u8,
            water_bits: config.water_level.to_bits(),
            valid: false,
            ..Default::default()
        };
    }

    let cell_origin_x = (mx as f32) * TOWN_CELL_SIZE;
    let cell_origin_z = (mz as f32) * TOWN_CELL_SIZE;

    // Safe inner margin (150.0 units from cell borders) so town flattening & road meanders
    // stay strictly bounded within the cell and its 4 cardinal neighbors.
    let margin = 150.0;
    let inner_span = TOWN_CELL_SIZE - margin * 2.0;

    let base_u = ((cell_hash & 0xFF) as f32) / 255.0;
    let base_v = (((cell_hash >> 8) & 0xFF) as f32) / 255.0;

    // Test candidate spots across the inner cell to find flat, habitable dry ground above water
    let offsets: [(f32, f32); 9] = [
        (base_u, base_v),
        (0.5, 0.5),
        (0.25, 0.25),
        (0.75, 0.25),
        (0.25, 0.75),
        (0.75, 0.75),
        (0.5, 0.2),
        (0.2, 0.5),
        (0.8, 0.5),
    ];

    let mut best_x = cell_origin_x + margin + base_u * inner_span;
    let mut best_z = cell_origin_z + margin + base_v * inner_span;
    let mut best_y = config.get_raw_height(best_x, best_z);
    let mut best_slope = 999.0_f32;
    let mut best_score = f32::MIN;

    for &(ou, ov) in &offsets {
        let u = (ou + base_u * 0.15).fract();
        let v = (ov + base_v * 0.15).fract();
        let cx = cell_origin_x + margin + u * inner_span;
        let cz = cell_origin_z + margin + v * inner_span;

        let h0 = config.get_raw_height(cx, cz);
        let h_e = config.get_raw_height(cx + 24.0, cz);
        let h_w = config.get_raw_height(cx - 24.0, cz);
        let h_n = config.get_raw_height(cx, cz - 24.0);
        let h_s = config.get_raw_height(cx, cz + 24.0);

        let min_h = h0.min(h_e).min(h_w).min(h_n).min(h_s);
        let avg_h = (h0 + h_e + h_w + h_n + h_s) * 0.2;
        let slope_penalty =
            (h_e - h0).abs() + (h_w - h0).abs() + (h_n - h0).abs() + (h_s - h0).abs();

        let rel_h = avg_h - config.water_level;
        let min_rel_h = min_h - config.water_level;

        // Strongly require all 5 sample points to be above water on gentle lowland/midland terrain
        let elev_score = if min_rel_h >= 3.5 && rel_h <= 52.0 {
            120.0 - (rel_h - 15.0).abs() * 0.9
        } else if is_starter_cell && min_rel_h >= -4.0 && rel_h <= 62.0 {
            40.0 - (3.5 - min_rel_h).max(0.0) * 4.0
        } else {
            -500.0 - rel_h.abs()
        };

        let score = elev_score - slope_penalty * 2.0;
        if score > best_score {
            best_score = score;
            best_x = cx;
            best_z = cz;
            best_y = avg_h;
            best_slope = slope_penalty;
        }
    }

    // Non-starter towns only spawn on genuinely dry, gentle land (never in the ocean or on steep cliffs)
    let valid = if is_starter_cell {
        best_y >= config.water_level - 5.0 && best_y <= config.water_level + 68.0
    } else {
        best_y >= config.water_level + 4.5
            && best_y <= config.water_level + 52.0
            && best_slope < 22.0
    };

    let plaza_y = best_y.max(config.water_level + 5.8);
    let radius = 52.0 + ((cell_hash >> 16) & 0x7) as f32 * 1.2;
    let blend_radius = 108.0;

    TownSite {
        mx,
        mz,
        seed: config.seed,
        preset_id: config.preset as u8,
        water_bits: config.water_level.to_bits(),
        x: best_x,
        y: plaza_y,
        z: best_z,
        radius,
        blend_radius,
        hash: cell_hash,
        conn_mask: 0,
        valid,
    }
}

/// Checks whether two neighboring towns can be connected by a land road or short river bridge
/// without spanning across wide oceans or deep canyons.
pub fn can_connect_towns(config: &WorldConfig, a: &TownSite, b: &TownSite) -> bool {
    if !a.valid || !b.valid {
        return false;
    }
    let dist_sq = (b.x - a.x).powi(2) + (b.z - a.z).powi(2);
    // Do not connect towns that are too far apart
    if dist_sq > (TOWN_CELL_SIZE * 1.35).powi(2) {
        return false;
    }

    // Sample 7 points along the curve to reject wide ocean crossings
    let mut water_samples = 0;
    for i in 1..=7 {
        let t = (i as f32) / 8.0;
        let (px, pz, _, _) = sample_road_curve(a, b, t);
        let raw_y = config.get_raw_height(px, pz);
        // Reject if road crosses deep water or extreme mountain peaks
        if raw_y < config.water_level - 6.5 || raw_y > config.water_level + 72.0 {
            return false;
        }
        if raw_y < config.water_level + 0.5 {
            water_samples += 1;
        }
    }

    // Allow at most 2 shallow water samples (narrow river / cove bridge only, never open sea)
    water_samples <= 2
}

/// Evaluates the road centerline offset and distance between two connected towns `a` and `b`.
/// Returns `Some((t, dist_to_centerline))` if `(x, z)` is near the road corridor.
#[inline]
pub fn evaluate_road_segment(
    a: &TownSite,
    b: &TownSite,
    x: f32,
    z: f32,
) -> Option<(f32, f32)> {
    if !a.valid || !b.valid {
        return None;
    }

    // Fast Axis-Aligned Bounding Box reject (with 42-unit margin for road meander + width)
    let min_x = a.x.min(b.x) - 42.0;
    let max_x = a.x.max(b.x) + 42.0;
    let min_z = a.z.min(b.z) - 42.0;
    let max_z = a.z.max(b.z) + 42.0;
    if x < min_x || x > max_x || z < min_z || z > max_z {
        return None;
    }

    let dx = b.x - a.x;
    let dz = b.z - a.z;
    let len_sq = dx * dx + dz * dz;
    if len_sq > (TOWN_CELL_SIZE * 1.35).powi(2) || len_sq < 1.0 {
        return None;
    }
    let len = len_sq.sqrt();
    let inv_len = 1.0 / len;
    let ux = dx * inv_len;
    let uz = dz * inv_len;
    let nx = -uz;
    let nz = ux;

    let rx = x - a.x;
    let rz = z - a.z;

    let proj = (rx * ux + rz * uz) * inv_len;
    if !(-0.01..=1.01).contains(&proj) {
        return None;
    }

    let raw_perp = rx * nx + rz * nz;
    if raw_perp.abs() > 42.0 {
        return None;
    }

    let t = proj.clamp(0.0, 1.0);
    let meander = road_meander_offset(a.hash, b.hash, t);
    let dist = (raw_perp - meander).abs();

    if dist > 11.0 {
        return None;
    }

    Some((t, dist))
}

/// Deterministic sinusoidal meander offset perpendicular to the chord connecting two towns.
/// Guaranteed to equal `0.0` at `t = 0.0` and `t = 1.0`.
#[inline]
pub fn road_meander_offset(hash_a: u32, hash_b: u32, t: f32) -> f32 {
    let sym_hash = hash_a.wrapping_add(hash_b) ^ (hash_a.bitxor_max(hash_b));
    let phase = (sym_hash & 0xFF) as f32 * 0.02454369;
    let sign = if (hash_a < hash_b) ^ ((sym_hash & 0x100) != 0) {
        1.0
    } else {
        -1.0
    };
    let envelope = (t * PI).sin();
    let wave = (t * PI * 2.0 + phase).sin() * 20.0 + (t * PI * 3.0 - phase).cos() * 9.0;
    envelope * wave * sign
}

trait BitXorMax {
    fn bitxor_max(self, other: Self) -> Self;
}
impl BitXorMax for u32 {
    #[inline(always)]
    fn bitxor_max(self, other: Self) -> Self {
        self.max(other).wrapping_mul(0x45d9f3b) ^ self.min(other)
    }
}

/// Computes the world-space 2D point `(x, z)` and unit tangent `(tx, tz)` along the road `a -> b` at `t`.
#[inline]
pub fn sample_road_curve(a: &TownSite, b: &TownSite, t: f32) -> (f32, f32, f32, f32) {
    let dx = b.x - a.x;
    let dz = b.z - a.z;
    let len = (dx * dx + dz * dz).sqrt().max(1.0);
    let ux = dx / len;
    let uz = dz / len;
    let nx = -uz;
    let nz = ux;

    let m0 = road_meander_offset(a.hash, b.hash, t);
    let px = a.x + ux * (t * len) + nx * m0;
    let pz = a.z + uz * (t * len) + nz * m0;

    let dt = 0.01;
    let t1 = (t + dt).min(1.0);
    let t0 = (t - dt).max(0.0);
    let dm = (road_meander_offset(a.hash, b.hash, t1) - road_meander_offset(a.hash, b.hash, t0))
        / ((t1 - t0) * len).max(0.1);

    let mut tx = ux + nx * dm;
    let mut tz = uz + nz * dm;
    let t_len = (tx * tx + tz * tz).sqrt().max(0.001);
    tx /= t_len;
    tz /= t_len;

    (px, pz, tx, tz)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{WorldConfig, WorldPreset};

    #[test]
    fn test_starter_town_is_valid_and_above_water() {
        let config = WorldConfig::new(4242, WorldPreset::Archipelago);
        let town = compute_town_for_cell(&config, 0, 0);
        assert!(town.valid, "Starter town at (0,0) must always be valid");
        assert!(
            town.y >= config.water_level,
            "Starter town plaza must be above sea level: y={}, water={}",
            town.y,
            config.water_level
        );
    }

    #[test]
    fn test_road_meander_is_zero_at_endpoints() {
        let h1 = 12345678;
        let h2 = 87654321;
        assert_eq!(road_meander_offset(h1, h2, 0.0), 0.0);
        assert!((road_meander_offset(h1, h2, 1.0)).abs() < 1e-5);
    }
}
