use rand::Rng;
use serde::{Deserialize, Serialize};

/// Dominance type for gene expression.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Dominance {
    /// Higher allele value dominates
    Complete,
    /// Blended (average of both alleles)
    Incomplete,
    /// Both alleles expressed (sum, clamped to [0, 1])
    Codominant,
}

/// A single gene with diploid alleles (simplified Mendelian model).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gene {
    /// First allele, range [0.0, 1.0]
    pub allele_a: f32,
    /// Second allele, range [0.0, 1.0]
    pub allele_b: f32,
    /// How alleles combine during expression
    pub dominance: Dominance,
}

impl Gene {
    pub fn new(allele_a: f32, allele_b: f32, dominance: Dominance) -> Self {
        Self {
            allele_a: allele_a.clamp(0.0, 1.0),
            allele_b: allele_b.clamp(0.0, 1.0),
            dominance,
        }
    }

    /// Express the gene as a single phenotype value in [0.0, 1.0].
    pub fn express(&self) -> f32 {
        match self.dominance {
            Dominance::Complete => self.allele_a.max(self.allele_b),
            Dominance::Incomplete => (self.allele_a + self.allele_b) / 2.0,
            Dominance::Codominant => (self.allele_a + self.allele_b).min(1.0),
        }
    }

    /// Generate a random gene with the given dominance type.
    pub fn random(rng: &mut impl Rng, dominance: Dominance) -> Self {
        Self {
            allele_a: rng.gen_range(0.0..=1.0),
            allele_b: rng.gen_range(0.0..=1.0),
            dominance,
        }
    }
}

/// Mutation rate per allele during crossover.
pub const MUTATION_RATE: f64 = 0.05;
/// Maximum perturbation during mutation.
pub const MUTATION_RANGE: f32 = 0.1;

/// Cross two genes, selecting one allele from each parent with mutation chance.
fn cross_gene(a: &Gene, b: &Gene, rng: &mut impl Rng) -> Gene {
    let allele_a = if rng.gen_bool(0.5) {
        a.allele_a
    } else {
        a.allele_b
    };
    let allele_b = if rng.gen_bool(0.5) {
        b.allele_a
    } else {
        b.allele_b
    };

    let mutate = |v: f32, rng: &mut dyn FnMut() -> (bool, f32)| -> f32 {
        let (should_mutate, perturbation) = rng();
        if should_mutate {
            (v + perturbation).clamp(0.0, 1.0)
        } else {
            v
        }
    };

    let mut make_mutation = || -> (bool, f32) {
        (
            rng.gen_bool(MUTATION_RATE),
            rng.gen_range(-MUTATION_RANGE..MUTATION_RANGE),
        )
    };

    Gene {
        allele_a: mutate(allele_a, &mut make_mutation),
        allele_b: mutate(allele_b, &mut make_mutation),
        dominance: a.dominance.clone(),
    }
}

/// Full plant genotype with all genetic parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlantGenotype {
    // Morphology genes
    pub stem_height: Gene,
    pub stem_thickness: Gene,
    pub branching_angle: Gene,
    pub branching_density: Gene,
    pub internode_length: Gene,

    // Leaf genes
    pub leaf_size: Gene,
    pub leaf_shape: Gene,
    pub leaf_density: Gene,
    pub leaf_color_hue: Gene,
    pub leaf_color_saturation: Gene,

    // Flower genes
    pub has_flowers: Gene,
    pub petal_count: Gene,
    pub petal_color_hue: Gene,
    pub petal_size: Gene,
    pub flower_density: Gene,

    // Fruit genes
    pub has_fruit: Gene,
    pub fruit_size: Gene,
    pub fruit_color_hue: Gene,
    pub fruit_shape: Gene,

    // Alchemy genes (hidden from player)
    pub potency: Gene,
    pub healing_affinity: Gene,
    pub stat_target: Gene,
    pub duration_gene: Gene,
    pub toxicity: Gene,
}

impl PlantGenotype {
    /// Generate a random wild plant genotype.
    pub fn random_wild(rng: &mut impl Rng) -> Self {
        Self {
            stem_height: Gene::random(rng, Dominance::Incomplete),
            stem_thickness: Gene::random(rng, Dominance::Incomplete),
            branching_angle: Gene::random(rng, Dominance::Incomplete),
            branching_density: Gene::random(rng, Dominance::Complete),
            internode_length: Gene::random(rng, Dominance::Incomplete),

            leaf_size: Gene::random(rng, Dominance::Incomplete),
            leaf_shape: Gene::random(rng, Dominance::Complete),
            leaf_density: Gene::random(rng, Dominance::Incomplete),
            leaf_color_hue: Gene::random(rng, Dominance::Incomplete),
            leaf_color_saturation: Gene::random(rng, Dominance::Incomplete),

            has_flowers: Gene::random(rng, Dominance::Complete),
            petal_count: Gene::random(rng, Dominance::Complete),
            petal_color_hue: Gene::random(rng, Dominance::Codominant),
            petal_size: Gene::random(rng, Dominance::Incomplete),
            flower_density: Gene::random(rng, Dominance::Incomplete),

            has_fruit: Gene::random(rng, Dominance::Complete),
            fruit_size: Gene::random(rng, Dominance::Incomplete),
            fruit_color_hue: Gene::random(rng, Dominance::Codominant),
            fruit_shape: Gene::random(rng, Dominance::Complete),

            potency: Gene::random(rng, Dominance::Incomplete),
            healing_affinity: Gene::random(rng, Dominance::Incomplete),
            stat_target: Gene::random(rng, Dominance::Complete),
            duration_gene: Gene::random(rng, Dominance::Incomplete),
            toxicity: Gene::random(rng, Dominance::Incomplete),
        }
    }

