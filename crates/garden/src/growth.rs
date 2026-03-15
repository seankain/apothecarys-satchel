use rand::Rng;

use crate::plots::{Garden, PlotState};

/// Configuration for growth simulation.
#[derive(Debug, Clone)]
pub struct GrowthConfig {
    /// Base growth per cycle when watered
    pub base_growth_rate: f32,
    /// Growth bonus from genetic vigor (multiplied by vigor gene expression)
    pub vigor_bonus: f32,
    /// Health boost per cycle when watered
    pub water_health_bonus: f32,
    /// Health penalty per cycle when not watered
    pub water_health_penalty: f32,
    /// Chance of pest damage per cycle (0.0 - 1.0)
    pub pest_chance: f64,
    /// Health damage from pests
    pub pest_damage: f32,
    /// Health threshold below which plant dies
    pub death_threshold: f32,
}

impl Default for GrowthConfig {
    fn default() -> Self {
        Self {
            base_growth_rate: 0.2,
            vigor_bonus: 0.1,
            water_health_bonus: 0.1,
            water_health_penalty: 0.2,
            pest_chance: 0.05,
            pest_damage: 0.15,
            death_threshold: 0.0,
        }
    }
}

/// Result of simulating one growth cycle for a single plot.
#[derive(Debug, Clone, PartialEq)]
pub enum GrowthEvent {
    /// Plant grew normally
    Grew { plot_index: usize, new_stage: f32 },
    /// Plant reached maturity
    Matured { plot_index: usize },
    /// Plant was damaged by pests
    PestDamage { plot_index: usize, health: f32 },
    /// Plant suffered from lack of water
    Dehydrated { plot_index: usize, health: f32 },
    /// Plant died
    Died { plot_index: usize },
}

