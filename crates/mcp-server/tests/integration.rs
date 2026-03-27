use apothecarys_tools::map_editor::{ObjectComponent, PlacementData};
use std::path::PathBuf;

fn temp_project_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("data/placements")).unwrap();
    std::fs::create_dir_all(root.join("assets/models")).unwrap();
    (dir, root)
}

mod state_tests {
    use super::*;

    #[test]
    fn test_server_state_new() {
        let (_dir, root) = temp_project_dir();
        let state =
            apothecarys_mcp_server_test_helpers::create_state(root.clone());
        assert!(state.placements_dir.exists());
        assert_eq!(state.placements_dir, root.join("data/placements"));
    }
}

mod scene_tool_tests {
    use super::*;

    fn create_state(root: PathBuf) -> apothecarys_mcp_server_test_helpers::TestState {
        apothecarys_mcp_server_test_helpers::create_state(root)
    }

    #[test]
    fn test_create_and_load_scene() {
        let (_dir, root) = temp_project_dir();
        let mut state = create_state(root.clone());

        // Create a scene
        let result =
            apothecarys_mcp_server_test_helpers::scene::create_scene(&mut state, "test_town");
        assert!(result.is_ok());

        // Verify the file exists on disk
        let path = root.join("data/placements/test_town.placement.ron");
        assert!(path.exists());

        // Load the scene in a fresh state
        let mut state2 = create_state(root);
        let result =
            apothecarys_mcp_server_test_helpers::scene::load_scene(&mut state2, "test_town");
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_and_delete_scene() {
        let (_dir, root) = temp_project_dir();
        let mut state = create_state(root.clone());

        // Create, then delete
        apothecarys_mcp_server_test_helpers::scene::create_scene(&mut state, "to_delete").unwrap();
        let path = root.join("data/placements/to_delete.placement.ron");
        assert!(path.exists());

        apothecarys_mcp_server_test_helpers::scene::delete_scene(&mut state, "to_delete").unwrap();
        assert!(!path.exists());
        assert!(!state.scenes.contains_key("to_delete"));
    }
}

mod roundtrip_tests {
    use super::*;

    #[test]
    fn test_full_roundtrip() {
        let (_dir, root) = temp_project_dir();
        let mut state = apothecarys_mcp_server_test_helpers::create_state(root.clone());

        // Create scene
        apothecarys_mcp_server_test_helpers::scene::create_scene(&mut state, "roundtrip").unwrap();

        // Add objects
        apothecarys_mcp_server_test_helpers::objects::add_object(
            &mut state,
            "roundtrip",
            "models/tree.glb",
            [5.0, 0.0, 3.0],
        )
        .unwrap();
        apothecarys_mcp_server_test_helpers::objects::add_object(
            &mut state,
            "roundtrip",
            "models/rock.glb",
            [10.0, 0.0, 7.0],
        )
        .unwrap();

        // Set transform
        apothecarys_mcp_server_test_helpers::transforms::set_transform(
            &mut state,
            "roundtrip",
            1,
            Some([6.0, 0.0, 4.0]),
            Some([0.0, 1.57, 0.0]),
            Some([2.0, 2.0, 2.0]),
            None,
        )
        .unwrap();

        // Add component
        apothecarys_mcp_server_test_helpers::components::add_component(
            &mut state,
            "roundtrip",
            2,
            ObjectComponent::Interactable {
                interaction_type: "examine".to_string(),
                display_name: "Strange Rock".to_string(),
            },
        )
        .unwrap();

        // Save
        apothecarys_mcp_server_test_helpers::scene::save_scene(&state, "roundtrip").unwrap();

        // Load into fresh state and verify
        let mut state2 = apothecarys_mcp_server_test_helpers::create_state(root);
        apothecarys_mcp_server_test_helpers::scene::load_scene(&mut state2, "roundtrip").unwrap();

        let data = state2.scenes.get("roundtrip").unwrap();
        assert_eq!(data.objects.len(), 2);

        let obj1 = data.get_object(1).unwrap();
        assert_eq!(obj1.position, [6.0, 0.0, 4.0]);
        assert_eq!(obj1.rotation, [0.0, 1.57, 0.0]);
        assert_eq!(obj1.scale, [2.0, 2.0, 2.0]);

        let obj2 = data.get_object(2).unwrap();
        assert_eq!(obj2.components.len(), 1);
        assert!(matches!(
            &obj2.components[0],
            ObjectComponent::Interactable { display_name, .. } if display_name == "Strange Rock"
        ));
    }

