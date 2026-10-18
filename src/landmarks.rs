use crate::sky::TimeOfDay;
use crate::terrain::LowPolyTerrain;
use crate::towns::{hash2d, TOWN_CELL_SIZE};
use crate::world::WorldConfig;
use muxapp::muxui::{d3::*, *};
use std::cell::RefCell;
use std::f32::consts::PI;

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

const MIN_LANDMARK_GPU_CAPACITY: usize = 32_000;
const LANDMARK_CULL_STEP: f32 = 80.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LandmarkMeshCacheKey {
    snap_cx: i32,
    snap_cz: i32,
    seed: u64,
    preset_id: u8,
    water_bits: u32,
    time_of_day: TimeOfDay,
}

/// Manages retained GPU meshes, spatial queries, and procedural 3D model generation for world landmarks.
pub struct LandmarkManager {
    pub enabled: bool,
    vertex_buffer: RefCell<Vec<Vertex3D>>,
    cached_key: RefCell<Option<LandmarkMeshCacheKey>>,
    gpu_mesh: RefCell<Option<GpuMesh>>,
    gpu_capacity: std::cell::Cell<usize>,
}

impl Default for LandmarkManager {
    fn default() -> Self {
        Self {
            enabled: true,
            vertex_buffer: RefCell::new(Vec::with_capacity(MIN_LANDMARK_GPU_CAPACITY)),
            cached_key: RefCell::new(None),
            gpu_mesh: RefCell::new(None),
            gpu_capacity: std::cell::Cell::new(MIN_LANDMARK_GPU_CAPACITY),
        }
    }
}

impl LandmarkManager {
    /// Checks whether the camera is near a landmark and returns its display name and distance in meters.
    pub fn check_discovery(
        &self,
        config: &WorldConfig,
        camera_pos: Vector3,
    ) -> Option<(&'static str, f32)> {
        if !self.enabled {
            return None;
        }

        let cam_lx = (camera_pos.x / LANDMARK_CELL_SIZE).floor() as i32;
        let cam_lz = (camera_pos.z / LANDMARK_CELL_SIZE).floor() as i32;

        let mut closest: Option<(&'static str, f32)> = None;
        let mut min_dist = 110.0; // Discovery trigger distance

        for dlx in -1..=1 {
            for dlz in -1..=1 {
                let lm = config.get_landmark_in_cell(cam_lx + dlx, cam_lz + dlz);
                if !lm.valid {
                    continue;
                }
                let dist = ((camera_pos.x - lm.x).powi(2)
                    + (camera_pos.y - lm.y).powi(2)
                    + (camera_pos.z - lm.z).powi(2))
                .sqrt();
                if dist < min_dist {
                    min_dist = dist;
                    closest = Some((lm.landmark_type.name(), dist));
                }
            }
        }

        closest
    }

    /// Renders all landmarks visible within the active horizon distance in a single batched GPU draw call.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        d3: &Render3DContext,
        terrain: &LowPolyTerrain,
        config: &WorldConfig,
        center_x: f32,
        center_z: f32,
        sun_dir: Vector3,
        ambient: f32,
        time_of_day: TimeOfDay,
    ) {
        if !self.enabled {
            return;
        }

        let snap_cx = (center_x / LANDMARK_CULL_STEP).floor() as i32;
        let snap_cz = (center_z / LANDMARK_CULL_STEP).floor() as i32;

        let current_key = LandmarkMeshCacheKey {
            snap_cx,
            snap_cz,
            seed: config.seed,
            preset_id: config.preset as u8,
            water_bits: config.water_level.to_bits(),
            time_of_day,
        };

        let needs_rebuild = *self.cached_key.borrow() != Some(current_key);

        if needs_rebuild {
            let mut verts = self.vertex_buffer.borrow_mut();
            verts.clear();

            let cull_x = (snap_cx as f32 + 0.5) * LANDMARK_CULL_STEP;
            let cull_z = (snap_cz as f32 + 0.5) * LANDMARK_CULL_STEP;
            let terrain_half_extent = (terrain.grid_size as f32 * 0.5) * terrain.cell_size;
            let max_extent = terrain_half_extent - 120.0;

            let cam_lx = (cull_x / LANDMARK_CELL_SIZE).floor() as i32;
            let cam_lz = (cull_z / LANDMARK_CELL_SIZE).floor() as i32;

            for dlx in -2..=2 {
                for dlz in -2..=2 {
                    let lm = config.get_landmark_in_cell(cam_lx + dlx, cam_lz + dlz);
                    if !lm.valid {
                        continue;
                    }

                    if (lm.x - cull_x).abs() <= max_extent && (lm.z - cull_z).abs() <= max_extent {
                        let (ground_y, _) = terrain.get_mesh_height(config, lm.x, lm.z);
                        let pos = Vector3::new(lm.x, ground_y, lm.z);
                        match lm.landmark_type {
                            LandmarkType::Stonehenge => {
                                Self::build_stonehenge(pos, lm.hash, sun_dir, ambient, &mut verts);
                            }
                            LandmarkType::Lighthouse => {
                                Self::build_lighthouse(
                                    pos,
                                    lm.hash,
                                    config.water_level,
                                    sun_dir,
                                    ambient,
                                    time_of_day,
                                    &mut verts,
                                );
                            }
                            LandmarkType::WizardTower => {
                                Self::build_wizard_tower(
                                    pos,
                                    lm.hash,
                                    sun_dir,
                                    ambient,
                                    time_of_day,
                                    &mut verts,
                                );
                            }
                            LandmarkType::WorldTree => {
                                Self::build_world_tree(pos, lm.hash, sun_dir, ambient, &mut verts);
                            }
                            LandmarkType::AncientTemple => {
                                Self::build_ancient_temple(
                                    pos, lm.hash, sun_dir, ambient, &mut verts,
                                );
                            }
                        }
                    }
                }
            }

            *self.cached_key.borrow_mut() = Some(current_key);

            let vert_count = verts.len();
            let mut gpu_slot = self.gpu_mesh.borrow_mut();

            if let Some(ref mut mesh) = *gpu_slot {
                if vert_count <= self.gpu_capacity.get() {
                    mesh.update_vertices(&verts);
                } else {
                    let new_cap = (vert_count * 3 / 2).max(MIN_LANDMARK_GPU_CAPACITY);
                    self.gpu_capacity.set(new_cap);
                    *gpu_slot = Some(GpuMesh::from_vertices(&verts));
                }
            } else {
                *gpu_slot = Some(GpuMesh::from_vertices(&verts));
            }
        }

        if let Some(ref mut mesh) = *self.gpu_mesh.borrow_mut() {
            d3.draw_mesh(mesh);
        }
    }

