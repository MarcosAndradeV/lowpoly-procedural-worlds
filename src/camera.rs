use crate::terrain::LowPolyTerrain;
use crate::world::WorldConfig;
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
                move_vec = move_vec + right * dx;
                move_vec = move_vec + forward * dz;
                move_vec = move_vec + up * dy;
                self.position = self.position + move_vec * (current_speed * dt);
            }
            CameraMode::GroundWalk => {
                // Horizontal movement restricted to ground plane
                let forward_flat = Vector3::new(yaw_rad.sin(), 0.0, yaw_rad.cos()).normalize();
                move_vec = move_vec + right * dx;
                move_vec = move_vec + forward_flat * dz;
                self.position = self.position + move_vec * (current_speed * 0.45 * dt);

                // Terrain following
                let ground_y = terrain.get_surface_height(config, self.position.x, self.position.z);
                let target_y = ground_y + self.eye_height;
                // Smooth elevation interpolation
                self.position.y += (target_y - self.position.y) * (dt * 12.0).min(1.0);
            }
        }
    }

    /// Converts into a MUX / Raylib `Camera3D`.
    pub fn to_camera_3d(&self) -> Camera3D {
        let yaw_rad = self.yaw.to_radians();
        let pitch_rad = self.pitch.to_radians();

        let forward = Vector3::new(
            pitch_rad.cos() * yaw_rad.sin(),
            pitch_rad.sin(),
            pitch_rad.cos() * yaw_rad.cos(),
        ).normalize();

        Camera3DBuilder::new()
            .position(self.position)
            .target(self.position + forward)
            .up(Vector3::new(0.0, 1.0, 0.0))
            .fovy(self.fov)
            .build()
    }
}
