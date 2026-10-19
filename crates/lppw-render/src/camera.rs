use crate::terrain::LowPolyTerrain;
use lppw_world::WorldConfig;
use muxapp::muxui::d3::*;
use muxapp::muxui::*;
use muxapp::Input;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Fly,
    GroundWalk,
}

impl CameraMode {
    pub fn toggle(&self) -> Self {
        match self {
            CameraMode::Fly => CameraMode::GroundWalk,
            CameraMode::GroundWalk => CameraMode::Fly,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            CameraMode::Fly => "Free Fly Cam",
            CameraMode::GroundWalk => "Ground Walk Cam",
        }
    }
}

pub struct ExplorerCamera {
    pub mode: CameraMode,
    pub position: Vector3,
    pub yaw: f32,   // Degrees
    pub pitch: f32, // Degrees
    pub speed: f32,
    pub fov: f32,
    pub eye_height: f32,
}

impl Default for ExplorerCamera {
    fn default() -> Self {
        Self {
            mode: CameraMode::Fly,
            position: Vector3::new(0.0, 45.0, 60.0),
            yaw: 180.0,
            pitch: -15.0,
            speed: 40.0,
            fov: 65.0,
            eye_height: 2.8,
        }
    }
}

impl ExplorerCamera {
    pub fn update(
        &mut self,
        input: &Input,
        terrain: &LowPolyTerrain,
        config: &WorldConfig,
        dt: f32,
    ) {
        // Continuous mouse look
        let mouse_yaw = input.get_axis("yaw");
        let mouse_pitch = input.get_axis("pitch");

        self.yaw += mouse_yaw * 65.0 * dt;
        self.pitch = (self.pitch + mouse_pitch * 65.0 * dt).clamp(-85.0, 85.0);

        // Calculate direction vectors from Euler angles
        let yaw_rad = self.yaw.to_radians();
        let pitch_rad = self.pitch.to_radians();

        let forward = Vector3::new(
            pitch_rad.cos() * yaw_rad.sin(),
            pitch_rad.sin(),
            pitch_rad.cos() * yaw_rad.cos(),
        ).normalize();

        let right = Vector3::new(
            yaw_rad.cos(),
            0.0,
            -yaw_rad.sin(),
        ).normalize();

        let up = Vector3::new(0.0, 1.0, 0.0);

        // Input axes
        let dx = input.get_axis("horizontal"); // A / D
        let dz = input.get_axis("vertical");   // S / W
        let dy = input.get_axis("elevation");  // Ctrl / Space

        let is_boost = input.is_action_down("boost");
        let current_speed = if is_boost { self.speed * 2.8 } else { self.speed };

        let mut move_vec = Vector3::zero();

        match self.mode {
            CameraMode::Fly => {
                // Free 6-DOF movement
                move_vec += right * dx;
                move_vec += forward * dz;
                move_vec += up * dy;
                self.position += move_vec * (current_speed * dt);
            }
            CameraMode::GroundWalk => {
                // Horizontal movement restricted to ground plane
                let forward_flat = Vector3::new(yaw_rad.sin(), 0.0, yaw_rad.cos()).normalize();
                move_vec += right * dx;
                move_vec += forward_flat * dz;
                self.position += move_vec * (current_speed * 0.45 * dt);

                // Terrain following
                let ground_y = terrain.get_surface_height(config, self.position.x, self.position.z);
                let target_y = ground_y + self.eye_height;
                // Smooth elevation interpolation
                self.position.y += (target_y - self.position.y) * (dt * 12.0).min(1.0);
            }
        }
    }

    /// Returns the unit forward view vector of the camera.
    pub fn forward(&self) -> Vector3 {
        let yaw_rad = self.yaw.to_radians();
        let pitch_rad = self.pitch.to_radians();

        Vector3::new(
            pitch_rad.cos() * yaw_rad.sin(),
            pitch_rad.sin(),
            pitch_rad.cos() * yaw_rad.cos(),
        )
        .normalize()
    }

    /// Tests whether a 3D axis-aligned bounding box `(min, max)` intersects the camera's view frustum.
    /// Uses an exact 5-plane bounding-sphere frustum test (Near, Left, Right, Top, Bottom) with a small
    /// safety margin so visible chunks at the screen edges never pop.
    pub fn is_aabb_in_frustum(&self, min: Vector3, max: Vector3) -> bool {
        let center = (min + max) * 0.5;
        let half_extents = (max - min) * 0.5;
        let radius = half_extents.length();

        let rel = center - self.position;
        if rel.length() <= radius {
            return true;
        }

        let yaw_rad = self.yaw.to_radians();
        let pitch_rad = self.pitch.to_radians();

        let forward = Vector3::new(
            pitch_rad.cos() * yaw_rad.sin(),
            pitch_rad.sin(),
            pitch_rad.cos() * yaw_rad.cos(),
        )
        .normalize();

        // 1. Near plane check (completely behind camera)
        let depth = rel.dot(forward);
        if depth + radius < 0.0 {
            return false;
        }

        let right = Vector3::new(yaw_rad.cos(), 0.0, -yaw_rad.sin()).normalize();
        let cam_up = right.cross(forward).normalize();

        let half_v_tan = (self.fov * 0.5).to_radians().tan();
        // Generous horizontal aspect ratio bound (supports 16:9 up to ultrawide 21:9)
        let half_h_tan = half_v_tan * 2.15;

        // 2. Left / Right frustum planes
        let horiz_offset = rel.dot(right).abs();
        let horiz_sphere_pad = radius * (1.0 + half_h_tan * half_h_tan).sqrt();
        if horiz_offset - horiz_sphere_pad > depth.max(0.0) * half_h_tan {
            return false;
        }

        // 3. Top / Bottom frustum planes
        let vert_offset = rel.dot(cam_up).abs();
        let vert_sphere_pad = radius * (1.0 + half_v_tan * half_v_tan).sqrt();
        if vert_offset - vert_sphere_pad > depth.max(0.0) * half_v_tan {
            return false;
        }

        true
    }

    /// Converts into a MUX / Raylib `Camera3D`.
    pub fn to_camera_3d(&self) -> Camera3D {
        let forward = self.forward();

        Camera3DBuilder::new()
            .position(self.position)
            .target(self.position + forward)
            .up(Vector3::new(0.0, 1.0, 0.0))
            .fovy(self.fov)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frustum_culling_accepts_front_and_rejects_behind() {
        let cam = ExplorerCamera {
            mode: CameraMode::Fly,
            position: Vector3::new(0.0, 50.0, 0.0),
            yaw: 0.0, // Looking along +Z
            pitch: 0.0,
            speed: 40.0,
            fov: 65.0,
            eye_height: 2.8,
        };

        // Chunk directly in front (+Z)
        assert!(cam.is_aabb_in_frustum(
            Vector3::new(-100.0, 0.0, 200.0),
            Vector3::new(100.0, 80.0, 400.0)
        ));

        // Chunk directly behind (-Z)
        assert!(!cam.is_aabb_in_frustum(
            Vector3::new(-100.0, 0.0, -600.0),
            Vector3::new(100.0, 80.0, -400.0)
        ));

        // Chunk far off to the side (+X at z = 100)
        assert!(!cam.is_aabb_in_frustum(
            Vector3::new(900.0, 0.0, 50.0),
            Vector3::new(1200.0, 80.0, 250.0)
        ));
    }
}
