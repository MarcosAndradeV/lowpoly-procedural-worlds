# LPPW Noise (`lppw-noise`)

Deterministic, pure-Rust procedural noise algorithms with **zero external dependencies**.

Part of the **Low-Poly Procedural Worlds** library ecosystem.

## Features

- **Perlin Gradient Noise (`perlin2d`)**: Fast, smooth 2D continuous gradient noise.
- **Fractal Brownian Motion (`fbm2d`)**: Multi-octave fractal noise with configurable frequency, persistence, and lacunarity.
- **Ridged Multifractal (`ridged2d`)**: Sharp mountain ridges, terraced plateaus, and crags using inverted absolute noise.
- **Domain Warping (`domain_warp_fbm`)**: Coordinate distortion using noise layers for organic continents and meandering rivers.
- **Cellular / Worley Noise (`cellular2d`)**: Voronoi feature distance calculations for craters, rock textures, and biomes.
- **Fast 2D Hash (`hash2d`)**: Low-overhead 64-bit integer spatial hash for deterministic procedural macro-cell distribution.

## Usage

```rust
use lppw_noise::NoiseGenerator;

let noise = NoiseGenerator::new(4242);

// Evaluate smooth fBm
let elevation = noise.fbm2d(100.0, 200.0, 6, 0.0018, 2.0, 0.5);

// Sharp mountain ridges
let ridge = noise.ridged2d(100.0, 200.0, 5, 0.0012, 2.05, 0.52);

// Organic domain warped terrain
let continent = noise.domain_warp_fbm(100.0, 200.0, 120.0, 5, 0.0016);
```
