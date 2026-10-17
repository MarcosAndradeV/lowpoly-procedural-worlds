use crate::camera::{CameraMode, ExplorerCamera};
use crate::sky::TimeOfDay;
use crate::terrain::LowPolyTerrain;
use crate::world::{WorldConfig, WorldPreset};
use muxapp::muxui::d3::*;
use muxapp::muxui::*;
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const HUMAN_OBJ_SOURCE: &str = include_str!("../assets/human_low.obj");
const AVATAR_SCALE: f32 = 2.4;
const BEACON_DURATION: f32 = 20.0;

///Distinct adventurer tunic palette indexed by `player_id`.
pub fn player_color(player_id: u32) -> Color {
    match player_id % 6 {
        1 => Color { r: 55, g: 145, b: 245, a: 255 },  // Royal Azure
        2 => Color { r: 235, g: 75, b: 65, a: 255 },   // Crimson Tunic
        3 => Color { r: 65, g: 205, b: 105, a: 255 },  // Emerald Ranger
        4 => Color { r: 245, g: 185, b: 55, a: 255 },  // Sun Gold
        5 => Color { r: 175, g: 95, b: 240, a: 255 },  // Amethyst Mage
        _ => Color { r: 55, g: 215, b: 215, a: 255 },  // Celestial Cyan
    }
}

/// Remote player state interpolated smoothly on the client.
#[derive(Debug, Clone)]
pub struct RemotePlayer {
    pub id: u32,
    pub position: Vector3,
    pub target_position: Vector3,
    pub yaw: f32,
    pub target_yaw: f32,
    pub pitch: f32,
    pub mode: CameraMode,
    pub walk_phase: f32,
    pub last_seen_age: f32,
}

/// Shared 3D signal beacon / flare effect triggered by any player (`E` key).
#[derive(Debug, Clone)]
pub struct WorldBeacon {
    pub owner_id: u32,
    pub position: Vector3,
    pub age: f32,
}

/// Parsed template triangle from `human_low.obj` in model space.
#[derive(Debug, Clone, Copy)]
struct ObjTriangle {
    p0: Vector3,
    p1: Vector3,
    p2: Vector3,
    normal: Vector3,
    avg_y: f32,
    avg_abs_x: f32,
}

/// Parses Wavefront OBJ geometry (`v`, `vn`, `f` triangles & quads) from `human_low.obj`.
fn parse_human_obj(source: &str) -> Vec<ObjTriangle> {
    let mut positions = Vec::with_capacity(160);
    let mut normals = Vec::with_capacity(160);
    let mut triangles = Vec::with_capacity(280);

    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("v ") {
            let mut parts = rest.split_whitespace();
            if let (Some(xs), Some(ys), Some(zs)) = (parts.next(), parts.next(), parts.next())
                && let (Ok(x), Ok(y), Ok(z)) = (xs.parse::<f32>(), ys.parse::<f32>(), zs.parse::<f32>())
            {
                positions.push(Vector3::new(x, y, z));
            }
        } else if let Some(rest) = line.strip_prefix("vn ") {
            let mut parts = rest.split_whitespace();
            if let (Some(xs), Some(ys), Some(zs)) = (parts.next(), parts.next(), parts.next())
                && let (Ok(x), Ok(y), Ok(z)) = (xs.parse::<f32>(), ys.parse::<f32>(), zs.parse::<f32>())
            {
                normals.push(Vector3::new(x, y, z).normalize());
            }
        } else if let Some(rest) = line.strip_prefix("f ") {
            let mut face_verts: Vec<(usize, Option<usize>)> = Vec::with_capacity(4);
            for token in rest.split_whitespace() {
                let mut indices = token.split('/');
                if let Some(v_str) = indices.next()
                    && let Ok(v_idx) = v_str.parse::<usize>()
                {
                    let _vt = indices.next();
                    let vn_idx = indices.next().and_then(|s| s.parse::<usize>().ok());
                    if v_idx >= 1 && v_idx <= positions.len() {
                        face_verts.push((v_idx - 1, vn_idx.map(|i| i.saturating_sub(1))));
                    }
                }
            }

            if face_verts.len() >= 3 {
                for i in 1..(face_verts.len() - 1) {
                    let (ia, na) = face_verts[0];
                    let (ib, nb) = face_verts[i];
                    let (ic, nc) = face_verts[i + 1];

                    let raw_a = positions[ia];
                    let raw_b = positions[ib];
                    let raw_c = positions[ic];

                    let avg_y = (raw_a.y + raw_b.y + raw_c.y) / 3.0;
                    let avg_abs_x = (raw_a.x.abs() + raw_b.x.abs() + raw_c.x.abs()) / 3.0;

                    let pa = raw_a * AVATAR_SCALE;
                    let pb = raw_b * AVATAR_SCALE;
                    let pc = raw_c * AVATAR_SCALE;

                    let normal = match (na, nb, nc) {
                        (Some(idx_a), Some(idx_b), Some(idx_c))
                            if idx_a < normals.len()
                                && idx_b < normals.len()
                                && idx_c < normals.len() =>
                        {
                            (normals[idx_a] + normals[idx_b] + normals[idx_c]).normalize()
                        }
                        _ => (pb - pa).cross(pc - pa).normalize(),
                    };

                    triangles.push(ObjTriangle {
                        p0: pa,
                        p1: pb,
                        p2: pc,
                        normal,
                        avg_y,
                        avg_abs_x,
                    });
                }
            }
        }
    }

    triangles
}

