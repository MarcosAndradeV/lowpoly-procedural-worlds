mod biomes;
mod camera;
mod noise;
mod props;
mod sky;
mod terrain;
mod water;
mod world;

use camera::ExplorerCamera;
use muxapp::muxui::d3::*;
use muxapp::muxui::*;
use muxapp::{App, Input, MouseAxis, Scene, SceneAction};
use props::PropManager;
use sky::SkyDome;
use terrain::LowPolyTerrain;
use water::LowPolyWater;
use world::{WorldConfig, WorldPreset};

/// Shared state across scenes in the MUX SceneRouter.
pub struct AppState {
    pub config: WorldConfig,
    pub randomize_requested: bool,
    pub preset_request: Option<WorldPreset>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: WorldConfig::default(),
            randomize_requested: false,
            preset_request: None,
        }
    }
}

/// Primary 3D exploration scene for navigating procedural low-poly worlds.
pub struct WorldScene {
    pub terrain: LowPolyTerrain,
    pub water: LowPolyWater,
    pub props: PropManager,
    pub sky: SkyDome,
    pub camera: ExplorerCamera,
    pub show_hud: bool,
}

impl Default for WorldScene {
    fn default() -> Self {
        Self {
            terrain: LowPolyTerrain::default(),
            water: LowPolyWater::default(),
            props: PropManager::default(),
            sky: SkyDome::default(),
            camera: ExplorerCamera::default(),
            show_hud: true,
        }
    }
}

impl Scene<AppState> for WorldScene {
    fn on_enter(&mut self, _ctx: &mut AppState, _style: &Style) {
        disable_cursor();
    }
    fn on_exit(&mut self, _ctx: &mut AppState) {
        enable_cursor();
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
        }

        if state.randomize_requested {
            state.randomize_requested = false;
            state.config.randomize_seed();
        }

        // Semantic action hotkeys
        if input.is_action_pressed("randomize_seed") {
            state.config.randomize_seed();
        }
        if input.is_action_pressed("preset_1") {
            state.config.set_preset(WorldPreset::Archipelago);
        }
        if input.is_action_pressed("preset_2") {
            state.config.set_preset(WorldPreset::AlpineHighlands);
        }
        if input.is_action_pressed("preset_3") {
            state.config.set_preset(WorldPreset::Canyonlands);
        }
        if input.is_action_pressed("preset_4") {
            state.config.set_preset(WorldPreset::AlienCrags);
        }
        if input.is_action_pressed("toggle_props") {
            self.props.enabled = !self.props.enabled;
        }
        if input.is_action_pressed("toggle_water") {
            self.water.animated = !self.water.animated;
        }
        if input.is_action_pressed("cycle_time") {
            self.sky.cycle_time();
        }
        if input.is_action_pressed("toggle_cam_mode") {
            self.camera.mode = self.camera.mode.toggle();
        }
        if input.is_action_pressed("toggle_hud") {
            self.show_hud = !self.show_hud;
        }

        // Update systems
        self.camera.update(input, &self.terrain, &state.config, dt);
        self.water.update(dt);
        self.sky.update(dt);

