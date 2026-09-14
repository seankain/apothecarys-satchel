//! Turtle interpretation: a derived L-system string becomes geometry.
//!
//! MIT, like the rest of `crates/botany`. This is the L-Py ↔ PlantGL boundary
//! upstream draws: the driver knows the alphabet and the phenotype, the turtle
//! knows the geometry, and nothing translated from PlantGL is pasted across.
//!
//! # What replaced `turtle.rs`
//!
//! The crate used to carry its own `Vec3`, its own axis-angle rotation and its
//! own `TurtleInterpreter`, which walked the string accumulating
//! `StemSegment`s and marker instances. All three are gone: the frame, the
//! rotations, the push/pop stack, the sweep accumulation and the surface
//! placement are [`plantgl::modelling::Turtle`]'s, and this module is the
//! `match` that drives it.
//!
//! The visible consequence is that a branch is now one
//! [`Extrusion`](plantgl::Extrusion) — a cross-section swept along the whole
//! axis under rotation-minimising frames, narrowing as it goes — rather than a
//! ring pair per internode with a hard seam at every node.
//!
//! # Which drawer
//!
//! [`interpret`] is generic over [`TurtleDrawer`], so the same string can be
//! turned into a [`Scene`] to export or edit, one merged mesh per appearance
//! for the renderer, or nothing but surface area and volume for a harvest
//! yield. [`build_scene`], [`build_batches`] and [`measure`] are the three
//! that get used.

use plantgl::algo::discretize::Discretizer;
use plantgl::algo::{merge_scene, tessellate};
use plantgl::codec::obj::{ObjFiles, ObjOptions};
use plantgl::math::{Real, Vec3};
use plantgl::modelling::{MeasureDrawer, Measures, MeshBatch, MeshDrawer, SceneDrawer};
use plantgl::scenegraph::{Appearance, AppearanceRef, Color3, Material, Shape};
use plantgl::{BoundingBox, Geometry, Result, Scene, Turtle, TurtleDrawer};
use rand::Rng;
use std::sync::{Arc, OnceLock};

use crate::genetics::PlantGenotype;
use crate::lod::LodTier;
use crate::lsystem::{LSymbol, LSystem};
use crate::phenotype::{express_phenotype, PlantColor, PlantPhenotype};
use crate::surfaces::{cross_section, organ_library, Organ, SurfaceId};

/// The direction a stem bends towards under its own weight. The game's world
/// is Y-up, so gravity is `-Y`; `tropism_elasticity` decides how far a segment
/// gives to it.
const GRAVITY: Vec3 = Vec3::new(0.0, -1.0, 0.0);

/// The four appearances a plant is drawn with — one per organ kind and one for
/// the stem.
///
/// They are `Arc`ed and shared by every shape of their kind, which is what
/// lets [`plantgl::merge_scene`] batch a whole plant into four draw calls:
/// grouping is by `Arc` identity, not by material equality.
#[derive(Debug, Clone)]
pub struct PlantMaterials {
    pub stem: AppearanceRef,
    pub leaf: AppearanceRef,
    pub petal: AppearanceRef,
    pub fruit: AppearanceRef,
}

impl PlantMaterials {
    /// The colours the phenotype expressed, plus a default bark brown for the
    /// stem, which no gene sets.
    pub fn from_phenotype(phenotype: &PlantPhenotype) -> Self {
        Self {
            stem: material("stem", PlantColor { r: 0.40, g: 0.26, b: 0.13, a: 1.0 }, 0.10),
            leaf: material("leaf", phenotype.leaf_color, 0.05),
            petal: material("petal", phenotype.petal_color, 0.20),
            fruit: material("fruit", phenotype.fruit_color, 0.15),
        }
    }

    fn for_organ(&self, organ: Organ) -> AppearanceRef {
        match organ {
            Organ::Leaf => self.leaf.clone(),
            Organ::Petal => self.petal.clone(),
            Organ::Fruit => self.fruit.clone(),
        }
    }
}