/// Builds a colored, flat-shaded `GpuMesh` for a player avatar from parsed `ObjTriangle`s.
fn build_avatar_vertices(template: &[ObjTriangle], tunic_color: Color) -> Vec<Vertex3D> {
    let mut verts = Vec::with_capacity(template.len() * 3);
    let light_dir = Vector3::new(0.45, 0.8, 0.35).normalize();

    for tri in template {
        // Determine body-part material color from model-space height and arm offset
        let base_col = if tri.avg_y > 1.87 {
            // Hair cap / hood
            Color { r: 62, g: 42, b: 28, a: 255 }
        } else if tri.avg_y > 1.63 || (tri.avg_abs_x > 0.28 && tri.avg_y < 1.15) {
            // Head & hands
            Color { r: 232, g: 188, b: 152, a: 255 }
        } else if tri.avg_y >= 0.90 {
            // Adventurer tunic / armor
            tunic_color
        } else if tri.avg_y >= 0.26 {
            // Trousers
            Color { r: 58, g: 64, b: 78, a: 255 }
        } else {
            // Boots
            Color { r: 72, g: 46, b: 30, a: 255 }
        };

        let shade = 0.52 + tri.normal.dot(light_dir).max(0.0) * 0.48;
        let col = Color {
            r: (base_col.r as f32 * shade).clamp(0.0, 255.0) as u8,
            g: (base_col.g as f32 * shade).clamp(0.0, 255.0) as u8,
            b: (base_col.b as f32 * shade).clamp(0.0, 255.0) as u8,
            a: 255,
        };

        verts.push(Vertex3D::new(tri.p0, col).with_normal(tri.normal));
        verts.push(Vertex3D::new(tri.p1, col).with_normal(tri.normal));
        verts.push(Vertex3D::new(tri.p2, col).with_normal(tri.normal));
    }

    verts
}

/// Non-blocking multiplayer client managing GSS state sync, remote player avatars, and signal beacons.
pub struct MultiplayerClient {
    pub server_addr: String,
    stream: Option<TcpStream>,
    recv_buf: String,
    pub local_id: u32,
    pub remote_players: HashMap<u32, RemotePlayer>,
    pub beacons: Vec<WorldBeacon>,
    send_timer: f32,
    obj_template: Vec<ObjTriangle>,
    avatar_meshes: HashMap<u32, GpuMesh>,
    last_sent_seed: u64,
    last_sent_preset: u8,
    last_sent_water_bits: u32,
    last_sent_time: u8,
}

impl Default for MultiplayerClient {
    fn default() -> Self {
        let mut client = Self {
            server_addr: config_get_host_and_port(),
            stream: None,
            recv_buf: String::with_capacity(4096),
            local_id: 0,
            remote_players: HashMap::new(),
            beacons: Vec::new(),
            send_timer: 0.0,
            obj_template: parse_human_obj(HUMAN_OBJ_SOURCE),
            avatar_meshes: HashMap::new(),
            last_sent_seed: 0,
            last_sent_preset: 0,
            last_sent_water_bits: 0,
            last_sent_time: 1,
        };
        client.try_connect();
        client
    }
}

