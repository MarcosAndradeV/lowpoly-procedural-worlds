use crate::biomes::BiomeType;
use crate::terrain::LowPolyTerrain;
use crate::world::WorldConfig;
use muxapp::muxui::{d3::*, *};
use std::f32::consts::PI;

/// Low-poly vegetation and landscape decoration models.
pub struct PropGenerator;

impl PropGenerator {
    /// Generates a low-poly conifer / pine tree mesh with 3 shaded tiers.
    pub fn build_pine_tree(
        pos: Vector3,
        scale: f32,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let trunk_col = Color {
            r: 88,
            g: 54,
            b: 34,
            a: 255,
        };
        let tier_colors = [
            Color {
                r: 28,
                g: 82,
                b: 40,
                a: 255,
            },
            Color {
                r: 36,
                g: 100,
                b: 48,
                a: 255,
            },
            Color {
                r: 46,
                g: 118,
                b: 56,
                a: 255,
            },
        ];

        let trunk_radius = 0.45 * scale;
        let trunk_height = 2.6 * scale;
        let sides = 5;

        // Trunk (5-sided cylinder)
        Self::append_cylinder(
            pos,
            trunk_radius,
            trunk_radius * 0.75,
            trunk_height,
            sides,
            trunk_col,
            light_dir,
            ambient,
            verts,
        );

        // Tiered Conical Foliage (3 overlapping faceted cones with solid undersides)
        for (t, &needle_col) in tier_colors.iter().enumerate() {
            let tf = t as f32;
            let base_y = pos.y + (1.8 + tf * 1.45) * scale;
            let cone_h = (3.0 - tf * 0.32) * scale;
            let radius = (2.45 - tf * 0.48) * scale;

            Self::append_cone(
                Vector3::new(pos.x, base_y, pos.z),
                radius,
                cone_h,
                sides,
                needle_col,
                light_dir,
                ambient,
                verts,
            );
        }
    }

    /// Generates a low-poly deciduous / broadleaf tree mesh with a water-tight faceted canopy.
    pub fn build_deciduous_tree(
        pos: Vector3,
        scale: f32,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let trunk_col = Color {
            r: 96,
            g: 62,
            b: 40,
            a: 255,
        };
        let lower_leaf_col = Color {
            r: 44,
            g: 122,
            b: 48,
            a: 255,
        };
        let mid_leaf_col = Color {
            r: 58,
            g: 146,
            b: 56,
            a: 255,
        };
        let top_leaf_col = Color {
            r: 74,
            g: 164,
            b: 64,
            a: 255,
        };

        let trunk_radius = 0.48 * scale;
        let trunk_height = 2.8 * scale;
        let sides = 6;

        Self::append_cylinder(
            pos,
            trunk_radius,
            trunk_radius * 0.68,
            trunk_height,
            5,
            trunk_col,
            light_dir,
            ambient,
            verts,
        );

        // 1. Lower outward-flaring foliage frustum
        let y0 = pos.y + trunk_height * 0.72;
        let h0 = 1.35 * scale;
        let r0_bot = 1.25 * scale;
        let r0_top = 2.65 * scale;
        Self::append_cylinder(
            Vector3::new(pos.x, y0, pos.z),
            r0_bot,
            r0_top,
            h0,
            sides,
            lower_leaf_col,
            light_dir,
            ambient,
            verts,
        );

        // 2. Middle tapering crown frustum (seamlessly joined at y0 + h0 with radius r0_top)
        let y1 = y0 + h0;
        let h1 = 1.45 * scale;
        let r1_top = 1.95 * scale;
        Self::append_cylinder(
            Vector3::new(pos.x, y1, pos.z),
            r0_top,
            r1_top,
            h1,
            sides,
            mid_leaf_col,
            light_dir,
            ambient,
            verts,
        );

        // 3. Upper crown peak cone (seamlessly joined at y1 + h1 with radius r1_top)
        let y2 = y1 + h1;
        let h2 = 1.65 * scale;
        Self::append_cone(
            Vector3::new(pos.x, y2, pos.z),
            r1_top,
            h2,
            sides,
            top_leaf_col,
            light_dir,
            ambient,
            verts,
        );
    }