    // =========================================================================
    // 3D Procedural Landmark Model Generators
    // =========================================================================

    /// Builds the ancient Stonehenge megalith circle with standing trilithons, broken fallen stones, and an altar.
    fn build_stonehenge(
        center: Vector3,
        hash: u32,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let stone_col = Color { r: 135, g: 132, b: 138, a: 255 }; // Weathered granite
        let dark_stone = Color { r: 108, g: 104, b: 112, a: 255 };
        let runic_glow = Color { r: 120, g: 215, b: 245, a: 255 }; // Glowing runic inscription

        // 1. Central Altar Stone
        Self::append_box(
            center + Vector3::new(0.0, -0.2, 0.0),
            4.2,
            1.2,
            2.4,
            0.15,
            stone_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            center + Vector3::new(0.0, 1.0, 0.0),
            2.2,
            0.15,
            1.2,
            0.15,
            runic_glow,
            sun_dir,
            1.0,
            verts,
        );

        // 2. Circular Ring of 8 Megalithic Trilithon Pillars (Radius = 14m)
        let ring_radius = 14.5;
        let pillar_count = 8;
        for i in 0..pillar_count {
            let angle = (i as f32 / pillar_count as f32) * 2.0 * PI;
            let px = center.x + angle.cos() * ring_radius;
            let pz = center.z + angle.sin() * ring_radius;
            let pos = Vector3::new(px, center.y, pz);

            // Upright megalith column
            let col_h = 5.6 + ((hash.wrapping_add(i as u32) & 7) as f32) * 0.25;
            Self::append_box(
                pos + Vector3::new(0.0, -0.6, 0.0),
                1.4,
                col_h,
                1.1,
                angle + PI * 0.5,
                stone_col,
                sun_dir,
                ambient,
                verts,
            );

            // Horizontal lintel spanning across adjacent pillars (every 2 pillars)
            if i % 2 == 0 {
                let next_angle = ((i + 1) as f32 / pillar_count as f32) * 2.0 * PI;
                let mid_angle = (angle + next_angle) * 0.5;
                let lx = center.x + mid_angle.cos() * ring_radius;
                let lz = center.z + mid_angle.sin() * ring_radius;
                let lintel_pos = Vector3::new(lx, center.y + col_h - 0.4, lz);
                Self::append_box(
                    lintel_pos,
                    1.6,
                    1.0,
                    5.2,
                    mid_angle,
                    dark_stone,
                    sun_dir,
                    ambient,
                    verts,
                );
            }
        }

        // 3. Fallen broken stones & mossy outlying monoliths
        let outer_stones: [(Vector3, f32, f32); 3] = [
            (Vector3::new(18.0, 0.0, 6.0), 0.7, 0.4),
            (Vector3::new(-16.0, 0.0, -11.0), 1.1, -0.5),
            (Vector3::new(-8.0, 0.0, 19.0), 0.2, 0.8),
        ];
        for (offset, yaw, pitch) in outer_stones {
            let p = center + offset;
            Self::append_box(
                p + Vector3::new(0.0, -0.5, 0.0),
                1.3,
                3.8 * (pitch.abs() + 0.5),
                1.2,
                yaw,
                dark_stone,
                sun_dir,
                ambient,
                verts,
            );
        }
    }

