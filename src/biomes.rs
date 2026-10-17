use muxapp::muxui::Color;

/// Classification of low-poly terrain biomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiomeType {
    DeepOcean,
    Ocean,
    Beach,
    Plains,
    Forest,
    Desert,
    Canyon,
    Highland,
    MountainRock,
    SnowPeak,
    VolcanicBasalt,
}

impl BiomeType {
    /// Friendly display name for the HUD.
    pub fn name(&self) -> &'static str {
        match self {
            BiomeType::DeepOcean => "Deep Ocean",
            BiomeType::Ocean => "Coastal Ocean",
            BiomeType::Beach => "Sandy Beach",
            BiomeType::Plains => "Lush Plains",
            BiomeType::Forest => "Dense Forest",
            BiomeType::Desert => "Arid Desert",
            BiomeType::Canyon => "Red Canyon",
            BiomeType::Highland => "Alpine Highland",
            BiomeType::MountainRock => "Mountain Crags",
            BiomeType::SnowPeak => "Snow Peak",
            BiomeType::VolcanicBasalt => "Volcanic Basin",
        }
    }

    /// Primary surface color palette for the biome.
    pub fn base_color(&self) -> Color {
        match self {
            BiomeType::DeepOcean => Color { r: 16, g: 42, b: 76, a: 255 },
            BiomeType::Ocean => Color { r: 26, g: 78, b: 118, a: 255 },
            BiomeType::Beach => Color { r: 218, g: 198, b: 148, a: 255 },
            BiomeType::Plains => Color { r: 68, g: 154, b: 62, a: 255 },
            BiomeType::Forest => Color { r: 38, g: 108, b: 48, a: 255 },
            BiomeType::Desert => Color { r: 218, g: 165, b: 92, a: 255 },
            BiomeType::Canyon => Color { r: 184, g: 94, b: 62, a: 255 },
            BiomeType::Highland => Color { r: 96, g: 136, b: 78, a: 255 },
            BiomeType::MountainRock => Color { r: 124, g: 122, b: 128, a: 255 },
            BiomeType::SnowPeak => Color { r: 242, g: 246, b: 252, a: 255 },
            BiomeType::VolcanicBasalt => Color { r: 74, g: 62, b: 86, a: 255 },
        }
    }

    /// Color used when the face is steep (cliff face / bare rock).
    pub fn cliff_color(&self) -> Color {
        match self {
            BiomeType::Desert | BiomeType::Canyon => Color { r: 148, g: 72, b: 48, a: 255 },
            BiomeType::VolcanicBasalt => Color { r: 118, g: 82, b: 96, a: 255 },
            _ => Color { r: 104, g: 102, b: 108, a: 255 }, // Granite slate
        }
    }

    /// Determines whether trees or vegetation can spawn on this biome.
    pub fn allows_trees(&self) -> bool {
        matches!(self, BiomeType::Plains | BiomeType::Forest | BiomeType::Highland | BiomeType::Beach)
    }

    /// Determines whether rocks/boulders can spawn on this biome.
    pub fn allows_rocks(&self) -> bool {
        !matches!(self, BiomeType::DeepOcean | BiomeType::Ocean)
    }
}

#[inline]
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: (a.r as f32 + (b.r as f32 - a.r as f32) * t) as u8,
        g: (a.g as f32 + (b.g as f32 - a.g as f32) * t) as u8,
        b: (a.b as f32 + (b.b as f32 - a.b as f32) * t) as u8,
        a: 255,
    }
}

/// Evaluates the biome at given terrain parameters.
///
/// - `height`: terrain elevation in world units
/// - `water_level`: sea level elevation
/// - `moisture`: simulated rainfall / humidity in `[0.0, 1.0]`
/// - `steepness`: vertical alignment of normal `normal_y` in `[0.0, 1.0]` (1.0 = flat, 0.0 = vertical wall)
/// - `is_volcanic`: world flag for alien/volcanic presets
pub fn classify_biome(
    height: f32,
    water_level: f32,
    moisture: f32,
    steepness: f32,
    is_volcanic: bool,
) -> BiomeType {
    if is_volcanic {
        if height < water_level - 12.0 {
            return BiomeType::DeepOcean;
        } else if height < water_level {
            return BiomeType::Ocean;
        } else if steepness < 0.6 || height > water_level + 75.0 {
            return BiomeType::MountainRock;
        } else {
            return BiomeType::VolcanicBasalt;
        }
    }

    let rel_h = height - water_level;

    // Underwater
    if rel_h < -18.0 {
        return BiomeType::DeepOcean;
    } else if rel_h < 0.0 {
        return BiomeType::Ocean;
    }

    // Coastal Beach
    if rel_h < 3.5 && steepness > 0.5 {
        return BiomeType::Beach;
    }

    // High elevation zones
    if rel_h > 80.0 {
        return BiomeType::SnowPeak;
    } else if rel_h > 55.0 {
        return if steepness < 0.72 {
            BiomeType::MountainRock
        } else {
            BiomeType::Highland
        };
    }

    // Steep cliffs anywhere above sea level expose bedrock
    if steepness < 0.55 {
        if moisture < 0.35 {
            return BiomeType::Canyon;
        } else {
            return BiomeType::MountainRock;
        }
    }

    // Mid/Low elevations determined by moisture
    if rel_h > 35.0 {
        if moisture < 0.3 {
            BiomeType::Canyon
        } else if moisture > 0.6 {
            BiomeType::Forest
        } else {
            BiomeType::Highland
        }
    } else if moisture < 0.28 {
        BiomeType::Desert
    } else if moisture < 0.55 {
        BiomeType::Plains
    } else {
        BiomeType::Forest
    }
}

