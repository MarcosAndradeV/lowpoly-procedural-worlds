# Low-Poly Procedural Worlds

A modular, multi-crate procedural 3D world generator, low-poly rendering engine, and multiplayer synchronization framework built in Rust.

## Workspace Architecture

The project is subdivided into focused, independently usable library crates and a facade executable crate:

| Crate | Path | Description | Dependencies |
|---|---|---|---|
| [`lppw-noise`](crates/lppw-noise) | `crates/lppw-noise` | Pure-Rust procedural noise primitives (fBm, ridged multifractal, Worley/cellular, Perlin, domain warping). | *None (zero dependencies)* |
| [`lppw-world`](crates/lppw-world) | `crates/lppw-world` | Headless procedural world simulation, biomes, topography presets, and settlement/landmark placement math with O(1) toroidal caching. | `lppw-noise`, `muxapp` |
| [`lppw-render`](crates/lppw-render) | `crates/lppw-render` | Low-poly 3D graphics presentation engine: chunked terrain with frustum culling, dynamic water/lava surfaces, atmospheric sky dome, props, and architectural models. | `lppw-noise`, `lppw-world`, `muxapp` |
| [`lppw-net`](crates/lppw-net) | `crates/lppw-net` | Multiplayer networking protocol, non-blocking TCP client, GSS state synchronization, beacon tracking, and 3D humanoid avatar rendering. | `lppw-world`, `lppw-render`, `muxapp` |
| [`lowpoly-procedural-worlds`](.) | Root | Umbrella facade re-exporting all sub-crates for backward compatibility, containing `lppw` client and `server` binaries. | All crates |

## Quick Start

### Running the Client

```bash
cargo run --bin lppw
```

### Running the Dedicated Multiplayer Server

```bash
cargo run --bin server
```

### Running Tests & Verification

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

## Using Individual Crates

### Procedural Noise (`lppw-noise`)

```toml
[dependencies]
lppw-noise = { path = "path/to/crates/lppw-noise" }
```

```rust
use lppw_noise::NoiseGenerator;

let noise = NoiseGenerator::new(4242);
let height = noise.domain_warp_fbm(12.5, 48.0, 100.0, 5, 0.002);
```

### Headless World Generation (`lppw-world`)

```toml
[dependencies]
lppw-world = { path = "path/to/crates/lppw-world" }
```

```rust
use lppw_world::{WorldConfig, WorldPreset};

let config = WorldConfig::new(1234, WorldPreset::AlpineHighlands);
let elevation = config.get_height(100.0, 250.0);
let moisture = config.get_moisture(100.0, 250.0);
```

### 3D Low-Poly Rendering (`lppw-render`)

```toml
[dependencies]
lppw-render = { path = "path/to/crates/lppw-render" }
```

```rust
use lppw_render::{ExplorerCamera, LowPolyTerrain, LowPolyWater, SkyDome};
```

### Multiplayer Networking (`lppw-net`)

```toml
[dependencies]
lppw-net = { path = "path/to/crates/lppw-net" }
```

```rust
use lppw_net::MultiplayerClient;

let mut client = MultiplayerClient::new("127.0.0.1:8080");
```