    /// Builds a coastal beacon lighthouse with red-and-white painted bands, glowing lantern room, and wooden pier.
    fn build_lighthouse(
        pos: Vector3,
        _hash: u32,
        water_level: f32,
        sun_dir: Vector3,
        ambient: f32,
        time_of_day: TimeOfDay,
        verts: &mut Vec<Vertex3D>,
    ) {
        let white_stone = Color { r: 236, g: 234, b: 230, a: 255 };
        let red_band = Color { r: 185, g: 45, b: 38, a: 255 };
        let granite_base = Color { r: 104, g: 102, b: 108, a: 255 };
        let roof_copper = Color { r: 42, g: 68, b: 62, a: 255 };
        let lantern_light = match time_of_day {
            TimeOfDay::Noon => Color { r: 255, g: 240, b: 160, a: 255 },
            TimeOfDay::Sunset => Color { r: 255, g: 220, b: 90, a: 255 },
            TimeOfDay::Night => Color { r: 255, g: 245, b: 120, a: 255 },
        };

        // 1. Heavy stone foundation plinth
        Self::append_box(
            pos + Vector3::new(0.0, -1.5, 0.0),
            9.2,
            3.5,
            9.2,
            0.0,
            granite_base,
            sun_dir,
            ambient,
            verts,
        );

        // 2. Tapered 4-band striped tower (White, Red, White, Red)
        let bands = [
            (0.0, 5.0, 6.4, white_stone),
            (5.0, 5.0, 5.8, red_band),
            (10.0, 5.0, 5.2, white_stone),
            (15.0, 5.0, 4.6, red_band),
        ];
        let base_y = pos.y + 2.0;
        for (y_off, h, width, col) in bands {
            Self::append_box(
                Vector3::new(pos.x, base_y + y_off, pos.z),
                width,
                h,
                width,
                0.0,
                col,
                sun_dir,
                ambient,
                verts,
            );
        }

        // 3. Observation gallery balcony & railing
        let balcony_y = base_y + 20.0;
        Self::append_box(
            Vector3::new(pos.x, balcony_y, pos.z),
            5.8,
            0.8,
            5.8,
            0.0,
            granite_base,
            sun_dir,
            ambient,
            verts,
        );

        // 4. Glass lantern room with emissive light source
        let lantern_y = balcony_y + 0.8;
        Self::append_box(
            Vector3::new(pos.x, lantern_y, pos.z),
            3.6,
            3.4,
            3.6,
            0.0,
            lantern_light,
            sun_dir,
            1.2, // Full emissive brightness
            verts,
        );

        // 5. Conical cupola roof
        let roof_y = lantern_y + 3.4;
        Self::append_pyramid(
            Vector3::new(pos.x, roof_y, pos.z),
            4.2,
            3.2,
            roof_copper,
            sun_dir,
            ambient,
            verts,
        );

        // 6. Coastal wooden dock extending out towards water
        let dock_start = pos + Vector3::new(7.0, -0.4, 0.0);
        let wood_plank = Color { r: 122, g: 82, b: 50, a: 255 };
        let dock_len = 16.0;
        let dock_w = 3.2;
        Self::append_box(
            dock_start,
            dock_len,
            0.45,
            dock_w,
            0.0,
            wood_plank,
            sun_dir,
            ambient,
            verts,
        );

        // Dock pilings
        let pilings = [
            dock_start + Vector3::new(-4.0, -1.8, -1.4),
            dock_start + Vector3::new(-4.0, -1.8, 1.4),
            dock_start + Vector3::new(2.0, -1.8, -1.4),
            dock_start + Vector3::new(2.0, -1.8, 1.4),
            dock_start + Vector3::new(7.5, -1.8, -1.4),
            dock_start + Vector3::new(7.5, -1.8, 1.4),
        ];
        let piling_base = (water_level - 3.5).min(pos.y - 4.0);
        for p in pilings {
            let h = (dock_start.y + 0.2) - piling_base;
            Self::append_box(
                Vector3::new(p.x, piling_base, p.z),
                0.45,
                h,
                0.45,
                0.0,
                Color { r: 80, g: 52, b: 32, a: 255 },
                sun_dir,
                ambient,
                verts,
            );
        }
    }

