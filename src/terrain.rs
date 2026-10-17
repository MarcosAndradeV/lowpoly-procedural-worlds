use crate::biomes::{calculate_facet_color, classify_biome, BiomeType};
use crate::camera::ExplorerCamera;
use crate::towns::SettlementSurface;
use crate::world::WorldConfig;
use muxapp::muxui::{d3::*, *};
use std::cell::{Cell, RefCell};

pub const CHUNK_CELLS: i32 = 24;
pub const CHUNK_RADIUS: i32 = 3;
pub const CHUNK_GRID_DIM: i32 = CHUNK_RADIUS * 2 + 1;
pub const DEFAULT_GRID_SIZE: i32 = CHUNK_CELLS * CHUNK_GRID_DIM;
pub const DEFAULT_CELL_SIZE: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerrainChunkKey {
    cx: i32,
    cz: i32,
    seed: u64,
    preset_id: u8,
    water_bits: u32,
    towns_enabled: bool,
    ambient_bits: u32,
}

struct TerrainChunkSlot {
    key: Option<TerrainChunkKey>,
    min_bound: Vector3,
    max_bound: Vector3,
    gpu_mesh: Option<GpuMesh>,
}

impl Default for TerrainChunkSlot {
    fn default() -> Self {
        Self {
            key: None,
            min_bound: Vector3::zero(),
            max_bound: Vector3::zero(),
            gpu_mesh: None,
        }
    }
}

/// Chunked low-poly terrain generator with a toroidal `GpuMesh` ring-buffer pool,
/// per-chunk dirty-state caching, adaptive diagonal quad splitting, and 3D frustum culling.
pub struct LowPolyTerrain {
    pub grid_size: i32,
    pub cell_size: f32,
    pub chunk_cells: i32,
    pub chunk_radius: i32,
    vertex_buffer: RefCell<Vec<Vertex3D>>,
    height_cache: RefCell<Vec<f32>>,
    chunks: RefCell<Vec<TerrainChunkSlot>>,
    visible_chunks: Cell<usize>,
}

impl Default for LowPolyTerrain {
    fn default() -> Self {
        Self::new(DEFAULT_GRID_SIZE, DEFAULT_CELL_SIZE)
    }
}

impl LowPolyTerrain {
    pub fn new(grid_size: i32, cell_size: f32) -> Self {
        let chunk_cells = CHUNK_CELLS;
        let chunk_radius = CHUNK_RADIUS;
        let dim = chunk_radius * 2 + 1;
        let num_chunks = (dim * dim) as usize;
        let chunk_verts = (chunk_cells * chunk_cells * 6) as usize;
        let chunk_points = ((chunk_cells + 1) * (chunk_cells + 1)) as usize;

        let mut slots = Vec::with_capacity(num_chunks);
        for _ in 0..num_chunks {
            slots.push(TerrainChunkSlot::default());
        }

        Self {
            grid_size,
            cell_size,
            chunk_cells,
            chunk_radius,
            vertex_buffer: RefCell::new(Vec::with_capacity(chunk_verts)),
            height_cache: RefCell::new(vec![0.0; chunk_points]),
            chunks: RefCell::new(slots),
            visible_chunks: Cell::new(0),
        }
    }

    /// Returns `(visible_chunks, total_chunks)` from the most recent `draw` pass.
    pub fn chunk_stats(&self) -> (usize, usize) {
        let dim = self.chunk_radius * 2 + 1;
        (self.visible_chunks.get(), (dim * dim) as usize)
    }

    /// Maps world chunk coordinates `(cx, cz)` to a deterministic toroidal slot index.
    #[inline]
    pub fn chunk_slot_index(&self, cx: i32, cz: i32) -> usize {
        let dim = self.chunk_radius * 2 + 1;
        (cx.rem_euclid(dim) * dim + cz.rem_euclid(dim)) as usize
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

    /// Generates the mesh vertices and vertical bounding range `(min_y, max_y)` for a single chunk `(cx, cz)`.
    fn build_chunk_geometry(
        &self,
        config: &WorldConfig,
        cx: i32,
        cz: i32,
        sun_dir: Vector3,
        ambient_light: f32,
        out_verts: &mut Vec<Vertex3D>,
    ) -> (Vector3, Vector3) {
        let chunk_span = (self.chunk_cells as f32) * self.cell_size;
        let start_x = (cx as f32) * chunk_span;
        let start_z = (cz as f32) * chunk_span;
        let end_x = start_x + chunk_span;
        let end_z = start_z + chunk_span;

        let stride = (self.chunk_cells + 1) as usize;
        let total_points = stride * stride;

        let mut height_cache = self.height_cache.borrow_mut();
        if height_cache.len() != total_points {
            height_cache.resize(total_points, 0.0);
        }

        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for i in 0..=self.chunk_cells {
            let x = start_x + (i as f32) * self.cell_size;
            let row_offset = (i as usize) * stride;
            for j in 0..=self.chunk_cells {
                let z = start_z + (j as f32) * self.cell_size;
                let h = config.get_height(x, z);
                if h < min_y {
                    min_y = h;
                }
                if h > max_y {
                    max_y = h;
                }
                height_cache[row_offset + (j as usize)] = h;
            }
        }

        out_verts.clear();
        let is_volcanic = config.is_volcanic();

        for i in 0..self.chunk_cells {
            let x0 = start_x + (i as f32) * self.cell_size;
            let x1 = x0 + self.cell_size;
            let row0 = (i as usize) * stride;
            let row1 = ((i + 1) as usize) * stride;

            for j in 0..self.chunk_cells {
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

                let quad_moist = config.get_moisture(mid_x, mid_z);
                let settlement = config.get_settlement_surface(mid_x, mid_z);

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
                        out_verts,
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
                        out_verts,
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
                        out_verts,
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
                        out_verts,
                    );
                }
            }
        }