    /// Generates a coastal palm tree mesh with a leaning segmented trunk and 7 arching 3D fronds.
    pub fn build_palm_tree(
        pos: Vector3,
        scale: f32,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let trunk_col = Color {
            r: 122,
            g: 86,
            b: 52,
            a: 255,
        };
        let frond_upper = Color {
            r: 58,
            g: 156,
            b: 48,
            a: 255,
        };
        let frond_outer = Color {
            r: 46,
            g: 134,
            b: 40,
            a: 255,
        };

        let sides = 5;
        let seg_h = 1.55 * scale;

        // 3-stage segmented trunk
        for s in 0..3 {
            let sf = s as f32;
            let r_bot = (0.42 - sf * 0.08) * scale;
            let r_top = (0.34 - sf * 0.07) * scale;
            let base = Vector3::new(pos.x, pos.y + sf * seg_h, pos.z);
            Self::append_cylinder(
                base, r_bot, r_top, seg_h, sides, trunk_col, light_dir, ambient, verts,
            );
        }

        // Crown of 7 arching 3D double-sided palm fronds
        let top = Vector3::new(pos.x, pos.y + 3.0 * seg_h, pos.z);
        let num_fronds = 7;
        for i in 0..num_fronds {
            let angle = (i as f32 / num_fronds as f32) * 2.0 * PI + 0.25;
            let dir = Vector3::new(angle.cos(), 0.0, angle.sin());
            let side = Vector3::new(-angle.sin(), 0.0, angle.cos()) * (0.62 * scale);

            let p_root = top + Vector3::new(0.0, 0.15 * scale, 0.0);
            let p_mid_l = top + dir * (1.85 * scale) + Vector3::new(0.0, 0.55 * scale, 0.0) + side;
            let p_mid_r = top + dir * (1.85 * scale) + Vector3::new(0.0, 0.55 * scale, 0.0) - side;
            let p_tip = top + dir * (3.65 * scale) + Vector3::new(0.0, -1.05 * scale, 0.0);

            Self::append_double_sided_tri(
                p_root,
                p_mid_l,
                p_mid_r,
                frond_upper,
                light_dir,
                ambient,
                verts,
            );
            Self::append_double_sided_tri(
                p_mid_r,
                p_mid_l,
                p_tip,
                frond_outer,
                light_dir,
                ambient,
                verts,
            );
        }
    }

    /// Generates a faceted boulder / rock.
    pub fn build_rock(
        pos: Vector3,
        scale: f32,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let rock_col = Color {
            r: 118,
            g: 116,
            b: 122,
            a: 255,
        };
        let sides = 6;
        let radius = 1.65 * scale;
        let height = 1.45 * scale;

        Self::append_cone(
            pos, radius, height, sides, rock_col, light_dir, ambient, verts,
        );
    }

    /// Generates a jagged obsidian / volcanic spire for alien volcanic landscapes.
    pub fn build_volcanic_spire(
        pos: Vector3,
        scale: f32,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let spire_col = Color {
            r: 95,
            g: 68,
            b: 92,
            a: 255,
        };
        let sides = 5;
        let radius = 1.35 * scale;
        let height = 2.8 * scale;

        Self::append_cone(
            pos,
            radius,
            height,
            sides,
            spire_col,
            light_dir,
            ambient.max(0.35),
            verts,
        );
    }

