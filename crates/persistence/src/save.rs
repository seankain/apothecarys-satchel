use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

use apothecarys_core::items::ItemStack;
use apothecarys_core::stats::{Attributes, DerivedStats};
use apothecarys_garden::plots::{Garden, GardenPlot, PlotState};
use apothecarys_inventory::container::Inventory;
use apothecarys_inventory::crafting::RecipeBook;
use apothecarys_party::generation::{AppearanceData, Equipment, PartyClass, PartyMember, Personality};
use apothecarys_party::recruitment::RecruitmentPool;
use apothecarys_party::roster::Roster;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

/// Current save file format version.
pub const SAVE_VERSION: u32 = 1;

/// Magic bytes identifying an Apothecary's Satchel save file.
pub const SAVE_MAGIC: [u8; 4] = *b"APOT";

/// Trait for game systems that can save and load their state.
pub trait Saveable {
    type SaveData: Serialize + DeserializeOwned;

    /// Capture the current state for saving.
    fn save(&self) -> Self::SaveData;

    /// Restore state from save data.
    fn load(&mut self, data: Self::SaveData) -> Result<(), SaveError>;
}

/// Top-level save file wrapper with header, raw data bytes, and integrity check.
///
/// The `data` field stores the MessagePack-serialized `SaveData` as raw bytes
/// so that the checksum can be verified without re-serialization (which may not
/// produce identical bytes after a round-trip).
#[derive(Debug, Serialize, Deserialize)]
pub struct SaveFile {
    pub magic: [u8; 4],
    pub version: u32,
    pub timestamp: u64,
    pub play_time_seconds: f64,
    pub data: Vec<u8>,
    pub checksum: u32,
}

/// All persistent game state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SaveData {
    // World state
    pub current_location: String,
    pub player_position: [f32; 3],
    pub world_flags: HashMap<String, String>,
    pub visited_locations: HashSet<String>,

    // Player
    pub player: PlayerSaveData,

    // Party
    pub active_party: Vec<PartyMemberSaveData>,
    pub recruitment_pool: Vec<PartyMemberSaveData>,
    pub dead_members: Vec<String>,

    // Inventory
    pub inventory: InventorySaveData,
    pub known_recipes: Vec<String>,

    // Garden
    pub garden: GardenSaveData,

    // Dialogue state
    pub dialogue_variables: HashMap<String, DialogueVarValue>,

    // Dungeon state (if saved mid-dungeon)
    pub dungeon_state: Option<DungeonSaveData>,
}

/// Serializable dialogue variable value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DialogueVarValue {
    Bool(bool),
    Number(f64),
    String(String),
}

/// Player character save data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerSaveData {
    pub name: String,
    pub level: u32,
    pub xp: u32,
    pub attributes: Attributes,
    pub derived: DerivedStats,
}

/// Party member save data matching the PartyMember struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartyMemberSaveData {
    pub id: Uuid,
    pub name: String,
    pub class: PartyClass,
    pub level: u32,
    pub xp: u32,
    pub attributes: Attributes,
    pub derived: DerivedStats,
    pub personality: Personality,
    pub equipment: Equipment,
    pub appearance: AppearanceData,
    pub alive: bool,
    pub backstory: String,
}

impl From<&PartyMember> for PartyMemberSaveData {
    fn from(m: &PartyMember) -> Self {
        Self {
            id: m.id,
            name: m.name.clone(),
            class: m.class,
            level: m.level,
            xp: m.xp,
            attributes: m.attributes.clone(),
            derived: m.derived.clone(),
            personality: m.personality.clone(),
            equipment: m.equipment.clone(),
            appearance: m.appearance.clone(),
            alive: m.alive,
            backstory: m.backstory.clone(),
        }
    }
}

