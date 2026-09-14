use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::phenotype::PlantPhenotype;
use crate::surfaces::SurfaceId;

/// Symbols in the L-system alphabet.
///
/// The first block is the cpfg command set the earlier generator had. The
/// second is what #21 added when the turtle moved onto `plantgl`: a stem is no
/// longer a stack of cans but a single swept axis, which needs a symbol to
/// open and close, and organs are no longer marker instances but real
/// surfaces, which need a symbol that names one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LSymbol {
    /// F(length) — grow a stem segment
    Forward(f32),
    /// +(angle) — turn left around up axis
    TurnLeft(f32),
    /// -(angle) — turn right around up axis
    TurnRight(f32),
    /// ^(angle) — pitch up
    PitchUp(f32),
    /// &(angle) — pitch down
    PitchDown(f32),
    /// /(angle) — roll left
    RollLeft(f32),
    /// \(angle) — roll right
    RollRight(f32),
    /// [ — push turtle state
    Push,
    /// ] — pop turtle state
    Pop,
    /// L — place a leaf of the phenotype's own shape and scale.
    ///
    /// Kept so a string written before #21 — or stored in a save — still
    /// interprets; [`Surface`](LSymbol::Surface) is the general form, and
    /// [`LSystem::from_phenotype`] emits that instead.
    Leaf,
    /// W — place a flower of the phenotype's own petal count and scale.
    Flower,
    /// R — place a fruit of the phenotype's own shape and scale.
    Fruit,
    /// !(width) — set current stem width
    Width(f32),
    /// A — growth apex, replaced by production rules
    Apex,

    /// `startGC` — begin accumulating an axis to sweep in one piece. Between
    /// this and [`StopGC`](LSymbol::StopGC) a `Forward` records a ring rather
    /// than drawing a tube of its own, so a branch comes out as one
    /// continuous, mitred, tapering surface.
    ///
    /// Sweeps do not nest: a second `StartGC` inside an open one discards the
    /// axis accumulated so far. Branches need no second one — the turtle
    /// splits the sweep at every `Push`/`Pop` by itself.
    StartGC,
    /// `stopGC` — close the axis and draw it.
    StopGC,
    /// The stem profile to sweep, by index into
    /// [`surfaces::cross_section`](crate::surfaces::cross_section). Index 0 is
    /// the round default. Must precede [`StartGC`](LSymbol::StartGC): a sweep
    /// is drawn with the profile it was *opened* with.
    SetCrossSection(usize),
    /// How strongly each forward move bends towards gravity — the turtle's
    /// `setElasticity`. Zero switches tropism off.
    SetTropism(f32),
    /// Place a named surface from the plant's library, scaled. This is how a
    /// leaf, a petal or a fruit reaches the scene.
    Surface(SurfaceId, f32),
}

impl LSymbol {
    /// Check if this symbol matches a rule predecessor (ignoring parameters).
    fn matches_predecessor(&self, predecessor: &LSymbol) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(predecessor)
    }
}

/// A production rule that rewrites a symbol into a sequence of symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionRule {
    pub predecessor: LSymbol,
    pub successor: Vec<LSymbol>,
    /// Probability this rule fires (0.0–1.0). For stochastic rules.
    pub probability: f32,
}

/// A parametric, stochastic L-system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSystem {
    pub axiom: Vec<LSymbol>,
    pub rules: Vec<ProductionRule>,
}

/// How often a node puts out a flower alongside its growth.
const FLOWER_PROBABILITY: f32 = 0.18;
/// How often a node sets fruit alongside its growth.
const FRUIT_PROBABILITY: f32 = 0.12;
/// How far a leaf is pitched away from the axis it hangs off, in degrees.
const LEAF_PITCH: f32 = 55.0;
/// How far a petal is pitched away from the flower's axis, in degrees.
const PETAL_PITCH: f32 = 62.0;
/// How far a flower's or a fruit's stalk turns off the axis that bears it.
const PEDUNCLE_ANGLE: f32 = 55.0;