    /// Builds a solitary wizard spire on an elevated peak with conical roof and a levitating magical crystal orb.
    fn build_wizard_tower(
        pos: Vector3,
        _hash: u32,
        sun_dir: Vector3,
        ambient: f32,
        _time_of_day: TimeOfDay,
        verts: &mut Vec<Vertex3D>,
    ) {
        let wall_col = Color { r: 115, g: 110, b: 120, a: 255 };
        let trim_col = Color { r: 85, g: 78, b: 92, a: 255 };
        let wizard_roof_col = Color { r: 64, g: 46, b: 98, a: 255 }; // Deep indigo / arcane violet
        let orb_col = Color { r: 120, g: 235, b: 255, a: 255 }; // Arcane cyan crystal glow

        // 1. Buttressed octagonal base
        Self::append_box(
            pos + Vector3::new(0.0, -1.0, 0.0),
            8.4,
            4.0,
            8.4,
            0.0,
            trim_col,
            sun_dir,
            ambient,
            verts,
        );

        // 2. Main tower shaft (Height = 22m)
        let shaft_base = pos.y + 3.0;
        Self::append_box(
            Vector3::new(pos.x, shaft_base, pos.z),
            6.2,
            12.0,
            6.2,
            0.0,
            wall_col,
            sun_dir,
            ambient,
            verts,
        );

        // 3. Cantilevered upper observatory tier
        let upper_y = shaft_base + 12.0;
        Self::append_box(
            Vector3::new(pos.x, upper_y, pos.z),
            7.4,
            6.0,
            7.4,
            0.0,
            wall_col,
            sun_dir,
            ambient,
            verts,
        );

        // 4. Arcane Conical Steep Spire Roof
        let roof_y = upper_y + 6.0;
        Self::append_pyramid(
            Vector3::new(pos.x, roof_y, pos.z),
            8.2,
            10.5,
            wizard_roof_col,
            sun_dir,
            ambient,
            verts,
        );

        // 5. Levitating Arcane Crystal Orb above spire peak
        let orb_y = roof_y + 11.2;
        Self::append_box(
            Vector3::new(pos.x, orb_y, pos.z),
            1.8,
            1.8,
            1.8,
            0.4,
            orb_col,
            sun_dir,
            1.4, // Emissive glow
            verts,
        );
        Self::append_box(
            Vector3::new(pos.x, orb_y, pos.z),
            1.3,
            1.3,
            1.3,
            0.8,
            Color { r: 235, g: 250, b: 255, a: 255 },
            sun_dir,
            1.5,
            verts,
        );

        // 6. Overhanging wooden wrap-around observatory balcony
        let balc_y = shaft_base + 11.0;
        Self::append_box(
            Vector3::new(pos.x + 4.2, balc_y, pos.z),
            2.2,
            0.35,
            4.0,
            0.0,
            Color { r: 110, g: 74, b: 46, a: 255 },
            sun_dir,
            ambient,
            verts,
        );
    }

