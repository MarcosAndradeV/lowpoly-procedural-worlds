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
            BiomeType::DeepOcean => Color { r: 16, g: 38, b: 68, a: 255 },
            BiomeType::Ocean => Color { r: 24, g: 70, b: 105, a: 255 },
            BiomeType::Beach => Color { r: 226, g: 206, b: 154, a: 255 },
            BiomeType::Plains => Color { r: 68, g: 154, b: 62, a: 255 },
            BiomeType::Forest => Color { r: 38, g: 108, b: 48, a: 255 },
            BiomeType::Desert => Color { r: 218, g: 165, b: 92, a: 255 },
            BiomeType::Canyon => Color { r: 184, g: 94, b: 62, a: 255 },
            BiomeType::Highland => Color { r: 104, g: 138, b: 84, a: 255 },
            BiomeType::MountainRock => Color { r: 118, g: 116, b: 120, a: 255 },
            BiomeType::SnowPeak => Color { r: 242, g: 246, b: 252, a: 255 },
            BiomeType::VolcanicBasalt => Color { r: 35, g: 32, b: 38, a: 255 },
        }
    }

    /// Color used when the face is steep (cliff face / bare rock).
    pub fn cliff_color(&self) -> Color {
        match self {
            BiomeType::Desert | BiomeType::Canyon => Color { r: 148, g: 72, b: 48, a: 255 },
            BiomeType::VolcanicBasalt => Color { r: 22, g: 18, b: 24, a: 255 },
            _ => Color { r: 96, g: 94, b: 98, a: 255 }, // Dark granite slate
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
    } else {
        if moisture < 0.28 {
            BiomeType::Desert
        } else if moisture < 0.55 {
            BiomeType::Plains
        } else {
            BiomeType::Forest
        }
    }
}

/// Applies directional lighting and slope-based facet color.
pub fn calculate_facet_color(
    biome: BiomeType,
    normal_y: f32,
    light_factor: f32,
) -> Color {
    // If the slope is steep (normal_y < 0.68), blend toward the cliff stone color
    let base = if normal_y < 0.68 && biome.allows_trees() {
        let t = ((0.68 - normal_y) / 0.25).clamp(0.0, 1.0);
        let b = biome.base_color();
        let c = biome.cliff_color();
        Color {
            r: (b.r as f32 * (1.0 - t) + c.r as f32 * t) as u8,
            g: (b.g as f32 * (1.0 - t) + c.g as f32 * t) as u8,
            b: (b.b as f32 * (1.0 - t) + c.b as f32 * t) as u8,
            a: 255,
        }
    } else {
        biome.base_color()
    };

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