impl MultiplayerClient {
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn player_count(&self) -> usize {
        if self.is_connected() {
            1 + self.remote_players.len()
        } else {
            1
        }
    }

    /// Returns a concise HUD status label describing the multiplayer connection and player count.
    pub fn status_label(&self) -> String {
        if self.is_connected() {
            let id = if self.local_id > 0 { self.local_id } else { 1 };
            format!("Online #{} ({}P)", id, self.player_count())
        } else {
            "Offline [M]".to_string()
        }
    }

    /// Places a 3D Signal Beacon in front of the player's camera on the terrain surface.
    pub fn place_beacon(
        &mut self,
        camera: &ExplorerCamera,
        terrain: &LowPolyTerrain,
        config: &WorldConfig,
    ) {
        let yaw_rad = camera.yaw.to_radians();
        let bx = camera.position.x + yaw_rad.sin() * 18.0;
        let bz = camera.position.z + yaw_rad.cos() * 18.0;
        let by = terrain
            .get_surface_height(config, bx, bz)
            .max(config.water_level);
        self.trigger_beacon(Vector3::new(bx, by, bz));
    }

    /// Attempts a fast non-blocking connection to the multiplayer server.
    pub fn try_connect(&mut self) -> bool {
        if self.stream.is_some() {
            return true;
        }
        let Ok(addr) = self.server_addr.parse::<SocketAddr>() else {
            return false;
        };
        match TcpStream::connect_timeout(&addr, Duration::from_millis(120)) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_nonblocking(true);
                log_info!("Multiplayer: Connected to server at {}", self.server_addr);
                self.stream = Some(stream);
                self.recv_buf.clear();
                true
            }
            Err(_) => false,
        }
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
        self.remote_players.clear();
        self.local_id = 0;
    }

    fn send_line(&mut self, line: &str) {
        if let Some(ref mut stream) = self.stream
            && let Err(e) = stream.write_all(line.as_bytes())
            && e.kind() != ErrorKind::WouldBlock
        {
            log_warn!("Multiplayer: Connection lost ({})", e);
            self.disconnect();
        }
    }

    /// Emits a 3D Signal Beacon at `pos` locally and broadcasts it to all players on the server.
    pub fn trigger_beacon(&mut self, pos: Vector3) {
        let owner = if self.local_id > 0 { self.local_id } else { 1 };
        self.beacons.push(WorldBeacon {
            owner_id: owner,
            position: pos,
            age: 0.0,
        });

        if self.is_connected() {
            let msg = format!(
                "type = \"beacon\"\npos = {{x = {:.2}, y = {:.2}, z = {:.2}}}\n---\n",
                pos.x, pos.y, pos.z
            );
            self.send_line(&msg);
        }
    }

    /// Broadcasts local world setting changes (seed, preset, water level, time of day) to the server.
    pub fn notify_world_changed(&mut self, config: &WorldConfig, time_of_day: TimeOfDay) {
        let preset_id = Self::preset_to_u8(config.preset);
        let time_id = Self::time_to_u8(time_of_day);
        let water_bits = config.water_level.to_bits();

        if !self.is_connected() {
            self.last_sent_seed = config.seed;
            self.last_sent_preset = preset_id;
            self.last_sent_water_bits = water_bits;
            self.last_sent_time = time_id;
            return;
        }

        if config.seed != self.last_sent_seed
            || preset_id != self.last_sent_preset
            || water_bits != self.last_sent_water_bits
            || time_id != self.last_sent_time
        {
            self.last_sent_seed = config.seed;
            self.last_sent_preset = preset_id;
            self.last_sent_water_bits = water_bits;
            self.last_sent_time = time_id;

            let msg = format!(
                "type = \"world\"\nseed = {}\npreset = {}\nwater = {:.2}\ntime = {}\n---\n",
                config.seed as i64,
                preset_id,
                config.water_level,
                time_id
            );
            self.send_line(&msg);
        }
    }

    /// Steps network I/O, parses incoming GSS packets, and smoothly interpolates remote players & beacons.
    pub fn update(
        &mut self,
        camera: &mut ExplorerCamera,
        config: &mut WorldConfig,
        time_of_day: &mut TimeOfDay,
        dt: f32,
    ) {
        // Update active beacons
        for beacon in &mut self.beacons {
            beacon.age += dt;
        }
        self.beacons.retain(|b| b.age < BEACON_DURATION);

        // Read all available non-blocking bytes from the server
        if let Some(ref mut stream) = self.stream {
            let mut buf = [0u8; 4096];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => {
                        log_warn!("Multiplayer: Server closed connection");
                        self.disconnect();
                        break;
                    }
                    Ok(n) => {
                        if let Ok(chunk) = std::str::from_utf8(&buf[..n]) {
                            self.recv_buf.push_str(chunk);
                        }
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) => {
                        log_warn!("Multiplayer: Read error ({})", e);
                        self.disconnect();
                        break;
                    }
                }
            }
        }

        // Process complete GSS packets delimited by "\n---\n" (or single-block legacy packets)
        while let Some(sep_idx) = self.recv_buf.find("\n---\n") {
            let packet: String = self.recv_buf.drain(..sep_idx).collect();
            self.recv_buf.drain(..5); // remove "\n---\n"
            self.handle_gss_packet(&packet, camera, config, time_of_day);
        }

        // Smoothly interpolate remote players
        let interp = (dt * 12.0).min(1.0);
        for player in self.remote_players.values_mut() {
            player.last_seen_age += dt;
            let delta = player.target_position - player.position;
            let dist = Vector3::new(delta.x, 0.0, delta.z).length();
            player.position += delta * interp;

            // Shortest-angle yaw interpolation
            let mut yaw_diff = (player.target_yaw - player.yaw).rem_euclid(360.0);
            if yaw_diff > 180.0 {
                yaw_diff -= 360.0;
            }
            player.yaw = (player.yaw + yaw_diff * interp).rem_euclid(360.0);

            if dist > 0.05 {
                player.walk_phase += dt * 10.0;
            }
        }
        self.remote_players.retain(|_, p| p.last_seen_age < 6.0);

        // Periodically send local player pose (25 Hz)
        if self.is_connected() {
            self.send_timer += dt;
            if self.send_timer >= 0.04 {
                self.send_timer = 0.0;
                let mode_id = match camera.mode {
                    CameraMode::Fly => 0,
                    CameraMode::GroundWalk => 1,
                };
                let msg = format!(
                    "type = \"pose\"\npos = {{x = {:.2}, y = {:.2}, z = {:.2}}}\nyaw = {:.1}\npitch = {:.1}\nmode = {}\n---\n",
                    camera.position.x,
                    camera.position.y,
                    camera.position.z,
                    camera.yaw,
                    camera.pitch,
                    mode_id
                );
                self.send_line(&msg);
            }
        }
    }

    fn handle_gss_packet(
        &mut self,
        source: &str,
        camera: &mut ExplorerCamera,
        config: &mut WorldConfig,
        time_of_day: &mut TimeOfDay,
    ) {
        let Ok(obj) = gss::parse_str(source) else {
            return;
        };

        let pkt_type = String::from_gss(&obj, &["type"]).unwrap_or_default();

        match pkt_type.as_str() {
            "welcome" | "sync" => {
                if let Some(id) = usize::from_gss(&obj, &["id"]) {
                    self.local_id = id as u32;
                }
                if let Some(seed) = i32::from_gss(&obj, &["seed"]).map(|s| s as u64).or_else(|| usize::from_gss(&obj, &["seed"]).map(|s| s as u64)) {
                    let new_seed = seed;
                    if config.seed != new_seed {
                        config.seed = new_seed;
                        self.last_sent_seed = new_seed;
                    }
                }
                if let Some(preset_id) = usize::from_gss(&obj, &["preset"]) {
                    let preset = Self::u8_to_preset(preset_id as u8);
                    if config.preset != preset {
                        config.set_preset(preset);
                        self.last_sent_preset = preset_id as u8;
                    }
                }
                if let Some(water) = f32::from_gss(&obj, &["water"]) {
                    config.water_level = water;
                    self.last_sent_water_bits = water.to_bits();
                }
                if let Some(time_id) = usize::from_gss(&obj, &["time"]) {
                    *time_of_day = Self::u8_to_time(time_id as u8);
                    self.last_sent_time = time_id as u8;
                }
                if pkt_type == "welcome"
                    && let Some(pos) = Vector3::from_gss(&obj, &["pos"])
                {
                    camera.position = pos;
                }
            }
            "state" => {
                let count = usize::from_gss(&obj, &["count"]).unwrap_or(0);
                let mut seen_ids = Vec::with_capacity(count);
                for i in 0..count {
                    let id_key = format!("id_{}", i);
                    let pos_key = format!("pos_{}", i);
                    let yaw_key = format!("yaw_{}", i);
                    let pitch_key = format!("pitch_{}", i);
                    let mode_key = format!("mode_{}", i);

                    let Some(pid) = usize::from_gss(&obj, &[&id_key]).map(|v| v as u32) else {
                        continue;
                    };
                    if pid == self.local_id || pid == 0 {
                        continue;
                    }
                    let Some(pos) = Vector3::from_gss(&obj, &[&pos_key]) else {
                        continue;
                    };
                    let yaw = f32::from_gss(&obj, &[&yaw_key]).unwrap_or(0.0);
                    let pitch = f32::from_gss(&obj, &[&pitch_key]).unwrap_or(0.0);
                    let mode_id = usize::from_gss(&obj, &[&mode_key]).unwrap_or(0);
                    let mode = if mode_id == 1 {
                        CameraMode::GroundWalk
                    } else {
                        CameraMode::Fly
                    };

                    seen_ids.push(pid);
                    self.remote_players
                        .entry(pid)
                        .and_modify(|p| {
                            p.target_position = pos;
                            p.target_yaw = yaw;
                            p.pitch = pitch;
                            p.mode = mode;
                            p.last_seen_age = 0.0;
                        })
                        .or_insert(RemotePlayer {
                            id: pid,
                            position: pos,
                            target_position: pos,
                            yaw,
                            target_yaw: yaw,
                            pitch,
                            mode,
                            walk_phase: 0.0,
                            last_seen_age: 0.0,
                        });
                }
                self.remote_players.retain(|id, _| seen_ids.contains(id));
            }
            "beacon" => {
                let owner = usize::from_gss(&obj, &["owner"]).unwrap_or(1) as u32;
                if owner != self.local_id
                    && let Some(pos) = Vector3::from_gss(&obj, &["pos"])
                {
                    self.beacons.push(WorldBeacon {
                        owner_id: owner,
                        position: pos,
                        age: 0.0,
                    });
                }
            }
            _ => {
                if let Some(pos) = Vector3::from_gss(&obj, &["pos"]) {
                    camera.position = pos;
                }
            }
        }
    }

    /// Creates a 4x4 model matrix with rotation around the Y-axis and translation to `pos`.
    ///
    /// NOTE: Raylib's C `Matrix` structure places translation in `m12, m13, m14` (column 3)
    /// and perspective denominators in `m3, m7, m11, m15` (row 3).
    pub fn avatar_transform(pos: Vector3, yaw_deg: f32) -> Matrix {
    let yaw_rad = yaw_deg.to_radians();
    let (sin_y, cos_y) = yaw_rad.sin_cos();
    Matrix {
        m0: cos_y,
        m4: 0.0,
        m8: sin_y,
        m12: pos.x,

        m1: 0.0,
        m5: 1.0,
        m9: 0.0,
        m13: pos.y,

        m2: -sin_y,
        m6: 0.0,
        m10: cos_y,
        m14: pos.z,

        m3: 0.0,
        m7: 0.0,
        m11: 0.0,
        m15: 1.0,
    }
}

    /// Renders all remote player 3D avatars (`human_low.obj`) and active Signal Beacons.
    pub fn draw_3d(
        &mut self,
        d3: &mut Render3DContext,
        terrain: &LowPolyTerrain,
        config: &WorldConfig,
    ) {
        // 1. Render remote players using `human_low.obj` mesh
        let player_ids: Vec<u32> = self.remote_players.keys().copied().collect();
        for pid in player_ids {
            let color_bucket = pid % 6;
            if !self.avatar_meshes.contains_key(&color_bucket) {
                let verts = build_avatar_vertices(&self.obj_template, player_color(pid));
                self.avatar_meshes
                    .insert(color_bucket, GpuMesh::from_vertices(&verts));
            }

            let Some(player) = self.remote_players.get(&pid) else {
                continue;
            };

            // Place feet at ground height when walking, or at eye-offset height when flying
            let ground_y = terrain.get_surface_height(config, player.position.x, player.position.z);
            let bob = if player.mode == CameraMode::GroundWalk {
                (player.walk_phase.sin()).abs() * 0.22
            } else {
                (player.last_seen_age * 3.0).sin() * 0.15
            };
            let feet_y = (player.position.y - 2.8).max(ground_y) + bob;
            let avatar_pos = Vector3::new(player.position.x, feet_y, player.position.z);
            let transform = Self::avatar_transform(avatar_pos, player.yaw);

            if let Some(mesh) = self.avatar_meshes.get_mut(&color_bucket) {
                d3.draw_mesh_transformed(mesh, transform);
            }

            // If flying, render a glowing wind/levitation ring beneath the player's feet
            if player.mode == CameraMode::Fly {
                let mut ring_col = player_color(pid);
                ring_col.a = 190;
                d3.draw_cylinder(
                    Vector3::new(avatar_pos.x, avatar_pos.y - 0.15, avatar_pos.z),
                    1.35,
                    1.35,
                    0.18,
                    10,
                    ring_col,
                );
            }
        }

        // 2. Render active 3D Signal Beacons / Waypoint Pillars
        for beacon in &self.beacons {
            let base_col = player_color(beacon.owner_id);
            let fade = (1.0 - beacon.age / BEACON_DURATION).clamp(0.0, 1.0);
            let pulse = (beacon.age * 4.5).sin() * 0.5 + 0.5;

            let beam_col = Color {
                r: base_col.r,
                g: base_col.g,
                b: base_col.b,
                a: (185.0 * fade) as u8,
            };
            let core_col = Color {
                r: 255,
                g: 250,
                b: 220,
                a: (230.0 * fade) as u8,
            };

            // Inner bright crystal core + outer skyward light pillar
            let beam_radius = 1.1 + pulse * 0.45;
            d3.draw_cylinder(beacon.position, 0.55, 0.55, 220.0, 6, core_col);
            d3.draw_cylinder(beacon.position, beam_radius, beam_radius * 1.8, 220.0, 8, beam_col);

            // Expanding ground shockwave ring
            let ring_phase = (beacon.age * 1.4).fract();
            let ring_r = 2.0 + ring_phase * 22.0;
            let ring_col = Color {
                r: base_col.r,
                g: base_col.g,
                b: base_col.b,
                a: ((1.0 - ring_phase) * 210.0 * fade) as u8,
            };
            d3.draw_cylinder(
                Vector3::new(beacon.position.x, beacon.position.y + 0.25, beacon.position.z),
                ring_r,
                ring_r,
                0.35,
                12,
                ring_col,
            );
        }
    }

    /// Renders 2D floating nameplates and distance tags over remote players and active beacons.
    pub fn draw_2d_overlays(&self, camera: &ExplorerCamera) {
        let cam3d = camera.to_camera_3d();

        for player in self.remote_players.values() {
            let head_pos = Vector3::new(player.position.x, player.position.y + 2.4, player.position.z);
            if let Some(screen_pos) = get_virtual_world_to_screen(&cam3d, head_pos) {
                let dist = (player.position - camera.position).length();
                let label = format!("Player #{} [{:.0}m]", player.id, dist);
                let col = player_color(player.id);
                let badge_w = (label.len() as f32) * 8.5 + 14.0;
                draw_rectangle_rec(
                    Rectangle::new(screen_pos.x - badge_w * 0.5, screen_pos.y - 22.0, badge_w, 22.0),
                    get_color(0x0D131ACC),
                );
                draw_text(&label, (screen_pos.x - badge_w * 0.5 + 7.0) as i32, (screen_pos.y - 18.0) as i32, 15, col);
            }
        }

        for beacon in &self.beacons {
            let marker_pos = Vector3::new(beacon.position.x, beacon.position.y + 18.0, beacon.position.z);
            if let Some(screen_pos) = get_virtual_world_to_screen(&cam3d, marker_pos) {
                let dist = (beacon.position - camera.position).length();
                let label = format!("* Beacon #{} [{:.0}m]", beacon.owner_id, dist);
                let col = player_color(beacon.owner_id);
                let badge_w = (label.len() as f32) * 8.5 + 14.0;
                draw_rectangle_rec(
                    Rectangle::new(screen_pos.x - badge_w * 0.5, screen_pos.y - 22.0, badge_w, 22.0),
                    get_color(0x0D131ACC),
                );
                draw_text(&label, (screen_pos.x - badge_w * 0.5 + 7.0) as i32, (screen_pos.y - 18.0) as i32, 15, col);
            }
        }
    }

    pub fn preset_to_u8(preset: WorldPreset) -> u8 {
        match preset {
            WorldPreset::Archipelago => 0,
            WorldPreset::AlpineHighlands => 1,
            WorldPreset::Canyonlands => 2,
            WorldPreset::AlienCrags => 3,
        }
    }

    pub fn u8_to_preset(id: u8) -> WorldPreset {
        match id {
            1 => WorldPreset::AlpineHighlands,
            2 => WorldPreset::Canyonlands,
            3 => WorldPreset::AlienCrags,
            _ => WorldPreset::Archipelago,
        }
    }

    pub fn time_to_u8(time: TimeOfDay) -> u8 {
        match time {
            TimeOfDay::Noon => 0,
            TimeOfDay::Sunset => 1,
            TimeOfDay::Night => 2,
        }
    }

    pub fn u8_to_time(id: u8) -> TimeOfDay {
        match id {
            0 => TimeOfDay::Noon,
            2 => TimeOfDay::Night,
            _ => TimeOfDay::Sunset,
        }
    }
}

