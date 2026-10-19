//! # lowpoly-procedural-worlds
//!
//! Monolithic facade crate providing unified access to procedural generation,
//! world simulation, low-poly rendering, and networking modules.

pub use lppw_noise as noise;
pub use lppw_world::{biomes, world};
pub use lppw_render::{camera, landmarks, props, sky, terrain, towns, water};
pub use lppw_net as network;

pub mod config;