/// Applies directional lighting, elevation zone transitions, and slope-based facet color.
pub fn calculate_facet_color(
    biome: BiomeType,
    rel_h: f32,
    normal_y: f32,
    light_factor: f32,
    is_volcanic: bool,
) -> Color {
    if is_volcanic {
        let base = if rel_h < 0.0 {
            Color { r: 155, g: 38, b: 14, a: 255 }
        } else if rel_h < 5.5 {
            // Glowing magma-scorched shoreline rim blending into obsidian basalt
            let ember = Color { r: 225, g: 82, b: 24, a: 255 };
            let basalt = BiomeType::VolcanicBasalt.base_color();
            lerp_color(ember, basalt, rel_h / 5.5)
        } else if normal_y < 0.68 {
            let t = ((0.68 - normal_y) / 0.28).clamp(0.0, 1.0);
            lerp_color(
                BiomeType::VolcanicBasalt.base_color(),
                BiomeType::VolcanicBasalt.cliff_color(),
                t,
            )
        } else if rel_h > 65.0 {
            let t = ((rel_h - 65.0) / 30.0).clamp(0.0, 1.0);
            lerp_color(
                BiomeType::VolcanicBasalt.base_color(),
                Color { r: 145, g: 98, b: 112, a: 255 },
                t,
            )
        } else {
            BiomeType::VolcanicBasalt.base_color()
        };

        // Boost facet contrast slightly on dark volcanic stone so facets remain crisp at sunset
        let v_light = if (0.0..4.0).contains(&rel_h) {
            light_factor.max(0.78)
        } else {
            (light_factor * 1.22).clamp(0.32, 1.35)
        };

        return Color {
            r: (base.r as f32 * v_light).clamp(0.0, 255.0) as u8,
            g: (base.g as f32 * v_light).clamp(0.0, 255.0) as u8,
            b: (base.b as f32 * v_light).clamp(0.0, 255.0) as u8,
            a: 255,
        };
    }

    // Standard biome base color with smooth elevation-band transitions to prevent sawtooth edges
    let mut base = match biome {
        BiomeType::Beach => {
            let sand = BiomeType::Beach.base_color();
            let dune_grass = Color { r: 128, g: 166, b: 88, a: 255 };
            let t = ((rel_h - 1.2) / 2.6).clamp(0.0, 1.0);
            lerp_color(sand, dune_grass, t * 0.55)
        }
        BiomeType::Plains | BiomeType::Forest if rel_h < 7.0 => {
            let dune_grass = Color { r: 122, g: 162, b: 82, a: 255 };
            let t = ((rel_h - 3.2) / 3.8).clamp(0.0, 1.0);
            lerp_color(dune_grass, biome.base_color(), t)
        }
        BiomeType::Highland if rel_h > 46.0 => {
            let t = ((rel_h - 46.0) / 14.0).clamp(0.0, 1.0);
            lerp_color(
                BiomeType::Highland.base_color(),
                BiomeType::MountainRock.base_color(),
                t * 0.65,
            )
        }
        BiomeType::MountainRock if rel_h > 68.0 => {
            let t = ((rel_h - 68.0) / 14.0).clamp(0.0, 1.0);
            lerp_color(
                BiomeType::MountainRock.base_color(),
                BiomeType::SnowPeak.base_color(),
                t * 0.75,
            )
        }
        _ => biome.base_color(),
    };

    // Slope-based cliff stone blending
    if normal_y < 0.72 && biome.allows_trees() {
        let t = ((0.72 - normal_y) / 0.25).clamp(0.0, 1.0);
        base = lerp_color(base, biome.cliff_color(), t);
    }

    Color {
        r: (base.r as f32 * light_factor).clamp(0.0, 255.0) as u8,
        g: (base.g as f32 * light_factor).clamp(0.0, 255.0) as u8,
        b: (base.b as f32 * light_factor).clamp(0.0, 255.0) as u8,
        a: 255,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_underwater_classification() {
        assert_eq!(classify_biome(-25.0, 0.0, 0.5, 0.9, false), BiomeType::DeepOcean);
        assert_eq!(classify_biome(-5.0, 0.0, 0.5, 0.9, false), BiomeType::Ocean);
    }

    #[test]
    fn test_beach_classification() {
        assert_eq!(classify_biome(2.0, 0.0, 0.5, 0.9, false), BiomeType::Beach);
    }

    #[test]
    fn test_high_altitude_snow() {
        assert_eq!(classify_biome(95.0, 0.0, 0.5, 0.9, false), BiomeType::SnowPeak);
    }

    #[test]
    fn test_steep_slope_exposes_rock() {
        // Flat normal_y = 1.0 -> Plains
        assert_eq!(classify_biome(20.0, 0.0, 0.45, 0.95, false), BiomeType::Plains);
        // Vertical cliff normal_y = 0.3 -> MountainRock
        assert_eq!(classify_biome(20.0, 0.0, 0.45, 0.3, false), BiomeType::MountainRock);
    }
}
