//! # lppw-render
//!
//! Low-poly 3D graphics presentation engine for procedural worlds.
//! Contains camera controls with frustum culling, atmospheric sky dome & clouds,
//! animated faceted water & lava surfaces, chunked terrain mesh generation with
//! toroidal caching, procedural vegetation props, and 3D settlement and landmark structures.

pub mod camera;
pub mod landmarks;
pub mod props;
pub mod sky;
pub mod terrain;
pub mod towns;
pub mod water;

pub use camera::{CameraMode, ExplorerCamera};
pub use landmarks::LandmarkManager;
pub use props::{PropGenerator, PropManager};
pub use sky::{SkyDome, TimeOfDay};
pub use terrain::LowPolyTerrain;
pub use towns::TownManager;
pub use water::LowPolyWater;
