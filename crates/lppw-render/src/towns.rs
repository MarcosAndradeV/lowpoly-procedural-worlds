pub use lppw_world::towns::*;

use crate::sky::TimeOfDay;
use crate::terrain::LowPolyTerrain;
use lppw_world::world::WorldConfig;
use muxapp::muxui::{d3::*, *};
use std::cell::RefCell;
use std::f32::consts::PI;

const MIN_TOWN_GPU_CAPACITY: usize = 48_000;
const TOWN_CULL_STEP: f32 = 72.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TownMeshCacheKey {
    snap_cx: i32,
    snap_cz: i32,
    seed: u64,
    preset_id: u8,
    water_bits: u32,
    time_of_day: TimeOfDay,
}

/// Manages, caches, and batches 3D fantasy town buildings, towers, windmills, lanterns, cobblestones, and bridges.
/// Uses dirty-state caching and terrain-horizon culling so towns never render outside the terrain mesh.
pub struct TownManager {
    pub enabled: bool,
    vertex_buffer: RefCell<Vec<Vertex3D>>,
    cached_key: RefCell<Option<TownMeshCacheKey>>,
    gpu_mesh: RefCell<Option<GpuMesh>>,
    gpu_capacity: std::cell::Cell<usize>,
}

impl Default for TownManager {
    fn default() -> Self {
        Self {
            enabled: true,
            vertex_buffer: RefCell::new(Vec::with_capacity(MIN_TOWN_GPU_CAPACITY)),
            cached_key: RefCell::new(None),
            gpu_mesh: RefCell::new(None),
            gpu_capacity: std::cell::Cell::new(MIN_TOWN_GPU_CAPACITY),
        }
    }
}

impl TownManager {
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

        let snap_cx = (center_x / TOWN_CULL_STEP).floor() as i32;
        let snap_cz = (center_z / TOWN_CULL_STEP).floor() as i32;

        let current_key = TownMeshCacheKey {
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

            // Window & lantern emissive warmth (bright golden glow at Sunset and Night)
            let window_col = match time_of_day {
                TimeOfDay::Noon => Color { r: 195, g: 220, b: 235, a: 255 },
                TimeOfDay::Sunset => Color { r: 255, g: 205, b: 88, a: 255 },
                TimeOfDay::Night => Color { r: 255, g: 220, b: 105, a: 255 },
            };

            let cull_x = (snap_cx as f32 + 0.5) * TOWN_CULL_STEP;
            let cull_z = (snap_cz as f32 + 0.5) * TOWN_CULL_STEP;
            let terrain_half_extent = (terrain.grid_size as f32 * 0.5) * terrain.cell_size;
            // Keep towns + outer cottages strictly inside the rendered terrain square
            let max_town_extent = terrain_half_extent - 165.0;
            let max_road_extent = terrain_half_extent - 65.0;

            let cam_mx = (cull_x / TOWN_CELL_SIZE).floor() as i32;
            let cam_mz = (cull_z / TOWN_CELL_SIZE).floor() as i32;

            // Check towns and connecting roads in the 3x3 macro-cell neighborhood,
            // culling anything beyond the active terrain render distance.
            for dmx in -1..=1 {
                for dmz in -1..=1 {
                    let mx = cam_mx + dmx;
                    let mz = cam_mz + dmz;
                    let town = config.get_town_in_cell(mx, mz);
                    if !town.valid {
                        continue;
                    }

                    if (town.x - cull_x).abs() <= max_town_extent
                        && (town.z - cull_z).abs() <= max_town_extent
                    {
                        Self::build_town(
                            &town,
                            terrain,
                            config,
                            sun_dir,
                            ambient,
                            window_col,
                            &mut verts,
                        );
                    }

                    // Build East road (mx, mz) -> (mx + 1, mz) if valid land/river route
                    let east = config.get_town_in_cell(mx + 1, mz);
                    if can_connect_towns(config, &town, &east) {
                        Self::build_road_and_bridges(
                            &town,
                            &east,
                            terrain,
                            config,
                            cull_x,
                            cull_z,
                            max_road_extent,
                            sun_dir,
                            ambient,
                            window_col,
                            &mut verts,
                        );
                    }

                    // Build South road (mx, mz) -> (mx, mz + 1) if valid land/river route
                    let south = config.get_town_in_cell(mx, mz + 1);
                    if can_connect_towns(config, &town, &south) {
                        Self::build_road_and_bridges(
                            &town,
                            &south,
                            terrain,
                            config,
                            cull_x,
                            cull_z,
                            max_road_extent,
                            sun_dir,
                            ambient,
                            window_col,
                            &mut verts,
                        );
                    }
                }
            }

            *self.cached_key.borrow_mut() = Some(current_key);

            let mut cap = self.gpu_capacity.get();
            if verts.len() > cap {
                cap = (verts.len() * 3 / 2).div_ceil(3) * 3;
                self.gpu_capacity.set(cap);
            }
            verts.resize(
                cap,
                Vertex3D::new(Vector3::zero(), Color { r: 0, g: 0, b: 0, a: 0 }),
            );

            // Upload or update retained GPU mesh in-place
            let mut gpu_slot = self.gpu_mesh.borrow_mut();
            if let Some(ref mut mesh) = *gpu_slot {
                mesh.update_vertices(&verts);
            } else {
                *gpu_slot = Some(GpuMesh::from_vertices(&verts));
            }
        }

