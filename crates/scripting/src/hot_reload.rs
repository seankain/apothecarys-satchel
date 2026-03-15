use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::vm::{ScriptError, ScriptingEngine};

/// Watches a scripts directory for file changes and reloads modified scripts.
pub struct ScriptWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    scripts_dir: PathBuf,
}

impl ScriptWatcher {
    /// Create a new script watcher for the given directory.
    pub fn new(scripts_dir: impl AsRef<Path>) -> Result<Self, ScriptError> {
        let scripts_dir = scripts_dir.as_ref().to_path_buf();

        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| ScriptError::Io(std::io::Error::other(e)))?;

        if scripts_dir.exists() {
            watcher
                .watch(&scripts_dir, RecursiveMode::Recursive)
                .map_err(|e| ScriptError::Io(std::io::Error::other(e)))?;
        }

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            scripts_dir,
        })
    }

    /// Poll for changed scripts and reload them. Returns the paths that were reloaded.
    pub fn poll_and_reload(
        &mut self,
        engine: &ScriptingEngine,
    ) -> Result<Vec<PathBuf>, ScriptError> {
        let mut modified: HashSet<PathBuf> = HashSet::new();

        // Drain all pending events
        while let Ok(event_result) = self.receiver.try_recv() {
            if let Ok(event) = event_result {
                if matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_)
                ) {
                    for path in event.paths {
                        if path.extension().is_some_and(|ext| ext == "lua") {
                            modified.insert(path);
                        }
                    }
                }
            }
        }

        // Reload modified scripts
        let mut reloaded = Vec::new();
        for path in &modified {
            match engine.run_file(path) {
                Ok(()) => {
                    reloaded.push(path.clone());
                }
                Err(e) => {
                    eprintln!(
                        "Hot reload error for {}: {e}",
                        path.display()
                    );
                }
            }
        }

        Ok(reloaded)
    }

    /// Get the watched scripts directory.
    pub fn scripts_dir(&self) -> &Path {
        &self.scripts_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Note: Hot reload tests are inherently timing-sensitive.
    // We test the basic watcher creation and the reload mechanism.

    #[test]
    fn test_watcher_creation_with_nonexistent_dir() {
        // Should handle non-existent directory gracefully
        let result = ScriptWatcher::new("/tmp/nonexistent_scripts_dir_12345");
        // The watcher won't fail on creation, it just won't watch anything
        assert!(result.is_ok());
    }

    #[test]
    fn test_poll_empty() {
        let dir = TempDir::new().unwrap();
        let mut watcher = ScriptWatcher::new(dir.path()).unwrap();
        let engine = ScriptingEngine::new().unwrap();

        // Should return empty vec when no files changed
        let reloaded = watcher.poll_and_reload(&engine).unwrap();
        assert!(reloaded.is_empty());
    }

    #[test]
    fn test_scripts_dir() {
        let dir = TempDir::new().unwrap();
        let watcher = ScriptWatcher::new(dir.path()).unwrap();
        assert_eq!(watcher.scripts_dir(), dir.path());
    }

    #[test]
    fn test_reload_existing_script() {
        let dir = TempDir::new().unwrap();
        let script_path = dir.path().join("test.lua");
        fs::write(&script_path, "test_var = 42").unwrap();

        let engine = ScriptingEngine::new().unwrap();
        engine.run_file(&script_path).unwrap();

        assert_eq!(engine.get_global_number("test_var").unwrap(), Some(42.0));
    }
}