    /// Builds a colossal ancient World Tree with massive gnarled trunk, sprawling roots, and multi-tier canopy.
    fn build_world_tree(
        pos: Vector3,
        _hash: u32,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let trunk_col = Color { r: 76, g: 48, b: 30, a: 255 }; // Ancient bark
        let leaves_main = Color { r: 42, g: 122, b: 54, a: 255 }; // Lush emerald
        let leaves_high = Color { r: 62, g: 154, b: 72, a: 255 }; // Vibrant golden canopy
        let leaves_gold = Color { r: 198, g: 165, b: 48, a: 255 }; // Mystical blossom accents

        // 1. Sprawling buttress roots sinking into ground
        let root_angles: [f32; 6] = [0.0, 1.05, 2.1, PI, 4.2, 5.25];
        for angle in root_angles {
            let rx = pos.x + angle.cos() * 5.8;
            let rz = pos.z + angle.sin() * 5.8;
            Self::append_box(
                Vector3::new(rx, pos.y - 1.2, rz),
                2.6,
                3.4,
                2.6,
                angle,
                trunk_col,
                sun_dir,
                ambient,
                verts,
            );
        }

        // 2. Colossal main tree trunk (Width = 8.5m, Height = 19m)
        Self::append_box(
            pos + Vector3::new(0.0, -1.0, 0.0),
            8.2,
            19.0,
            8.2,
            0.12,
            trunk_col,
            sun_dir,
            ambient,
            verts,
        );

        // 3. Multi-tier sprawling canopy of low-poly foliage polyhedra
        let canopy_layers = [
            (Vector3::new(0.0, 15.0, 0.0), 22.0, 7.5, leaves_main),
            (Vector3::new(-6.0, 17.5, -4.0), 14.0, 6.0, leaves_high),
            (Vector3::new(5.5, 17.0, 5.0), 15.0, 6.2, leaves_high),
            (Vector3::new(0.0, 22.0, 0.0), 16.5, 7.0, leaves_high),
            (Vector3::new(0.0, 28.0, 0.0), 11.0, 5.5, leaves_gold),
        ];

        for (offset, width, height, col) in canopy_layers {
            Self::append_pyramid(pos + offset, width, height, col, sun_dir, ambient, verts);
            // Inverted bottom pyramid to give full faceted 3D canopy volume
            Self::append_inverted_pyramid(
                pos + offset,
                width * 0.95,
                height * 0.45,
                col,
                sun_dir,
                ambient,
                verts,
            );
        }
    }

    /// Builds an ancient sandstone ziggurat / desert temple with colonnade and relic altar.
    fn build_ancient_temple(
        pos: Vector3,
        _hash: u32,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let sand_stone = Color { r: 196, g: 148, b: 96, a: 255 };
        let dark_sand = Color { r: 154, g: 108, b: 68, a: 255 };
        let gold_relic = Color { r: 245, g: 195, b: 50, a: 255 };

        // 1. Stepped 3-tier ziggurat platform
        let tiers = [
            (Vector3::new(0.0, -1.5, 0.0), 26.0, 2.8),
            (Vector3::new(0.0, 1.3, 0.0), 19.0, 2.6),
            (Vector3::new(0.0, 3.9, 0.0), 13.0, 2.4),
        ];
        for (offset, w, h) in tiers {
            Self::append_box(
                pos + offset,
                w,
                h,
                w,
                0.0,
                sand_stone,
                sun_dir,
                ambient,
                verts,
            );
        }

        // 2. Temple colonnade atop the top tier
        let temple_y = pos.y + 6.3;
        let pillar_offsets = [
            Vector3::new(-4.5, 0.0, -4.5),
            Vector3::new(4.5, 0.0, -4.5),
            Vector3::new(-4.5, 0.0, 4.5),
            Vector3::new(4.5, 0.0, 4.5),
        ];
        for poff in pillar_offsets {
            Self::append_box(
                Vector3::new(pos.x + poff.x, temple_y, pos.z + poff.z),
                1.4,
                5.0,
                1.4,
                0.0,
                dark_sand,
                sun_dir,
                ambient,
                verts,
            );
        }

        // 3. Temple roof slab
        let roof_y = temple_y + 5.0;
        Self::append_box(
            Vector3::new(pos.x, roof_y, pos.z),
            12.0,
            1.2,
            12.0,
            0.0,
            dark_sand,
            sun_dir,
            ambient,
            verts,
        );

        // 4. Central golden relic pedestal
        Self::append_box(
            Vector3::new(pos.x, temple_y, pos.z),
            1.6,
            2.2,
            1.6,
            0.0,
            gold_relic,
            sun_dir,
            1.2,
            verts,
        );
    }

