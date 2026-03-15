use rand::Rng;

use apothecarys_botany::genetics::PlantGenotype;

use crate::plots::{Garden, GardenError, PlantInstance};

/// Result of a breeding operation.
#[derive(Debug, Clone)]
pub struct BreedingResult {
    pub child: PlantInstance,
    pub parent_a_name: String,
    pub parent_b_name: String,
}

/// Check if two plot indices are adjacent (within 1 position of each other).
fn plots_adjacent(a: usize, b: usize) -> bool {
    a.abs_diff(b) == 1
}

/// Breed two mature plants from adjacent plots, producing a child seed.
pub fn breed_plants(
    garden: &Garden,
    plot_a: usize,
    plot_b: usize,
    rng: &mut impl Rng,
) -> Result<BreedingResult, GardenError> {
    if !plots_adjacent(plot_a, plot_b) {
        return Err(GardenError::PlotsNotAdjacent(plot_a, plot_b));
    }

    let pa = garden.get_plot(plot_a)?;
    let pb = garden.get_plot(plot_b)?;

    if !pa.state.is_mature() {
        return Err(GardenError::PlantNotMatureForBreeding(plot_a));
    }
    if !pb.state.is_mature() {
        return Err(GardenError::PlantNotMatureForBreeding(plot_b));
    }

    let plant_a = pa.state.plant().unwrap();
    let plant_b = pb.state.plant().unwrap();

    let child_genotype = PlantGenotype::crossover(&plant_a.genotype, &plant_b.genotype, rng);

    let child_generation = plant_a.generation.max(plant_b.generation) + 1;
    let child_name = format!(
        "{} × {} Gen{}",
        plant_a.species_name, plant_b.species_name, child_generation
    );

    let child = PlantInstance::new_bred(
        child_genotype,
        child_name,
        child_generation,
        plant_a.id,
        plant_b.id,
    );

    Ok(BreedingResult {
        child,
        parent_a_name: plant_a.species_name.clone(),
        parent_b_name: plant_b.species_name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plots::{Garden, PlantInstance, PlotState};
    use apothecarys_botany::genetics::PlantGenotype;
    use rand::SeedableRng;

    fn make_mature_garden() -> Garden {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut garden = Garden::new(4);

        let plant_a = PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Alpha Herb");
        let plant_b = PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Beta Bloom");

        garden.get_plot_mut(0).unwrap().plant_seed(plant_a).unwrap();
        garden.get_plot_mut(1).unwrap().plant_seed(plant_b).unwrap();

        // Set both to mature
        for i in 0..2 {
            if let PlotState::Planted { growth_stage, .. } =
                &mut garden.get_plot_mut(i).unwrap().state
            {
                *growth_stage = 1.0;
            }
        }

        garden
    }

    #[test]
    fn test_breed_adjacent_mature_plants() {
        let garden = make_mature_garden();
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let result = breed_plants(&garden, 0, 1, &mut rng).unwrap();

        assert_eq!(result.parent_a_name, "Alpha Herb");
        assert_eq!(result.parent_b_name, "Beta Bloom");
        assert_eq!(result.child.generation, 1);
        assert!(result.child.parent_ids.is_some());
    }

    #[test]
    fn test_breed_non_adjacent_fails() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut garden = make_mature_garden();

        // Plant in plot 2 (not adjacent to plot 0)
        let plant_c =
            PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Gamma Grass");
        garden.get_plot_mut(2).unwrap().plant_seed(plant_c).unwrap();
        if let PlotState::Planted { growth_stage, .. } =
            &mut garden.get_plot_mut(2).unwrap().state
        {
            *growth_stage = 1.0;
        }

        let result = breed_plants(&garden, 0, 2, &mut rng);
        assert_eq!(result.unwrap_err(), GardenError::PlotsNotAdjacent(0, 2));
    }

    #[test]
    fn test_breed_immature_plant_fails() {
        let mut garden = make_mature_garden();

        // Make plot 1 immature again
        if let PlotState::Planted { growth_stage, .. } =
            &mut garden.get_plot_mut(1).unwrap().state
        {
            *growth_stage = 0.5;
        }

        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let result = breed_plants(&garden, 0, 1, &mut rng);
        assert_eq!(
            result.unwrap_err(),
            GardenError::PlantNotMatureForBreeding(1)
        );
    }

    #[test]
    fn test_breed_empty_plot_fails() {
        let garden = Garden::new(4);
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let result = breed_plants(&garden, 0, 1, &mut rng);
        assert_eq!(
            result.unwrap_err(),
            GardenError::PlantNotMatureForBreeding(0)
        );
    }

    #[test]
    fn test_child_inherits_generation() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut garden = Garden::new(4);

        // Create gen-2 plant and gen-0 plant
        let plant_a = PlantInstance::new_bred(
            PlantGenotype::random_wild(&mut rng),
            "Gen2 Plant",
            2,
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        );
        let plant_b = PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Wild Plant");

        garden.get_plot_mut(0).unwrap().plant_seed(plant_a).unwrap();
        garden.get_plot_mut(1).unwrap().plant_seed(plant_b).unwrap();

        for i in 0..2 {
            if let PlotState::Planted { growth_stage, .. } =
                &mut garden.get_plot_mut(i).unwrap().state
            {
                *growth_stage = 1.0;
            }
        }

        let result = breed_plants(&garden, 0, 1, &mut rng).unwrap();
        // Child should be max(2, 0) + 1 = 3
        assert_eq!(result.child.generation, 3);
    }

    #[test]
    fn test_breeding_follows_mendelian_inheritance() {
        let garden = make_mature_garden();

        // Breed many offspring and check alleles are valid
        for seed in 0..50 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let result = breed_plants(&garden, 0, 1, &mut rng).unwrap();
            let child = &result.child.genotype;

            // All alleles should be in [0, 1]
            assert!((0.0..=1.0).contains(&child.potency.allele_a));
            assert!((0.0..=1.0).contains(&child.potency.allele_b));
            assert!((0.0..=1.0).contains(&child.healing_affinity.allele_a));
        }
    }

    #[test]
    fn test_plots_adjacent_check() {
        assert!(plots_adjacent(0, 1));
        assert!(plots_adjacent(1, 0));
        assert!(plots_adjacent(5, 6));
        assert!(!plots_adjacent(0, 2));
        assert!(!plots_adjacent(0, 0)); // Same plot
    }
}