pub fn config_get_host_and_port() -> String {
    match gss::load_gss_from_file("./config.gss") {
        Ok(ok) => {
            let host = ok.get_or(&["network", "host"], "127.0.0.1".to_string());
            let port = ok.get_or(&["network", "port"], "8080".to_string());
            format!("{}:{}", host, port)
        }
        Err(err) => {
            log_error!("NET: Cannot load `config.gss` {err}");
            log_info!("NET: Using default address: `127.0.0.1:8080`");
            "127.0.0.1:8080".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_obj_parses_triangles_and_normals() {
        let tris = parse_human_obj(HUMAN_OBJ_SOURCE);
        assert!(
            tris.len() > 200,
            "Expected >200 triangles from human_low.obj, got {}",
            tris.len()
        );
        let verts = build_avatar_vertices(&tris, player_color(1));
        assert_eq!(verts.len(), tris.len() * 3);
    }

    #[test]
    fn test_gss_multiplayer_state_packet_parsing() {
        let packet = "type = \"state\"\ncount = 1\nid_0 = 2\npos_0 = {x = 12.5, y = 40.0, z = -30.0}\nyaw_0 = -90.0\npitch_0 = -15.5\nmode_0 = 1\n";
        let obj = gss::parse_str(packet).expect("Valid GSS state packet with signed numbers");
        assert_eq!(String::from_gss(&obj, &["type"]).as_deref(), Some("state"));
        assert_eq!(usize::from_gss(&obj, &["count"]), Some(1));
        let pos = Vector3::from_gss(&obj, &["pos_0"]).expect("Valid pos_0 Vector3");
        assert!((pos.x - 12.5).abs() < 1e-2);
        assert!((pos.y - 40.0).abs() < 1e-2);
        assert!((pos.z - -30.0).abs() < 1e-2);
        let yaw = f32::from_gss(&obj, &["yaw_0"]).expect("Valid yaw_0");
        assert!((yaw - -90.0).abs() < 1e-2);
        let pitch = f32::from_gss(&obj, &["pitch_0"]).expect("Valid pitch_0");
        assert!((pitch - -15.5).abs() < 1e-2);
    }

    #[test]
    fn test_avatar_transform_layout() {
        let pos = Vector3::new(15.0, 25.0, -35.0);
        let mat = MultiplayerClient::avatar_transform(pos, 0.0);
        // Translation must be in column 3 (m12, m13, m14)
        assert_eq!(mat.m12, 15.0);
        assert_eq!(mat.m13, 25.0);
        assert_eq!(mat.m14, -35.0);
        // Perspective denominator row must be [0, 0, 0, 1]
        assert_eq!(mat.m3, 0.0);
        assert_eq!(mat.m7, 0.0);
        assert_eq!(mat.m11, 0.0);
        assert_eq!(mat.m15, 1.0);
        // Identity rotation at yaw 0
        assert_eq!(mat.m0, 1.0);
        assert_eq!(mat.m5, 1.0);
        assert_eq!(mat.m10, 1.0);
    }
}