/// A named material of the given diffuse colour.
///
/// PlantGL stores an ambient colour and a diffuse *multiplier* whose product is
/// the diffuse colour; a multiplier of one makes the stored ambient the colour
/// itself, which keeps the round trip through OBJ's `Kd` exact.
fn material(name: &str, color: PlantColor, specular: Real) -> AppearanceRef {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let specular = channel(specular);
    Arc::new(Appearance::Material(Material {
        name: Some(name.to_string()),
        ambient: Color3::new(channel(color.r), channel(color.g), channel(color.b)),
        diffuse: 1.0,
        specular: Color3::new(specular, specular, specular),
        transparency: (1.0 - color.a).clamp(0.0, 1.0),
        ..Material::default()
    }))
}

/// What the interpreter counted on its way through the string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlantStats {
    /// How many `Forward`s were drawn — the amount of stem.
    pub segment_count: usize,
    pub leaf_count: usize,
    pub petal_count: usize,
    pub fruit_count: usize,
    /// How long the derived string was.
    pub symbol_count: usize,
}

/// Walks a derived string, driving `turtle`.
///
/// The turtle is left as the string leaves it — an unbalanced string will end
/// with an open sweep or a non-empty stack, which is the caller's business.
/// [`interpret_with`] sets one up, runs this, and closes it off.
pub fn interpret<D: TurtleDrawer>(
    turtle: &mut Turtle<D>,
    symbols: &[LSymbol],
    phenotype: &PlantPhenotype,
) -> Result<PlantStats> {
    let materials = PlantMaterials::from_phenotype(phenotype);
    let mut stats = PlantStats {
        symbol_count: symbols.len(),
        ..PlantStats::default()
    };
    let taper = phenotype.taper_curve.segment_ratio();
    let curvature = phenotype.axis_curvature;
    let draws_organs = phenotype.lod_tier.draws_organs();

    for symbol in symbols {
        match symbol {
            LSymbol::Forward(length) => {
                turtle.forward_tapered(*length, turtle.width() * taper)?;
                stats.segment_count += 1;
                if curvature.abs() > Real::EPSILON {
                    // Pitching after each segment is what makes an axis arc
                    // rather than run dead straight; tropism then pulls the
                    // arc back towards gravity.
                    turtle.down(curvature);
                }
            }
            LSymbol::TurnLeft(angle) => turtle.left(*angle),
            LSymbol::TurnRight(angle) => turtle.right(*angle),
            LSymbol::PitchUp(angle) => turtle.up(*angle),
            LSymbol::PitchDown(angle) => turtle.down(*angle),
            LSymbol::RollLeft(angle) => turtle.roll_left(*angle),
            LSymbol::RollRight(angle) => turtle.roll_right(*angle),
            LSymbol::Push => turtle.push(),
            LSymbol::Pop => turtle.pop()?,
            LSymbol::Width(width) => turtle.set_width(*width)?,
            LSymbol::StartGC => turtle.start_gc(),
            LSymbol::StopGC => turtle.stop_gc()?,
            LSymbol::SetCrossSection(index) => {
                let resolution = turtle.section_resolution();
                match cross_section(*index, resolution) {
                    Some(profile) => turtle.set_cross_section(profile, true),
                    None => turtle.set_default_cross_section(resolution),
                }
            }
            LSymbol::SetTropism(elasticity) => {
                turtle.set_tropism(GRAVITY);
                turtle.set_elasticity(*elasticity);
            }
            LSymbol::Surface(id, scale) => {
                if draws_organs {
                    place_surface(turtle, &materials, *id, *scale)?;
                    stats.count(id.organ());
                }
            }
            // The three marker symbols the alphabet had before #21. They mean
            // "an organ of the phenotype's own kind goes here".
            LSymbol::Leaf => {
                if draws_organs {
                    let id = SurfaceId::Leaf(phenotype.leaf_mesh_index);
                    place_surface(turtle, &materials, id, phenotype.leaf_scale)?;
                    stats.leaf_count += 1;
                }
            }
            LSymbol::Flower => {
                if draws_organs && phenotype.produces_flowers {
                    let id = SurfaceId::Petal(phenotype.petal_mesh_index);
                    let petals = phenotype.petal_count.max(1);
                    for i in 0..petals {
                        turtle.push();
                        turtle.roll_left(360.0 * i as Real / petals as Real);
                        turtle.down(62.0);
                        place_surface(turtle, &materials, id, phenotype.petal_scale)?;
                        turtle.pop()?;
                        stats.petal_count += 1;
                    }
                }
            }
            LSymbol::Fruit => {
                if draws_organs && phenotype.produces_fruit {
                    let id = SurfaceId::Fruit(phenotype.fruit_mesh_index);
                    place_surface(turtle, &materials, id, phenotype.fruit_scale)?;
                    stats.fruit_count += 1;
                }
            }
            // An apex that outlived the derivation is a bud, and draws nothing.
            LSymbol::Apex => {}
        }
    }

    Ok(stats)
}