/// Simulate one growth cycle for the entire garden.
pub fn simulate_growth_cycle(
    garden: &mut Garden,
    config: &GrowthConfig,
    rng: &mut impl Rng,
) -> Vec<GrowthEvent> {
    let mut events = Vec::new();

    for plot in &mut garden.plots {
        let plot_index = plot.index;

        if let PlotState::Planted {
            plant,
            growth_stage,
            watered,
            health,
        } = &mut plot.state
        {
            let was_mature = *growth_stage >= 1.0;

            // Watering effect on health
            if *watered {
                *health = (*health + config.water_health_bonus).min(1.0);
            } else {
                *health -= config.water_health_penalty;
                events.push(GrowthEvent::Dehydrated {
                    plot_index,
                    health: *health,
                });
            }

            // Pest chance
            if rng.gen_bool(config.pest_chance) {
                *health -= config.pest_damage;
                events.push(GrowthEvent::PestDamage {
                    plot_index,
                    health: *health,
                });
            }

            // Death check
            if *health <= config.death_threshold {
                events.push(GrowthEvent::Died { plot_index });
                // We'll clear dead plants after the loop
                continue;
            }

            // Growth (only if watered and not already mature)
            if *watered && !was_mature {
                // Vigor from stem_height gene as a proxy for genetic vigor
                let vigor = plant.genotype.stem_height.express();
                let growth = config.base_growth_rate + config.vigor_bonus * vigor;
                *growth_stage = (*growth_stage + growth).min(1.0);

                if *growth_stage >= 1.0 {
                    events.push(GrowthEvent::Matured { plot_index });
                } else {
                    events.push(GrowthEvent::Grew {
                        plot_index,
                        new_stage: *growth_stage,
                    });
                }
            }

            // Reset watered flag for next cycle
            *watered = false;
        }
    }

    // Clear dead plants
    for event in &events {
        if let GrowthEvent::Died { plot_index } = event {
            if let Some(plot) = garden.plots.get_mut(*plot_index) {
                plot.clear();
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plots::{Garden, PlantInstance};
    use apothecarys_botany::genetics::PlantGenotype;
    use rand::SeedableRng;

    fn make_plant() -> PlantInstance {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Test Plant")
    }

    fn setup_garden_with_watered_plant() -> Garden {
        let mut garden = Garden::new(4);
        let plant = make_plant();
        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();
        garden.get_plot_mut(0).unwrap().water().unwrap();
        garden
    }

    #[test]
    fn test_growth_cycle_watered() {
        let mut garden = setup_garden_with_watered_plant();
        let config = GrowthConfig {
            pest_chance: 0.0, // No pests for this test
            ..Default::default()
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let events = simulate_growth_cycle(&mut garden, &config, &mut rng);

        assert!(events.iter().any(|e| matches!(e, GrowthEvent::Grew { .. })));

        if let PlotState::Planted { growth_stage, .. } = &garden.get_plot(0).unwrap().state {
            assert!(*growth_stage > 0.0);
        } else {
            panic!("Expected planted state");
        }
    }

    #[test]
    fn test_growth_cycle_not_watered() {
        let mut garden = Garden::new(4);
        let plant = make_plant();
        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();
        // Don't water

        let config = GrowthConfig {
            pest_chance: 0.0,
            ..Default::default()
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let events = simulate_growth_cycle(&mut garden, &config, &mut rng);

        assert!(events
            .iter()
            .any(|e| matches!(e, GrowthEvent::Dehydrated { .. })));

        // Should not have grown
        if let PlotState::Planted {
            growth_stage,
            health,
            ..
        } = &garden.get_plot(0).unwrap().state
        {
            assert!((*growth_stage - 0.0).abs() < f32::EPSILON);
            assert!(*health < 1.0); // Health should have decreased
        }
    }

    #[test]
    fn test_plant_reaches_maturity() {
        let mut garden = setup_garden_with_watered_plant();
        let config = GrowthConfig {
            base_growth_rate: 0.5,
            vigor_bonus: 0.0,
            pest_chance: 0.0,
            ..Default::default()
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        // Run multiple cycles
        for _ in 0..5 {
            garden.water_all();
            simulate_growth_cycle(&mut garden, &config, &mut rng);
        }

        assert!(garden.get_plot(0).unwrap().state.is_mature());
    }

    #[test]
    fn test_pest_damage() {
        let mut garden = setup_garden_with_watered_plant();
        let config = GrowthConfig {
            pest_chance: 1.0, // Always pests
            pest_damage: 0.15,
            ..Default::default()
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let events = simulate_growth_cycle(&mut garden, &config, &mut rng);

        assert!(events
            .iter()
            .any(|e| matches!(e, GrowthEvent::PestDamage { .. })));
    }

    #[test]
    fn test_plant_death() {
        let mut garden = Garden::new(4);
        let plant = make_plant();
        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();
        // Set health very low
        if let PlotState::Planted { health, .. } = &mut garden.get_plot_mut(0).unwrap().state {
            *health = 0.1;
        }
        // Don't water

        let config = GrowthConfig {
            pest_chance: 0.0,
            water_health_penalty: 0.2, // Will drop below 0
            ..Default::default()
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let events = simulate_growth_cycle(&mut garden, &config, &mut rng);

        assert!(events.iter().any(|e| matches!(e, GrowthEvent::Died { .. })));
        assert!(garden.get_plot(0).unwrap().state.is_empty());
    }

    #[test]
    fn test_watered_flag_resets_after_cycle() {
        let mut garden = setup_garden_with_watered_plant();
        let config = GrowthConfig {
            pest_chance: 0.0,
            ..Default::default()
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        simulate_growth_cycle(&mut garden, &config, &mut rng);

        if let PlotState::Planted { watered, .. } = &garden.get_plot(0).unwrap().state {
            assert!(!watered, "Watered flag should reset after cycle");
        }
    }

    #[test]
    fn test_mature_plants_dont_keep_growing() {
        let mut garden = setup_garden_with_watered_plant();
        // Set to already mature
        if let PlotState::Planted { growth_stage, .. } =
            &mut garden.get_plot_mut(0).unwrap().state
        {
            *growth_stage = 1.0;
        }

        let config = GrowthConfig {
            pest_chance: 0.0,
            ..Default::default()
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        garden.water_all();
        let events = simulate_growth_cycle(&mut garden, &config, &mut rng);

        // Should not get a Grew or Matured event since already mature
        assert!(!events.iter().any(|e| matches!(
            e,
            GrowthEvent::Grew { .. } | GrowthEvent::Matured { .. }
        )));
    }

    #[test]
    fn test_empty_plots_produce_no_events() {
        let mut garden = Garden::new(4);
        let config = GrowthConfig::default();
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let events = simulate_growth_cycle(&mut garden, &config, &mut rng);
        assert!(events.is_empty());
    }
}