    fn append_double_sided_tri(
        p0: Vector3,
        p1: Vector3,
        p2: Vector3,
        color: Color,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let e1 = p1 - p0;
        let e2 = p2 - p0;
        let mut n = e1.cross(e2).normalize();
        if n.y < 0.0 {
            n = -n;
        }
        let lit_top = ambient + (n.dot(light_dir)).max(0.0) * (1.0 - ambient);
        let c_top = Color {
            r: (color.r as f32 * lit_top) as u8,
            g: (color.g as f32 * lit_top) as u8,
            b: (color.b as f32 * lit_top) as u8,
            a: 255,
        };
        let c_bot = Color {
            r: (color.r as f32 * (lit_top * 0.78)) as u8,
            g: (color.g as f32 * (lit_top * 0.78)) as u8,
            b: (color.b as f32 * (lit_top * 0.78)) as u8,
            a: 255,
        };

        // Front / top face
        verts.push(Vertex3D::new(p0, c_top).with_normal(n));
        verts.push(Vertex3D::new(p1, c_top).with_normal(n));
        verts.push(Vertex3D::new(p2, c_top).with_normal(n));

        // Back / bottom face (reversed winding so never culled from below)
        verts.push(Vertex3D::new(p0, c_bot).with_normal(-n));
        verts.push(Vertex3D::new(p2, c_bot).with_normal(-n));
        verts.push(Vertex3D::new(p1, c_bot).with_normal(-n));
    }