impl From<PartyMemberSaveData> for PartyMember {
    fn from(d: PartyMemberSaveData) -> Self {
        Self {
            id: d.id,
            name: d.name,
            class: d.class,
            level: d.level,
            xp: d.xp,
            attributes: d.attributes,
            derived: d.derived,
            personality: d.personality,
            equipment: d.equipment,
            appearance: d.appearance,
            alive: d.alive,
            backstory: d.backstory,
        }
    }
}

/// Inventory save data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InventorySaveData {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
}

impl From<&Inventory> for InventorySaveData {
    fn from(inv: &Inventory) -> Self {
        Self {
            slots: inv.slots.clone(),
            max_slots: inv.max_slots,
        }
    }
}

impl From<InventorySaveData> for Inventory {
    fn from(d: InventorySaveData) -> Self {
        Self {
            slots: d.slots,
            max_slots: d.max_slots,
        }
    }
}

/// Garden save data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GardenSaveData {
    pub plots: Vec<GardenPlotSaveData>,
    pub max_plots: usize,
}

/// Garden plot save data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GardenPlotSaveData {
    pub index: usize,
    pub state: PlotState,
}

impl From<&Garden> for GardenSaveData {
    fn from(g: &Garden) -> Self {
        Self {
            plots: g
                .plots
                .iter()
                .map(|p| GardenPlotSaveData {
                    index: p.index,
                    state: p.state.clone(),
                })
                .collect(),
            max_plots: g.max_plots,
        }
    }
}

impl From<GardenSaveData> for Garden {
    fn from(d: GardenSaveData) -> Self {
        Self {
            plots: d
                .plots
                .into_iter()
                .map(|p| GardenPlot {
                    index: p.index,
                    state: p.state,
                })
                .collect(),
            max_plots: d.max_plots,
        }
    }
}

/// Dungeon state when saved mid-exploration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DungeonSaveData {
    pub floor: u32,
    pub difficulty: u32,
    pub rooms_cleared: Vec<String>,
    pub current_room: String,
}

/// Errors that can occur during save/load operations.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] rmp_serde::encode::Error),
    #[error("Deserialization error: {0}")]
    Deserialize(#[from] rmp_serde::decode::Error),
    #[error("Invalid magic bytes")]
    InvalidMagic,
    #[error("Checksum mismatch (expected {expected}, got {actual})")]
    ChecksumMismatch { expected: u32, actual: u32 },
    #[error("Unsupported save version: {0}")]
    UnsupportedVersion(u32),
    #[error("Migration error: {0}")]
    Migration(String),
}

/// Input sources for collecting save data from game systems.
pub struct SaveSources<'a> {
    pub current_location: &'a str,
    pub player_position: [f32; 3],
    pub world_flags: &'a HashMap<String, String>,
    pub visited_locations: &'a HashSet<String>,
    pub player: &'a PlayerSaveData,
    pub roster: &'a Roster,
    pub recruitment_pool: &'a RecruitmentPool,
    pub dead_members: &'a [String],
    pub inventory: &'a Inventory,
    pub recipe_book: &'a RecipeBook,
    pub garden: &'a Garden,
    pub dialogue_variables: &'a HashMap<String, DialogueVarValue>,
    pub dungeon_state: Option<DungeonSaveData>,
}

/// Collect all game state into a SaveData struct.
pub fn collect_save_data(sources: &SaveSources<'_>) -> SaveData {
    SaveData {
        current_location: sources.current_location.to_string(),
        player_position: sources.player_position,
        world_flags: sources.world_flags.clone(),
        visited_locations: sources.visited_locations.clone(),
        player: sources.player.clone(),
        active_party: sources.roster.members.iter().map(PartyMemberSaveData::from).collect(),
        recruitment_pool: sources
            .recruitment_pool
            .candidates
            .iter()
            .map(PartyMemberSaveData::from)
            .collect(),
        dead_members: sources.dead_members.to_vec(),
        inventory: InventorySaveData::from(sources.inventory),
        known_recipes: sources
            .recipe_book
            .known_recipes
            .iter()
            .map(|r| r.id.clone())
            .collect(),
        garden: GardenSaveData::from(sources.garden),
        dialogue_variables: sources.dialogue_variables.clone(),
        dungeon_state: sources.dungeon_state.clone(),
    }
}