        SceneAction::None
    }

    fn draw_3d(&mut self, state: &AppState) {
        let cam = self.camera.to_camera_3d();
        let sun_dir = self.sky.time_of_day.sun_dir();
        let ambient = self.sky.time_of_day.ambient_light();
        let cam_pos = self.camera.position;

        draw_3d(cam, |d3| {
            // 1. Distant horizon dome & clouds
            self.sky.draw(d3, cam_pos.x, cam_pos.z);

            // 2. Low-poly terrain mesh
            self.terrain
                .draw(d3, &state.config, cam_pos.x, cam_pos.z, sun_dir, ambient);

            // 3. Procedural vegetation and rock props
            self.props.draw(
                d3,
                &self.terrain,
                &state.config,
                cam_pos.x,
                cam_pos.z,
                sun_dir,
                ambient,
            );

            // 4. Faceted animated ocean water
            self.water.draw(
                d3,
                state.config.water_level,
                cam_pos.x,
                cam_pos.z,
                sun_dir,
                ambient,
            );
        });
    }

    fn draw_2d(&mut self, state: &AppState, style: &Style) {
        if !self.show_hud {
            return;
        }

        // Top HUD Header
        TextElement::new("LOW-POLY PROCEDURAL WORLDS").place(style, "hud_title");

        let pos = self.camera.position;
        let biome = self.terrain.get_biome_at(&state.config, pos.x, pos.z);
        let fps = get_fps();

        let stats_text = format!(
            "FPS: {} | Cam: [{:.1}, {:.1}, {:.1}] | Mode: {}",
            fps,
            pos.x,
            pos.y,
            pos.z,
            self.camera.mode.name()
        );
        TextElement::new(stats_text).place(style, "hud_stats");

        let biome_text = format!(
            "Biome: {} | Sun: {} | Props: {}",
            biome.name(),
            self.sky.time_of_day.name(),
            if self.props.enabled { "ON" } else { "OFF" }
        );
        TextElement::new(biome_text).place(style, "hud_biome");

        let preset_text = format!(
            "Preset: {} | Seed: #{}",
            state.config.preset.name(),
            state.config.seed & 0xFFFFFF
        );
        TextElement::new(preset_text).place(style, "hud_preset_info");

        // Top-Right Preset Buttons
        let btn_arch = Button::new("1: Archipelago");
        let btn_alpine = Button::new("2: Alpine");
        let btn_canyon = Button::new("3: Canyon");
        let btn_alien = Button::new("4: Alien");
        let btn_rand = Button::new("R: Random Seed");

        StackLayout::new(&[
            ("btn_preset_arch", &btn_arch),
            ("btn_preset_alpine", &btn_alpine),
            ("btn_preset_canyon", &btn_canyon),
            ("btn_preset_alien", &btn_alien),
            ("btn_random_seed", &btn_rand),
        ])
        .place(style, "preset_button_row");

        if btn_arch.clicked() {
            unsafe {
                // Workaround to bypass immutable borrow of state in draw_2d
                let st = state as *const AppState as *mut AppState;
                (*st).config.set_preset(WorldPreset::Archipelago);
            }
        }
        if btn_alpine.clicked() {
            unsafe {
                let st = state as *const AppState as *mut AppState;
                (*st).config.set_preset(WorldPreset::AlpineHighlands);
            }
        }
        if btn_canyon.clicked() {
            unsafe {
                let st = state as *const AppState as *mut AppState;
                (*st).config.set_preset(WorldPreset::Canyonlands);
            }
        }
        if btn_alien.clicked() {
            unsafe {
                let st = state as *const AppState as *mut AppState;
                (*st).config.set_preset(WorldPreset::AlienCrags);
            }
        }
        if btn_rand.clicked() {
            unsafe {
                let st = state as *const AppState as *mut AppState;
                (*st).config.randomize_seed();
            }
        }

        // Bottom Controls Bar
        RectangleElement.place(style, "footer_bar");
        TextElement::new("WASD: Move | Mouse: Look | Shift: Boost | Ctrl/Space: Down/Up | F: Mode | R: Seed | 1-4: Presets | T: Trees | O: Waves | L: Sun | H: HUD | P: Pause").place(style, "footer_text");
    }
}

/// Modal overlay when simulation is paused.
#[derive(Default)]
pub struct PauseOverlayScene {
    resume: bool,
    randomize: bool,
}

impl Scene<AppState> for PauseOverlayScene {
    fn on_enter(&mut self, _ctx: &mut AppState, _style: &Style) {
        enable_cursor();
    }
    fn update(
        &mut self,
        state: &mut AppState,
        input: &Input,
        _style: &Style,
        _dt: f32,
    ) -> SceneAction {
        if self.randomize {
            self.resume = false;
            self.randomize = false;
            state.config.randomize_seed();
            return SceneAction::Pop;
        }
        if input.is_action_pressed("pause") || self.resume {
            self.resume = false;
            self.randomize = false;
            return SceneAction::Pop;
        }
        SceneAction::None
    }

    fn draw_2d(&mut self, _state: &AppState, style: &Style) {
        // Dark translucent backdrop
        draw_rectangle_rec(get_virtual_screen_rect(), get_color(0x0C121ACC));

        TextElement::new("EXPLORATION PAUSED").place(style, "pause_title");
        TextElement::new("Press [P] or [Esc] to resume exploring").place(style, "pause_subtitle");

        let resume_btn = Button::new("Resume World");
        let rand_btn = Button::new("Randomize Seed");

        StackLayout::new(&[("btn_resume", &resume_btn), ("btn_randomize", &rand_btn)])
            .place(style, "pause_btn_col");

        if resume_btn.clicked() {
            self.resume = true;
            return;
        }
        if rand_btn.clicked() {
            self.randomize = true;
            return;
        }
    }
}

fn main() {
    App::init(
        1280,
        720,
        "Low-Poly Procedural Worlds - MUX 0.3.0",
        AppState::default,
    )
    .on_load(|_, _| {
        maximize_window();
        unsafe {
            SetExitKey(0);
        }
    })
    .set_virtual_resolution(1280, 720)
    .set_style_file("style.gss")
    .set_fps(60)
    // Semantic input actions
    .register_action("pause", [KEY_P, KEY_ESCAPE])
    .register_action("randomize_seed", [KEY_R])
    .register_action("preset_1", [KEY_ONE])
    .register_action("preset_2", [KEY_TWO])
    .register_action("preset_3", [KEY_THREE])
    .register_action("preset_4", [KEY_FOUR])
    .register_action("toggle_props", [KEY_T])
    .register_action("toggle_water", [KEY_O])
    .register_action("cycle_time", [KEY_L])
    .register_action("toggle_cam_mode", [KEY_F])
    .register_action("toggle_hud", [KEY_H])
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
    .add_scene("pause", PauseOverlayScene::default())
    .set_active_scene("world")
    .run();
}