    /// Cross two parent genotypes to produce offspring.
    pub fn crossover(parent_a: &PlantGenotype, parent_b: &PlantGenotype, rng: &mut impl Rng) -> PlantGenotype {
        PlantGenotype {
            stem_height: cross_gene(&parent_a.stem_height, &parent_b.stem_height, rng),
            stem_thickness: cross_gene(&parent_a.stem_thickness, &parent_b.stem_thickness, rng),
            branching_angle: cross_gene(&parent_a.branching_angle, &parent_b.branching_angle, rng),
            branching_density: cross_gene(&parent_a.branching_density, &parent_b.branching_density, rng),
            internode_length: cross_gene(&parent_a.internode_length, &parent_b.internode_length, rng),

            leaf_size: cross_gene(&parent_a.leaf_size, &parent_b.leaf_size, rng),
            leaf_shape: cross_gene(&parent_a.leaf_shape, &parent_b.leaf_shape, rng),
            leaf_density: cross_gene(&parent_a.leaf_density, &parent_b.leaf_density, rng),
            leaf_color_hue: cross_gene(&parent_a.leaf_color_hue, &parent_b.leaf_color_hue, rng),
            leaf_color_saturation: cross_gene(&parent_a.leaf_color_saturation, &parent_b.leaf_color_saturation, rng),

            has_flowers: cross_gene(&parent_a.has_flowers, &parent_b.has_flowers, rng),
            petal_count: cross_gene(&parent_a.petal_count, &parent_b.petal_count, rng),
            petal_color_hue: cross_gene(&parent_a.petal_color_hue, &parent_b.petal_color_hue, rng),
            petal_size: cross_gene(&parent_a.petal_size, &parent_b.petal_size, rng),
            flower_density: cross_gene(&parent_a.flower_density, &parent_b.flower_density, rng),

            has_fruit: cross_gene(&parent_a.has_fruit, &parent_b.has_fruit, rng),
            fruit_size: cross_gene(&parent_a.fruit_size, &parent_b.fruit_size, rng),
            fruit_color_hue: cross_gene(&parent_a.fruit_color_hue, &parent_b.fruit_color_hue, rng),
            fruit_shape: cross_gene(&parent_a.fruit_shape, &parent_b.fruit_shape, rng),

            potency: cross_gene(&parent_a.potency, &parent_b.potency, rng),
            healing_affinity: cross_gene(&parent_a.healing_affinity, &parent_b.healing_affinity, rng),
            stat_target: cross_gene(&parent_a.stat_target, &parent_b.stat_target, rng),
            duration_gene: cross_gene(&parent_a.duration_gene, &parent_b.duration_gene, rng),
            toxicity: cross_gene(&parent_a.toxicity, &parent_b.toxicity, rng),
        }
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
    fn test_gene_expression_complete() {
        let gene = Gene::new(0.3, 0.7, Dominance::Complete);
        assert!((gene.express() - 0.7).abs() < f32::EPSILON);

        let gene = Gene::new(0.9, 0.1, Dominance::Complete);
        assert!((gene.express() - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_gene_expression_incomplete() {
        let gene = Gene::new(0.2, 0.8, Dominance::Incomplete);
        assert!((gene.express() - 0.5).abs() < f32::EPSILON);

        let gene = Gene::new(0.6, 0.4, Dominance::Incomplete);
        assert!((gene.express() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_gene_expression_codominant() {
        let gene = Gene::new(0.3, 0.4, Dominance::Codominant);
        assert!((gene.express() - 0.7).abs() < f32::EPSILON);

        // Codominant clamps to 1.0
        let gene = Gene::new(0.8, 0.9, Dominance::Codominant);
        assert!((gene.express() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_gene_clamping() {
        let gene = Gene::new(-0.5, 1.5, Dominance::Incomplete);
        assert!((gene.allele_a - 0.0).abs() < f32::EPSILON);
        assert!((gene.allele_b - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_random_gene_in_range() {
        let mut rng = seeded_rng(42);
        for _ in 0..100 {
            let gene = Gene::random(&mut rng, Dominance::Incomplete);
            assert!((0.0..=1.0).contains(&gene.allele_a));
            assert!((0.0..=1.0).contains(&gene.allele_b));
        }
    }

    #[test]
    fn test_crossover_produces_valid_offspring() {
        let mut rng = seeded_rng(42);
        let parent_a = PlantGenotype::random_wild(&mut rng);
        let parent_b = PlantGenotype::random_wild(&mut rng);

        let child = PlantGenotype::crossover(&parent_a, &parent_b, &mut rng);

        // All alleles should be in valid range
        let check_gene = |g: &Gene| {
            assert!((0.0..=1.0).contains(&g.allele_a));
            assert!((0.0..=1.0).contains(&g.allele_b));
        };
        check_gene(&child.stem_height);
        check_gene(&child.potency);
        check_gene(&child.healing_affinity);
        check_gene(&child.toxicity);
    }

    #[test]
    fn test_crossover_inherits_dominance() {
        let mut rng = seeded_rng(42);
        let parent_a = PlantGenotype::random_wild(&mut rng);
        let parent_b = PlantGenotype::random_wild(&mut rng);

        let child = PlantGenotype::crossover(&parent_a, &parent_b, &mut rng);

        // Dominance should be inherited from parent_a
        assert_eq!(child.stem_height.dominance, parent_a.stem_height.dominance);
        assert_eq!(child.potency.dominance, parent_a.potency.dominance);
    }

    #[test]
    fn test_mendelian_ratios_complete_dominance() {
        // For Complete dominance with parent alleles (0.0, 1.0) x (0.0, 1.0):
        // Each child allele is randomly picked from parent's two alleles.
        // With complete dominance, express() = max(a, b).
        // Possible allele combos: (0,0), (0,1), (1,0), (1,1)
        // Expression: 0, 1, 1, 1 → ~75% express high, ~25% express low
        let mut rng = seeded_rng(123);
        let parent_a = Gene::new(0.0, 1.0, Dominance::Complete);
        let parent_b = Gene::new(0.0, 1.0, Dominance::Complete);

        let mut high_count = 0;
        let trials = 10000;
        for _ in 0..trials {
            let child = cross_gene(&parent_a, &parent_b, &mut rng);
            if child.express() > 0.5 {
                high_count += 1;
            }
        }

        let ratio = high_count as f64 / trials as f64;
        // Expected ~0.75 (3:1 Mendelian ratio), allow ±5%
        assert!(
            (0.70..=0.80).contains(&ratio),
            "Mendelian 3:1 ratio expected ~0.75, got {ratio}"
        );
    }

    #[test]
    fn test_mutation_rate_approximately_correct() {
        let mut rng = seeded_rng(999);
        // Use identical parents so any allele change must be from mutation
        let parent = Gene::new(0.5, 0.5, Dominance::Incomplete);

        let mut mutated = 0;
        let trials = 20000;
        for _ in 0..trials {
            let child = cross_gene(&parent, &parent, &mut rng);
            // Without mutation, alleles would be exactly 0.5
            if (child.allele_a - 0.5).abs() > f32::EPSILON {
                mutated += 1;
            }
            if (child.allele_b - 0.5).abs() > f32::EPSILON {
                mutated += 1;
            }
        }

        let total_alleles = trials * 2;
        let rate = mutated as f64 / total_alleles as f64;
        // Expected ~5%, allow ±2%
        assert!(
            (0.03..=0.07).contains(&rate),
            "Mutation rate expected ~0.05, got {rate}"
        );
    }

    #[test]
    fn test_crossover_deterministic_with_same_seed() {
        let mut rng1 = seeded_rng(42);
        let mut rng2 = seeded_rng(42);

        let pa1 = PlantGenotype::random_wild(&mut rng1);
        let pa2 = PlantGenotype::random_wild(&mut rng2);
        assert_eq!(pa1, pa2);

        let pb1 = PlantGenotype::random_wild(&mut rng1);
        let pb2 = PlantGenotype::random_wild(&mut rng2);

        let child1 = PlantGenotype::crossover(&pa1, &pb1, &mut rng1);
        let child2 = PlantGenotype::crossover(&pa2, &pb2, &mut rng2);
        assert_eq!(child1, child2);
    }

    #[test]
    fn test_genotype_serde_roundtrip() {
        let mut rng = seeded_rng(42);
        let genotype = PlantGenotype::random_wild(&mut rng);

        let json = serde_json::to_string(&genotype).unwrap();
        let deserialized: PlantGenotype = serde_json::from_str(&json).unwrap();
        assert_eq!(genotype, deserialized);
    }

    #[test]
    fn test_wild_plants_have_reasonable_distributions() {
        let mut rng = seeded_rng(42);
        let mut expressions = Vec::new();
        for _ in 0..1000 {
            let plant = PlantGenotype::random_wild(&mut rng);
            expressions.push(plant.potency.express());
        }

        let mean: f32 = expressions.iter().sum::<f32>() / expressions.len() as f32;
        let min = expressions.iter().cloned().fold(f32::MAX, f32::min);
        let max = expressions.iter().cloned().fold(f32::MIN, f32::max);

        // Mean should be roughly centered, min near 0, max near 1
        assert!(mean > 0.3 && mean < 0.7, "Mean potency {mean} not centered");
        assert!(min < 0.2, "Min potency {min} not low enough");
        assert!(max > 0.8, "Max potency {max} not high enough");
    }
}
