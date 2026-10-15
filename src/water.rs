use muxapp::muxui::{d3::*, *};

/// Low-poly animated water surface with faceted waves and sun specular highlights.
pub struct LowPolyWater {
    pub grid_size: i32,
    pub cell_size: f32,
    pub animated: bool,
    pub time: f32,
}

impl Default for LowPolyWater {
    fn default() -> Self {
        Self {
            grid_size: 42,
            cell_size: 32.0,
            animated: true,
            time: 0.0,
        }
    }
}

impl LowPolyWater {
    pub fn update(&mut self, dt: f32) {
        if self.animated {
            self.time += dt;
        }
    }

    /// Evaluates wave elevation at `(x, z)` at the current time.
    #[inline(always)]
    pub fn wave_height(&self, base_level: f32, x: f32, z: f32) -> f32 {
        if !self.animated {
            return base_level;
        }
        let t = self.time;
        let w1 = (x * 0.035 + t * 1.5).sin() * 0.45;
        let w2 = (z * 0.042 + t * 1.2).cos() * 0.35;
        let w3 = ((x + z) * 0.02 + t * 0.8).sin() * 0.25;
        base_level + w1 + w2 + w3
    }

    /// Draws the faceted water surface centered around the camera.
    pub fn draw(
        &self,
        d3: &Render3DContext,
        base_water_level: f32,
        center_x: f32,
        center_z: f32,
        sun_dir: Vector3,
        ambient: f32,
    ) {
        let snap_x = (center_x / self.cell_size).floor() * self.cell_size;
        let snap_z = (center_z / self.cell_size).floor() * self.cell_size;

        let half = self.grid_size / 2;
        let start_x = snap_x - (half as f32) * self.cell_size;
        let start_z = snap_z - (half as f32) * self.cell_size;

        let total_verts = (self.grid_size * self.grid_size * 6) as usize;
        let mut water_vertices = Vec::with_capacity(total_verts);

        let water_base_col = Color { r: 28, g: 110, b: 185, a: 220 };
        let specular_col = Color { r: 245, g: 250, b: 255, a: 240 };

        for i in 0..self.grid_size {
            let x0 = start_x + (i as f32) * self.cell_size;
            let x1 = x0 + self.cell_size;

            for j in 0..self.grid_size {
                let z0 = start_z + (j as f32) * self.cell_size;
                let z1 = z0 + self.cell_size;

                let y00 = self.wave_height(base_water_level, x0, z0);
                let y10 = self.wave_height(base_water_level, x1, z0);
                let y01 = self.wave_height(base_water_level, x0, z1);
                let y11 = self.wave_height(base_water_level, x1, z1);

                let p00 = Vector3::new(x0, y00, z0);
                let p10 = Vector3::new(x1, y10, z0);
                let p01 = Vector3::new(x0, y01, z1);
                let p11 = Vector3::new(x1, y11, z1);

                // --- Triangle 1: p00 -> p01 -> p10 ---
                let e1 = p01 - p00;
                let e2 = p10 - p00;
                let mut n1 = e1.cross(e2).normalize();
                if n1.y < 0.0 {
                    n1 = -n1;
                }

                let diff1 = ambient + (n1.dot(sun_dir)).max(0.0) * (1.0 - ambient);
                // Low-poly faceted specular glint
                let spec1 = (n1.dot(sun_dir)).max(0.0).powf(16.0) * 0.45;

                let c1 = Color {
                    r: ((water_base_col.r as f32 * diff1 + specular_col.r as f32 * spec1).clamp(0.0, 255.0)) as u8,
                    g: ((water_base_col.g as f32 * diff1 + specular_col.g as f32 * spec1).clamp(0.0, 255.0)) as u8,
                    b: ((water_base_col.b as f32 * diff1 + specular_col.b as f32 * spec1).clamp(0.0, 255.0)) as u8,
                    a: water_base_col.a,
                };

                water_vertices.push(Vertex3D::new(p00, c1).with_normal(n1));
                water_vertices.push(Vertex3D::new(p01, c1).with_normal(n1));
                water_vertices.push(Vertex3D::new(p10, c1).with_normal(n1));

                // --- Triangle 2: p10 -> p01 -> p11 ---
                let e3 = p01 - p10;
                let e4 = p11 - p10;
                let mut n2 = e3.cross(e4).normalize();
                if n2.y < 0.0 {
                    n2 = -n2;
                }

                let diff2 = ambient + (n2.dot(sun_dir)).max(0.0) * (1.0 - ambient);
                let spec2 = (n2.dot(sun_dir)).max(0.0).powf(16.0) * 0.45;

                let c2 = Color {
                    r: ((water_base_col.r as f32 * diff2 + specular_col.r as f32 * spec2).clamp(0.0, 255.0)) as u8,
                    g: ((water_base_col.g as f32 * diff2 + specular_col.g as f32 * spec2).clamp(0.0, 255.0)) as u8,
                    b: ((water_base_col.b as f32 * diff2 + specular_col.b as f32 * spec2).clamp(0.0, 255.0)) as u8,
                    a: water_base_col.a,
                };

                water_vertices.push(Vertex3D::new(p10, c2).with_normal(n2));
                water_vertices.push(Vertex3D::new(p01, c2).with_normal(n2));
                water_vertices.push(Vertex3D::new(p11, c2).with_normal(n2));
            }
        }

        d3.draw_trimesh_buffered(&water_vertices);
    }
}
