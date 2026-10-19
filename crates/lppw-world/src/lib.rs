//! # lppw-world
//!
//! Procedural world simulation and spatial models for low-poly environments.
//! Contains biome classification, world generation configuration, elevation modeling,
//! and spatial placement models for settlements and landmarks.

pub mod biomes;
pub mod landmarks;
pub mod towns;
pub mod world;

pub use biomes::{calculate_facet_color, classify_biome, BiomeType};
pub use landmarks::{compute_landmark_for_cell, LandmarkSite, LandmarkType, LANDMARK_CELL_SIZE};
pub use towns::{
    can_connect_towns, compute_town_for_cell, evaluate_road_segment, road_meander_offset,
    sample_road_curve, SettlementSurface, TownSite, TOWN_CELL_SIZE,
};
pub use world::{WorldConfig, WorldPreset};