    #[allow(clippy::too_many_arguments)]
    fn append_cylinder(
        base: Vector3,
        r_bottom: f32,
        r_top: f32,
        height: f32,
        sides: i32,
        color: Color,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let top = base + Vector3::new(0.0, height, 0.0);
        let cap_n = Vector3::new(0.0, -1.0, 0.0);
        let cap_c = Color {
            r: (color.r as f32 * ambient * 0.85) as u8,
            g: (color.g as f32 * ambient * 0.85) as u8,
            b: (color.b as f32 * ambient * 0.85) as u8,
            a: 255,
        };

        for i in 0..sides {
            let a0 = (i as f32 / sides as f32) * 2.0 * PI;
            let a1 = ((i + 1) as f32 / sides as f32) * 2.0 * PI;

            let b0 = base + Vector3::new(a0.cos() * r_bottom, 0.0, a0.sin() * r_bottom);
            let b1 = base + Vector3::new(a1.cos() * r_bottom, 0.0, a1.sin() * r_bottom);
            let t0 = top + Vector3::new(a0.cos() * r_top, 0.0, a0.sin() * r_top);
            let t1 = top + Vector3::new(a1.cos() * r_top, 0.0, a1.sin() * r_top);

            // Quad split into 2 Counter-Clockwise triangles
            let e1 = t0 - b0;
            let e2 = b1 - b0;
            let n = e1.cross(e2).normalize();
            let lit = ambient + (n.dot(light_dir)).max(0.0) * (1.0 - ambient);
            let c = Color {
                r: (color.r as f32 * lit) as u8,
                g: (color.g as f32 * lit) as u8,
                b: (color.b as f32 * lit) as u8,
                a: 255,
            };

            verts.push(Vertex3D::new(b0, c).with_normal(n));
            verts.push(Vertex3D::new(t0, c).with_normal(n));
            verts.push(Vertex3D::new(b1, c).with_normal(n));

            verts.push(Vertex3D::new(b1, c).with_normal(n));
            verts.push(Vertex3D::new(t0, c).with_normal(n));
            verts.push(Vertex3D::new(t1, c).with_normal(n));

            // Bottom cap so wide foliage skirts are solid when viewed from below
            if r_bottom > 0.8 {
                verts.push(Vertex3D::new(b0, cap_c).with_normal(cap_n));
                verts.push(Vertex3D::new(b1, cap_c).with_normal(cap_n));
                verts.push(Vertex3D::new(base, cap_c).with_normal(cap_n));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn append_cone(
        base: Vector3,
        radius: f32,
        height: f32,
        sides: i32,
        color: Color,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let apex = base + Vector3::new(0.0, height, 0.0);
        let cap_n = Vector3::new(0.0, -1.0, 0.0);
        let cap_c = Color {
            r: (color.r as f32 * ambient * 0.82) as u8,
            g: (color.g as f32 * ambient * 0.82) as u8,
            b: (color.b as f32 * ambient * 0.82) as u8,
            a: 255,
        };

        for i in 0..sides {
            let a0 = (i as f32 / sides as f32) * 2.0 * PI;
            let a1 = ((i + 1) as f32 / sides as f32) * 2.0 * PI;

            let b0 = base + Vector3::new(a0.cos() * radius, 0.0, a0.sin() * radius);
            let b1 = base + Vector3::new(a1.cos() * radius, 0.0, a1.sin() * radius);

            // Counter-Clockwise winding from outside: b0 -> apex -> b1
            let e1 = apex - b0;
            let e2 = b1 - b0;
            let n = e1.cross(e2).normalize();
            let lit = ambient + (n.dot(light_dir)).max(0.0) * (1.0 - ambient);
            let c = Color {
                r: (color.r as f32 * lit) as u8,
                g: (color.g as f32 * lit) as u8,
                b: (color.b as f32 * lit) as u8,
                a: 255,
            };

            verts.push(Vertex3D::new(b0, c).with_normal(n));
            verts.push(Vertex3D::new(apex, c).with_normal(n));
            verts.push(Vertex3D::new(b1, c).with_normal(n));

            // Bottom cap (CCW from below: b0 -> b1 -> base) so cones are solid from underneath
            verts.push(Vertex3D::new(b0, cap_c).with_normal(cap_n));
            verts.push(Vertex3D::new(b1, cap_c).with_normal(cap_n));
            verts.push(Vertex3D::new(base, cap_c).with_normal(cap_n));
        }
    }
}

use crate::camera::ExplorerCamera;
use std::cell::RefCell;

pub const PROP_DENSIT: i32 = 1; // NOTE: 4 is perfect
pub const PROP_CELL_SIZE: f32 = 32.0 / PROP_DENSIT as f32;
pub const PROP_CHUNK_CELLS: i32 = 8;
pub const PROP_CHUNK_RADIUS: i32 = 2 * PROP_DENSIT;
pub const PROP_CHUNK_DIM: i32 = PROP_CHUNK_RADIUS * 2 + 1;
pub const TOTAL_PROP_CHUNKS: usize = (PROP_CHUNK_DIM * PROP_CHUNK_DIM) as usize;
const CHUNK_PROP_GPU_CAPACITY: usize = 9_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PropChunkKey {
    cx: i32,
    cz: i32,
    seed: u64,
    preset_id: u8,
    water_bits: u32,
    towns_enabled: bool,
    ambient_bits: u32,
}

struct PropChunkSlot {
    key: Option<PropChunkKey>,
    min_bound: Vector3,
    max_bound: Vector3,
    active_verts: usize,
    gpu_capacity: usize,
    gpu_mesh: Option<GpuMesh>,
}

impl Default for PropChunkSlot {
    fn default() -> Self {
        Self {
            key: None,
            min_bound: Vector3::zero(),
            max_bound: Vector3::zero(),
            active_verts: 0,
            gpu_capacity: CHUNK_PROP_GPU_CAPACITY,
            gpu_mesh: None,
        }
    }
}

/// Spatially manages, caches, and batches procedural props across a `5x5` toroidal chunk grid
/// with per-chunk `GpuMesh` retention and 3D frustum culling.
pub struct PropManager {
    pub enabled: bool,
    pub cell_size: f32,
    pub chunk_cells: i32,
    pub chunk_radius: i32,
    vertex_buffer: RefCell<Vec<Vertex3D>>,
    chunks: RefCell<Vec<PropChunkSlot>>,
}

impl Default for PropManager {
    fn default() -> Self {
        let mut slots = Vec::with_capacity(TOTAL_PROP_CHUNKS);
        for _ in 0..TOTAL_PROP_CHUNKS {
            slots.push(PropChunkSlot::default());
        }
        Self {
            enabled: true,
            cell_size: PROP_CELL_SIZE,
            chunk_cells: PROP_CHUNK_CELLS,
            chunk_radius: PROP_CHUNK_RADIUS,
            vertex_buffer: RefCell::new(Vec::with_capacity(CHUNK_PROP_GPU_CAPACITY)),
            chunks: RefCell::new(slots),
        }
    }
}

impl PropManager {
    #[inline]
    fn chunk_slot_index(&self, cx: i32, cz: i32) -> usize {
        let dim = self.chunk_radius * 2 + 1;
        (cx.rem_euclid(dim) * dim + cz.rem_euclid(dim)) as usize
    }

    #[allow(clippy::too_many_arguments)]
    fn build_chunk_props(
        &self,
        cx: i32,
        cz: i32,
        terrain: &LowPolyTerrain,
        config: &WorldConfig,
        sun_dir: Vector3,
        ambient: f32,
        prop_vertices: &mut Vec<Vertex3D>,
    ) -> (Vector3, Vector3) {
        prop_vertices.clear();
        let start_cell_x = cx * self.chunk_cells;
        let start_cell_z = cz * self.chunk_cells;
        let chunk_span = (self.chunk_cells as f32) * self.cell_size;
        let start_wx = (start_cell_x as f32) * self.cell_size;
        let start_wz = (start_cell_z as f32) * self.cell_size;

        let mut min_y = config.water_level;
        let mut max_y = config.water_level + 20.0;

        for lx in 0..self.chunk_cells {
            for lz in 0..self.chunk_cells {
                let cell_x = start_cell_x + lx;
                let cell_z = start_cell_z + lz;

                let h = Self::hash2d(cell_x, cell_z, config.seed);
                if (h & 0xFF) > 115 {
                    continue;
                }

                let sub_x = ((h >> 8) & 0xFF) as f32 * 0.003921569;
                let sub_z = ((h >> 16) & 0xFF) as f32 * 0.003921569;

                let wx = (cell_x as f32 + sub_x) * self.cell_size;
                let wz = (cell_z as f32 + sub_z) * self.cell_size;

                let (wy, normal) = terrain.get_mesh_height(config, wx, wz);

                if wy < config.water_level + 3.8 || wy > config.water_level + 82.0 {
                    continue;
                }
                if normal.y < 0.74 {
                    continue;
                }
                if config.get_settlement_surface(wx, wz) != crate::towns::SettlementSurface::None {
                    continue;
                }

                let moisture = config.get_moisture(wx, wz);
                let biome = crate::biomes::classify_biome(
                    wy,
                    config.water_level,
                    moisture,
                    normal.y,
                    config.is_volcanic(),
                );
                let scale = 1.35 + (((h >> 24) & 0x7F) as f32 * 0.0078125) * 0.75;
                let pos = Vector3::new(wx, wy, wz);

                min_y = min_y.min(wy);
                max_y = max_y.max(wy + 16.0 * scale);

                match biome {
                    BiomeType::Beach => {
                        if (h & 3) == 0 {
                            PropGenerator::build_palm_tree(
                                pos,
                                scale * 1.05,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        } else {
                            PropGenerator::build_rock(
                                pos,
                                scale * 0.75,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        }
                    }
                    BiomeType::Plains => {
                        if (h & 3) == 0 && wy < config.water_level + 8.5 {
                            PropGenerator::build_palm_tree(
                                pos,
                                scale * 1.05,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        } else if (h & 3) < 2 {
                            PropGenerator::build_deciduous_tree(
                                pos,
                                scale,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        } else if (h & 3) == 2 {
                            PropGenerator::build_pine_tree(
                                pos,
                                scale * 0.95,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        } else {
                            PropGenerator::build_rock(
                                pos,
                                scale * 0.65,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        }
                    }
                    BiomeType::Forest => {
                        if (h & 3) < 3 {
                            PropGenerator::build_pine_tree(
                                pos,
                                scale * 1.15,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        } else {
                            PropGenerator::build_deciduous_tree(
                                pos,
                                scale,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        }
                    }
                    BiomeType::Highland => {
                        if (h & 3) < 2 {
                            PropGenerator::build_pine_tree(
                                pos,
                                scale * 0.9,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        } else {
                            PropGenerator::build_rock(
                                pos,
                                scale * 1.0,
                                sun_dir,
                                ambient,
                                prop_vertices,
                            );
                        }
                    }
                    BiomeType::VolcanicBasalt if (h & 3) == 0 => {
                        PropGenerator::build_volcanic_spire(
                            pos,
                            scale * 1.15,
                            sun_dir,
                            ambient,
                            prop_vertices,
                        );
                    }
                    BiomeType::Desert | BiomeType::Canyon | BiomeType::MountainRock
                        if (h & 1) == 0 =>
                    {
                        PropGenerator::build_rock(
                            pos,
                            scale * 1.1,
                            sun_dir,
                            ambient,
                            prop_vertices,
                        );
                    }
                    _ => {}
                }
            }
        }

        (
            Vector3::new(start_wx - 8.0, min_y - 2.0, start_wz - 8.0),
            Vector3::new(
                start_wx + chunk_span + 8.0,
                max_y + 4.0,
                start_wz + chunk_span + 8.0,
            ),
        )
    }

    /// Streams, frustum-culls, and renders all procedural prop chunks around `camera`.
    pub fn draw(
        &self,
        d3: &Render3DContext,
        terrain: &LowPolyTerrain,
        config: &WorldConfig,
        camera: &ExplorerCamera,
        sun_dir: Vector3,
        ambient: f32,
    ) {
        if !self.enabled {
            return;
        }

        let chunk_span = (self.chunk_cells as f32) * self.cell_size;
        let cam_cx = (camera.position.x / chunk_span).floor() as i32;
        let cam_cz = (camera.position.z / chunk_span).floor() as i32;

        let mut chunks = self.chunks.borrow_mut();
        let mut prop_vertices = self.vertex_buffer.borrow_mut();

        for dcx in -self.chunk_radius..=self.chunk_radius {
            for dcz in -self.chunk_radius..=self.chunk_radius {
                let cx = cam_cx + dcx;
                let cz = cam_cz + dcz;
                let slot_idx = self.chunk_slot_index(cx, cz);

                let expected_key = PropChunkKey {
                    cx,
                    cz,
                    seed: config.seed,
                    preset_id: config.preset as u8,
                    water_bits: config.water_level.to_bits(),
                    towns_enabled: config.towns_enabled,
                    ambient_bits: ambient.to_bits(),
                };

                if chunks[slot_idx].key != Some(expected_key) {
                    let (min_bound, max_bound) = self.build_chunk_props(
                        cx,
                        cz,
                        terrain,
                        config,
                        sun_dir,
                        ambient,
                        &mut prop_vertices,
                    );

                    let active_count = prop_vertices.len();
                    let slot = &mut chunks[slot_idx];
                    slot.min_bound = min_bound;
                    slot.max_bound = max_bound;
                    slot.active_verts = active_count;
                    slot.key = Some(expected_key);

                    if active_count > 0 {
                        if active_count > slot.gpu_capacity {
                            slot.gpu_capacity = (active_count * 3 / 2).div_ceil(3) * 3;
                        }
                        prop_vertices.resize(
                            slot.gpu_capacity,
                            Vertex3D::new(
                                Vector3::zero(),
                                Color {
                                    r: 0,
                                    g: 0,
                                    b: 0,
                                    a: 0,
                                },
                            ),
                        );
                        if let Some(ref mut mesh) = slot.gpu_mesh {
                            mesh.update_vertices(&prop_vertices);
                        } else {
                            slot.gpu_mesh = Some(GpuMesh::from_vertices(&prop_vertices));
                        }
                    }
                }

                let slot = &mut chunks[slot_idx];
                if slot.active_verts > 0
                    && camera.is_aabb_in_frustum(slot.min_bound, slot.max_bound)
                    && let Some(ref mut mesh) = slot.gpu_mesh
                {
                    d3.draw_mesh(mesh);
                }
            }
        }
    }

    #[inline(always)]
    fn hash2d(x: i32, z: i32, seed: u64) -> u32 {
        let mut h = (x as u32).wrapping_mul(0x85ebca6b)
            ^ (z as u32).wrapping_mul(0xc2b2ae35)
            ^ (seed as u32);
        h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
        h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
        h ^ (h >> 16)
    }
}