        // Draw via retained GPU VAO/VBO in a single call
        if let Some(ref mut mesh) = *self.gpu_mesh.borrow_mut() {
            d3.draw_mesh(mesh);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_town(
        town: &TownSite,
        terrain: &LowPolyTerrain,
        config: &WorldConfig,
        sun_dir: Vector3,
        ambient: f32,
        window_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        let center_y = terrain.get_mesh_height(config, town.x, town.z).0;
        let center_pos = Vector3::new(town.x, center_y, town.z);

        // 1. Central Plaza Landmark: Medieval Keep/Guildhall or Market Well + Bell Tower
        if (town.hash & 1) == 0 {
            Self::build_guildhall_keep(
                center_pos,
                (town.hash as f32) * 0.01,
                sun_dir,
                ambient,
                window_col,
                verts,
            );
        } else {
            Self::build_town_well_and_market(
                center_pos,
                town.hash,
                sun_dir,
                ambient,
                window_col,
                verts,
            );
        }

        // 2. Determine road angles leaving this town so we never place buildings on top of roads
        let mut road_angles = [f32::MAX; 4];
        let neighbors = [
            config.get_town_in_cell(town.mx + 1, town.mz),
            config.get_town_in_cell(town.mx - 1, town.mz),
            config.get_town_in_cell(town.mx, town.mz + 1),
            config.get_town_in_cell(town.mx, town.mz - 1),
        ];
        for (idx, n) in neighbors.iter().enumerate() {
            if can_connect_towns(config, town, n) {
                let (_, _, tx, tz) = sample_road_curve(town, n, 0.05);
                road_angles[idx] = tz.atan2(tx);
            }
        }

        // 3. Inner Ring of Cottages & Taverns (around the cobblestone plaza)
        let inner_count = 5;
        for i in 0..inner_count {
            let h = hash2d(i, (town.hash & 0xFFFF) as i32, config.seed);
            let base_angle = (i as f32 / inner_count as f32) * 2.0 * PI
                + ((h & 0xFF) as f32 * 0.0012);

            if Self::is_angle_near_road(base_angle, &road_angles, 0.36) {
                continue;
            }

            let dist = 24.0 + ((h >> 8) & 0xF) as f32 * 0.35;
            let bx = town.x + base_angle.cos() * dist;
            let bz = town.z + base_angle.sin() * dist;
            let (by, _) = terrain.get_mesh_height(config, bx, bz);
            if by < config.water_level + 2.0 {
                continue;
            }

            let yaw = (town.x - bx).atan2(town.z - bz);
            let bpos = Vector3::new(bx, by, bz);

            if i == 0 {
                Self::build_medieval_tavern(bpos, yaw, h, sun_dir, ambient, window_col, verts);
            } else {
                Self::build_fantasy_cottage(bpos, yaw, 1.0, h, sun_dir, ambient, window_col, verts);
            }
        }

        // 4. Outer Ring of Cottages, Watchtowers, and Windmill
        let outer_count = 7;
        for i in 0..outer_count {
            let h = hash2d(i + 31, (town.hash >> 8) as i32, config.seed);
            let base_angle = (i as f32 / outer_count as f32) * 2.0 * PI + 0.22;

            if Self::is_angle_near_road(base_angle, &road_angles, 0.28) {
                let gate_angle = base_angle + 0.28;
                let gx = town.x + gate_angle.cos() * 44.0;
                let gz = town.z + gate_angle.sin() * 44.0;
                let (gy, _) = terrain.get_mesh_height(config, gx, gz);
                if gy >= config.water_level + 2.5 {
                    Self::build_watchtower(
                        Vector3::new(gx, gy, gz),
                        h,
                        sun_dir,
                        ambient,
                        window_col,
                        verts,
                    );
                }
                continue;
            }

            let dist = 42.0 + ((h >> 8) & 0xF) as f32 * 0.5;
            let bx = town.x + base_angle.cos() * dist;
            let bz = town.z + base_angle.sin() * dist;
            let (by, _) = terrain.get_mesh_height(config, bx, bz);
            if by < config.water_level + 2.0 {
                continue;
            }

            let yaw = (town.x - bx).atan2(town.z - bz);
            let bpos = Vector3::new(bx, by, bz);

            if i == 0 {
                Self::build_windmill(bpos, yaw, sun_dir, ambient, window_col, verts);
            } else if i == 3 {
                Self::build_watchtower(bpos, h, sun_dir, ambient, window_col, verts);
            } else {
                let scale = 0.92 + ((h & 0xF) as f32) * 0.015;
                Self::build_fantasy_cottage(
                    bpos, yaw, scale, h, sun_dir, ambient, window_col, verts,
                );
            }
        }

        // 5. Plaza Street Lanterns
        for i in 0..4 {
            let a = (i as f32 / 4.0) * 2.0 * PI + 0.5;
            let lx = town.x + a.cos() * 15.5;
            let lz = town.z + a.sin() * 15.5;
            let (ly, _) = terrain.get_mesh_height(config, lx, lz);
            Self::build_lantern_post(
                Vector3::new(lx, ly, lz),
                sun_dir,
                ambient,
                window_col,
                verts,
            );
        }
    }

    fn is_angle_near_road(angle: f32, road_angles: &[f32; 4], threshold: f32) -> bool {
        for &ra in road_angles {
            if ra == f32::MAX {
                continue;
            }
            let mut diff = (angle - ra).rem_euclid(2.0 * PI);
            if diff > PI {
                diff = 2.0 * PI - diff;
            }
            if diff < threshold {
                return true;
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn build_road_and_bridges(
        a: &TownSite,
        b: &TownSite,
        terrain: &LowPolyTerrain,
        config: &WorldConfig,
        cull_x: f32,
        cull_z: f32,
        max_road_extent: f32,
        sun_dir: Vector3,
        ambient: f32,
        window_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        let total_dist = ((b.x - a.x).powi(2) + (b.z - a.z).powi(2)).sqrt();
        let steps = (total_dist / 14.0).ceil() as i32;
        if steps < 4 {
            return;
        }

        let stone_col = Color { r: 148, g: 142, b: 132, a: 255 };
        let cobble_alt = Color { r: 132, g: 124, b: 114, a: 255 };
        let wood_deck = Color { r: 128, g: 88, b: 52, a: 255 };
        let wood_beam = Color { r: 92, g: 60, b: 36, a: 255 };

        for s in 2..(steps - 2) {
            let t0 = (s as f32) / (steps as f32);
            let t1 = ((s + 1) as f32) / (steps as f32);
            let (x0, z0, tx0, tz0) = sample_road_curve(a, b, t0);
            let (x1, z1, tx1, tz1) = sample_road_curve(a, b, t1);

            if (x0 - cull_x).abs() > max_road_extent
                || (z0 - cull_z).abs() > max_road_extent
                || (x1 - cull_x).abs() > max_road_extent
                || (z1 - cull_z).abs() > max_road_extent
            {
                continue;
            }

            let (y0, _) = terrain.get_mesh_height(config, x0, z0);
            let (y1, _) = terrain.get_mesh_height(config, x1, z1);

            let water_y = config.water_level;
            let is_bridge = y0 < water_y + 1.35 || y1 < water_y + 1.35;

            if is_bridge {
                // Wooden Bridge Span across narrow river/cove
                let deck_y0 = (y0 + 0.35).max(water_y + 2.4);
                let deck_y1 = (y1 + 0.35).max(water_y + 2.4);
                let half_w = 3.6;

                let nx0 = -tz0 * half_w;
                let nz0 = tx0 * half_w;
                let nx1 = -tz1 * half_w;
                let nz1 = tx1 * half_w;

                let p0_l = Vector3::new(x0 - nx0, deck_y0, z0 - nz0);
                let p0_r = Vector3::new(x0 + nx0, deck_y0, z0 + nz0);
                let p1_l = Vector3::new(x1 - nx1, deck_y1, z1 - nz1);
                let p1_r = Vector3::new(x1 + nx1, deck_y1, z1 + nz1);

                Self::emit_quad(
                    p0_l,
                    p0_r,
                    p1_r,
                    p1_l,
                    Vector3::new(0.0, 1.0, 0.0),
                    wood_deck,
                    sun_dir,
                    ambient,
                    verts,
                );

                let rail_h = 1.3;
                Self::emit_double_sided_quad(
                    p0_l + Vector3::new(0.0, 0.65, 0.0),
                    p1_l + Vector3::new(0.0, 0.65, 0.0),
                    p1_l + Vector3::new(0.0, rail_h, 0.0),
                    p0_l + Vector3::new(0.0, rail_h, 0.0),
                    wood_beam,
                    sun_dir,
                    ambient,
                    verts,
                );
                Self::emit_double_sided_quad(
                    p0_r + Vector3::new(0.0, 0.65, 0.0),
                    p1_r + Vector3::new(0.0, 0.65, 0.0),
                    p1_r + Vector3::new(0.0, rail_h, 0.0),
                    p0_r + Vector3::new(0.0, rail_h, 0.0),
                    wood_beam,
                    sun_dir,
                    ambient,
                    verts,
                );

                if s % 2 == 0 {
                    let piling_base = y0.min(water_y - 2.5);
                    let piling_h = (deck_y0 + rail_h) - piling_base;
                    Self::append_box(
                        Vector3::new(x0 - nx0, piling_base, z0 - nz0),
                        0.45,
                        piling_h,
                        0.45,
                        0.0,
                        wood_beam,
                        sun_dir,
                        ambient,
                        verts,
                    );
                    Self::append_box(
                        Vector3::new(x0 + nx0, piling_base, z0 + nz0),
                        0.45,
                        piling_h,
                        0.45,
                        0.0,
                        wood_beam,
                        sun_dir,
                        ambient,
                        verts,
                    );
                }
            } else {
                // Land Road: 3D Cobblestone Paving Stones & Roadside Lanterns
                let col = if s % 2 == 0 { stone_col } else { cobble_alt };

                let half_w = 2.6;
                let nx0 = -tz0 * half_w;
                let nz0 = tx0 * half_w;
                let nx1 = -tz1 * half_w;
                let nz1 = tx1 * half_w;

                let lift = 0.18;
                let p0_l = Vector3::new(x0 - nx0, y0 + lift, z0 - nz0);
                let p0_r = Vector3::new(x0 + nx0, y0 + lift, z0 + nz0);
                let p1_l = Vector3::new(x1 - nx1, y1 + lift, z1 - nz1);
                let p1_r = Vector3::new(x1 + nx1, y1 + lift, z1 + nz1);

                Self::emit_quad(
                    p0_l,
                    p0_r,
                    p1_r,
                    p1_l,
                    Vector3::new(0.0, 1.0, 0.0),
                    col,
                    sun_dir,
                    ambient,
                    verts,
                );

                // Periodic roadside lantern posts every 8 segments
                if s % 8 == 0 {
                    let mid_x = (x0 + x1) * 0.5;
                    let mid_z = (z0 + z1) * 0.5;
                    let side_sign = if (s / 8) % 2 == 0 { 1.0 } else { -1.0 };
                    let lx = mid_x + (-tz0) * 5.0 * side_sign;
                    let lz = mid_z + tx0 * 5.0 * side_sign;
                    let (ly, _) = terrain.get_mesh_height(config, lx, lz);
                    if ly > water_y + 1.5 {
                        Self::build_lantern_post(
                            Vector3::new(lx, ly, lz),
                            sun_dir,
                            ambient,
                            window_col,
                            verts,
                        );
                    }
                }
            }
        }
    }

    /// Builds a timber-framed medieval fantasy cottage with stone foundation, pitched roof, chimney, and glowing windows.
    #[allow(clippy::too_many_arguments)]
    fn build_fantasy_cottage(
        pos: Vector3,
        yaw: f32,
        scale: f32,
        hash: u32,
        sun_dir: Vector3,
        ambient: f32,
        window_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        let plinth_col = Color { r: 112, g: 110, b: 116, a: 255 };
        let wall_col = Color { r: 234, g: 222, b: 196, a: 255 };
        let timber_col = Color { r: 82, g: 52, b: 34, a: 255 };
        let roof_col = match hash & 3 {
            0 => Color { r: 178, g: 68, b: 48, a: 255 },  // Warm terracotta red
            1 => Color { r: 64, g: 92, b: 118, a: 255 },  // Medieval slate blue
            2 => Color { r: 78, g: 104, b: 72, a: 255 },  // Mossy forest shingle
            _ => Color { r: 142, g: 82, b: 46, a: 255 },  // Golden-brown thatch/wood
        };

        let w = 6.2 * scale;
        let d = 7.6 * scale;
        let wall_h = 4.2 * scale;
        let roof_h = 3.4 * scale;

        // 1. Stone foundation plinth extending into sloped ground
        let plinth_base = pos + Vector3::new(0.0, -2.0, 0.0);
        Self::append_box(
            plinth_base,
            w + 0.6 * scale,
            2.7,
            d + 0.6 * scale,
            yaw,
            plinth_col,
            sun_dir,
            ambient,
            verts,
        );

        let wall_base = pos + Vector3::new(0.0, 0.7, 0.0);

        // 2. Main plaster walls
        Self::append_box(
            wall_base,
            w,
            wall_h,
            d,
            yaw,
            wall_col,
            sun_dir,
            ambient,
            verts,
        );

        // 3. Timber frame band around the cottage
        Self::append_box(
            wall_base + Vector3::new(0.0, wall_h * 0.88, 0.0),
            w + 0.22,
            wall_h * 0.14,
            d + 0.22,
            yaw,
            timber_col,
            sun_dir,
            ambient,
            verts,
        );

        // 4. Overhanging Pitched Gable Roof
        let roof_base = wall_base + Vector3::new(0.0, wall_h, 0.0);
        Self::append_gable_roof(
            roof_base,
            w + 1.1 * scale,
            roof_h,
            d + 1.1 * scale,
            yaw,
            roof_col,
            wall_col,
            sun_dir,
            ambient,
            verts,
        );

        // 5. Stone Chimney on the side of the roof
        let (r_vec, _, f_vec) = Self::basis(yaw);
        let chimney_pos = wall_base + r_vec * (w * 0.34) - f_vec * (d * 0.2);
        Self::append_box(
            chimney_pos,
            1.1 * scale,
            wall_h + roof_h * 0.95,
            1.1 * scale,
            yaw,
            plinth_col,
            sun_dir,
            ambient,
            verts,
        );

        // 6. Front Wooden Door & Glowing Windows
        let door_pos = wall_base + f_vec * (d * 0.5 + 0.08);
        Self::append_box(
            door_pos,
            1.3 * scale,
            2.4 * scale,
            0.22,
            yaw,
            timber_col,
            sun_dir,
            ambient,
            verts,
        );

        let win_y = wall_base.y + wall_h * 0.45;
        let win_l = Vector3::new(pos.x, win_y, pos.z) + f_vec * (d * 0.5 + 0.08) + r_vec * (w * 0.28);
        let win_r = Vector3::new(pos.x, win_y, pos.z) + f_vec * (d * 0.5 + 0.08) - r_vec * (w * 0.28);
        Self::append_box(win_l, 0.95 * scale, 1.05 * scale, 0.2, yaw, window_col, sun_dir, 0.92, verts);
        Self::append_box(win_r, 0.95 * scale, 1.05 * scale, 0.2, yaw, window_col, sun_dir, 0.92, verts);
    }

    /// Builds a two-story medieval tavern / inn with a jettied upper floor.
    #[allow(clippy::too_many_arguments)]
    fn build_medieval_tavern(
        pos: Vector3,
        yaw: f32,
        hash: u32,
        sun_dir: Vector3,
        ambient: f32,
        window_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        let stone_col = Color { r: 120, g: 116, b: 122, a: 255 };
        let plaster_col = Color { r: 238, g: 224, b: 192, a: 255 };
        let timber_col = Color { r: 76, g: 48, b: 30, a: 255 };
        let roof_col = if (hash & 1) == 0 {
            Color { r: 164, g: 56, b: 42, a: 255 }
        } else {
            Color { r: 56, g: 82, b: 108, a: 255 }
        };

        Self::append_box(
            pos + Vector3::new(0.0, -2.0, 0.0),
            9.2,
            6.0,
            10.6,
            yaw,
            stone_col,
            sun_dir,
            ambient,
            verts,
        );

        let upper_base = pos + Vector3::new(0.0, 4.0, 0.0);
        Self::append_box(
            upper_base,
            9.8,
            4.2,
            11.2,
            yaw,
            plaster_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            upper_base,
            10.0,
            0.45,
            11.4,
            yaw,
            timber_col,
            sun_dir,
            ambient,
            verts,
        );

        Self::append_gable_roof(
            upper_base + Vector3::new(0.0, 4.2, 0.0),
            10.8,
            4.6,
            12.2,
            yaw,
            roof_col,
            plaster_col,
            sun_dir,
            ambient,
            verts,
        );

        let (r_vec, _, f_vec) = Self::basis(yaw);
        for side in [-1.0_f32, 1.0_f32] {
            let wpos = upper_base + Vector3::new(0.0, 1.5, 0.0) + f_vec * 5.65 + r_vec * (2.6 * side);
            Self::append_box(wpos, 1.3, 1.4, 0.25, yaw, window_col, sun_dir, 0.95, verts);
        }
    }

    /// Builds a central medieval Guildhall & Keep with a stone tower spire.
    #[allow(clippy::too_many_arguments)]
    fn build_guildhall_keep(
        pos: Vector3,
        yaw: f32,
        sun_dir: Vector3,
        ambient: f32,
        window_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        let stone_col = Color { r: 132, g: 128, b: 136, a: 255 };
        let plaster_col = Color { r: 236, g: 226, b: 204, a: 255 };
        let roof_col = Color { r: 52, g: 82, b: 122, a: 255 };
        let spire_col = Color { r: 172, g: 56, b: 46, a: 255 };

        Self::append_box(
            pos + Vector3::new(0.0, -2.0, 0.0),
            11.5,
            7.2,
            14.5,
            yaw,
            stone_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            pos + Vector3::new(0.0, 5.2, 0.0),
            12.0,
            4.2,
            15.0,
            yaw,
            plaster_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_gable_roof(
            pos + Vector3::new(0.0, 9.4, 0.0),
            13.2,
            5.4,
            16.2,
            yaw,
            roof_col,
            plaster_col,
            sun_dir,
            ambient,
            verts,
        );

        let (r_vec, _, f_vec) = Self::basis(yaw);
        let tower_pos = pos + r_vec * 5.8 - f_vec * 5.5;
        Self::append_box(
            tower_pos + Vector3::new(0.0, -2.0, 0.0),
            5.8,
            17.5,
            5.8,
            yaw,
            stone_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_pyramid_roof(
            tower_pos + Vector3::new(0.0, 15.5, 0.0),
            6.8,
            6.2,
            yaw,
            spire_col,
            sun_dir,
            ambient,
            verts,
        );

        for offset in [-3.2_f32, 0.0, 3.2] {
            let wpos = pos + Vector3::new(0.0, 6.2, 0.0) + f_vec * 7.55 + r_vec * offset;
            Self::append_box(wpos, 1.2, 2.2, 0.25, yaw, window_col, sun_dir, 0.95, verts);
        }
    }

    /// Builds a medieval town square well, bell-tower, and colorful market stalls.
    #[allow(clippy::too_many_arguments)]
    fn build_town_well_and_market(
        pos: Vector3,
        hash: u32,
        sun_dir: Vector3,
        ambient: f32,
        window_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        Self::build_watchtower(
            pos + Vector3::new(5.5, 0.0, -4.5),
            hash,
            sun_dir,
            ambient,
            window_col,
            verts,
        );

        let stone_col = Color { r: 136, g: 134, b: 140, a: 255 };
        let wood_col = Color { r: 102, g: 66, b: 40, a: 255 };
        let roof_col = Color { r: 58, g: 92, b: 128, a: 255 };

        Self::append_box(
            pos + Vector3::new(0.0, -1.0, 0.0),
            3.2,
            2.2,
            3.2,
            0.0,
            stone_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            pos + Vector3::new(-1.2, 1.2, 0.0),
            0.35,
            2.4,
            0.35,
            0.0,
            wood_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            pos + Vector3::new(1.2, 1.2, 0.0),
            0.35,
            2.4,
            0.35,
            0.0,
            wood_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_pyramid_roof(
            pos + Vector3::new(0.0, 3.5, 0.0),
            3.8,
            2.2,
            0.0,
            roof_col,
            sun_dir,
            ambient,
            verts,
        );

        let awning_cols = [
            Color { r: 196, g: 58, b: 48, a: 255 },
            Color { r: 218, g: 162, b: 48, a: 255 },
        ];
        for (i, &acol) in awning_cols.iter().enumerate() {
            let spos = pos + Vector3::new(-6.0 + (i as f32) * 5.0, 0.0, 5.5);
            Self::append_box(spos, 3.6, 1.3, 2.2, 0.3, wood_col, sun_dir, ambient, verts);
            Self::append_gable_roof(
                spos + Vector3::new(0.0, 2.6, 0.0),
                4.0,
                1.2,
                2.6,
                0.3,
                acol,
                acol,
                sun_dir,
                ambient,
                verts,
            );
        }
    }

    /// Builds a medieval stone watchtower with a parapet and pointed roof.
    fn build_watchtower(
        pos: Vector3,
        hash: u32,
        sun_dir: Vector3,
        ambient: f32,
        window_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        let stone_col = Color { r: 126, g: 124, b: 132, a: 255 };
        let parapet_col = Color { r: 144, g: 142, b: 150, a: 255 };
        let spire_col = if (hash & 1) == 0 {
            Color { r: 168, g: 54, b: 44, a: 255 }
        } else {
            Color { r: 54, g: 84, b: 124, a: 255 }
        };

        Self::append_box(
            pos + Vector3::new(0.0, -2.5, 0.0),
            5.0,
            14.0,
            5.0,
            0.0,
            stone_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            pos + Vector3::new(0.0, 11.2, 0.0),
            5.8,
            2.2,
            5.8,
            0.0,
            parapet_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_pyramid_roof(
            pos + Vector3::new(0.0, 13.4, 0.0),
            6.4,
            5.5,
            0.0,
            spire_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            pos + Vector3::new(0.0, 8.8, 0.0),
            5.25,
            1.2,
            1.0,
            0.0,
            window_col,
            sun_dir,
            0.95,
            verts,
        );
        Self::append_box(
            pos + Vector3::new(0.0, 8.8, 0.0),
            1.0,
            1.2,
            5.25,
            0.0,
            window_col,
            sun_dir,
            0.95,
            verts,
        );
    }

    /// Builds a fantasy countryside windmill with 4 low-poly wooden sails.
    fn build_windmill(
        pos: Vector3,
        yaw: f32,
        sun_dir: Vector3,
        ambient: f32,
        window_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        let base_col = Color { r: 128, g: 124, b: 128, a: 255 };
        let body_col = Color { r: 232, g: 220, b: 194, a: 255 };
        let roof_col = Color { r: 138, g: 76, b: 44, a: 255 };
        let sail_col = Color { r: 242, g: 234, b: 212, a: 255 };

        Self::append_box(
            pos + Vector3::new(0.0, -2.0, 0.0),
            5.8,
            5.5,
            5.8,
            yaw,
            base_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            pos + Vector3::new(0.0, 3.5, 0.0),
            4.8,
            6.8,
            4.8,
            yaw,
            body_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_pyramid_roof(
            pos + Vector3::new(0.0, 10.3, 0.0),
            5.4,
            3.8,
            yaw,
            roof_col,
            sun_dir,
            ambient,
            verts,
        );

        let (r_vec, u_vec, f_vec) = Self::basis(yaw);
        let hub = pos + Vector3::new(0.0, 8.8, 0.0) + f_vec * 2.65;
        Self::append_box(hub - Vector3::new(0.0, 0.3, 0.0), 0.7, 0.7, 0.8, yaw, roof_col, sun_dir, ambient, verts);

        for i in 0..4 {
            let a = (i as f32) * (PI * 0.5) + 0.38;
            let dir = r_vec * a.cos() + u_vec * a.sin();
            let perp = -r_vec * a.sin() + u_vec * a.cos();

            let p0 = hub + dir * 0.6 - perp * 0.25;
            let p1 = hub + dir * 0.6 + perp * 0.25;
            let p2 = hub + dir * 6.4 + perp * 0.85;
            let p3 = hub + dir * 6.4 - perp * 0.35;

            Self::emit_double_sided_quad(p0, p1, p2, p3, sail_col, sun_dir, ambient, verts);
        }

        let wpos = pos + Vector3::new(0.0, 5.2, 0.0) + f_vec * 2.48;
        Self::append_box(wpos, 1.0, 1.2, 0.2, yaw, window_col, sun_dir, 0.95, verts);
    }

    /// Builds a wooden roadside/plaza lantern post with a warm glowing lamp.
    fn build_lantern_post(
        pos: Vector3,
        sun_dir: Vector3,
        ambient: f32,
        lantern_col: Color,
        verts: &mut Vec<Vertex3D>,
    ) {
        let wood_col = Color { r: 86, g: 56, b: 36, a: 255 };
        Self::append_box(
            pos + Vector3::new(0.0, -0.5, 0.0),
            0.36,
            4.2,
            0.36,
            0.0,
            wood_col,
            sun_dir,
            ambient,
            verts,
        );
        Self::append_box(
            pos + Vector3::new(0.0, 3.5, 0.0),
            0.78,
            0.78,
            0.78,
            0.0,
            lantern_col,
            sun_dir,
            0.98,
            verts,
        );
    }

    #[inline]
    fn basis(yaw: f32) -> (Vector3, Vector3, Vector3) {
        let c = yaw.cos();
        let s = yaw.sin();
        let right = Vector3::new(c, 0.0, -s);
        let up = Vector3::new(0.0, 1.0, 0.0);
        let forward = Vector3::new(s, 0.0, c);
        (right, up, forward)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_box(
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

        Self::emit_quad(b_fl, b_fr, t_fr, t_fl, f, color, sun_dir, ambient, verts);
        Self::emit_quad(b_br, b_bl, t_bl, t_br, -f, color, sun_dir, ambient, verts);
        Self::emit_quad(b_fr, b_br, t_br, t_fr, r, color, sun_dir, ambient, verts);
        Self::emit_quad(b_bl, b_fl, t_fl, t_bl, -r, color, sun_dir, ambient, verts);
        Self::emit_quad(t_fl, t_fr, t_br, t_bl, u, color, sun_dir, ambient, verts);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_gable_roof(
        base_center: Vector3,
        width: f32,
        height: f32,
        depth: f32,
        yaw: f32,
        roof_color: Color,
        gable_color: Color,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let (r, u, f) = Self::basis(yaw);
        let hw = r * (width * 0.5);
        let hd = f * (depth * 0.5);
        let ridge_h = u * height;

        let b_fl = base_center - hw + hd;
        let b_fr = base_center + hw + hd;
        let b_bl = base_center - hw - hd;
        let b_br = base_center + hw - hd;

        let top_f = base_center + hd + ridge_h;
        let top_b = base_center - hd + ridge_h;

        Self::emit_quad(
            b_bl,
            b_fl,
            top_f,
            top_b,
            -r + u,
            roof_color,
            sun_dir,
            ambient,
            verts,
        );
        Self::emit_quad(
            b_fr,
            b_br,
            top_b,
            top_f,
            r + u,
            roof_color,
            sun_dir,
            ambient,
            verts,
        );

        Self::emit_tri(b_fl, b_fr, top_f, f, gable_color, sun_dir, ambient, verts);
        Self::emit_tri(b_br, b_bl, top_b, -f, gable_color, sun_dir, ambient, verts);

        Self::emit_quad(
            b_fl,
            b_bl,
            b_br,
            b_fr,
            -u,
            roof_color,
            sun_dir,
            ambient * 0.8,
            verts,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_pyramid_roof(
        base_center: Vector3,
        width: f32,
        height: f32,
        yaw: f32,
        color: Color,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let (r, u, f) = Self::basis(yaw);
        let hw = r * (width * 0.5);
        let hd = f * (width * 0.5);
        let apex = base_center + u * height;

        let b_fl = base_center - hw + hd;
        let b_fr = base_center + hw + hd;
        let b_bl = base_center - hw - hd;
        let b_br = base_center + hw - hd;

        Self::emit_tri(b_fl, b_fr, apex, f + u * 0.5, color, sun_dir, ambient, verts);
        Self::emit_tri(b_fr, b_br, apex, r + u * 0.5, color, sun_dir, ambient, verts);
        Self::emit_tri(b_br, b_bl, apex, -f + u * 0.5, color, sun_dir, ambient, verts);
        Self::emit_tri(b_bl, b_fl, apex, -r + u * 0.5, color, sun_dir, ambient, verts);
        Self::emit_quad(b_fl, b_bl, b_br, b_fr, -u, color, sun_dir, ambient * 0.8, verts);
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
        let mut n = (p1 - p0).cross(p2 - p0).normalize();
        let (v1, v2) = if n.dot(outward_dir) < 0.0 {
            n = -n;
            (p2, p1)
        } else {
            (p1, p2)
        };

        let lit = ambient + (n.dot(sun_dir)).max(0.0) * (1.0 - ambient);
        let c = Color {
            r: (color.r as f32 * lit).clamp(0.0, 255.0) as u8,
            g: (color.g as f32 * lit).clamp(0.0, 255.0) as u8,
            b: (color.b as f32 * lit).clamp(0.0, 255.0) as u8,
            a: 255,
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

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn emit_double_sided_quad(
        p0: Vector3,
        p1: Vector3,
        p2: Vector3,
        p3: Vector3,
        color: Color,
        sun_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let n = (p1 - p0).cross(p2 - p0).normalize();
        Self::emit_quad(p0, p1, p2, p3, n, color, sun_dir, ambient, verts);
        Self::emit_quad(p0, p3, p2, p1, -n, color, sun_dir, ambient * 0.85, verts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lppw_world::world::WorldPreset;

    #[test]
    fn test_starter_town_is_valid_and_above_water() {
        let config = WorldConfig::new(4242, WorldPreset::Archipelago);
        let town = compute_town_for_cell(&config, 0, 0);
        assert!(town.valid);
        assert!(town.y > config.water_level + 4.0);
    }

    #[test]
    fn test_road_meander_is_zero_at_endpoints() {
        let m0 = road_meander_offset(12345, 67890, 0.0);
        let m1 = road_meander_offset(12345, 67890, 1.0);
        assert!(m0.abs() < 1e-5);
        assert!(m1.abs() < 1e-5);
    }

    #[test]
    fn test_box_and_gable_normals_point_outward() {
        let mut verts = Vec::new();
        let center = Vector3::new(0.0, 0.0, 0.0);
        let sun = Vector3::new(0.0, 1.0, 0.0);
        let col = Color { r: 200, g: 200, b: 200, a: 255 };
        TownManager::append_box(center, 6.0, 4.0, 8.0, 0.5, col, sun, 0.3, &mut verts);
        TownManager::append_gable_roof(
            Vector3::new(0.0, 4.0, 0.0),
            7.0,
            3.0,
            9.0,
            0.5,
            col,
            col,
            sun,
            0.3,
            &mut verts,
        );

        let centroid = Vector3::new(0.0, 2.5, 0.0);
        for chunk in verts.as_chunks::<3>().0 {
            let p0 = chunk[0].position;
            let p1 = chunk[1].position;
            let p2 = chunk[2].position;
            let face_center = (p0 + p1 + p2) * (1.0 / 3.0);
            let n = (p1 - p0).cross(p2 - p0).normalize();
            assert!((n - chunk[0].normal.unwrap()).length() < 0.01);
            if n.y > -0.95 {
                assert!(
                    n.dot(face_center - centroid) > 0.0,
                    "Face normal {:?} at {:?} points inward!",
                    n,
                    face_center
                );
            }
        }
    }
}