    // =========================================================================
    // Low-Poly Geometry Utility Functions
    // =========================================================================

    #[inline]
    fn basis(yaw: f32) -> (Vector3, Vector3, Vector3) {
        let c = yaw.cos();
        let s = yaw.sin();
        let right = Vector3::new(c, 0.0, -s);
        let up = Vector3::new(0.0, 1.0, 0.0);
        let forward = Vector3::new(s, 0.0, c);
        (right, up, forward)
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn append_box(
        base_center: Vector3,
        width: f32,
        height: f32,
        depth: f32,
        yaw: f32,
        color: Color,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let (r, u, f) = Self::basis(yaw);
        let hw = r * (width * 0.5);
        let hd = f * (depth * 0.5);
        let h_vec = u * height;

        let b_fl = base_center - hw + hd;
        let b_fr = base_center + hw + hd;
        let b_bl = base_center - hw - hd;
        let b_br = base_center + hw - hd;

        let t_fl = b_fl + h_vec;
        let t_fr = b_fr + h_vec;
        let t_bl = b_bl + h_vec;
        let t_br = b_br + h_vec;

        // Front wall (+f)
        Self::emit_quad(b_fl, b_fr, t_fr, t_fl, f, color, sun_dir, ambient, verts);
        // Back wall (-f)
        Self::emit_quad(b_br, b_bl, t_bl, t_br, -f, color, sun_dir, ambient, verts);
        // Right wall (+r)
        Self::emit_quad(b_fr, b_br, t_br, t_fr, r, color, sun_dir, ambient, verts);
        // Left wall (-r)
        Self::emit_quad(b_bl, b_fl, t_fl, t_bl, -r, color, sun_dir, ambient, verts);
        // Top face (+u)
        Self::emit_quad(t_fl, t_fr, t_br, t_bl, u, color, sun_dir, ambient, verts);
        // Bottom face (-u) - ensures geometry is solid when viewed from below
        Self::emit_quad(b_bl, b_br, b_fr, b_fl, -u, color, sun_dir, ambient * 0.85, verts);
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn append_pyramid(
        base_center: Vector3,
        width: f32,
        height: f32,
        color: Color,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let (r, u, f) = Self::basis(0.0);
        let hw = r * (width * 0.5);
        let hd = f * (width * 0.5);
        let apex = base_center + u * height;

        let b_fl = base_center - hw + hd;
        let b_fr = base_center + hw + hd;
        let b_bl = base_center - hw - hd;
        let b_br = base_center + hw - hd;

        // 4 sloping sides with outward directions ensuring CCW winding
        Self::emit_tri(b_fl, b_fr, apex, f + u * 0.5, color, sun_dir, ambient, verts);
        Self::emit_tri(b_fr, b_br, apex, r + u * 0.5, color, sun_dir, ambient, verts);
        Self::emit_tri(b_br, b_bl, apex, -f + u * 0.5, color, sun_dir, ambient, verts);
        Self::emit_tri(b_bl, b_fl, apex, -r + u * 0.5, color, sun_dir, ambient, verts);
        // Base underside (-u)
        Self::emit_quad(b_fl, b_bl, b_br, b_fr, -u, color, sun_dir, ambient * 0.85, verts);
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn append_inverted_pyramid(
        base_center: Vector3,
        width: f32,
        depth: f32,
        color: Color,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let (r, u, f) = Self::basis(0.0);
        let hw = r * (width * 0.5);
        let hd = f * (width * 0.5);
        let apex = base_center - u * depth;

        let b_fl = base_center - hw + hd;
        let b_fr = base_center + hw + hd;
        let b_bl = base_center - hw - hd;
        let b_br = base_center + hw - hd;

        // 4 downward sloping sides with outward-downward directions
        Self::emit_tri(b_fr, b_fl, apex, f - u * 0.5, color, sun_dir, ambient * 0.85, verts);
        Self::emit_tri(b_br, b_fr, apex, r - u * 0.5, color, sun_dir, ambient * 0.85, verts);
        Self::emit_tri(b_bl, b_br, apex, -f - u * 0.5, color, sun_dir, ambient * 0.85, verts);
        Self::emit_tri(b_fl, b_bl, apex, -r - u * 0.5, color, sun_dir, ambient * 0.85, verts);
        // Top base (+u)
        Self::emit_quad(b_fl, b_fr, b_br, b_bl, u, color, sun_dir, ambient, verts);
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn emit_tri(
        p0: Vector3,
        p1: Vector3,
        p2: Vector3,
        outward_dir: Vector3,
        color: Color,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let cross = (p1 - p0).cross(p2 - p0);
        let len = cross.length();
        let mut n = if len > 1e-5 {
            cross / len
        } else {
            outward_dir.normalize()
        };

        // Guarantee counter-clockwise front-facing winding towards outward_dir
        let (v1, v2) = if n.dot(outward_dir) < 0.0 {
            n = -n;
            (p2, p1)
        } else {
            (p1, p2)
        };

        let shade = (ambient + n.dot(sun_dir).max(0.0) * (1.0 - ambient)).clamp(0.12, 1.35);
        let c = Color {
            r: (color.r as f32 * shade).clamp(0.0, 255.0) as u8,
            g: (color.g as f32 * shade).clamp(0.0, 255.0) as u8,
            b: (color.b as f32 * shade).clamp(0.0, 255.0) as u8,
            a: color.a,
        };

        verts.push(Vertex3D::new(p0, c).with_normal(n));
        verts.push(Vertex3D::new(v1, c).with_normal(n));
        verts.push(Vertex3D::new(v2, c).with_normal(n));
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn emit_quad(
        p0: Vector3,
        p1: Vector3,
        p2: Vector3,
        p3: Vector3,
        outward_dir: Vector3,
        color: Color,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        Self::emit_tri(p0, p1, p2, outward_dir, color, sun_dir, ambient, verts);
        Self::emit_tri(p0, p2, p3, outward_dir, color, sun_dir, ambient, verts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldPreset;

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

    #[test]
    fn test_landmark_geometry_produces_normals() {
        let mut verts = Vec::new();
        let center = Vector3::new(100.0, 50.0, 100.0);
        let sun = Vector3::new(0.4, 0.8, 0.3).normalize();
        LandmarkManager::build_stonehenge(center, 12345, sun, 0.35, &mut verts);

        assert!(!verts.is_empty());
        assert_eq!(verts.len() % 3, 0);
        for v in &verts {
            assert!(v.normal.is_some());
        }
    }

    #[test]
    fn test_box_winding_order_prevents_culled_faces() {
        let mut verts = Vec::new();
        let sun = Vector3::new(0.0, 1.0, 0.0);
        LandmarkManager::append_box(
            Vector3::new(0.0, 0.0, 0.0),
            10.0,
            6.0,
            8.0,
            0.35,
            Color { r: 255, g: 255, b: 255, a: 255 },
            sun,
            0.5,
            &mut verts,
        );

        // 6 faces * 2 triangles * 3 vertices = 36 vertices
        assert_eq!(verts.len(), 36);

        // For every triangle, the computed cross-product normal must point in the same direction
        // as the assigned normal, proving the vertex winding is counter-clockwise (front-facing)
        for chunk in verts.chunks(3) {
            let p0 = chunk[0].position;
            let p1 = chunk[1].position;
            let p2 = chunk[2].position;
            let cross = (p1 - p0).cross(p2 - p0);
            let n = chunk[0].normal.unwrap();
            assert!(n.dot(cross) > 0.0, "Triangle winding was clockwise / back-facing relative to normal!");
        }
    }

    #[test]
    fn test_pyramid_winding_order_prevents_culled_faces() {
        let mut verts = Vec::new();
        let sun = Vector3::new(0.0, 1.0, 0.0);
        LandmarkManager::append_pyramid(
            Vector3::new(0.0, 0.0, 0.0),
            12.0,
            8.0,
            Color { r: 255, g: 255, b: 255, a: 255 },
            sun,
            0.5,
            &mut verts,
        );

        // 4 sloping sides * 3 verts + 2 base triangles * 3 verts = 18 vertices
        assert_eq!(verts.len(), 18);

        for chunk in verts.chunks(3) {
            let p0 = chunk[0].position;
            let p1 = chunk[1].position;
            let p2 = chunk[2].position;
            let cross = (p1 - p0).cross(p2 - p0);
            let n = chunk[0].normal.unwrap();
            assert!(n.dot(cross) > 0.0, "Pyramid triangle winding was clockwise / back-facing!");
        }
    }
}