        (
            Vector3::new(start_x, min_y - 2.0, start_z),
            Vector3::new(end_x, max_y + 2.0, end_z),
        )
    }

    /// Streams, frustum-culls, and renders the spatial chunk grid around `camera`.
    ///
    /// Chunk Rendering Architecture:
    /// - Divides the world into `24x24`-cell chunks (`432x432` world units each) across a `7x7` toroidal ring buffer.
    /// - Only newly entered perimeter chunks at the horizon are generated as the camera crosses chunk boundaries,
    ///   leaving 86% of chunks untouched in GPU VRAM.
    /// - Reuses fixed-size chunk `GpuMesh` slots in-place via `UpdateMeshBuffer` (0 VAO allocations after startup).
    /// - Applies 3D frustum culling per chunk AABB before issuing `d3.draw_mesh(...)` calls.
    pub fn draw(
        &self,
        d3: &Render3DContext,
        config: &WorldConfig,
        camera: &ExplorerCamera,
        sun_dir: Vector3,
        ambient_light: f32,
    ) {
        let chunk_span = (self.chunk_cells as f32) * self.cell_size;
        let cam_cx = (camera.position.x / chunk_span).floor() as i32;
        let cam_cz = (camera.position.z / chunk_span).floor() as i32;

        let mut chunks = self.chunks.borrow_mut();
        let mut scratch_verts = self.vertex_buffer.borrow_mut();

        let mut visible_count = 0usize;

        for dcx in -self.chunk_radius..=self.chunk_radius {
            for dcz in -self.chunk_radius..=self.chunk_radius {
                let cx = cam_cx + dcx;
                let cz = cam_cz + dcz;
                let slot_idx = self.chunk_slot_index(cx, cz);

                let expected_key = TerrainChunkKey {
                    cx,
                    cz,
                    seed: config.seed,
                    preset_id: config.preset as u8,
                    water_bits: config.water_level.to_bits(),
                    towns_enabled: config.towns_enabled,
                    ambient_bits: ambient_light.to_bits(),
                };

                if chunks[slot_idx].key != Some(expected_key) {
                    let (min_bound, max_bound) = self.build_chunk_geometry(
                        config,
                        cx,
                        cz,
                        sun_dir,
                        ambient_light,
                        &mut scratch_verts,
                    );

                    let slot = &mut chunks[slot_idx];
                    slot.min_bound = min_bound;
                    slot.max_bound = max_bound;
                    slot.key = Some(expected_key);

                    if let Some(ref mut mesh) = slot.gpu_mesh {
                        mesh.update_vertices(&scratch_verts);
                    } else {
                        slot.gpu_mesh = Some(GpuMesh::from_vertices(&scratch_verts));
                    }
                }

                let slot = &mut chunks[slot_idx];
                if camera.is_aabb_in_frustum(slot.min_bound, slot.max_bound)
                    && let Some(ref mut mesh) = slot.gpu_mesh
                {
                    d3.draw_mesh(mesh);
                    visible_count += 1;
                }
            }
        }

        self.visible_chunks.set(visible_count);
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
        assert!(
            (len - 1.0).abs() < 0.01,
            "Normal vector should be unit length, got {}",
            len
        );
    }

    #[test]
    fn test_chunk_seam_continuity_and_toroidal_mapping() {
        let config = WorldConfig::new(4242, WorldPreset::Archipelago);
        let terrain = LowPolyTerrain::default();
        let sun = Vector3::new(0.4, 0.8, 0.3).normalize();
        let mut verts_a = Vec::new();
        let mut verts_b = Vec::new();

        let (min_a, max_a) = terrain.build_chunk_geometry(&config, 0, 0, sun, 0.35, &mut verts_a);
        let (min_b, _max_b) = terrain.build_chunk_geometry(&config, 1, 0, sun, 0.35, &mut verts_b);

        // Every 24x24 chunk has 24 * 24 * 6 = 3456 vertices
        assert_eq!(verts_a.len(), (CHUNK_CELLS * CHUNK_CELLS * 6) as usize);
        assert_eq!(verts_b.len(), (CHUNK_CELLS * CHUNK_CELLS * 6) as usize);

        // East boundary of chunk (0, 0) equals West boundary of chunk (1, 0)
        assert!((max_a.x - min_b.x).abs() < 1e-4);
        assert!(max_a.y > min_a.y);

        // All 7x7 chunk offsets around any center map to 49 unique toroidal slots
        let mut seen = [false; (CHUNK_GRID_DIM * CHUNK_GRID_DIM) as usize];
        for dcx in -CHUNK_RADIUS..=CHUNK_RADIUS {
            for dcz in -CHUNK_RADIUS..=CHUNK_RADIUS {
                let idx = terrain.chunk_slot_index(10 + dcx, -7 + dcz);
                assert!(!seen[idx], "Duplicate toroidal slot index {}", idx);
                seen[idx] = true;
            }
        }
    }
}