impl LSystem {
    /// Build an L-system from phenotype parameters.
    ///
    /// # What #21 changed
    ///
    /// **The axis continues.** The earlier rule ended every apex in two
    /// pushed branches, so each `Forward` stood alone between a `Push` and a
    /// `Pop` — which is exactly the shape that made stems look like stacked
    /// cans, and which one swept generalized cylinder per segment would not
    /// have fixed. The rule here grows one lateral inside `Push`/`Pop` and
    /// then *carries on* with a bare `Apex`, so consecutive `Forward`s land on
    /// the same open sweep and a branch is one continuous tapering surface.
    /// The apex count still doubles per step, so the derived string is the
    /// same size it always was.
    ///
    /// **The fertile rules come first, and no longer terminate.**
    /// `find_matching_rule` walks the matching rules accumulating their
    /// probabilities, so a rule behind one of probability 1.0 can never be
    /// reached: the flower and fruit rules were dead code, and no plant has
    /// ever grown either. They are ordered ahead of the growth rule now, which
    /// takes the remaining probability, and each hangs its organ off a stalk
    /// and then carries on with an `Apex` rather than ending the axis.
    ///
    /// **Organs are surfaces.** `Leaf`/`Flower`/`Fruit` were markers the old
    /// mesh builder turned into instance records. They are now
    /// [`LSymbol::Surface`] placements naming a template in the plant's
    /// library — a real blade with area a harvest yield can be read off.
    pub fn from_phenotype(phenotype: &PlantPhenotype) -> Self {
        let angle = phenotype.branch_angle;
        let length = phenotype.branch_length;
        let pitch_angle = angle * 0.5;
        // The angle between one lateral and the next, and between one leaf and
        // the next around a node: 180° for a two-ranked plant, 120°, 90°, 72°.
        let divergence = 360.0 / (phenotype.branching_factor + 1) as f32;

        let mut rules = Vec::new();
        let mut growth_probability = 1.0;

        // -- the fertile rules, ahead of the growth rule so they can be reached --
        //
        // Both hang their organ off a short stalk inside `Push`/`Pop` and then
        // carry on with an `Apex`, so a node that flowers is still a node that
        // grows. A rule that *ended* at the flower would cut the whole plant
        // short whenever it fired on the axiom's first apex, which is a fifth
        // of all plants: those came out as a single half-internode with a
        // blossom on it.

        if phenotype.produces_flowers {
            let petal = SurfaceId::Petal(phenotype.petal_mesh_index);
            let petals = phenotype.petal_count.max(1);
            let mut flower = vec![
                LSymbol::Push,
                LSymbol::TurnLeft(PEDUNCLE_ANGLE),
                LSymbol::Forward(length * 0.45),
            ];
            for i in 0..petals {
                flower.push(LSymbol::Push);
                flower.push(LSymbol::RollLeft(360.0 * i as f32 / petals as f32));
                flower.push(LSymbol::PitchDown(PETAL_PITCH));
                flower.push(LSymbol::Surface(petal, phenotype.petal_scale));
                flower.push(LSymbol::Pop);
            }
            flower.push(LSymbol::Pop);
            flower.push(LSymbol::Apex);

            rules.push(ProductionRule {
                predecessor: LSymbol::Apex,
                successor: flower,
                probability: FLOWER_PROBABILITY,
            });
            growth_probability -= FLOWER_PROBABILITY;
        }

        if phenotype.produces_fruit {
            rules.push(ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![
                    LSymbol::Push,
                    LSymbol::TurnRight(PEDUNCLE_ANGLE),
                    LSymbol::Forward(length * 0.3),
                    LSymbol::Surface(
                        SurfaceId::Fruit(phenotype.fruit_mesh_index),
                        phenotype.fruit_scale,
                    ),
                    LSymbol::Pop,
                    LSymbol::Apex,
                ],
                probability: FRUIT_PROBABILITY,
            });
            growth_probability -= FRUIT_PROBABILITY;
        }

        // -- the growth rule: one internode, its leaves, one lateral, carry on --

        let mut growth = vec![LSymbol::Forward(length)];

        let leaf = SurfaceId::Leaf(phenotype.leaf_mesh_index);
        for i in 0..phenotype.leaves_per_segment.max(1) {
            growth.push(LSymbol::Push);
            growth.push(LSymbol::RollLeft(divergence * i as f32));
            growth.push(LSymbol::PitchDown(LEAF_PITCH));
            growth.push(LSymbol::Surface(leaf, phenotype.leaf_scale));
            growth.push(LSymbol::Pop);
        }