impl PlantStats {
    fn count(&mut self, organ: Organ) {
        match organ {
            Organ::Leaf => self.leaf_count += 1,
            Organ::Petal => self.petal_count += 1,
            Organ::Fruit => self.fruit_count += 1,
        }
    }

    /// Leaves, petals and fruit together.
    pub fn organ_count(&self) -> usize {
        self.leaf_count + self.petal_count + self.fruit_count
    }
}

/// Draws one organ in its own appearance, putting the stem's back afterwards.
///
/// The appearance is swapped rather than pushed because a `Push`/`Pop` around
/// a surface would also split the open sweep, and an organ is not a branch.
fn place_surface<D: TurtleDrawer>(
    turtle: &mut Turtle<D>,
    materials: &PlantMaterials,
    id: SurfaceId,
    scale: Real,
) -> Result<()> {
    let previous = turtle.state().draw.custom_material.clone();
    turtle.set_custom_appearance(Some(materials.for_organ(id.organ())));
    let drawn = turtle.surface(id.name(), scale);
    turtle.set_custom_appearance(previous);
    drawn
}

/// Builds a turtle for `phenotype`, runs `symbols` through it, and hands back
/// the drawer with what it drew.
///
/// The turtle starts upright — heading `+Y`, the direction the game's plants
/// grow — at the phenotype's section resolution, with the organ library its
/// tier asks for and the stem's appearance already in hand, because a sweep is
/// drawn with the appearance it was *opened* with.
pub fn interpret_with<D: TurtleDrawer>(
    drawer: D,
    symbols: &[LSymbol],
    phenotype: &PlantPhenotype,
) -> Result<(D, PlantStats)> {
    let mut turtle = Turtle::upright(drawer);
    turtle.set_section_resolution(phenotype.lod_tier.section_resolution());
    *turtle.surfaces_mut() = organ_library(phenotype.lod_tier);
    turtle.set_custom_appearance(Some(PlantMaterials::from_phenotype(phenotype).stem));
    turtle.set_width(phenotype.branch_thickness)?;

    let stats = interpret(&mut turtle, symbols, phenotype)?;
    // Draws whatever an unbalanced string left open rather than dropping it.
    turtle.stop()?;
    Ok((turtle.into_drawer(), stats))
}

/// The string as a [`Scene`] of parametric shapes — inspectable, exportable,
/// re-tessellatable at another tier.
pub fn build_scene(symbols: &[LSymbol], phenotype: &PlantPhenotype) -> Result<(Scene, PlantStats)> {
    let (drawer, stats) = interpret_with(SceneDrawer::new(), symbols, phenotype)?;
    Ok((drawer.into_scene(), stats))
}

/// The string as one merged triangle set per appearance, skipping the scene
/// graph — the renderer's path.
pub fn build_batches(
    symbols: &[LSymbol],
    phenotype: &PlantPhenotype,
) -> Result<(Vec<MeshBatch>, PlantStats)> {
    let drawer = MeshDrawer::with_ctx(phenotype.lod_tier.discretize_ctx());
    let (drawer, stats) = interpret_with(drawer, symbols, phenotype)?;
    Ok((drawer.batches()?, stats))
}

