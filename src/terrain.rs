use crate::biomes::{calculate_facet_color, classify_biome, BiomeType};
use crate::towns::SettlementSurface;
use crate::world::WorldConfig;
use muxapp::muxui::{d3::*, *};
use std::cell::RefCell;

pub const DEFAULT_GRID_SIZE: i32 = 64 * 2;
pub const DEFAULT_CELL_SIZE: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerrainCacheKey {
    snap_x_bits: u32,
    snap_z_bits: u32,
    seed: u64,
    preset_id: u8,
    water_bits: u32,
    towns_enabled: bool,
    ambient_bits: u32,
}

/// Low-poly terrain mesh generator with flat-shaded facets, slope detection,
/// grid caching, retained GPU VBO rendering, and camera-centered streaming.
pub struct LowPolyTerrain {
    pub grid_size: i32,
    pub cell_size: f32,
    vertex_buffer: RefCell<Vec<Vertex3D>>,
    height_cache: RefCell<Vec<f32>>,
    cached_key: RefCell<Option<TerrainCacheKey>>,
    gpu_mesh: RefCell<Option<GpuMesh>>,
}

impl Default for LowPolyTerrain {
    fn default() -> Self {
        Self::new(DEFAULT_GRID_SIZE, DEFAULT_CELL_SIZE)
    }
}

impl LowPolyTerrain {
    pub fn new(grid_size: i32, cell_size: f32) -> Self {
        let total_verts = (grid_size * grid_size * 6) as usize;
        let total_points = ((grid_size + 1) * (grid_size + 1)) as usize;
        Self {
            grid_size,
            cell_size,
            vertex_buffer: RefCell::new(Vec::with_capacity(total_verts)),
            height_cache: RefCell::new(vec![0.0; total_points]),
            cached_key: RefCell::new(None),
            gpu_mesh: RefCell::new(None),
        }
    }

    /// Evaluates raw ground elevation at world coordinates `(x, z)`.
    #[inline]
    pub fn get_height(&self, config: &WorldConfig, x: f32, z: f32) -> f32 {
        config.get_height(x, z)
    }

    /// Evaluates surface elevation (clamped to water level).
    #[inline]
    pub fn get_surface_height(&self, config: &WorldConfig, x: f32, z: f32) -> f32 {
        self.get_mesh_height(config, x, z).0.max(config.water_level)
    }

    /// Evaluates the exact interpolated elevation and face normal on the rendered low-poly
    /// triangle mesh at world coordinates `(x, z)`. Ensures props and the ground-walk camera
    /// sit 100% flush on the rendered triangle facets without clipping or floating.
    pub fn get_mesh_height(&self, config: &WorldConfig, x: f32, z: f32) -> (f32, Vector3) {
        let x0 = (x / self.cell_size).floor() * self.cell_size;
        let z0 = (z / self.cell_size).floor() * self.cell_size;
        let x1 = x0 + self.cell_size;
        let z1 = z0 + self.cell_size;

        let u = ((x - x0) / self.cell_size).clamp(0.0, 1.0);
        let v = ((z - z0) / self.cell_size).clamp(0.0, 1.0);

        let y00 = config.get_height(x0, z0);
        let y10 = config.get_height(x1, z0);
        let y01 = config.get_height(x0, z1);
        let y11 = config.get_height(x1, z1);

        let p00 = Vector3::new(x0, y00, z0);
        let p10 = Vector3::new(x1, y10, z0);
        let p01 = Vector3::new(x0, y01, z1);
        let p11 = Vector3::new(x1, y11, z1);

        if (y00 - y11).abs() < (y10 - y01).abs() {
            // Split along p00 -> p11
            if u <= v {
                let h = y00 + v * (y01 - y00) + u * (y11 - y01);
                let mut n = (p01 - p00).cross(p11 - p00).normalize();
                if n.y < 0.0 {
                    n = -n;
                }
                (h, n)
            } else {
                let h = y00 + u * (y10 - y00) + v * (y11 - y10);
                let mut n = (p11 - p00).cross(p10 - p00).normalize();
                if n.y < 0.0 {
                    n = -n;
                }
                (h, n)
            }
        } else if u + v <= 1.0 {
            // Split along p10 -> p01 (lower-left triangle)
            let h = y00 + u * (y10 - y00) + v * (y01 - y00);
            let mut n = (p01 - p00).cross(p10 - p00).normalize();
            if n.y < 0.0 {
                n = -n;
            }
            (h, n)
        } else {
            // Split along p10 -> p01 (upper-right triangle)
            let h = y11 + (1.0 - u) * (y01 - y11) + (1.0 - v) * (y10 - y11);
            let mut n = (p01 - p10).cross(p11 - p10).normalize();
            if n.y < 0.0 {
                n = -n;
            }
            (h, n)
        }
    }

