use apothecarys_botany::genetics::PlantGenotype;
use apothecarys_botany::mesh_gen::{generate_plant_mesh, PlantMeshData};
use apothecarys_botany::phenotype::{express_phenotype, PlantPhenotype};
use apothecarys_botany::stat_mapping::genetics_to_effects;
use apothecarys_core::items::AlchemyEffect;
use rand::SeedableRng;

/// All data about a generated plant for preview purposes.
pub struct PlantPreviewData {
    pub seed: u64,
    pub genotype: PlantGenotype,
    pub phenotype: PlantPhenotype,
    pub mesh: PlantMeshData,
    pub alchemy_effects: Vec<AlchemyEffect>,
}

impl PlantPreviewData {
    /// Generate a complete plant preview from a seed value.
    pub fn from_seed(seed: u64) -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let phenotype = express_phenotype(&genotype);
        let mesh = generate_plant_mesh(&genotype, &mut rng);
        let alchemy_effects = genetics_to_effects(&genotype);

        Self {
            seed,
            genotype,
            phenotype,
            mesh,
            alchemy_effects,
        }
    }

    /// Print a summary of the plant's properties to stdout.
    pub fn print_summary(&self) {
        println!("=== Plant Preview (seed: {}) ===", self.seed);
        println!();

        println!("--- Phenotype ---");
        println!("  Branch angle:    {:.1}°", self.phenotype.branch_angle);
        println!("  Branch length:   {:.2}", self.phenotype.branch_length);
        println!("  Branch thickness:{:.3}", self.phenotype.branch_thickness);
        println!("  Complexity:      {} iterations", self.phenotype.axiom_complexity);
        println!("  Branching factor:{}", self.phenotype.branching_factor);
        println!();
        println!("  Leaf scale:      {:.2}", self.phenotype.leaf_scale);
        println!("  Leaves/segment:  {}", self.phenotype.leaves_per_segment);
        println!(
            "  Leaf color:      RGB({:.2}, {:.2}, {:.2})",
            self.phenotype.leaf_color.r, self.phenotype.leaf_color.g, self.phenotype.leaf_color.b
        );
        println!();
        println!("  Has flowers:     {}", self.phenotype.produces_flowers);
        if self.phenotype.produces_flowers {
            println!("  Petal count:     {}", self.phenotype.petal_count);
            println!("  Petal scale:     {:.2}", self.phenotype.petal_scale);
            println!(
                "  Petal color:     RGB({:.2}, {:.2}, {:.2})",
                self.phenotype.petal_color.r,
                self.phenotype.petal_color.g,
                self.phenotype.petal_color.b
            );
        }
        println!();
        println!("  Has fruit:       {}", self.phenotype.produces_fruit);
        if self.phenotype.produces_fruit {
            println!("  Fruit scale:     {:.2}", self.phenotype.fruit_scale);
            println!(
                "  Fruit color:     RGB({:.2}, {:.2}, {:.2})",
                self.phenotype.fruit_color.r,
                self.phenotype.fruit_color.g,
                self.phenotype.fruit_color.b
            );
        }

        println!();
        println!("--- Mesh Statistics ---");
        println!("  Stem segments:   {}", self.mesh.stem_segments.len());
        println!("  Vertices:        {}", self.mesh.vertex_count());
        println!("  Triangles:       {}", self.mesh.triangle_count());
        println!("  Leaf instances:  {}", self.mesh.leaf_instances.len());
        println!("  Flower instances:{}", self.mesh.flower_instances.len());
        println!("  Fruit instances: {}", self.mesh.fruit_instances.len());

        println!();
        println!("--- Alchemy Effects ---");
        for effect in &self.alchemy_effects {
            match effect {
                AlchemyEffect::Heal { amount } => println!("  Heal: {amount} HP"),
                AlchemyEffect::Damage { amount, damage_type } => {
                    println!("  Damage: {amount} ({damage_type:?})")
                }
                AlchemyEffect::Buff { effect, turns } => {
                    println!("  Buff: {effect:?} for {turns} turns")
                }
                AlchemyEffect::Cure { cures } => println!("  Cure: {cures:?}"),
                AlchemyEffect::StatBoost {
                    attribute,
                    amount,
                    turns,
                } => println!("  Stat Boost: {attribute:?} +{amount} for {turns} turns"),
            }
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plant_preview_from_seed() {
        let preview = PlantPreviewData::from_seed(42);
        assert!(preview.mesh.vertex_count() > 0);
        assert!(preview.mesh.triangle_count() > 0);
        assert!(!preview.alchemy_effects.is_empty());
    }

    #[test]
    fn test_different_seeds_produce_different_plants() {
        let p1 = PlantPreviewData::from_seed(1);
        let p2 = PlantPreviewData::from_seed(999);

        // Different seeds should produce different phenotypes
        assert_ne!(p1.phenotype, p2.phenotype);
    }

    #[test]
    fn test_same_seed_is_deterministic() {
        let p1 = PlantPreviewData::from_seed(42);
        let p2 = PlantPreviewData::from_seed(42);

        assert_eq!(p1.phenotype, p2.phenotype);
        assert_eq!(p1.mesh.stem_segments.len(), p2.mesh.stem_segments.len());
        assert_eq!(p1.mesh.vertex_count(), p2.mesh.vertex_count());
    }

    #[test]
    fn test_obj_export_from_preview() {
        let preview = PlantPreviewData::from_seed(42);
        let obj = preview.mesh.to_obj("plant_seed_42.mtl");

        assert!(obj.contains("# Plant mesh"));
        assert!(obj.contains("mtllib plant_seed_42.mtl"));
        assert!(obj.contains("v "));
        assert!(obj.contains("f "));
        assert!(obj.contains("g stems"));
        assert!(obj.contains("usemtl stem"));
    }

    #[test]
    fn test_mtl_export_from_preview() {
        let preview = PlantPreviewData::from_seed(42);
        let mtl = preview.mesh.to_mtl();

        assert!(mtl.contains("newmtl stem"));
        assert!(mtl.contains("Kd"));
        // Leaf material should be present since seed 42 generates leaves
        assert!(mtl.contains("newmtl leaf"));
    }

    #[test]
    fn test_obj_export_different_seeds() {
        for seed in [1, 100, 999, 12345] {
            let preview = PlantPreviewData::from_seed(seed);
            let mtl_name = format!("plant_seed_{seed}.mtl");
            let obj = preview.mesh.to_obj(&mtl_name);

            assert!(obj.contains("# Plant mesh"));
            assert!(obj.contains(&format!("mtllib {mtl_name}")));
            assert!(obj.contains("g stems"));
            assert!(obj.contains("usemtl stem"));
        }
    }
}