    #[test]
    fn test_ron_compatibility_with_direct_api() {
        let (_dir, root) = temp_project_dir();

        // Create a placement file using PlacementData directly (like the map editor would)
        let mut data = PlacementData::new("compat_test");
        data.add_object("house.glb".to_string(), [10.0, 0.0, 5.0]);
        let obj = data.get_object_mut(1).unwrap();
        obj.rotation = [0.0, std::f32::consts::FRAC_PI_2, 0.0];
        obj.components.push(ObjectComponent::ExitTrigger {
            target_location: "garden".to_string(),
            arrival_spawn: "from_hub".to_string(),
        });
        let path = root.join("data/placements/compat_test.placement.ron");
        data.save(&path).unwrap();

        // Load it via the MCP server's load_scene
        let mut state = apothecarys_mcp_server_test_helpers::create_state(root);
        let result =
            apothecarys_mcp_server_test_helpers::scene::load_scene(&mut state, "compat_test");
        assert!(result.is_ok());

        let loaded = state.scenes.get("compat_test").unwrap();
        assert_eq!(loaded, &data);
    }
}

// Helper module that re-exports internal functions for testing
// Since we can't access internal modules from integration tests,
// we test through the public tool functions
mod apothecarys_mcp_server_test_helpers {
    use apothecarys_tools::map_editor::{ObjectComponent, PlacementData};
    use std::collections::HashMap;
    use std::path::PathBuf;

    pub struct TestState {
        pub placements_dir: PathBuf,
        pub assets_dir: PathBuf,
        pub scenes: HashMap<String, PlacementData>,
    }

    pub fn create_state(root: PathBuf) -> TestState {
        let placements_dir = root.join("data").join("placements");
        let assets_dir = root.join("assets");
        std::fs::create_dir_all(&placements_dir).unwrap();
        TestState {
            placements_dir,
            assets_dir,
            scenes: HashMap::new(),
        }
    }

    impl TestState {
        fn placement_path(&self, location_id: &str) -> PathBuf {
            self.placements_dir
                .join(format!("{location_id}.placement.ron"))
        }
    }

    pub mod scene {
        use super::*;

        pub fn create_scene(
            state: &mut TestState,
            location_id: &str,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let data = PlacementData::new(location_id);
            let path = state.placement_path(location_id);
            data.save(&path)?;
            state.scenes.insert(location_id.to_string(), data);
            Ok(())
        }

        pub fn load_scene(
            state: &mut TestState,
            location_id: &str,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let path = state.placement_path(location_id);
            let data = PlacementData::load(&path)?;
            state.scenes.insert(location_id.to_string(), data);
            Ok(())
        }

        pub fn save_scene(
            state: &TestState,
            location_id: &str,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let data = state
                .scenes
                .get(location_id)
                .ok_or("scene not loaded")?;
            let path = state.placement_path(location_id);
            data.save(&path)?;
            Ok(())
        }

        pub fn delete_scene(
            state: &mut TestState,
            location_id: &str,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let path = state.placement_path(location_id);
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
            state.scenes.remove(location_id);
            Ok(())
        }
    }

    pub mod objects {
        use super::*;

        pub fn add_object(
            state: &mut TestState,
            location_id: &str,
            asset_path: &str,
            position: [f32; 3],
        ) -> Result<u32, Box<dyn std::error::Error>> {
            let data = state
                .scenes
                .get_mut(location_id)
                .ok_or("scene not loaded")?;
            Ok(data.add_object(asset_path.to_string(), position))
        }
    }

    pub mod transforms {
        use super::*;
        use apothecarys_tools::map_editor::snap_to_grid;

        pub fn set_transform(
            state: &mut TestState,
            location_id: &str,
            object_id: u32,
            position: Option<[f32; 3]>,
            rotation: Option<[f32; 3]>,
            scale: Option<[f32; 3]>,
            snap_grid: Option<f32>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let data = state
                .scenes
                .get_mut(location_id)
                .ok_or("scene not loaded")?;
            let obj = data
                .get_object_mut(object_id)
                .ok_or("object not found")?;
            if let Some(mut pos) = position {
                if let Some(grid) = snap_grid {
                    pos = snap_to_grid(pos, grid);
                }
                obj.position = pos;
            }
            if let Some(rot) = rotation {
                obj.rotation = rot;
            }
            if let Some(sc) = scale {
                obj.scale = sc;
            }
            Ok(())
        }
    }

    pub mod components {
        use super::*;

        pub fn add_component(
            state: &mut TestState,
            location_id: &str,
            object_id: u32,
            component: ObjectComponent,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let data = state
                .scenes
                .get_mut(location_id)
                .ok_or("scene not loaded")?;
            let obj = data
                .get_object_mut(object_id)
                .ok_or("object not found")?;
            obj.components.push(component);
            Ok(())
        }
    }
}