    /// Estimates ground surface normal at `(x, z)` using finite differences.
    pub fn get_normal(&self, config: &WorldConfig, x: f32, z: f32) -> Vector3 {
        let delta = 1.0;
        let h_l = config.get_height(x - delta, z);
        let h_r = config.get_height(x + delta, z);
        let h_d = config.get_height(x, z - delta);
        let h_u = config.get_height(x, z + delta);

        Vector3::new(h_l - h_r, 2.0 * delta, h_d - h_u).normalize()
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn emit_facet(
        pa: Vector3,
        pb: Vector3,
        pc: Vector3,
        water_level: f32,
        quad_moist: f32,
        settlement: SettlementSurface,
        is_volcanic: bool,
        sun_dir: Vector3,
        ambient_light: f32,
        out: &mut Vec<Vertex3D>,
    ) {
        let e1 = pb - pa;
        let e2 = pc - pa;
        let mut n = e1.cross(e2).normalize();
        if n.y < 0.0 {
            n = -n;
        }

        let min_y = pa.y.min(pb.y).min(pc.y);
        let max_y = pa.y.max(pb.y).max(pc.y);
        let avg_y = (pa.y + pb.y + pc.y) * 0.33333334;

        // When a facet crosses water_level, classify by its above-water portion so dark underwater
        // colors never protrude above the water surface.
        let effective_y = if min_y < water_level && max_y >= water_level {
            water_level + (max_y - water_level) * 0.55
        } else {
            avg_y
        };

        let rel_h = effective_y - water_level;
        let light = ambient_light + (n.dot(sun_dir)).max(0.0) * (1.0 - ambient_light);

        let biome = if rel_h >= 0.8 {
            match settlement {
                SettlementSurface::TownPlaza | SettlementSurface::CobbleRoad => {
                    BiomeType::TownPlaza
                }
                SettlementSurface::DirtRoad => BiomeType::Road,
                SettlementSurface::TownGreen => BiomeType::Plains,
                SettlementSurface::None => {
                    classify_biome(effective_y, water_level, quad_moist, n.y, is_volcanic)
                }
            }
        } else {
            classify_biome(effective_y, water_level, quad_moist, n.y, is_volcanic)
        };

        let color = calculate_facet_color(biome, rel_h, n.y, light, is_volcanic);

        out.push(Vertex3D::new(pa, color).with_normal(n));
        out.push(Vertex3D::new(pb, color).with_normal(n));
        out.push(Vertex3D::new(pc, color).with_normal(n));
    }

    /// Builds and draws the low-poly terrain mesh centered around `(center_x, center_z)`.
    ///
    /// Performance optimizations:
    /// - Skips rebuilding vertex buffer when camera remains within the same 18x18 grid snap cell.
    /// - Reuses persistent vertex buffer (0 heap allocations per frame).
    /// - Precomputes unique (grid_size + 1)^2 height samples once, reducing noise evaluations by ~74%.
    /// - Adaptive diagonal quad splitting aligns facet edges with ridges, valleys, and coastlines.
    pub fn draw(
        &self,
        d3: &Render3DContext,
        config: &WorldConfig,
        center_x: f32,
        center_z: f32,
        sun_dir: Vector3,
        ambient_light: f32,
    ) {
        // Snap center coordinates to grid step increments to avoid vertex jitter
        let snap_x = (center_x / self.cell_size).floor() * self.cell_size;
        let snap_z = (center_z / self.cell_size).floor() * self.cell_size;

        let current_key = TerrainCacheKey {
            snap_x_bits: snap_x.to_bits(),
            snap_z_bits: snap_z.to_bits(),
            seed: config.seed,
            preset_id: config.preset as u8,
            water_bits: config.water_level.to_bits(),
            towns_enabled: config.towns_enabled,
            ambient_bits: ambient_light.to_bits(),
        };

        if *self.cached_key.borrow() != Some(current_key) {
            let half_grid = self.grid_size / 2;
            let start_x = snap_x - (half_grid as f32) * self.cell_size;
            let start_z = snap_z - (half_grid as f32) * self.cell_size;

            let stride = (self.grid_size + 1) as usize;
            let total_points = stride * stride;

            let mut height_cache = self.height_cache.borrow_mut();
            if height_cache.len() != total_points {
                height_cache.resize(total_points, 0.0);
            }

            // Fast batch height sampling across unique grid vertices
            for i in 0..=self.grid_size {
                let x = start_x + (i as f32) * self.cell_size;
                let row_offset = (i as usize) * stride;
                for j in 0..=self.grid_size {
                    let z = start_z + (j as f32) * self.cell_size;
                    height_cache[row_offset + (j as usize)] = config.get_height(x, z);
                }
            }

            let mut terrain_vertices = self.vertex_buffer.borrow_mut();
            terrain_vertices.clear();

            let is_volcanic = config.is_volcanic();

            for i in 0..self.grid_size {
                let x0 = start_x + (i as f32) * self.cell_size;
                let x1 = x0 + self.cell_size;
                let row0 = (i as usize) * stride;
                let row1 = ((i + 1) as usize) * stride;

                for j in 0..self.grid_size {
                    let z0 = start_z + (j as f32) * self.cell_size;
                    let z1 = z0 + self.cell_size;
                    let j0 = j as usize;
                    let j1 = (j + 1) as usize;

                    let y00 = height_cache[row0 + j0];
                    let y10 = height_cache[row1 + j0];
                    let y01 = height_cache[row0 + j1];
                    let y11 = height_cache[row1 + j1];

                    let p00 = Vector3::new(x0, y00, z0);
                    let p10 = Vector3::new(x1, y10, z0);
                    let p01 = Vector3::new(x0, y01, z1);
                    let p11 = Vector3::new(x1, y11, z1);

                    let mid_x = (x0 + x1) * 0.5;
                    let mid_z = (z0 + z1) * 0.5;

                    // Quad moisture and settlement surface sampled once for both triangles
                    let quad_moist = config.get_moisture(mid_x, mid_z);
                    let settlement = config.get_settlement_surface(mid_x, mid_z);

                    // Adaptive diagonal split: connect the pair of opposite vertices with smaller height diff
                    if (y00 - y11).abs() < (y10 - y01).abs() {
                        Self::emit_facet(
                            p00,
                            p01,
                            p11,
                            config.water_level,
                            quad_moist,
                            settlement,
                            is_volcanic,
                            sun_dir,
                            ambient_light,
                            &mut terrain_vertices,
                        );
                        Self::emit_facet(
                            p00,
                            p11,
                            p10,
                            config.water_level,
                            quad_moist,
                            settlement,
                            is_volcanic,
                            sun_dir,
                            ambient_light,
                            &mut terrain_vertices,
                        );
                    } else {
                        Self::emit_facet(
                            p00,
                            p01,
                            p10,
                            config.water_level,
                            quad_moist,
                            settlement,
                            is_volcanic,
                            sun_dir,
                            ambient_light,
                            &mut terrain_vertices,
                        );
                        Self::emit_facet(
                            p10,
                            p01,
                            p11,
                            config.water_level,
                            quad_moist,
                            settlement,
                            is_volcanic,
                            sun_dir,
                            ambient_light,
                            &mut terrain_vertices,
                        );
                    }
                }
            }

            *self.cached_key.borrow_mut() = Some(current_key);

            let mut gpu_slot = self.gpu_mesh.borrow_mut();
            if let Some(ref mut mesh) = *gpu_slot {
                mesh.update_vertices(&terrain_vertices);
            } else {
                *gpu_slot = Some(GpuMesh::from_vertices(&terrain_vertices));
            }
        }

        // Draw terrain via retained GPU VAO/VBO in a single draw call
        if let Some(ref mut mesh) = *self.gpu_mesh.borrow_mut() {
            d3.draw_mesh(mesh);
        }
    }

    /// Identifies the dominant biome at the given location for the HUD.
    pub fn get_biome_at(&self, config: &WorldConfig, x: f32, z: f32) -> BiomeType {
        let (h, n) = self.get_mesh_height(config, x, z);
        if h >= config.water_level + 0.8 {
            match config.get_settlement_surface(x, z) {
                SettlementSurface::TownPlaza | SettlementSurface::TownGreen => {
                    return BiomeType::TownPlaza;
                }
                SettlementSurface::CobbleRoad | SettlementSurface::DirtRoad => {
                    return BiomeType::Road;
                }
                SettlementSurface::None => {}
            }
        }
        let m = config.get_moisture(x, z);
        classify_biome(h, config.water_level, m, n.y, config.is_volcanic())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldPreset;

    #[test]
    fn test_terrain_elevation_matches_config() {
        let config = WorldConfig::new(42, WorldPreset::Archipelago);
        let terrain = LowPolyTerrain::default();
        let h = terrain.get_height(&config, 50.0, 50.0);
        assert_eq!(h, config.get_height(50.0, 50.0));
    }

    #[test]
    fn test_surface_elevation_clamps_to_water() {
        let config = WorldConfig::new(42, WorldPreset::Archipelago);
        let terrain = LowPolyTerrain::default();
        let sh = terrain.get_surface_height(&config, 0.0, 0.0);
        assert!(sh >= config.water_level);
    }

    #[test]
    fn test_normal_vector_normalized() {
        let config = WorldConfig::new(42, WorldPreset::AlpineHighlands);
        let terrain = LowPolyTerrain::default();
        let norm = terrain.get_normal(&config, 100.0, 100.0);
        let len = norm.length();
        assert!((len - 1.0).abs() < 0.01, "Normal vector should be unit length, got {}", len);
    }
}