/// Serialize a SaveData into a complete SaveFile with header and checksum.
pub fn create_save_file(data: SaveData, play_time_seconds: f64) -> Result<Vec<u8>, SaveError> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Serialize the data portion to raw bytes for checksum computation
    let data_bytes = rmp_serde::to_vec(&data)?;
    let checksum = crc32fast::hash(&data_bytes);

    let save_file = SaveFile {
        magic: SAVE_MAGIC,
        version: SAVE_VERSION,
        timestamp,
        play_time_seconds,
        data: data_bytes,
        checksum,
    };

    let bytes = rmp_serde::to_vec(&save_file)?;
    Ok(bytes)
}

/// Write a save file to disk.
pub fn write_save_file(path: impl AsRef<Path>, data: SaveData, play_time: f64) -> Result<(), SaveError> {
    let bytes = create_save_file(data, play_time)?;

    // Create parent directories if needed
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, bytes)?;
    Ok(())
}

/// Number of manual save slots.
pub const MAX_MANUAL_SLOTS: usize = 3;

/// Get the path for a save slot.
pub fn save_slot_path(slot: usize) -> String {
    format!("saves/slot_{slot}.sav")
}

/// Get the path for the autosave.
pub fn autosave_path() -> String {
    "saves/autosave.sav".to_string()
}

