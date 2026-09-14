use apothecarys_botany::genetics::PlantGenotype;
use apothecarys_botany::interpret::{generate_plant_at, PlantModel};
use apothecarys_botany::lod::LodTier;
use apothecarys_botany::phenotype::{express_phenotype, PlantPhenotype};
use apothecarys_botany::stat_mapping::genetics_to_effects;
use apothecarys_core::items::AlchemyEffect;
use plantgl::codec::obj::ObjFiles;
use rand::SeedableRng;

/// All data about a generated plant for preview purposes.
pub struct PlantPreviewData {
    pub seed: u64,
    pub genotype: PlantGenotype,
    pub phenotype: PlantPhenotype,
    /// The `plantgl` scene the turtle drew, plus what it is made of.
    pub plant: PlantModel,
    pub alchemy_effects: Vec<AlchemyEffect>,
}

impl PlantPreviewData {
    /// Generate a complete plant preview from a seed value, at hub quality.
    pub fn from_seed(seed: u64) -> Self {
        Self::from_seed_at(seed, LodTier::Hub)
    }

    /// The same at a given quality tier, so the previewer can show what the
    /// distant and icon tiers actually look like.
    pub fn from_seed_at(seed: u64, lod: LodTier) -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let phenotype = express_phenotype(&genotype).with_lod(lod);
        let plant = generate_plant_at(&genotype, &mut rng, lod).expect("a plant");
        let alchemy_effects = genetics_to_effects(&genotype);

        Self {
            seed,
            genotype,
            phenotype,
            plant,
            alchemy_effects,
        }
    }

    /// Wavefront OBJ and MTL for the plant, written by `plantgl::codec`.
    pub fn to_obj(&self, mtl_filename: &str) -> ObjFiles {
        self.plant
            .to_obj(mtl_filename)
            .expect("a plant that drew can be exported")
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
        println!("  Cross-section:   profile {}", self.phenotype.cross_section_index);
        println!("  Taper:           {:?}", self.phenotype.taper_curve);
        println!("  Tropism:         elasticity {:.3}", self.phenotype.tropism_elasticity);
        println!("  Axis curvature:  {:.1}°/segment", self.phenotype.axis_curvature);
        println!();
        println!("  Leaf shape:      template {}", self.phenotype.leaf_mesh_index);
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
            println!("  Petal shape:     template {}", self.phenotype.petal_mesh_index);
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
            println!("  Fruit shape:     template {}", self.phenotype.fruit_mesh_index);
            println!("  Fruit scale:     {:.2}", self.phenotype.fruit_scale);
            println!(
                "  Fruit color:     RGB({:.2}, {:.2}, {:.2})",
                self.phenotype.fruit_color.r,
                self.phenotype.fruit_color.g,
                self.phenotype.fruit_color.b
            );
        }

        let stats = &self.plant.stats;
        println!();
        println!("--- Geometry ({:?} LOD) ---", self.plant.lod());
        println!("  Symbols:         {}", stats.symbol_count);
        println!("  Stem segments:   {}", stats.segment_count);
        println!("  Shapes:          {}", self.plant.scene.len());
        match self.plant.batches() {
            Ok(batches) => {
                let triangles: usize = batches.iter().map(|b| b.mesh.face_count()).sum();
                println!("  Draw calls:      {}", batches.len());
                println!(
                    "  Triangles:       {triangles} (budget {})",
                    self.plant.lod().triangle_budget()
                );
            }
            Err(e) => println!("  Triangles:       unavailable: {e}"),
        }
        println!("  Leaves:          {}", stats.leaf_count);
        println!("  Petals:          {}", stats.petal_count);
        println!("  Fruit:           {}", stats.fruit_count);

        println!();
        println!("--- Measurements ---");
        match self.plant.measures() {
            Ok(measures) => {
                println!("  Surface area:    {:.4}", measures.surface_area);
                println!("  Volume:          {:.6}", measures.volume);
                match measures.bbox {
                    Some(bbox) => {
                        let size = bbox.size();
                        println!(
                            "  Bounding box:    {:.2} x {:.2} x {:.2}  (height {:.2})",
                            size.x, size.y, size.z, size.y
                        );
                    }
                    None => println!("  Bounding box:    empty"),
                }
            }
            Err(e) => println!("  unavailable: {e}"),
        }

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
        assert!(!preview.plant.scene.is_empty());
        assert!(preview.plant.triangle_count().unwrap() > 0);
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
        assert_eq!(p1.plant.stats, p2.plant.stats);
        assert_eq!(p1.to_obj("x.mtl"), p2.to_obj("x.mtl"));
    }

    #[test]
    fn test_obj_export_from_preview() {
        let preview = PlantPreviewData::from_seed(42);
        let files = preview.to_obj("plant_seed_42.mtl");

        assert!(files.obj.contains("mtllib plant_seed_42.mtl"));
        assert!(files.obj.contains("v "));
        assert!(files.obj.contains("f "));
        assert!(files.obj.contains("usemtl stem"));
    }

    #[test]
    fn test_mtl_export_from_preview() {
        let preview = PlantPreviewData::from_seed(42);
        let files = preview.to_obj("plant_seed_42.mtl");

        assert!(files.mtl.contains("newmtl stem"));
        assert!(files.mtl.contains("Kd"));
        // Leaf material should be present since seed 42 generates leaves
        assert!(files.mtl.contains("newmtl leaf"));
    }

    #[test]
    fn test_obj_export_different_seeds() {
        for seed in [1, 100, 999, 12345] {
            let preview = PlantPreviewData::from_seed(seed);
            let mtl_name = format!("plant_seed_{seed}.mtl");
            let files = preview.to_obj(&mtl_name);

            assert!(files.obj.contains(&format!("mtllib {mtl_name}")));
            assert!(files.obj.contains("usemtl stem"));
        }
    }

    /// The measurements the previewer reports alongside the mesh statistics.
    #[test]
    fn test_preview_reports_real_measurements() {
        let preview = PlantPreviewData::from_seed(42);
        let measures = preview.plant.measures().unwrap();
        assert!(measures.surface_area > 0.0);
        assert!(measures.volume > 0.0);
        assert!(measures.bbox.is_some());
    }

    /// Every tier has to stay inside the budget the previewer prints.
    #[test]
    fn test_every_tier_previews_inside_its_budget() {
        for lod in LodTier::ALL {
            let preview = PlantPreviewData::from_seed_at(42, lod);
            let triangles = preview.plant.triangle_count().unwrap();
            assert!(
                triangles <= lod.triangle_budget(),
                "{lod:?}: {triangles} triangles over a budget of {}",
                lod.triangle_budget()
            );
        }
    }
}