/// Surface area, volume, bounding box and segment count, without building a
/// mesh at all — the harvest-yield path.
pub fn measure(symbols: &[LSymbol], phenotype: &PlantPhenotype) -> Result<(Measures, PlantStats)> {
    let (drawer, stats) = interpret_with(MeasureDrawer::new(), symbols, phenotype)?;
    Ok((drawer.measures(), stats))
}

/// One generated plant: the scene it was drawn into, and what it is made of.
#[derive(Debug, Clone)]
pub struct PlantModel {
    /// One [`Shape`](plantgl::Shape) per swept axis and per organ, each
    /// carrying the appearance of its kind.
    pub scene: Scene,
    /// The phenotype it was grown from, tier included.
    pub phenotype: PlantPhenotype,
    /// What the interpreter counted on the way.
    pub stats: PlantStats,
    /// The derived string the scene was drawn from, kept so the plant can be
    /// measured, or re-interpreted into another drawer, without re-deriving.
    pub symbols: Vec<LSymbol>,
    /// Filled in the first time [`PlantModel::measures`] is asked for.
    measured: OnceLock<Measures>,
}

impl PlantModel {
    /// A model from an already-drawn scene and the string it came from.
    pub fn new(
        scene: Scene,
        phenotype: PlantPhenotype,
        stats: PlantStats,
        symbols: Vec<LSymbol>,
    ) -> Self {
        Self {
            scene,
            phenotype,
            stats,
            symbols,
            measured: OnceLock::new(),
        }
    }

    /// Surface area, volume and bounding box, taken from the same command
    /// sequence by [`MeasureDrawer`] rather than read back off a mesh, and
    /// computed on first use.
    ///
    /// Going back to the turtle rather than to the mesh is deliberate. A
    /// turtle's tubes are drawn open — upstream never caps them — so their
    /// meshes bound no volume at all and [`plantgl::volume`] rightly refuses
    /// them, while the volume of the stem is exactly what a yield model wants.
    /// The drawer sums it as though the ends were closed.
    ///
    /// It is computed lazily because it is the *alternative* to building a
    /// mesh, not a step on the way to one: a garden being rendered never pays
    /// for it, and a harvest that only wants the numbers never builds a mesh.
    pub fn measures(&self) -> Result<Measures> {
        if let Some(measures) = self.measured.get() {
            return Ok(*measures);
        }
        let (measures, _) = measure(&self.symbols, &self.phenotype)?;
        let _ = self.measured.set(measures);
        Ok(measures)
    }

    /// The tier this plant was built at.
    pub fn lod(&self) -> LodTier {
        self.phenotype.lod_tier
    }

    /// A discretizer at this plant's tier.
    fn discretizer(&self) -> Discretizer {
        Discretizer::new(self.phenotype.lod_tier.discretize_ctx())
    }

    /// The scene merged into one triangle set per appearance — what the
    /// renderer wants, and what makes a plant four draw calls rather than one
    /// per leaf.
    pub fn batches(&self) -> Result<Vec<MeshBatch>> {
        let mut discretizer = self.discretizer();
        merge_scene(&self.scene, &mut discretizer)?
            .into_iter()
            .map(|batch| {
                Ok(MeshBatch {
                    appearance: batch.appearance,
                    mesh: tessellate(&batch.geometry)?,
                })
            })
            .collect()
    }

    /// Total triangles at this tier. This is the number
    /// [`LodTier::triangle_budget`] bounds.
    pub fn triangle_count(&self) -> Result<usize> {
        Ok(self
            .batches()?
            .iter()
            .map(|batch| batch.mesh.face_count())
            .sum())
    }

    /// Total surface area — the principled harvest-yield driver, tied to the
    /// phenotype the player can actually see.
    pub fn surface_area(&self) -> Result<Real> {
        Ok(self.measures()?.surface_area)
    }

    /// Total enclosed volume, stems counted as though their ends were capped.
    pub fn volume(&self) -> Result<Real> {
        Ok(self.measures()?.volume)
    }

