use std::path::Path;

use crate::save::{SaveData, SaveError, SaveFile, SAVE_MAGIC, SAVE_VERSION};
use crate::versioning::MigrationChain;

/// Result of loading a save file.
#[derive(Debug)]
pub struct LoadResult {
    pub data: SaveData,
    pub timestamp: u64,
    pub play_time_seconds: f64,
    pub version: u32,
}

/// Load and verify a save file from raw bytes.
pub fn load_save_bytes(bytes: &[u8], migrations: &MigrationChain) -> Result<LoadResult, SaveError> {
    let save_file: SaveFile =
        rmp_serde::from_slice(bytes).map_err(SaveError::Deserialize)?;

    // Verify magic bytes
    if save_file.magic != SAVE_MAGIC {
        return Err(SaveError::InvalidMagic);
    }

    // Check version and apply migrations if needed
    let data = if save_file.version == SAVE_VERSION {
        // Verify checksum on current-version saves
        verify_checksum(&save_file)?;
        // Deserialize the raw data bytes into SaveData
        rmp_serde::from_slice(&save_file.data).map_err(SaveError::Deserialize)?
    } else if save_file.version < SAVE_VERSION {
        // Migrate from older version
        let migrated = migrations.migrate(save_file.data, save_file.version, SAVE_VERSION)?;
        rmp_serde::from_slice(&migrated).map_err(SaveError::Deserialize)?
    } else {
        return Err(SaveError::UnsupportedVersion(save_file.version));
    };

    Ok(LoadResult {
        data,
        timestamp: save_file.timestamp,
        play_time_seconds: save_file.play_time_seconds,
        version: save_file.version,
    })
}

/// Load a save file from disk.
pub fn load_save_file(
    path: impl AsRef<Path>,
    migrations: &MigrationChain,
) -> Result<LoadResult, SaveError> {
    let bytes = std::fs::read(path)?;
    load_save_bytes(&bytes, migrations)
}

/// Verify the CRC32 checksum of a save file's raw data bytes.
fn verify_checksum(save_file: &SaveFile) -> Result<(), SaveError> {
    let computed = crc32fast::hash(&save_file.data);
    if computed != save_file.checksum {
        return Err(SaveError::ChecksumMismatch {
            expected: save_file.checksum,
            actual: computed,
        });
    }
    Ok(())
}

/// Information about a save slot for display in a load menu.
#[derive(Debug, Clone)]
pub struct SaveSlotInfo {
    pub slot: usize,
    pub exists: bool,
    pub location: Option<String>,
    pub play_time: Option<f64>,
    pub timestamp: Option<u64>,
    pub player_level: Option<u32>,
}

/// Scan save slots and return info about each.
pub fn scan_save_slots(migrations: &MigrationChain) -> Vec<SaveSlotInfo> {
    let mut slots = Vec::new();

    // Check manual slots
    for slot in 0..crate::save::MAX_MANUAL_SLOTS {
        let path = crate::save::save_slot_path(slot);
        let info = probe_save_slot(slot, &path, migrations);
        slots.push(info);
    }

    slots
}

/// Probe a single save slot.
fn probe_save_slot(slot: usize, path: &str, migrations: &MigrationChain) -> SaveSlotInfo {
    let path = Path::new(path);
    if !path.exists() {
        return SaveSlotInfo {
            slot,
            exists: false,
            location: None,
            play_time: None,
            timestamp: None,
            player_level: None,
        };
    }

    match load_save_file(path, migrations) {
        Ok(result) => SaveSlotInfo {
            slot,
            exists: true,
            location: Some(result.data.current_location.clone()),
            play_time: Some(result.play_time_seconds),
            timestamp: Some(result.timestamp),
            player_level: Some(result.data.player.level),
        },
        Err(_) => SaveSlotInfo {
            slot,
            exists: true,
            location: None,
            play_time: None,
            timestamp: None,
            player_level: None,
        },
    }
}

/// Check if an autosave exists.
pub fn has_autosave() -> bool {
    Path::new(&crate::save::autosave_path()).exists()
}

