use lowpoly_procedural_worlds::camera::ExplorerCamera;
use lowpoly_procedural_worlds::config;
use lowpoly_procedural_worlds::landmarks::LandmarkManager;
use lowpoly_procedural_worlds::network::MultiplayerClient;
use lowpoly_procedural_worlds::props::PropManager;
use lowpoly_procedural_worlds::sky::SkyDome;
use lowpoly_procedural_worlds::terrain::LowPolyTerrain;
use lowpoly_procedural_worlds::towns::TownManager;
use lowpoly_procedural_worlds::water::LowPolyWater;
use lowpoly_procedural_worlds::world::{WorldConfig, WorldPreset};
use muxapp::muxui::d3::*;
use muxapp::muxui::*;
use muxapp::{App, Input, MouseAxis, Scene, SceneAction, SceneLayerMode};

/// Shared state across scenes in the MUX SceneRouter.
pub struct AppState {
    pub net: MultiplayerClient,
    pub config: WorldConfig,
    pub preset_request: Option<WorldPreset>,
    pub camera_speed: f32,
    pub props_enabled: bool,
    pub water_animated: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: WorldConfig::default(),
            preset_request: None,
            camera_speed: 40.0,
            props_enabled: true,
            water_animated: true,
            net: MultiplayerClient::new(config::get_host_and_port()),
        }
    }
}

/// Primary 3D exploration scene for navigating procedural low-poly worlds.
pub struct WorldScene {
    pub terrain: LowPolyTerrain,
    pub water: LowPolyWater,
    pub props: PropManager,
    pub towns: TownManager,
    pub landmarks: LandmarkManager,
    pub sky: SkyDome,
    pub camera: ExplorerCamera,
    pub show_hud: bool,
}

impl Default for WorldScene {
    fn default() -> Self {
        let default_cfg = WorldConfig::default();
        let starter_town = default_cfg.get_town_in_cell(0, 0);
        let mut camera = ExplorerCamera::default();
        if starter_town.valid {
            camera.position = Vector3::new(
                starter_town.x,
                starter_town.y + 42.0,
                starter_town.z + 115.0,
            );
            camera.yaw = 180.0;
            camera.pitch = -16.0;
        }

        Self {
            terrain: LowPolyTerrain::default(),
            water: LowPolyWater::default(),
            props: PropManager::default(),
            towns: TownManager::default(),
            landmarks: LandmarkManager::default(),
            sky: SkyDome::default(),
            camera,
            show_hud: true,
        }
    }
}

impl Scene<AppState> for WorldScene {
    fn on_enter(&mut self, _ctx: &mut AppState, _style: &Style) {
        set_cursor_mode(CursorMode::Captured);
    }

    fn on_exit(&mut self, _ctx: &mut AppState) {
        set_cursor_mode(CursorMode::Normal);
    }