    /// The box the whole plant sits in, or `None` if nothing was drawn.
    pub fn bbox(&self) -> Result<Option<BoundingBox>> {
        Ok(self.measures()?.bbox)
    }

    /// The height of the plant in world units — the `+Y` extent of its box,
    /// since the turtle starts upright.
    pub fn height(&self) -> Result<Real> {
        Ok(self.bbox()?.map(|b| b.size().y).unwrap_or(0.0))
    }

    /// The plant as a scene of explicit meshes, one shape per appearance.
    ///
    /// [`PlantModel::scene`] holds parametric shapes — an `Extrusion` per
    /// axis, a `BezierPatch` per blade — which is what makes it
    /// re-tessellatable, and what makes it unexportable: OBJ has no
    /// parametric geometry. This is the discretised form the codec wants.
    pub fn explicit_scene(&self) -> Result<Scene> {
        let shapes = self
            .batches()?
            .into_iter()
            .enumerate()
            .map(|(index, batch)| {
                let mut shape =
                    Shape::new(Geometry::from(batch.mesh).into_ref()).with_id(index as u32);
                shape.appearance = batch.appearance;
                shape
            })
            .collect();
        Ok(Scene::from_shapes(shapes))
    }

    /// Wavefront OBJ and the matching MTL, written by `plantgl::codec` rather
    /// than by hand.
    pub fn to_obj(&self, mtl_filename: &str) -> Result<ObjFiles> {
        plantgl::codec::obj::to_obj_with(
            &self.explicit_scene()?,
            &ObjOptions {
                mtllib: Some(mtl_filename.to_string()),
                ..ObjOptions::default()
            },
        )
    }
}

/// Genotype → phenotype → L-system → turtle → geometry, at hub quality.
///
/// Deterministic for a fixed `rng`: the only randomness is the L-system's
/// choice of production, and everything downstream of the derived string is a
/// pure function of it.
pub fn generate_plant_mesh(genotype: &PlantGenotype, rng: &mut impl Rng) -> Result<PlantModel> {
    generate_plant_at(genotype, rng, LodTier::Hub)
}

/// The same, at a given quality tier.
///
/// Because [`LSystem::derive`] re-derives from the axiom and draws in
/// iteration order, a tier that caps the derivation produces the *prefix* of
/// the taller plant for the same seed rather than a different plant.
pub fn generate_plant_at(
    genotype: &PlantGenotype,
    rng: &mut impl Rng,
    lod: LodTier,
) -> Result<PlantModel> {
    let phenotype = express_phenotype(genotype).with_lod(lod);
    generate_from_phenotype(&phenotype, rng)
}

