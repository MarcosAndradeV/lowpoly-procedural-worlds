use crate::biomes::{calculate_facet_color, classify_biome, BiomeType};
use crate::world::WorldConfig;
use muxapp::muxui::{d3::*, *};
use std::cell::RefCell;

pub const DEFAULT_GRID_SIZE: i32 = 64 * 2;
pub const DEFAULT_CELL_SIZE: f32 = 18.0 ;

/// Low-poly terrain mesh generator with flat-shaded facets, slope detection,
/// grid caching, and camera-centered streaming.
pub struct LowPolyTerrain {
    pub grid_size: i32,
    pub cell_size: f32,
    vertex_buffer: RefCell<Vec<Vertex3D>>,
    height_cache: RefCell<Vec<f32>>,
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
        self.get_height(config, x, z).max(config.water_level)
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

    /// Builds and draws the low-poly terrain mesh centered around `(center_x, center_z)`.
    ///
    /// Performance optimizations:
    /// - Reuses persistent vertex buffer (0 heap allocations per frame).
    /// - Precomputes unique (grid_size + 1)^2 height samples once, reducing noise evaluations by ~74%.
    /// - Shared moisture evaluation per quad.
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

                // Quad moisture sampled once for both triangles
                let quad_moist = config.get_moisture((x0 + x1) * 0.5, (z0 + z1) * 0.5);

                // --- Triangle 1: p00 -> p01 -> p10 (Counter-Clockwise) ---
                let e1 = p01 - p00;
                let e2 = p10 - p00;
                let mut n1 = e1.cross(e2).normalize();
                if n1.y < 0.0 {
                    n1 = -n1;
                }

                let light1 = ambient_light + (n1.dot(sun_dir)).max(0.0) * (1.0 - ambient_light);
                let avg_y1 = (y00 + y01 + y10) * 0.33333334;
                let biome1 = classify_biome(avg_y1, config.water_level, quad_moist, n1.y, is_volcanic);
                let c1 = calculate_facet_color(biome1, n1.y, light1);

                terrain_vertices.push(Vertex3D::new(p00, c1).with_normal(n1));
                terrain_vertices.push(Vertex3D::new(p01, c1).with_normal(n1));
                terrain_vertices.push(Vertex3D::new(p10, c1).with_normal(n1));

                // --- Triangle 2: p10 -> p01 -> p11 (Counter-Clockwise) ---
                let e3 = p01 - p10;
                let e4 = p11 - p10;
                let mut n2 = e3.cross(e4).normalize();
                if n2.y < 0.0 {
                    n2 = -n2;
                }

                let light2 = ambient_light + (n2.dot(sun_dir)).max(0.0) * (1.0 - ambient_light);
                let avg_y2 = (y10 + y01 + y11) * 0.33333334;
                let biome2 = classify_biome(avg_y2, config.water_level, quad_moist, n2.y, is_volcanic);
                let c2 = calculate_facet_color(biome2, n2.y, light2);

                terrain_vertices.push(Vertex3D::new(p10, c2).with_normal(n2));
                terrain_vertices.push(Vertex3D::new(p01, c2).with_normal(n2));
                terrain_vertices.push(Vertex3D::new(p11, c2).with_normal(n2));
            }
        }

        // Draw terrain safely chunked to protect against Raylib batch limits
        d3.draw_trimesh_buffered(&terrain_vertices);
    }

    /// Identifies the dominant biome at the given location for the HUD.
    pub fn get_biome_at(&self, config: &WorldConfig, x: f32, z: f32) -> BiomeType {
        let h = self.get_height(config, x, z);
        let m = config.get_moisture(x, z);
        let n = self.get_normal(config, x, z);
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
