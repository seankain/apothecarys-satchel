use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::phenotype::PlantPhenotype;

/// Symbols in the L-system alphabet.
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
    /// L — place a leaf
    Leaf,
    /// W — place a flower
    Flower,
    /// R — place a fruit
    Fruit,
    /// !(width) — set current stem width
    Width(f32),
    /// A — growth apex, replaced by production rules
    Apex,
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

impl LSystem {
    /// Build an L-system from phenotype parameters.
    pub fn from_phenotype(phenotype: &PlantPhenotype) -> Self {
        let angle = phenotype.branch_angle;
        let length = phenotype.branch_length;
        let thickness = phenotype.branch_thickness;
        let pitch_angle = angle * 0.5;

        let mut rules = Vec::new();

        // Main branching rule: Apex → stem + two branches with leaves
        let mut main_successor = vec![
            LSymbol::Width(thickness),
            LSymbol::Forward(length),
        ];

        // Add leaves along the stem based on leaf density
        for _ in 0..phenotype.leaves_per_segment {
            main_successor.push(LSymbol::Push);
            main_successor.push(LSymbol::RollLeft(90.0));
            main_successor.push(LSymbol::Leaf);
            main_successor.push(LSymbol::Pop);
        }

        // Left branch
        main_successor.push(LSymbol::Push);
        main_successor.push(LSymbol::TurnLeft(angle));
        main_successor.push(LSymbol::PitchUp(pitch_angle));
        main_successor.push(LSymbol::Apex);
        main_successor.push(LSymbol::Pop);

        // Right branch
        main_successor.push(LSymbol::Push);
        main_successor.push(LSymbol::TurnRight(angle));
        main_successor.push(LSymbol::PitchDown(pitch_angle * 0.5));
        main_successor.push(LSymbol::Apex);
        main_successor.push(LSymbol::Pop);

        rules.push(ProductionRule {
            predecessor: LSymbol::Apex,
            successor: main_successor,
            probability: 1.0,
        });

        // Add flower rule if plant produces flowers
        if phenotype.produces_flowers {
            rules.push(ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![
                    LSymbol::Width(thickness * 0.5),
                    LSymbol::Forward(length * 0.5),
                    LSymbol::Flower,
                ],
                probability: 0.3,
            });
        }

        // Add fruit rule if plant produces fruit
        if phenotype.produces_fruit {
            rules.push(ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![
                    LSymbol::Width(thickness * 0.5),
                    LSymbol::Forward(length * 0.3),
                    LSymbol::Fruit,
                ],
                probability: 0.2,
            });
        }

        LSystem {
            axiom: vec![LSymbol::Apex],
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