/// Load the autosave.
pub fn load_autosave(migrations: &MigrationChain) -> Result<LoadResult, SaveError> {
    load_save_file(crate::save::autosave_path(), migrations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::*;
    use std::collections::{HashMap, HashSet};

    fn make_test_save_data() -> SaveData {
        SaveData {
            current_location: "hub".to_string(),
            player_position: [0.0, 0.0, 0.0],
            world_flags: HashMap::new(),
            visited_locations: HashSet::new(),
            player: PlayerSaveData {
                name: "Test".to_string(),
                level: 1,
                xp: 0,
                attributes: apothecarys_core::stats::Attributes::default(),
                derived: apothecarys_core::stats::DerivedStats::default(),
            },
            active_party: Vec::new(),
            recruitment_pool: Vec::new(),
            dead_members: Vec::new(),
            inventory: InventorySaveData {
                slots: vec![None; 10],
                max_slots: 10,
            },
            known_recipes: Vec::new(),
            garden: GardenSaveData {
                plots: Vec::new(),
                max_plots: 4,
            },
            dialogue_variables: HashMap::new(),
            dungeon_state: None,
        }
    }

    #[test]
    fn test_load_save_roundtrip() {
        let data = make_test_save_data();
        let bytes = create_save_file(data.clone(), 100.0).unwrap();
        let migrations = MigrationChain::new();

        let result = load_save_bytes(&bytes, &migrations).unwrap();
        assert_eq!(result.data, data);
        assert!((result.play_time_seconds - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_load_invalid_magic() {
        let data = make_test_save_data();
        let bytes = create_save_file(data.clone(), 0.0).unwrap();

        // Corrupt the file by modifying magic bytes
        let mut save_file: SaveFile = rmp_serde::from_slice(&bytes).unwrap();
        save_file.magic = *b"BAAD";
        let corrupted = rmp_serde::to_vec(&save_file).unwrap();

        let migrations = MigrationChain::new();
        let result = load_save_bytes(&corrupted, &migrations);
        assert!(matches!(result, Err(SaveError::InvalidMagic)));
    }

    #[test]
    fn test_load_checksum_mismatch() {
        let data = make_test_save_data();
        let bytes = create_save_file(data, 0.0).unwrap();

        // Corrupt checksum
        let mut save_file: SaveFile = rmp_serde::from_slice(&bytes).unwrap();
        save_file.checksum = save_file.checksum.wrapping_add(1);
        let corrupted = rmp_serde::to_vec(&save_file).unwrap();

        let migrations = MigrationChain::new();
        let result = load_save_bytes(&corrupted, &migrations);
        assert!(matches!(result, Err(SaveError::ChecksumMismatch { .. })));
    }

    #[test]
    fn test_load_unsupported_version() {
        let data = make_test_save_data();
        let bytes = create_save_file(data, 0.0).unwrap();

        // Set a future version
        let mut save_file: SaveFile = rmp_serde::from_slice(&bytes).unwrap();
        save_file.version = 999;
        let modified = rmp_serde::to_vec(&save_file).unwrap();

        let migrations = MigrationChain::new();
        let result = load_save_bytes(&modified, &migrations);
        assert!(matches!(result, Err(SaveError::UnsupportedVersion(999))));
    }

    #[test]
    fn test_write_and_load_file() {
        let data = make_test_save_data();
        let temp_dir = std::env::temp_dir().join("apothecarys_load_test");
        let path = temp_dir.join("test.sav");

        write_save_file(&path, data.clone(), 50.0).unwrap();

        let migrations = MigrationChain::new();
        let result = load_save_file(&path, &migrations).unwrap();
        assert_eq!(result.data, data);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let migrations = MigrationChain::new();
        let result = load_save_file("/nonexistent/path.sav", &migrations);
        assert!(matches!(result, Err(SaveError::Io(_))));
    }

    #[test]
    fn test_save_slot_info_default() {
        let info = SaveSlotInfo {
            slot: 0,
            exists: false,
            location: None,
            play_time: None,
            timestamp: None,
            player_level: None,
        };
        assert!(!info.exists);
        assert_eq!(info.slot, 0);
    }

    #[test]
    fn test_has_autosave_when_missing() {
        // Autosave path won't exist in test environment
        assert!(!has_autosave());
    }
}
