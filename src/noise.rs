/// Pure Rust deterministic procedural noise generator.
///
/// Implements 2D Perlin gradient noise, Fractal Brownian Motion (fBm),
/// ridged multifractals, domain warping, and cellular (Worley) noise.

#[derive(Debug, Clone)]
pub struct NoiseGenerator {
    perm: [u8; 512],
}

impl NoiseGenerator {
    /// Constructs a new `NoiseGenerator` initialized with a given 64-bit seed.
    pub fn new(seed: u64) -> Self {
        let mut p = [0u8; 256];
        for i in 0..256 {
            p[i] = i as u8;
        }

        // Fisher-Yates shuffle with simple 64-bit LCG / XorShift RNG
        let mut state = seed ^ 0x5DEECE66D;
        for i in (1..256).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = ((state >> 32) as usize) % (i + 1);
            p.swap(i, j);
        }

        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }

        Self { perm }
    }

    #[inline(always)]
    fn fade(t: f32) -> f32 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    #[inline(always)]
    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + t * (b - a)
    }

    #[inline(always)]
    fn grad(hash: u8, x: f32, z: f32) -> f32 {
        // 8 gradient directions in 2D
        match hash & 7 {
            0 => x + z,
            1 => -x + z,
            2 => x - z,
            3 => -x - z,
            4 => x,
            5 => -x,
            6 => z,
            7 => -z,
            _ => 0.0,
        }
    }

    /// Evaluates raw 2D Perlin gradient noise at `(x, z)`.
    /// Returns a value in approximately `[-1.0, 1.0]`.
    #[inline]
    pub fn perlin2d(&self, x: f32, z: f32) -> f32 {
        let xi = x.floor() as i32 & 255;
        let zi = z.floor() as i32 & 255;

        let xf = x - x.floor();
        let zf = z - z.floor();

        let u = Self::fade(xf);
        let v = Self::fade(zf);

        let aa = self.perm[self.perm[xi as usize] as usize + zi as usize];
        let ab = self.perm[self.perm[xi as usize] as usize + zi as usize + 1];
        let ba = self.perm[self.perm[(xi + 1) as usize] as usize + zi as usize];
        let bb = self.perm[self.perm[(xi + 1) as usize] as usize + zi as usize + 1];

        let g_aa = Self::grad(aa, xf, zf);
        let g_ba = Self::grad(ba, xf - 1.0, zf);
        let g_ab = Self::grad(ab, xf, zf - 1.0);
        let g_bb = Self::grad(bb, xf - 1.0, zf - 1.0);

        let x1 = Self::lerp(g_aa, g_ba, u);
        let x2 = Self::lerp(g_ab, g_bb, u);

        Self::lerp(x1, x2, v)
    }

    /// Fractal Brownian Motion (fBm) combining multiple octaves of Perlin noise.
    ///
    /// - `octaves`: number of frequency layers (e.g. 4..8)
    /// - `freq`: base spatial frequency
    /// - `lacunarity`: frequency multiplier per octave (typically 2.0)
    /// - `persistence`: amplitude multiplier per octave (typically 0.5)
    #[inline]
    pub fn fbm2d(&self, x: f32, z: f32, octaves: usize, freq: f32, lacunarity: f32, persistence: f32) -> f32 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut max_amp = 0.0;
        let mut f = freq;

        for _ in 0..octaves {
            total += self.perlin2d(x * f, z * f) * amplitude;
            max_amp += amplitude;
            amplitude *= persistence;
            f *= lacunarity;
        }

        if max_amp > 0.0 {
            total / max_amp
        } else {
            0.0
        }
    }

    /// Ridged multifractal noise, creating sharp mountain ridges and crags.
    /// Inverts the absolute value of noise: `1.0 - abs(noise)`.
    #[inline]
    pub fn ridged2d(&self, x: f32, z: f32, octaves: usize, freq: f32, lacunarity: f32, persistence: f32) -> f32 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut weight = 1.0;
        let mut max_amp = 0.0;
        let mut f = freq;

        for _ in 0..octaves {
            let n = self.perlin2d(x * f, z * f).abs();
            let mut signal = 1.0 - n;
            signal *= signal; // sharpen ridges
            signal *= weight;
            weight = (signal * 2.0).clamp(0.0, 1.0); // weight subsequent octaves by signal

            total += signal * amplitude;
            max_amp += amplitude;
            amplitude *= persistence;
            f *= lacunarity;
        }

        if max_amp > 0.0 {
            total / max_amp
        } else {
            0.0
        }
    }

    /// Domain warping: perturbs the evaluation coordinates with noise before sampling fBm.
    /// Produces organic, swirling mountain ranges, canyons, and river courses.
    #[inline]
    pub fn domain_warp_fbm(&self, x: f32, z: f32, warp_strength: f32, octaves: usize, freq: f32) -> f32 {
        let qx = self.fbm2d(x, z, 3, freq * 1.5, 2.0, 0.5);
        let qz = self.fbm2d(x + 5.2, z + 1.3, 3, freq * 1.5, 2.0, 0.5);

        let rx = x + warp_strength * qx;
        let rz = z + warp_strength * qz;

        self.fbm2d(rx, rz, octaves, freq, 2.0, 0.5)
    }

    /// 2D Cellular / Worley distance noise.
    /// Computes distance to the nearest feature point in a randomized grid.
    /// Returns distance normalized roughly to `[0.0, 1.0]`.
    #[inline]
    pub fn cellular2d(&self, x: f32, z: f32) -> f32 {
        let xi = x.floor() as i32;
        let zi = z.floor() as i32;
        let xf = x - x.floor();
        let zf = z - z.floor();

        let mut min_dist = 10.0f32;

        for dx in -1..=1 {
            for dz in -1..=1 {
                let cell_x = (xi + dx) & 255;
                let cell_z = (zi + dz) & 255;
                let h1 = self.perm[(self.perm[cell_x as usize] as usize + cell_z as usize) & 511];
                let h2 = self.perm[(self.perm[cell_z as usize] as usize + cell_x as usize) & 511];

                // Point position in neighboring cell
                let px = dx as f32 + (h1 as f32 / 255.0);
                let pz = dz as f32 + (h2 as f32 / 255.0);

                let dist = ((px - xf) * (px - xf) + (pz - zf) * (pz - zf)).sqrt();
                if dist < min_dist {
                    min_dist = dist;
                }
            }
        }

        min_dist.clamp(0.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_is_deterministic() {
        let n1 = NoiseGenerator::new(12345);
        let n2 = NoiseGenerator::new(12345);
        for x in [-10.0, 0.0, 12.34, 100.5] {
            for z in [-20.0, 0.0, 56.78, 200.1] {
                assert_eq!(n1.perlin2d(x, z), n2.perlin2d(x, z));
                assert_eq!(n1.fbm2d(x, z, 4, 0.01, 2.0, 0.5), n2.fbm2d(x, z, 4, 0.01, 2.0, 0.5));
            }
        }
    }

    #[test]
    fn test_different_seeds_produce_different_values() {
        let n1 = NoiseGenerator::new(100);
        let n2 = NoiseGenerator::new(200);
        let val1 = n1.fbm2d(15.0, 25.0, 4, 0.01, 2.0, 0.5);
        let val2 = n2.fbm2d(15.0, 25.0, 4, 0.01, 2.0, 0.5);
        assert_ne!(val1, val2);
    }

    #[test]
    fn test_fbm_within_reasonable_bounds() {
        let n = NoiseGenerator::new(42);
        for x in (0..50).map(|i| i as f32 * 10.0) {
            for z in (0..50).map(|i| i as f32 * 10.0) {
                let v = n.fbm2d(x, z, 5, 0.005, 2.0, 0.5);
                assert!(v >= -1.5 && v <= 1.5, "fBm value out of expected range: {}", v);
            }
        }
    }

    #[test]
    fn test_cellular_distance_positive() {
        let n = NoiseGenerator::new(999);
        for x in [-5.0, 0.0, 5.0, 15.0] {
            for z in [-5.0, 0.0, 5.0, 15.0] {
                let d = n.cellular2d(x, z);
                assert!(d >= 0.0);
            }
        }
    }
}
