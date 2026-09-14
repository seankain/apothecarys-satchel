//! Level-of-detail tiers for generated plants.
//!
//! A plant is built three times over its life: at full quality in the hub and
//! the garden, coarsely for plots the player is only walking past, and coarser
//! still for an inventory icon. The tier decides three things at once:
//!
//! 1. **How many derivation steps the L-system takes.** This is the only lever
//!    with real leverage at the low end — tessellation density cannot take a
//!    315-leaf plant under 400 triangles, and fewer iterations can. Because
//!    [`LSystem::derive`](crate::lsystem::LSystem::derive) re-derives from the
//!    axiom and consumes the RNG in iteration order, a capped derivation is the
//!    *prefix* of the uncapped one for the same seed: the icon is a smaller
//!    version of the same plant, not a different plant.
//! 2. **How finely the turtle's tubes and the organ patches are sampled** —
//!    [`DiscretizeCtx`] and the surface library's Bézier strides.
//! 3. **Whether organs are drawn at all** — an icon is a silhouette.
//!
//! The budgets below are the ones in issue #21 and are asserted by
//! `tests/budget.rs`.

use plantgl::algo::discretize::DiscretizeCtx;
use serde::{Deserialize, Serialize};

/// Which quality tier a plant is being built at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LodTier {
    /// The plant the player stands next to: full derivation, smooth sweeps,
    /// real leaf surfaces.
    #[default]
    Hub,
    /// A plot across the garden: fewer branches, coarse sweeps, flat organs.
    Distant,
    /// An inventory icon or a map pip: the silhouette of the stem alone.
    Icon,
}

impl LodTier {
    /// Every tier, coarsest last.
    pub const ALL: [LodTier; 3] = [LodTier::Hub, LodTier::Distant, LodTier::Icon];

    /// The triangle budget from #21, which `tests/budget.rs` enforces.
    pub fn triangle_budget(self) -> usize {
        match self {
            LodTier::Hub => 12_000,
            LodTier::Distant => 1_500,
            LodTier::Icon => 400,
        }
    }

    /// The most derivation steps this tier takes, whatever the phenotype's
    /// `axiom_complexity` asks for.
    pub fn max_iterations(self) -> u32 {
        match self {
            LodTier::Hub => 6,
            LodTier::Distant => 5,
            LodTier::Icon => 3,
        }
    }

    /// Facets around a swept stem — `Turtle::set_section_resolution`, the
    /// single biggest lever on a stem's triangle count.
    pub fn section_resolution(self) -> u32 {
        match self {
            LodTier::Hub => 10,
            LodTier::Distant => 5,
            LodTier::Icon => 3,
        }
    }

    /// The `(u, v)` sample counts a leaf or petal patch is discretised at. A
    /// patch of `(u, v)` samples is `(u - 1) * (v - 1)` quads, so `(5, 3)` is
    /// sixteen triangles of blade.
    pub fn patch_strides(self) -> (u32, u32) {
        match self {
            LodTier::Hub => (5, 4),
            LodTier::Distant => (3, 2),
            LodTier::Icon => (2, 2),
        }
    }

    /// Slices and stacks for a fruit's body of revolution.
    pub fn fruit_resolution(self) -> (u8, u8) {
        match self {
            LodTier::Hub => (8, 6),
            LodTier::Distant => (4, 3),
            LodTier::Icon => (3, 3),
        }
    }

    /// Whether leaves, petals and fruit are drawn at all.
    pub fn draws_organs(self) -> bool {
        self != LodTier::Icon
    }

    /// The density everything without a stride of its own falls back to.
    pub fn discretize_ctx(self) -> DiscretizeCtx {
        DiscretizeCtx {
            slices: self.section_resolution().clamp(1, u8::MAX as u32) as u8,
            stacks: self.fruit_resolution().1,
            curve_samples: match self {
                LodTier::Hub => 14,
                LodTier::Distant => 6,
                LodTier::Icon => 4,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiers_are_ordered_coarsest_last() {
        let mut previous = usize::MAX;
        for tier in LodTier::ALL {
            assert!(
                tier.triangle_budget() < previous,
                "{tier:?} is not coarser than the tier before it"
            );
            previous = tier.triangle_budget();
        }
    }

    #[test]
    fn a_capped_derivation_never_exceeds_the_phenotypes_own() {
        for tier in LodTier::ALL {
            assert!(tier.max_iterations() <= LodTier::Hub.max_iterations());
            assert!(tier.max_iterations() >= 1);
        }
    }

    #[test]
    fn only_the_icon_drops_its_organs() {
        assert!(LodTier::Hub.draws_organs());
        assert!(LodTier::Distant.draws_organs());
        assert!(!LodTier::Icon.draws_organs());
    }

    #[test]
    fn the_discretize_context_follows_the_section_resolution() {
        for tier in LodTier::ALL {
            let ctx = tier.discretize_ctx();
            assert_eq!(ctx.slices as u32, tier.section_resolution());
            assert!(ctx.curve_samples >= 4);
        }
    }
}
