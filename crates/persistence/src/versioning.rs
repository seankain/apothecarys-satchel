use std::collections::BTreeMap;

use crate::save::SaveError;

/// A function that migrates save data from one version to the next.
pub type MigrationFn = fn(Vec<u8>) -> Result<Vec<u8>, String>;

/// Chain of migration functions for upgrading save files across versions.
pub struct MigrationChain {
    migrations: BTreeMap<u32, MigrationFn>,
}

impl MigrationChain {
    pub fn new() -> Self {
        Self {
            migrations: BTreeMap::new(),
        }
    }

    /// Register a migration that transforms data from `from_version` to `from_version + 1`.
    pub fn register(&mut self, from_version: u32, migrate: MigrationFn) {
        self.migrations.insert(from_version, migrate);
    }

    /// Apply all necessary migrations from `from` to `to`.
    pub fn migrate(&self, data: Vec<u8>, from: u32, to: u32) -> Result<Vec<u8>, SaveError> {
        let mut current = data;
        for version in from..to {
            if let Some(migration) = self.migrations.get(&version) {
                current = migration(current)
                    .map_err(|e| SaveError::Migration(format!("v{version} -> v{}: {e}", version + 1)))?;
            }
            // If no migration is registered for a version step, the data passes through unchanged.
            // This allows for versions that only add new fields with defaults.
        }
        Ok(current)
    }

    /// Check if a direct migration path exists from `from` to `to`.
    pub fn can_migrate(&self, from: u32, to: u32) -> bool {
        // We allow migration even without explicit migration functions for each step
        // since the deserialization with serde defaults handles added fields.
        from < to
    }

    /// Number of registered migrations.
    pub fn migration_count(&self) -> usize {
        self.migrations.len()
    }
}

impl Default for MigrationChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default migration chain with all known migrations.
pub fn build_migration_chain() -> MigrationChain {
    // No migrations needed for version 1 (initial version).
    // Future versions would add entries like:
    // chain.register(1, migrate_v1_to_v2);
    MigrationChain::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_chain() {
        let chain = MigrationChain::new();
        assert_eq!(chain.migration_count(), 0);
    }

    #[test]
    fn test_passthrough_migration() {
        let chain = MigrationChain::new();
        let data = vec![1, 2, 3, 4];
        let result = chain.migrate(data.clone(), 1, 2).unwrap();
        // With no migration registered, data passes through unchanged
        assert_eq!(result, data);
    }

    #[test]
    fn test_registered_migration() {
        let mut chain = MigrationChain::new();
        chain.register(1, |mut data| {
            data.push(0xFF);
            Ok(data)
        });

        let data = vec![1, 2, 3];
        let result = chain.migrate(data, 1, 2).unwrap();
        assert_eq!(result, vec![1, 2, 3, 0xFF]);
    }

    #[test]
    fn test_chained_migrations() {
        let mut chain = MigrationChain::new();
        chain.register(1, |mut data| {
            data.push(0xAA);
            Ok(data)
        });
        chain.register(2, |mut data| {
            data.push(0xBB);
            Ok(data)
        });

        let data = vec![0x00];
        let result = chain.migrate(data, 1, 3).unwrap();
        assert_eq!(result, vec![0x00, 0xAA, 0xBB]);
    }

    #[test]
    fn test_migration_error() {
        let mut chain = MigrationChain::new();
        chain.register(1, |_| Err("something went wrong".to_string()));

        let data = vec![1, 2, 3];
        let result = chain.migrate(data, 1, 2);
        assert!(matches!(result, Err(SaveError::Migration(_))));
    }

    #[test]
    fn test_can_migrate() {
        let chain = MigrationChain::new();
        assert!(chain.can_migrate(1, 2));
        assert!(chain.can_migrate(1, 5));
        assert!(!chain.can_migrate(5, 3));
        assert!(!chain.can_migrate(2, 2));
    }

    #[test]
    fn test_build_default_chain() {
        let chain = build_migration_chain();
        // Version 1 has no migrations needed
        assert_eq!(chain.migration_count(), 0);
    }

    #[test]
    fn test_no_op_same_version() {
        let chain = MigrationChain::new();
        let data = vec![1, 2, 3];
        let result = chain.migrate(data.clone(), 3, 3).unwrap();
        assert_eq!(result, data);
    }
}