    fn update(
        &mut self,
        state: &mut AppState,
        input: &Input,
        _style: &Style,
        dt: f32,
    ) -> SceneAction {
        // Handle pause request
        if input.is_action_pressed("pause") {
            return SceneAction::Push("pause".to_string());
        }

        // Handle external preset requests from UI or pause menu
        if let Some(preset) = state.preset_request.take() {
            state.config.set_preset(preset);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }

        // Semantic action hotkeys
        if input.is_action_pressed("randomize_seed") {
            state.config.randomize_seed();
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if input.is_action_pressed("preset_1") {
            state.config.set_preset(WorldPreset::Archipelago);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if input.is_action_pressed("preset_2") {
            state.config.set_preset(WorldPreset::AlpineHighlands);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if input.is_action_pressed("preset_3") {
            state.config.set_preset(WorldPreset::Canyonlands);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if input.is_action_pressed("preset_4") {
            state.config.set_preset(WorldPreset::AlienCrags);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if input.is_action_pressed("toggle_props") {
            state.props_enabled = !state.props_enabled;
        }
        if input.is_action_pressed("toggle_towns") {
            state.config.towns_enabled = !state.config.towns_enabled;
        }
        if input.is_action_pressed("toggle_landmarks") {
            state.config.landmarks_enabled = !state.config.landmarks_enabled;
        }
        if input.is_action_pressed("toggle_water") {
            state.water_animated = !state.water_animated;
        }
        if input.is_action_pressed("cycle_time") {
            self.sky.cycle_time();
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if input.is_action_pressed("toggle_cam_mode") {
            self.camera.mode = self.camera.mode.toggle();
        }
        if input.is_action_pressed("toggle_hud") {
            self.show_hud = !self.show_hud;
        }
        if input.is_action_pressed("beacon") {
            state.net
                .place_beacon(&self.camera, &self.terrain, &state.config);
        }
        if input.is_action_pressed("reconnect") {
            state.net.try_connect();
        }

        // Synchronize state
        self.props.enabled = state.props_enabled;
        self.towns.enabled = state.config.towns_enabled;
        self.landmarks.enabled = state.config.landmarks_enabled;
        self.water.animated = state.water_animated;
        self.camera.speed = state.camera_speed;

        // Update systems
        self.camera.update(input, &self.terrain, &state.config, dt);
        state.net.update(
            &mut self.camera,
            &mut state.config,
            &mut self.sky.time_of_day,
            dt,
        );
        self.water.update(dt);
        self.sky.update(dt);

        SceneAction::None
    }

    fn draw_3d(&mut self, state: &mut AppState) {
        let cam = self.camera.to_camera_3d();
        let sun_dir = self.sky.time_of_day.sun_dir();
        let ambient = self.sky.time_of_day.ambient_light();
        let cam_pos = self.camera.position;

        draw_3d(cam, |d3| {
            // 1. Distant horizon dome & clouds
            self.sky.draw(d3, cam_pos.x, cam_pos.z);

            // 2. Chunked low-poly terrain mesh with 3D frustum culling
            self.terrain
                .draw(d3, &state.config, &self.camera, sun_dir, ambient);

            // 3. Medieval fantasy towns, cobblestone roads & wooden bridges
            self.towns.draw(
                d3,
                &self.terrain,
                &state.config,
                cam_pos.x,
                cam_pos.z,
                sun_dir,
                ambient,
                self.sky.time_of_day,
            );

            // 3b. Rare procedural landmarks & horizon anchors
            self.landmarks.draw(
                d3,
                &self.terrain,
                &state.config,
                cam_pos.x,
                cam_pos.z,
                sun_dir,
                ambient,
                self.sky.time_of_day,
            );

            // 4. Chunked procedural vegetation and rock props with 3D frustum culling
            self.props.draw(
                d3,
                &self.terrain,
                &state.config,
                &self.camera,
                sun_dir,
                ambient,
            );

            // 5. Faceted animated ocean water (or molten lava in volcanic worlds)
            self.water.draw(
                d3,
                state.config.water_level,
                cam_pos.x,
                cam_pos.z,
                sun_dir,
                ambient,
                state.config.is_volcanic(),
            );

            // 6. Remote multiplayer explorer avatars (human_low.obj) and 3D signal beacons
            state.net.draw_3d(d3, &self.terrain, &state.config);
        });
    }

    fn draw_2d(&mut self, state: &mut AppState, style: &Style) -> SceneAction {
        // Draw 2D floating nameplates and beacon distance markers in screen space
        state.net.draw_2d_overlays(&self.camera);

        if !self.show_hud {
            return SceneAction::None;
        }

        // Top HUD Header
        TextElement::new("LOW-POLY PROCEDURAL WORLDS").place(style, "hud_title");

        let pos = self.camera.position;
        let biome = self.terrain.get_biome_at(&state.config, pos.x, pos.z);
        let fps = get_fps();
        let (vis_chunks, total_chunks) = self.terrain.chunk_stats();

        let stats_text = format!(
            "FPS: {} | Chunks: {}/{} | Cam: [{:.1}, {:.1}, {:.1}] | Mode: {}",
            fps,
            vis_chunks,
            total_chunks,
            pos.x,
            pos.y,
            pos.z,
            self.camera.mode.name()
        );
        TextElement::new(stats_text).place(style, "hud_stats");

        let biome_text = format!(
            "Biome: {} | Sun: {} | Towns: {} | Landmarks: {} | Trees: {}",
            biome.name(),
            self.sky.time_of_day.name(),
            if state.config.towns_enabled {
                "ON"
            } else {
                "OFF"
            },
            if state.config.landmarks_enabled {
                "ON"
            } else {
                "OFF"
            },
            if self.props.enabled { "ON" } else { "OFF" }
        );
        TextElement::new(biome_text).place(style, "hud_biome");

        let preset_text = format!(
            "Preset: {} | Seed: #{} | Net: {}",
            state.config.preset.name(),
            state.config.seed & 0xFFFFFF,
            state.net.status_label()
        );
        TextElement::new(preset_text).place(style, "hud_preset_info");

        // Top-Right Preset & Pause Buttons
        let btn_arch = Button::new("1: Archipelago");
        let btn_alpine = Button::new("2: Alpine");
        let btn_canyon = Button::new("3: Canyon");
        let btn_alien = Button::new("4: Alien");
        let btn_rand = Button::new("R: Random Seed");
        let btn_pause = Button::new("Pause (P)");

        StackLayout::new(&[
            ("btn_preset_arch", &btn_arch),
            ("btn_preset_alpine", &btn_alpine),
            ("btn_preset_canyon", &btn_canyon),
            ("btn_preset_alien", &btn_alien),
            ("btn_random_seed", &btn_rand),
            ("btn_hud_pause", &btn_pause),
        ])
        .place(style, "preset_button_row");

        if btn_arch.clicked() {
            state.config.set_preset(WorldPreset::Archipelago);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if btn_alpine.clicked() {
            state.config.set_preset(WorldPreset::AlpineHighlands);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if btn_canyon.clicked() {
            state.config.set_preset(WorldPreset::Canyonlands);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if btn_alien.clicked() {
            state.config.set_preset(WorldPreset::AlienCrags);
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if btn_rand.clicked() {
            state.config.randomize_seed();
            state.net
                .notify_world_changed(&state.config, self.sky.time_of_day);
        }
        if btn_pause.clicked() {
            return SceneAction::Push("pause".to_string());
        }

        // Bottom Controls Bar
        RectangleElement.place(style, "footer_bar");
        TextElement::new("WASD: Move | Mouse: Look | Shift: Boost | F: Walk/Fly | E: Beacon | M: Reconnect | R: Seed | 1-4: Presets | V: Towns | K: Landmarks | T: Trees | L: Sun | P/Esc: Pause").place(style, "footer_text");

        SceneAction::None
    }
}

/// Modal overlay rendered directly over the frozen 3D procedural world.
#[derive(Default)]
pub struct PauseOverlayScene;

impl Scene<AppState> for PauseOverlayScene {
    fn layer_mode(&self) -> SceneLayerMode {
        SceneLayerMode::Overlay
    }

    fn on_enter(&mut self, _ctx: &mut AppState, _style: &Style) {
        set_cursor_mode(CursorMode::Normal);
    }

    fn on_exit(&mut self, _ctx: &mut AppState) {
        set_cursor_mode(CursorMode::Captured);
    }

    fn update(
        &mut self,
        _state: &mut AppState,
        input: &Input,
        _style: &Style,
        _dt: f32,
    ) -> SceneAction {
        if input.is_action_pressed("pause") {
            return SceneAction::Pop;
        }
        SceneAction::None
    }

    fn draw_2d(&mut self, state: &mut AppState, style: &Style) -> SceneAction {
        // Dark translucent backdrop over the underlying frozen 3D world
        draw_rectangle_rec(get_virtual_screen_rect(), get_color(0x0C121AE6));

        TextElement::new("EXPLORATION PAUSED").place(style, "pause_title");
        TextElement::new("Press [P] or [Esc] to resume exploring").place(style, "pause_subtitle");

        // Runtime tuning sliders
        let water_slider = Slider::new(0.0, 40.0, state.config.water_level)
            .with_step(0.5)
            .with_label(format!("Water Level: {:.1}", state.config.water_level));
        water_slider.place(style, "slider_water");
        water_slider.apply(&mut state.config.water_level);

        let speed_slider = Slider::new(10.0, 120.0, state.camera_speed)
            .with_step(5.0)
            .with_label(format!("Flight Speed: {:.0}", state.camera_speed));
        speed_slider.place(style, "slider_speed");
        speed_slider.apply(&mut state.camera_speed);

        // Feature toggle checkboxes
        Checkbox::bind(
            "Vegetation & Rocks",
            &mut state.props_enabled,
            style,
            "chk_props",
        );
        Checkbox::bind(
            "Animated Ocean Waves",
            &mut state.water_animated,
            style,
            "chk_waves",
        );
        Checkbox::bind(
            "Fantasy Towns & Roads",
            &mut state.config.towns_enabled,
            style,
            "chk_towns",
        );
        Checkbox::bind(
            "World Landmarks",
            &mut state.config.landmarks_enabled,
            style,
            "chk_landmarks",
        );

        // Modal action buttons
        let resume_btn = Button::new("Resume World");
        let rand_btn = Button::new("Randomize Seed");
        let quit_btn = Button::new("Exit Application");

        StackLayout::new(&[
            ("btn_resume", &resume_btn),
            ("btn_randomize", &rand_btn),
            ("btn_quit", &quit_btn),
        ])
        .place(style, "pause_btn_col");

        if resume_btn.clicked() {
            return SceneAction::Pop;
        }
        if rand_btn.clicked() {
            state.config.randomize_seed();
        }
        if quit_btn.clicked() {
            request_exit();
        }

        SceneAction::None
    }
}

fn main() {
    App::init(
        1280,
        720,
        "Low-Poly Procedural Worlds - MUX 0.4.0",
        AppState::default,
    )
    .disable_exit_key()
    .set_cursor_mode(CursorMode::Captured)
    .on_load(|_, _| {
        maximize_window();
    })
    .set_virtual_resolution(1280, 720)
    .set_style_file("style.gss")
    .set_fps(config::get_fps() as i32)
    // Semantic input actions
    .register_ui_action("pause", [KEY_P, KEY_ESCAPE])
    .register_action("randomize_seed", [KEY_R])
    .register_action("preset_1", [KEY_ONE])
    .register_action("preset_2", [KEY_TWO])
    .register_action("preset_3", [KEY_THREE])
    .register_action("preset_4", [KEY_FOUR])
    .register_action("toggle_props", [KEY_T])
    .register_action("toggle_towns", [KEY_V])
    .register_action("toggle_landmarks", [KEY_K])
    .register_action("toggle_water", [KEY_O])
    .register_action("cycle_time", [KEY_L])
    .register_action("toggle_cam_mode", [KEY_F])
    .register_action("toggle_hud", [KEY_H])
    .register_action("beacon", [KEY_E])
    .register_action("reconnect", [KEY_M])
    .register_action("boost", [KEY_LEFT_SHIFT, KEY_RIGHT_SHIFT])
    // Continuous movement axes
    .register_axis("horizontal", KEY_A, KEY_D)
    .register_axis("vertical", KEY_W, KEY_S)
    .register_axis("elevation", KEY_SPACE, KEY_LEFT_CONTROL)
    // Continuous mouse look axes
    .register_mouse_axis("yaw", MouseAxis::X, -0.04)
    .register_mouse_axis("pitch", MouseAxis::Y, -0.04)
    // Register scenes in MUX router
    .add_scene("world", WorldScene::default())
    .add_scene("pause", PauseOverlayScene)
    .set_active_scene("world")
    .run();
}