/// Grows a plant from a phenotype that has already been expressed — the entry
/// point for a plant whose traits came from a save rather than from genes.
pub fn generate_from_phenotype(
    phenotype: &PlantPhenotype,
    rng: &mut impl Rng,
) -> Result<PlantModel> {
    let lsystem = LSystem::from_phenotype(phenotype);
    let symbols = lsystem.derive(phenotype.iterations(), rng);
    let (scene, stats) = build_scene(&symbols, phenotype)?;
    Ok(PlantModel::new(scene, phenotype.clone(), stats, symbols))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn seeded(seed: u64) -> rand::rngs::StdRng {
        rand::rngs::StdRng::seed_from_u64(seed)
    }

    fn plant(seed: u64, lod: LodTier) -> PlantModel {
        let mut rng = seeded(seed);
        let genotype = PlantGenotype::random_wild(&mut rng);
        generate_plant_at(&genotype, &mut rng, lod).expect("a plant")
    }

    #[test]
    fn a_plant_is_drawn_and_measured() {
        let model = plant(42, LodTier::Hub);
        assert!(!model.scene.is_empty(), "nothing was drawn");
        assert!(model.stats.segment_count > 0);
        assert!(model.triangle_count().unwrap() > 0);
        assert!(model.surface_area().unwrap() > 0.0);
        assert!(model.volume().unwrap() > 0.0);
        assert!(model.bbox().unwrap().is_some());
        assert!(model.height().unwrap() > 0.0);
    }

    /// The acceptance criterion from T8.10.
    #[test]
    fn generation_is_deterministic_for_a_seed() {
        for seed in [1, 42, 100, 999, 12345] {
            let first = plant(seed, LodTier::Hub);
            let second = plant(seed, LodTier::Hub);
            assert_eq!(first.stats, second.stats, "seed {seed} drew different counts");
            assert_eq!(
                first.to_obj("x.mtl").unwrap(),
                second.to_obj("x.mtl").unwrap(),
                "seed {seed} is not deterministic"
            );
        }
    }

    #[test]
    fn different_genotypes_give_different_plants() {
        let mut rng = seeded(42);
        let a = PlantGenotype::random_wild(&mut rng);
        let b = PlantGenotype::random_wild(&mut rng);

        let first = generate_plant_mesh(&a, &mut seeded(100)).unwrap();
        let second = generate_plant_mesh(&b, &mut seeded(100)).unwrap();
        assert_ne!(
            first.to_obj("x.mtl").unwrap(),
            second.to_obj("x.mtl").unwrap()
        );
    }

    /// The headline of #21: an axis is one continuous swept surface, not a can
    /// per internode. Four `Forward`s inside one sweep must give exactly one
    /// shape, and that shape must carry a ring per node.
    #[test]
    fn consecutive_segments_become_one_swept_axis() {
        let mut rng = seeded(1);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let mut phenotype = express_phenotype(&genotype);
        // Straight and untapered, so the only thing under test is the joining.
        phenotype.axis_curvature = 0.0;
        phenotype.tropism_elasticity = 0.0;

        let symbols = vec![
            LSymbol::Width(0.05),
            LSymbol::StartGC,
            LSymbol::Forward(0.5),
            LSymbol::Forward(0.5),
            LSymbol::Forward(0.5),
            LSymbol::Forward(0.5),
            LSymbol::StopGC,
        ];
        let (scene, stats) = build_scene(&symbols, &phenotype).unwrap();
        assert_eq!(stats.segment_count, 4);
        assert_eq!(scene.len(), 1, "four segments drew {} shapes", scene.len());
        assert_eq!(scene.iter().next().unwrap().geometry.type_name(), "Extrusion");

        // Five rings of `section_resolution` points: one per node plus the base.
        let mesh = plantgl::discretize(&scene.iter().next().unwrap().geometry).unwrap();
        let ring = phenotype.lod_tier.section_resolution() as usize;
        assert!(
            mesh.points().len() >= 5 * ring,
            "{} points for five rings of {ring}",
            mesh.points().len()
        );
    }

    /// The same, over real plants: once an axis is more than one internode
    /// long there have to be fewer sweeps than segments.
    #[test]
    fn a_grown_plant_has_fewer_sweeps_than_segments() {
        for seed in [42, 100, 999, 12345] {
            let model = plant(seed, LodTier::Hub);
            assert!(
                model.phenotype.iterations() >= 4,
                "seed {seed} is too short to test the joining"
            );
            let sweeps = model
                .scene
                .iter()
                .filter(|shape| shape.geometry.type_name() == "Extrusion")
                .count();
            assert!(sweeps > 0, "seed {seed} drew no swept axes");
            assert!(
                sweeps < model.stats.segment_count,
                "seed {seed}: {sweeps} sweeps for {} segments — the axes are not being joined",
                model.stats.segment_count
            );
        }
    }

    /// Merging by appearance is what keeps a plant to a handful of draw calls.
    #[test]
    fn a_plant_merges_into_at_most_one_batch_per_organ_kind() {
        let model = plant(42, LodTier::Hub);
        let batches = model.batches().unwrap();
        assert!(!batches.is_empty());
        assert!(
            batches.len() <= 4,
            "{} batches — one per organ kind plus the stem is the most there should be",
            batches.len()
        );
    }

    #[test]
    fn a_coarser_tier_draws_a_smaller_plant() {
        let hub = plant(42, LodTier::Hub).triangle_count().unwrap();
        let distant = plant(42, LodTier::Distant).triangle_count().unwrap();
        let icon = plant(42, LodTier::Icon).triangle_count().unwrap();
        assert!(hub > distant, "hub {hub} vs distant {distant}");
        assert!(distant > icon, "distant {distant} vs icon {icon}");
    }

    #[test]
    fn the_icon_tier_draws_no_organs() {
        let model = plant(42, LodTier::Icon);
        assert_eq!(model.stats.organ_count(), 0);
    }

    /// The three drawers see the same string, so their tallies must agree.
    #[test]
    fn the_drawers_agree_on_what_was_drawn() {
        let mut rng = seeded(42);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let phenotype = express_phenotype(&genotype);
        let lsystem = LSystem::from_phenotype(&phenotype);
        let symbols = lsystem.derive(phenotype.iterations(), &mut rng);

        let (scene, scene_stats) = build_scene(&symbols, &phenotype).unwrap();
        let (batches, batch_stats) = build_batches(&symbols, &phenotype).unwrap();
        let (measures, measure_stats) = measure(&symbols, &phenotype).unwrap();

        assert_eq!(scene_stats, batch_stats);
        assert_eq!(scene_stats, measure_stats);
        assert!(!scene.is_empty());
        assert!(!batches.is_empty());
        assert!(measures.surface_area > 0.0);
        assert_eq!(measures.segment_count, scene_stats.segment_count);
    }

    /// Flowers and fruit were unreachable before #21 reordered the rules.
    #[test]
    fn a_flowering_plant_actually_grows_petals() {
        let mut found_petals = false;
        let mut found_fruit = false;
        for seed in 0..40u64 {
            let mut rng = seeded(seed);
            let genotype = PlantGenotype::random_wild(&mut rng);
            let phenotype = express_phenotype(&genotype);
            let model = generate_from_phenotype(&phenotype, &mut rng).unwrap();
            found_petals |= phenotype.produces_flowers && model.stats.petal_count > 0;
            found_fruit |= phenotype.produces_fruit && model.stats.fruit_count > 0;
        }
        assert!(found_petals, "no seed in 0..40 grew a petal");
        assert!(found_fruit, "no seed in 0..40 set fruit");
    }

    #[test]
    fn the_legacy_marker_symbols_still_interpret() {
        let mut rng = seeded(3);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let mut phenotype = express_phenotype(&genotype);
        phenotype.produces_flowers = true;
        phenotype.produces_fruit = true;

        let symbols = vec![
            LSymbol::Width(0.05),
            LSymbol::StartGC,
            LSymbol::Forward(1.0),
            LSymbol::StopGC,
            LSymbol::Leaf,
            LSymbol::Flower,
            LSymbol::Fruit,
        ];
        let (scene, stats) = build_scene(&symbols, &phenotype).unwrap();
        assert_eq!(stats.leaf_count, 1);
        assert_eq!(stats.petal_count, phenotype.petal_count as usize);
        assert_eq!(stats.fruit_count, 1);
        assert!(!scene.is_empty());
    }

    /// A `Pop` with nothing pushed is reported, not silently absorbed.
    #[test]
    fn an_unbalanced_string_is_an_error() {
        let mut rng = seeded(1);
        let genotype = PlantGenotype::random_wild(&mut rng);
        let phenotype = express_phenotype(&genotype);
        let result = build_scene(&[LSymbol::Pop], &phenotype);
        assert!(result.is_err());
    }

    #[test]
    fn the_obj_export_names_every_material_it_uses() {
        let model = plant(42, LodTier::Hub);
        let files = model.to_obj("plant.mtl").unwrap();
        assert!(files.obj.contains("mtllib plant.mtl"));
        assert!(files.obj.contains("usemtl stem"));
        assert!(files.mtl.contains("newmtl stem"));
        if model.stats.leaf_count > 0 {
            assert!(files.obj.contains("usemtl leaf"));
            assert!(files.mtl.contains("newmtl leaf"));
        }
    }
}