        growth.push(LSymbol::Push);
        growth.push(LSymbol::RollLeft(divergence));
        growth.push(LSymbol::TurnLeft(angle));
        growth.push(LSymbol::PitchUp(pitch_angle));
        growth.push(LSymbol::Apex);
        growth.push(LSymbol::Pop);

        // The roll stays on the axis, so successive laterals spiral round it
        // instead of stacking in one plane.
        growth.push(LSymbol::RollLeft(divergence));
        growth.push(LSymbol::Apex);

        rules.push(ProductionRule {
            predecessor: LSymbol::Apex,
            successor: growth,
            probability: growth_probability,
        });

        LSystem {
            axiom: vec![
                LSymbol::SetCrossSection(phenotype.cross_section_index),
                LSymbol::SetTropism(phenotype.tropism_elasticity),
                LSymbol::Width(phenotype.branch_thickness),
                LSymbol::StartGC,
                LSymbol::Apex,
                LSymbol::StopGC,
            ],
            rules,
        }
    }

    /// Apply production rules for n iterations.
    pub fn derive(&self, iterations: u32, rng: &mut impl Rng) -> Vec<LSymbol> {
        let mut current = self.axiom.clone();
        for _ in 0..iterations {
            current = self.apply_rules(&current, rng);
        }
        current
    }

    fn apply_rules(&self, input: &[LSymbol], rng: &mut impl Rng) -> Vec<LSymbol> {
        let mut output = Vec::new();
        for symbol in input {
            if let Some(rule) = self.find_matching_rule(symbol, rng) {
                output.extend(rule.successor.clone());
            } else {
                output.push(symbol.clone());
            }
        }
        output
    }

    fn find_matching_rule<'a>(
        &'a self,
        symbol: &LSymbol,
        rng: &mut impl Rng,
    ) -> Option<&'a ProductionRule> {
        // Collect all matching rules
        let matching: Vec<&ProductionRule> = self
            .rules
            .iter()
            .filter(|r| symbol.matches_predecessor(&r.predecessor))
            .collect();

        if matching.is_empty() {
            return None;
        }

        // For stochastic rules, select probabilistically
        // First try stochastic selection
        let roll: f32 = rng.gen();
        let mut cumulative = 0.0;
        for rule in &matching {
            cumulative += rule.probability;
            if roll < cumulative {
                return Some(rule);
            }
        }

        // Fallback to last matching rule if probabilities don't sum to 1
        matching.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    type TestRng = rand::rngs::StdRng;

    fn seeded_rng(seed: u64) -> TestRng {
        TestRng::seed_from_u64(seed)
    }

    #[test]
    fn test_simple_deterministic_lsystem() {
        // Simple Fibonacci-like: A → AB, B → A
        let system = LSystem {
            axiom: vec![LSymbol::Apex],
            rules: vec![
                ProductionRule {
                    predecessor: LSymbol::Apex,
                    successor: vec![LSymbol::Apex, LSymbol::Leaf],
                    probability: 1.0,
                },
                ProductionRule {
                    predecessor: LSymbol::Leaf,
                    successor: vec![LSymbol::Apex],
                    probability: 1.0,
                },
            ],
        };

        let mut rng = seeded_rng(42);

        // Iteration 0: A
        let result = system.derive(0, &mut rng);
        assert_eq!(result.len(), 1);

        // Iteration 1: A → AL (length 2)
        let result = system.derive(1, &mut rng);
        assert_eq!(result.len(), 2);

        // Iteration 2: AL → ALA (length 3)
        let result = system.derive(2, &mut rng);
        assert_eq!(result.len(), 3);

        // Iteration 3: ALA → ALAAL (length 5)
        let result = system.derive(3, &mut rng);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_deterministic_with_same_seed() {
        let system = LSystem {
            axiom: vec![LSymbol::Apex],
            rules: vec![
                ProductionRule {
                    predecessor: LSymbol::Apex,
                    successor: vec![
                        LSymbol::Forward(1.0),
                        LSymbol::Push,
                        LSymbol::TurnLeft(30.0),
                        LSymbol::Apex,
                        LSymbol::Pop,
                        LSymbol::Push,
                        LSymbol::TurnRight(30.0),
                        LSymbol::Apex,
                        LSymbol::Pop,
                    ],
                    probability: 0.7,
                },
                ProductionRule {
                    predecessor: LSymbol::Apex,
                    successor: vec![LSymbol::Forward(0.5), LSymbol::Leaf],
                    probability: 0.3,
                },
            ],
        };

        let mut rng1 = seeded_rng(42);
        let mut rng2 = seeded_rng(42);

        let result1 = system.derive(3, &mut rng1);
        let result2 = system.derive(3, &mut rng2);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_iteration_count_affects_length() {
        let system = LSystem {
            axiom: vec![LSymbol::Apex],
            rules: vec![ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![
                    LSymbol::Forward(1.0),
                    LSymbol::Push,
                    LSymbol::Apex,
                    LSymbol::Pop,
                    LSymbol::Apex,
                ],
                probability: 1.0,
            }],
        };

        let mut rng = seeded_rng(42);
        let len1 = system.derive(1, &mut rng).len();
        let mut rng = seeded_rng(42);
        let len2 = system.derive(2, &mut rng).len();
        let mut rng = seeded_rng(42);
        let len3 = system.derive(3, &mut rng).len();

        assert!(len1 < len2);
        assert!(len2 < len3);
    }

    #[test]
    fn test_stochastic_rules_produce_variation() {
        let system = LSystem {
            axiom: vec![LSymbol::Apex],
            rules: vec![
                ProductionRule {
                    predecessor: LSymbol::Apex,
                    successor: vec![
                        LSymbol::Forward(1.0),
                        LSymbol::Push,
                        LSymbol::TurnLeft(30.0),
                        LSymbol::Apex,
                        LSymbol::Pop,
                        LSymbol::Apex,
                    ],
                    probability: 0.5,
                },
                ProductionRule {
                    predecessor: LSymbol::Apex,
                    successor: vec![LSymbol::Forward(0.5), LSymbol::Leaf],
                    probability: 0.5,
                },
            ],
        };

        // Run many seeds and collect output lengths — stochastic rules
        // should produce at least 2 distinct lengths across many runs
        let mut lengths = std::collections::HashSet::new();
        for seed in 0..50 {
            let mut rng = seeded_rng(seed);
            let result = system.derive(3, &mut rng);
            lengths.insert(result.len());
        }
        assert!(
            lengths.len() > 1,
            "Stochastic rules should produce varying output lengths across seeds, got {lengths:?}"
        );
    }

    #[test]
    fn test_from_phenotype_produces_valid_system() {
        use crate::genetics::PlantGenotype;
        use crate::phenotype::express_phenotype;

        let mut rng = seeded_rng(42);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let phenotype = express_phenotype(&genotype);
        let system = LSystem::from_phenotype(&phenotype);

        assert!(!system.axiom.is_empty());
        assert!(!system.rules.is_empty());

        let result = system.derive(3, &mut rng);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_from_phenotype_wraps_the_axis_in_one_sweep() {
        use crate::genetics::PlantGenotype;
        use crate::phenotype::express_phenotype;

        let mut rng = seeded_rng(42);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let phenotype = express_phenotype(&genotype);
        let system = LSystem::from_phenotype(&phenotype);

        // The profile and the tropism have to be set before the sweep opens:
        // a generalized cylinder is drawn with the parameters it was opened
        // with, so a `setCrossSection` after `startGC` would be ignored.
        let start = system
            .axiom
            .iter()
            .position(|s| matches!(s, LSymbol::StartGC))
            .expect("the axiom opens a sweep");
        let stop = system
            .axiom
            .iter()
            .position(|s| matches!(s, LSymbol::StopGC))
            .expect("the axiom closes it");
        assert!(start < stop);
        for setter in [
            LSymbol::SetCrossSection(phenotype.cross_section_index),
            LSymbol::SetTropism(phenotype.tropism_elasticity),
            LSymbol::Width(phenotype.branch_thickness),
        ] {
            let at = system.axiom.iter().position(|s| *s == setter);
            assert!(at.is_some_and(|at| at < start), "{setter:?} is not set before the sweep");
        }

        // Rewriting only ever touches the apex, so the wrapper survives every
        // derivation and no second sweep is ever opened inside it.
        let derived = system.derive(4, &mut rng);
        assert_eq!(
            derived.iter().filter(|s| matches!(s, LSymbol::StartGC)).count(),
            1
        );
        assert_eq!(
            derived.iter().filter(|s| matches!(s, LSymbol::StopGC)).count(),
            1
        );
    }

    #[test]
    fn test_derived_strings_stay_balanced() {
        use crate::genetics::PlantGenotype;
        use crate::phenotype::express_phenotype;

        for seed in 0..20u64 {
            let mut rng = seeded_rng(seed);
            let genotype = PlantGenotype::random_wild(&mut rng);
            let phenotype = express_phenotype(&genotype);
            let derived = LSystem::from_phenotype(&phenotype).derive(4, &mut rng);

            let mut depth = 0i32;
            for symbol in &derived {
                match symbol {
                    LSymbol::Push => depth += 1,
                    LSymbol::Pop => depth -= 1,
                    _ => {}
                }
                assert!(depth >= 0, "seed {seed} pops with nothing pushed");
            }
            assert_eq!(depth, 0, "seed {seed} leaves {depth} pushes unclosed");
        }
    }

    /// The flower and fruit rules sat behind a rule of probability 1.0 and so
    /// could never fire. Ordering them first is what makes them reachable.
    #[test]
    fn test_the_fertile_rules_are_reachable() {
        use crate::genetics::PlantGenotype;
        use crate::phenotype::express_phenotype;

        let mut saw_petal = false;
        let mut saw_fruit = false;
        for seed in 0..40u64 {
            let mut rng = seeded_rng(seed);
            let genotype = PlantGenotype::random_wild(&mut rng);
            let phenotype = express_phenotype(&genotype);
            let derived = LSystem::from_phenotype(&phenotype).derive(5, &mut rng);
            for symbol in &derived {
                match symbol {
                    LSymbol::Surface(SurfaceId::Petal(_), _) => saw_petal = true,
                    LSymbol::Surface(SurfaceId::Fruit(_), _) => saw_fruit = true,
                    _ => {}
                }
            }
        }
        assert!(saw_petal, "no seed in 0..40 produced a petal");
        assert!(saw_fruit, "no seed in 0..40 produced a fruit");
    }

    /// A fertile node still grows: the rules end in an apex, so a flower on
    /// the axiom's first apex no longer truncates the whole plant.
    #[test]
    fn test_a_flowering_node_keeps_growing() {
        use crate::genetics::PlantGenotype;
        use crate::phenotype::express_phenotype;

        let mut rng = seeded_rng(11);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let mut phenotype = express_phenotype(&genotype);
        phenotype.produces_flowers = true;
        phenotype.produces_fruit = true;

        for rule in &LSystem::from_phenotype(&phenotype).rules {
            assert_eq!(
                rule.successor.last(),
                Some(&LSymbol::Apex),
                "a rule that does not end in an apex cuts the axis short"
            );
        }
    }

    #[test]
    fn test_no_matching_rule_preserves_symbol() {
        let system = LSystem {
            axiom: vec![LSymbol::Forward(1.0), LSymbol::Leaf, LSymbol::Push],
            rules: vec![], // No rules at all
        };

        let mut rng = seeded_rng(42);
        let result = system.derive(5, &mut rng);
        assert_eq!(result.len(), 3);
        assert_eq!(result, system.axiom);
    }

    #[test]
    fn test_push_pop_preserved() {
        let system = LSystem {
            axiom: vec![LSymbol::Apex],
            rules: vec![ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![
                    LSymbol::Forward(1.0),
                    LSymbol::Push,
                    LSymbol::Apex,
                    LSymbol::Pop,
                ],
                probability: 1.0,
            }],
        };

        let mut rng = seeded_rng(42);
        let result = system.derive(2, &mut rng);

        let push_count = result.iter().filter(|s| matches!(s, LSymbol::Push)).count();
        let pop_count = result.iter().filter(|s| matches!(s, LSymbol::Pop)).count();
        assert_eq!(push_count, pop_count);
    }

    #[test]
    fn test_lsystem_serde_roundtrip() {
        let system = LSystem {
            axiom: vec![LSymbol::Apex],
            rules: vec![ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![LSymbol::Forward(1.0), LSymbol::Leaf],
                probability: 1.0,
            }],
        };

        let json = serde_json::to_string(&system).unwrap();
        let deserialized: LSystem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.axiom.len(), system.axiom.len());
        assert_eq!(deserialized.rules.len(), system.rules.len());
    }
}
