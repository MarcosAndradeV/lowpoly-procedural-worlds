use crate::biomes::BiomeType;
use crate::terrain::LowPolyTerrain;
use crate::world::WorldConfig;
use muxapp::muxui::{d3::*, *};
use std::f32::consts::PI;

/// Low-poly vegetation and landscape decoration models.
pub struct PropGenerator;

impl PropGenerator {
    /// Generates a low-poly conifer / pine tree mesh.
    pub fn build_pine_tree(
        pos: Vector3,
        scale: f32,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let trunk_col = Color { r: 88, g: 54, b: 34, a: 255 };
        let needle_col = Color { r: 34, g: 92, b: 46, a: 255 };

        let trunk_radius = 0.45 * scale;
        let trunk_height = 2.5 * scale;
        let sides = 5;

        // Trunk (5-sided cylinder)
        Self::append_cylinder(
            pos,
            trunk_radius,
            trunk_radius * 0.8,
            trunk_height,
            sides,
            trunk_col,
            light_dir,
            ambient,
            verts,
        );

        // Tiered Conical Foliage (3 stacked faceted cones)
        let tiers = 3;
        for t in 0..tiers {
            let base_y = pos.y + (1.8 + t as f32 * 1.6) * scale;
            let cone_h = (3.2 - t as f32 * 0.4) * scale;
            let radius = (2.4 - t as f32 * 0.55) * scale;

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

    /// Generates a low-poly deciduous / oak tree mesh.
    pub fn build_deciduous_tree(
        pos: Vector3,
        scale: f32,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let trunk_col = Color { r: 96, g: 62, b: 40, a: 255 };
        let leaf_col = Color { r: 52, g: 138, b: 54, a: 255 };

        let trunk_radius = 0.5 * scale;
        let trunk_height = 3.0 * scale;
        let sides = 5;

        Self::append_cylinder(
            pos,
            trunk_radius,
            trunk_radius * 0.7,
            trunk_height,
            sides,
            trunk_col,
            light_dir,
            ambient,
            verts,
        );

        // Foliage: faceted dual-cone canopy
        let canopy_y = pos.y + trunk_height * 0.8;
        let radius = 2.8 * scale;
        let canopy_h = 3.5 * scale;

        Self::append_cone(
            Vector3::new(pos.x, canopy_y, pos.z),
            radius,
            canopy_h,
            sides + 1,
            leaf_col,
            light_dir,
            ambient,
            verts,
        );
    }

    /// Generates a coastal palm tree mesh.
    pub fn build_palm_tree(
        pos: Vector3,
        scale: f32,
        light_dir: Vector3,
        ambient: f32,
        verts: &mut Vec<Vertex3D>,
    ) {
        let trunk_col = Color { r: 115, g: 82, b: 50, a: 255 };
        let frond_col = Color { r: 48, g: 142, b: 42, a: 255 };

        let trunk_h = 4.5 * scale;
        let sides = 4;

        // Slanted trunk
        Self::append_cylinder(
            pos,
            0.4 * scale,
            0.25 * scale,
            trunk_h,
            sides,
            trunk_col,
            light_dir,
            ambient,
            verts,
        );

        // Palm fronds arching outward
        let top = Vector3::new(pos.x, pos.y + trunk_h, pos.z);
        let num_fronds = 6;
        for i in 0..num_fronds {
            let angle = (i as f32 / num_fronds as f32) * 2.0 * PI;
            let dir_x = angle.cos();
            let dir_z = angle.sin();

            let p0 = top;
            let p1 = top + Vector3::new(dir_x * 2.2 * scale, -0.6 * scale, dir_z * 2.2 * scale);
            let p2 = top + Vector3::new(dir_x * 3.4 * scale, -1.8 * scale, dir_z * 3.4 * scale);

            let n = Vector3::new(0.0, 1.0, 0.0);
            let lit = ambient + (n.dot(light_dir)).max(0.0) * (1.0 - ambient);
            let c = Color {
                r: (frond_col.r as f32 * lit) as u8,
                g: (frond_col.g as f32 * lit) as u8,
                b: (frond_col.b as f32 * lit) as u8,
                a: 255,
            };

            verts.push(Vertex3D::new(p0, c).with_normal(n));
            verts.push(Vertex3D::new(p1, c).with_normal(n));
            verts.push(Vertex3D::new(p2, c).with_normal(n));
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
        let rock_col = Color { r: 110, g: 108, b: 112, a: 255 };
        let sides = 6;
        let radius = 1.6 * scale;
        let height = 1.4 * scale;

        Self::append_cone(
            pos,
            radius,
            height,
            sides,
            rock_col,
            light_dir,
            ambient,
            verts,
        );
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
        for i in 0..sides {
            let a0 = (i as f32 / sides as f32) * 2.0 * PI;
            let a1 = ((i + 1) as f32 / sides as f32) * 2.0 * PI;

            let b0 = base + Vector3::new(a0.cos() * r_bottom, 0.0, a0.sin() * r_bottom);
            let b1 = base + Vector3::new(a1.cos() * r_bottom, 0.0, a1.sin() * r_bottom);
            let t0 = top + Vector3::new(a0.cos() * r_top, 0.0, a0.sin() * r_top);
            let t1 = top + Vector3::new(a1.cos() * r_top, 0.0, a1.sin() * r_top);

            // Quad split into 2 triangles
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
        for i in 0..sides {
            let a0 = (i as f32 / sides as f32) * 2.0 * PI;
            let a1 = ((i + 1) as f32 / sides as f32) * 2.0 * PI;

            let b0 = base + Vector3::new(a0.cos() * radius, 0.0, a0.sin() * radius);
            let b1 = base + Vector3::new(a1.cos() * radius, 0.0, a1.sin() * radius);

            let e1 = b1 - b0;
            let e2 = apex - b0;
            let n = e1.cross(e2).normalize();
            let lit = ambient + (n.dot(light_dir)).max(0.0) * (1.0 - ambient);
            let c = Color {
                r: (color.r as f32 * lit) as u8,
                g: (color.g as f32 * lit) as u8,
                b: (color.b as f32 * lit) as u8,
                a: 255,
            };

            verts.push(Vertex3D::new(b0, c).with_normal(n));
            verts.push(Vertex3D::new(b1, c).with_normal(n));
            verts.push(Vertex3D::new(apex, c).with_normal(n));
        }
    }
}

use std::cell::RefCell;

/// Spatially manages and batches procedural props across the landscape.
pub struct PropManager {
    pub enabled: bool,
    pub cell_size: f32,
    pub radius: i32,
    vertex_buffer: RefCell<Vec<Vertex3D>>,
}

impl Default for PropManager {
    fn default() -> Self {
        Self {
            enabled: true,
            cell_size: 28.0,
            radius: 12,
            vertex_buffer: RefCell::new(Vec::with_capacity(8192)),
        }
    }
}

impl PropManager {
    /// Renders all procedural props in view around `(center_x, center_z)`.
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
    ) {
        if !self.enabled {
            return;
        }

        let snap_x = (center_x / self.cell_size).floor() as i32;
        let snap_z = (center_z / self.cell_size).floor() as i32;

        let mut prop_vertices = self.vertex_buffer.borrow_mut();
        prop_vertices.clear();

        for dx in -self.radius..=self.radius {
            for dz in -self.radius..=self.radius {
                let cell_x = snap_x + dx;
                let cell_z = snap_z + dz;

                // Deterministic pseudo-random hash for this cell
                let h = Self::hash2d(cell_x, cell_z, config.seed);

                // Check spawn probability (approx 45% of cells spawn a prop)
                if (h & 0xFF) > 115 {
                    continue;
                }

                let sub_x = ((h >> 8) & 0xFF) as f32 * 0.003921569;
                let sub_z = ((h >> 16) & 0xFF) as f32 * 0.003921569;

                let wx = (cell_x as f32 + sub_x) * self.cell_size;
                let wz = (cell_z as f32 + sub_z) * self.cell_size;

                let wy = terrain.get_height(config, wx, wz);

                // Do not spawn underwater or in deep ice peaks
                if wy < config.water_level + 0.8 || wy > config.water_level + 85.0 {
                    continue;
                }

                let normal = terrain.get_normal(config, wx, wz);
                // Props don't grow on steep cliffs
                if normal.y < 0.72 {
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
                let scale = 0.85 + (((h >> 24) & 0x7F) as f32 * 0.0078125) * 0.5;
                let pos = Vector3::new(wx, wy, wz);

                match biome {
                    BiomeType::Beach => {
                        if (h & 3) == 0 {
                            PropGenerator::build_palm_tree(pos, scale * 1.1, sun_dir, ambient, &mut prop_vertices);
                        } else {
                            PropGenerator::build_rock(pos, scale * 0.8, sun_dir, ambient, &mut prop_vertices);
                        }
                    }
                    BiomeType::Plains => {
                        if (h & 1) == 0 {
                            PropGenerator::build_deciduous_tree(pos, scale, sun_dir, ambient, &mut prop_vertices);
                        } else {
                            PropGenerator::build_rock(pos, scale * 0.7, sun_dir, ambient, &mut prop_vertices);
                        }
                    }
                    BiomeType::Forest => {
                        if (h & 3) < 3 {
                            PropGenerator::build_pine_tree(pos, scale * 1.15, sun_dir, ambient, &mut prop_vertices);
                        } else {
                            PropGenerator::build_deciduous_tree(pos, scale, sun_dir, ambient, &mut prop_vertices);
                        }
                    }
                    BiomeType::Highland => {
                        if (h & 3) == 0 {
                            PropGenerator::build_pine_tree(pos, scale * 0.85, sun_dir, ambient, &mut prop_vertices);
                        } else {
                            PropGenerator::build_rock(pos, scale * 1.1, sun_dir, ambient, &mut prop_vertices);
                        }
                    }
                    BiomeType::Desert | BiomeType::Canyon | BiomeType::MountainRock | BiomeType::VolcanicBasalt => {
                        PropGenerator::build_rock(pos, scale * 1.25, sun_dir, ambient, &mut prop_vertices);
                    }
                    _ => {}
                }
            }
        }

        d3.draw_trimesh_buffered(&prop_vertices);
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
