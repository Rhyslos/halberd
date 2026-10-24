//! What the memory budget allows: how many pictures and textures Halberd
//! keeps at once. The rest of the program's memory (the map, the game's
//! file lists) doesn't depend on it.

use crate::Settings;

/// How much Halberd keeps, from the memory budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budgets {
    /// Pictures kept by each of the material and prop browsers.
    pub browser_pictures: usize,
    /// Material textures kept on the graphics card for the Textured mode.
    pub material_textures: u32,
}

/// Budget (in megabytes) the standard amounts are for.
const STANDARD_MB: u64 = 2048;
/// Pictures per browser at the standard budget.
const STANDARD_PICTURES: u64 = 600;
/// Material textures at the standard budget (also the most the renderer
/// takes).
const STANDARD_TEXTURES: u64 = 256;

impl Budgets {
    /// Fewest pictures kept, however small the budget.
    pub const MIN_PICTURES: usize = 150;
    /// Most pictures kept, however big the budget.
    pub const MAX_PICTURES: usize = 4000;
    /// Fewest material textures kept.
    pub const MIN_TEXTURES: u32 = 64;
    /// Most material textures kept.
    pub const MAX_TEXTURES: u32 = 256;

    /// The amounts for a budget of `memory_mb` megabytes: in proportion to
    /// the standard 2 GB, within limits.
    pub fn for_memory(memory_mb: u32) -> Self {
        let mb = u64::from(memory_mb);
        let scaled = |standard: u64| standard * mb / STANDARD_MB;
        Self {
            browser_pictures: (scaled(STANDARD_PICTURES) as usize)
                .clamp(Self::MIN_PICTURES, Self::MAX_PICTURES),
            material_textures: (scaled(STANDARD_TEXTURES).min(u64::from(u32::MAX)) as u32)
                .clamp(Self::MIN_TEXTURES, Self::MAX_TEXTURES),
        }
    }
}

impl Settings {
    /// What the memory budget allows.
    pub fn budgets(&self) -> Budgets {
        Budgets::for_memory(self.performance.memory_budget_mb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_budget_keeps_the_standard_amounts() {
        let b = Settings::default().budgets();
        assert_eq!(b.browser_pictures, 600);
        assert_eq!(b.material_textures, 256);
    }

    #[test]
    fn budgets_scale_within_limits() {
        let small = Budgets::for_memory(Settings::MIN_MEMORY_MB);
        assert_eq!(small.browser_pictures, 150);
        assert_eq!(small.material_textures, 64);
        let big = Budgets::for_memory(Settings::MAX_MEMORY_MB);
        assert_eq!(big.browser_pictures, Budgets::MAX_PICTURES);
        assert_eq!(big.material_textures, Budgets::MAX_TEXTURES);
        let extreme = Budgets::for_memory(u32::MAX);
        assert_eq!(extreme.browser_pictures, Budgets::MAX_PICTURES);
        assert_eq!(Budgets::for_memory(1024).browser_pictures, 300);
    }
}
