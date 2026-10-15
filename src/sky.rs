use muxapp::muxui::{d3::*, *};
use std::f32::consts::PI;

/// Time of day lighting preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Noon,
    Sunset,
    Night,
}

impl TimeOfDay {
    pub fn next(&self) -> Self {
        match self {
            TimeOfDay::Noon => TimeOfDay::Sunset,
            TimeOfDay::Sunset => TimeOfDay::Night,
            TimeOfDay::Night => TimeOfDay::Noon,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TimeOfDay::Noon => "Midday Sun",
            TimeOfDay::Sunset => "Golden Sunset",
            TimeOfDay::Night => "Starlit Night",
        }
    }

    /// Primary directional light vector (pointing FROM light TO origin or vice versa for normal dot).
    pub fn sun_dir(&self) -> Vector3 {
        match self {
            TimeOfDay::Noon => Vector3::new(0.45, 0.85, 0.3).normalize(),
            TimeOfDay::Sunset => Vector3::new(0.85, 0.25, 0.45).normalize(),
            TimeOfDay::Night => Vector3::new(0.2, 0.9, 0.35).normalize(), // Moon
        }
    }

    pub fn ambient_light(&self) -> f32 {
        match self {
            TimeOfDay::Noon => 0.38,
            TimeOfDay::Sunset => 0.28,
            TimeOfDay::Night => 0.16,
        }
    }

    pub fn sky_colors(&self) -> (Color, Color) {
        match self {
            TimeOfDay::Noon => (
                Color { r: 145, g: 195, b: 245, a: 210 }, // Horizon
                Color { r: 45, g: 110, b: 210, a: 0 },    // Zenith
            ),
            TimeOfDay::Sunset => (
                Color { r: 235, g: 130, b: 75, a: 220 },  // Golden orange horizon
                Color { r: 70, g: 35, b: 115, a: 0 },     // Purple twilight zenith
            ),
            TimeOfDay::Night => (
                Color { r: 18, g: 25, b: 48, a: 220 },    // Deep midnight horizon
                Color { r: 6, g: 10, b: 24, a: 0 },       // Inky zenith
            ),
        }
    }
}

/// Atmospheric haze dome and stylized floating low-poly clouds.
pub struct SkyDome {
    pub time_of_day: TimeOfDay,
    cloud_offset: f32,
}

impl Default for SkyDome {
    fn default() -> Self {
        Self {
            time_of_day: TimeOfDay::Noon,
            cloud_offset: 0.0,
        }
    }
}

impl SkyDome {
    pub fn update(&mut self, dt: f32) {
        self.cloud_offset += dt * 3.5;
    }

    pub fn cycle_time(&mut self) {
        self.time_of_day = self.time_of_day.next();
    }

    /// Renders the atmospheric horizon dome.
    pub fn draw(&self, d3: &Render3DContext, center_x: f32, center_z: f32) {
        let segments = 36;
        let radius = 1100.0;
        let height = 400.0;

        let (ground_color, sky_color) = self.time_of_day.sky_colors();
        let mut dome_vertices = Vec::with_capacity(segments * 6);

        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * 2.0 * PI;
            let a1 = ((i + 1) as f32 / segments as f32) * 2.0 * PI;

            let x0 = center_x + a0.cos() * radius;
            let z0 = center_z + a0.sin() * radius;
            let x1 = center_x + a1.cos() * radius;
            let z1 = center_z + a1.sin() * radius;

            dome_vertices.push(Vertex3D::new(Vector3::new(x0, -10.0, z0), ground_color));
            dome_vertices.push(Vertex3D::new(Vector3::new(x1, -10.0, z1), ground_color));
            dome_vertices.push(Vertex3D::new(Vector3::new(x0, height, z0), sky_color));

            dome_vertices.push(Vertex3D::new(Vector3::new(x1, -10.0, z1), ground_color));
            dome_vertices.push(Vertex3D::new(Vector3::new(x1, height, z1), sky_color));
            dome_vertices.push(Vertex3D::new(Vector3::new(x0, height, z0), sky_color));
        }

        d3.draw_trimesh_buffered(&dome_vertices);

        // Low-poly drifting clouds
        self.draw_clouds(d3, center_x, center_z);
    }

    fn draw_clouds(&self, d3: &Render3DContext, center_x: f32, center_z: f32) {
        let cloud_y = 135.0;
        let cloud_color = match self.time_of_day {
            TimeOfDay::Noon => Color { r: 250, g: 252, b: 255, a: 220 },
            TimeOfDay::Sunset => Color { r: 245, g: 195, b: 180, a: 220 },
            TimeOfDay::Night => Color { r: 65, g: 75, b: 95, a: 160 },
        };

        let cloud_spacing = 180.0;
        let snap_x = (center_x / cloud_spacing).floor() as i32;
        let snap_z = (center_z / cloud_spacing).floor() as i32;

        let radius = 3;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let cx = (snap_x + dx) as f32 * cloud_spacing + self.cloud_offset;
                let cz = (snap_z + dz) as f32 * cloud_spacing;

                // Deterministic variation
                let seed = ((snap_x + dx) * 73856093) ^ ((snap_z + dz) * 19349663);
                if (seed & 3) == 0 {
                    let w = 45.0 + ((seed >> 2) & 15) as f32 * 2.0;
                    let l = 35.0 + ((seed >> 6) & 15) as f32 * 2.0;
                    let h = 8.0;

                    d3.draw_cube(Vector3::new(cx, cloud_y, cz), w, h, l, cloud_color);
                    d3.draw_cube(Vector3::new(cx + 12.0, cloud_y + 4.0, cz - 5.0), w * 0.65, h * 1.2, l * 0.65, cloud_color);
                }
            }
        }
    }
}