/// Autosave trigger points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosaveTrigger {
    LocationTransition,
    CombatResolution,
    GardenAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn make_test_player() -> PlayerSaveData {
        PlayerSaveData {
            name: "Alchemist".to_string(),
            level: 5,
            xp: 1200,
            attributes: Attributes::default(),
            derived: DerivedStats::default(),
        }
    }

    fn make_test_save_data() -> SaveData {
        SaveData {
            current_location: "hub".to_string(),
            player_position: [1.0, 0.0, 2.0],
            world_flags: {
                let mut m = HashMap::new();
                m.insert("quest_started".to_string(), "true".to_string());
                m
            },
            visited_locations: {
                let mut s = HashSet::new();
                s.insert("hub".to_string());
                s.insert("garden".to_string());
                s
            },
            player: make_test_player(),
            active_party: Vec::new(),
            recruitment_pool: Vec::new(),
            dead_members: vec!["Fallen Hero".to_string()],
            inventory: InventorySaveData {
                slots: vec![None; 20],
                max_slots: 20,
            },
            known_recipes: vec!["heal_basic".to_string()],
            garden: GardenSaveData {
                plots: Vec::new(),
                max_plots: 8,
            },
            dialogue_variables: {
                let mut m = HashMap::new();
                m.insert("gold".to_string(), DialogueVarValue::Number(42.0));
                m
            },
            dungeon_state: None,
        }
    }

    #[test]
    fn test_save_file_creation() {
        let data = make_test_save_data();
        let bytes = create_save_file(data, 123.5).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_save_file_roundtrip() {
        let data = make_test_save_data();
        let bytes = create_save_file(data.clone(), 100.0).unwrap();

        let loaded: SaveFile = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(loaded.magic, SAVE_MAGIC);
        assert_eq!(loaded.version, SAVE_VERSION);
        let loaded_data: SaveData = rmp_serde::from_slice(&loaded.data).unwrap();
        assert_eq!(loaded_data, data);
    }

    #[test]
    fn test_checksum_validates() {
        let data = make_test_save_data();
        let bytes = create_save_file(data.clone(), 50.0).unwrap();

        // Verify we can load the file through the normal load path (which checks the checksum)
        let migrations = crate::versioning::MigrationChain::new();
        let result = crate::load::load_save_bytes(&bytes, &migrations);
        assert!(result.is_ok(), "Save file should pass checksum validation");
        assert_eq!(result.unwrap().data, data);
    }

    #[test]
    fn test_save_data_with_party_members() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;

        let mut rng = StdRng::seed_from_u64(42);
        let member = apothecarys_party::generation::generate_party_member(&mut rng, 3);
        let save_data = PartyMemberSaveData::from(&member);

        assert_eq!(save_data.name, member.name);
        assert_eq!(save_data.class, member.class);
        assert_eq!(save_data.level, member.level);

        // Round-trip
        let restored: PartyMember = save_data.into();
        assert_eq!(restored.name, member.name);
        assert_eq!(restored.class, member.class);
    }

    #[test]
    fn test_inventory_save_roundtrip() {
        use apothecarys_core::items::{Item, ItemType};

        let mut inv = Inventory::new(10);
        inv.add_item(Item::new("herb", "Herb", ItemType::Ingredient), 5);

        let save_data = InventorySaveData::from(&inv);
        let restored: Inventory = save_data.into();

        assert_eq!(restored.max_slots, inv.max_slots);
        assert_eq!(restored.get_count("herb"), 5);
    }

    #[test]
    fn test_garden_save_roundtrip() {
        let garden = Garden::new(4);
        let save_data = GardenSaveData::from(&garden);
        let restored: Garden = save_data.into();

        assert_eq!(restored.plots.len(), 4);
        assert_eq!(restored.max_plots, garden.max_plots);
    }

    #[test]
    fn test_write_and_read_save_file() {
        let data = make_test_save_data();
        let temp_dir = std::env::temp_dir().join("apothecarys_test_saves");
        let path = temp_dir.join("test_slot.sav");

        write_save_file(&path, data.clone(), 200.0).unwrap();
        assert!(path.exists());

        let bytes = std::fs::read(&path).unwrap();
        let loaded: SaveFile = rmp_serde::from_slice(&bytes).unwrap();
        let loaded_data: SaveData = rmp_serde::from_slice(&loaded.data).unwrap();
        assert_eq!(loaded_data, data);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_slot_paths() {
        assert_eq!(save_slot_path(0), "saves/slot_0.sav");
        assert_eq!(save_slot_path(2), "saves/slot_2.sav");
        assert_eq!(autosave_path(), "saves/autosave.sav");
    }

    #[test]
    fn test_dialogue_var_value_roundtrip() {
        let vars = vec![
            DialogueVarValue::Bool(true),
            DialogueVarValue::Number(42.5),
            DialogueVarValue::String("hello".to_string()),
        ];

        let serialized = rmp_serde::to_vec(&vars).unwrap();
        let deserialized: Vec<DialogueVarValue> = rmp_serde::from_slice(&serialized).unwrap();
        assert_eq!(vars, deserialized);
    }

    #[test]
    fn test_dungeon_save_data() {
        let dungeon = DungeonSaveData {
            floor: 3,
            difficulty: 5,
            rooms_cleared: vec!["room_a".to_string(), "room_b".to_string()],
            current_room: "room_c".to_string(),
        };

        let serialized = rmp_serde::to_vec(&dungeon).unwrap();
        let deserialized: DungeonSaveData = rmp_serde::from_slice(&serialized).unwrap();
        assert_eq!(dungeon, deserialized);
    }

    #[test]
    fn test_full_save_data_messagepack_roundtrip() {
        let data = make_test_save_data();
        let serialized = rmp_serde::to_vec(&data).unwrap();
        let deserialized: SaveData = rmp_serde::from_slice(&serialized).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_save_data_compact_size() {
        let data = make_test_save_data();
        let msgpack = rmp_serde::to_vec(&data).unwrap();
        let json = serde_json::to_string(&data).unwrap();
        // MessagePack should be more compact than JSON
        assert!(
            msgpack.len() < json.len(),
            "MessagePack ({}) should be smaller than JSON ({})",
            msgpack.len(),
            json.len()
        );
    }
}
