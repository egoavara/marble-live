//! Bevy plugins for the marble game.
//!
//! Provides three plugins:
//! - `MarbleCorePlugin`: Core functionality shared between game and editor
//! - `MarbleGamePlugin`: Game-specific features (includes Core)
//! - `MarbleEditorPlugin`: Editor-specific features (includes Core)

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::bevy::events::*;
use crate::bevy::resources::*;
use crate::bevy::state_store::StateStores;
use crate::bevy::systems;
use crate::physics::PHYSICS_DT;

/// Core plugin with shared functionality.
///
/// Includes:
/// - ECS components, resources, and events registration
/// - Physics configuration (bevy_rapier2d with 60Hz fixed timestep)
/// - Blackhole force application
/// - Roll animation
/// - Keyframe animation
/// - Trigger detection and game rules
pub struct MarbleCorePlugin {
    /// RNG seed for deterministic simulation.
    pub seed: u64,
    /// Command queue for external commands (WASM interop).
    pub command_queue: Option<CommandQueue>,
    /// State stores for Yew integration.
    pub state_stores: Option<StateStores>,
}

impl Default for MarbleCorePlugin {
    fn default() -> Self {
        Self {
            seed: 12345,
            command_queue: None,
            state_stores: None,
        }
    }
}

impl Plugin for MarbleCorePlugin {
    fn build(&self, app: &mut App) {
        // Configure fixed timestep
        app.insert_resource(Time::<Fixed>::from_seconds(PHYSICS_DT as f64));

        // Add Rapier physics plugin
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());

        // Register resources
        app.insert_resource(MarbleGameState::new(self.seed))
            .insert_resource(DeterministicRng::new(self.seed))
            .insert_resource(GameContextRes::new(self.seed))
            .insert_resource(KeyframeExecutors::new())
            .insert_resource(systems::KeyframeUpdates::default())
            .insert_resource(ObjectEntityMap::default())
            .insert_resource(InitialTransforms::default())
            .insert_resource(SyncState::default())
            .insert_resource(systems::LiveRankings::default())
            .insert_resource(self.command_queue.clone().unwrap_or_default())
            .insert_resource(self.state_stores.clone().unwrap_or_default());

        // Register messages (events)
        app.add_message::<MarbleArrivedEvent>()
            .add_message::<SpawnMarblesEvent>()
            .add_message::<MapLoadedEvent>()
            .add_message::<SyncSnapshotReceivedEvent>()
            .add_message::<LoadMapEvent>()
            .add_message::<ClearMarblesEvent>()
            .add_message::<AddPlayerEvent>()
            .add_message::<PlayerAddedEvent>()
            .add_message::<RemovePlayerEvent>()
            .add_message::<GameOverEvent>();

        // Add systems
        app.add_systems(
            FixedUpdate,
            (
                // Pre-physics
                systems::clear_external_forces,
                systems::update_game_context,
                systems::apply_vector_field_forces,
                systems::update_keyframe_animations,
                systems::apply_keyframe_updates,
            )
                .chain()
                .before(PhysicsSet::SyncBackend),
        );

        app.add_systems(
            FixedUpdate,
            (
                // Post-physics
                systems::check_trigger_arrivals,
                systems::handle_marble_arrivals,
                systems::check_game_over,
            )
                .chain()
                .after(PhysicsSet::Writeback),
        );

        // Event handlers (can run any time)
        app.add_systems(
            Update,
            (
                // Process external commands first
                systems::process_commands,
                // Then handle events
                systems::handle_load_map,
                // Clear must run before spawn so that spawn's clear event
                // is processed before newly spawned marbles exist
                systems::handle_clear_marbles,
                systems::handle_spawn_marbles,
            )
                .chain(),
        );

        // WASM exit system - checks if exit was requested and sends AppExit
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Update, crate::bevy::wasm_entry::check_exit_system);

        // State sync to shared stores (for Yew UI)
        // sync_live_rankings must run first to populate LiveRankings resource
        app.add_systems(
            PostUpdate,
            (
                systems::sync_live_rankings,
                systems::sync_game_state_to_stores,
            )
                .chain(),
        );
    }
}

/// Game plugin for the marble roulette game.
///
/// Includes `MarbleCorePlugin` plus game-specific features:
/// - Camera setup
/// - Game UI hooks
/// - Input handling
pub struct MarbleGamePlugin {
    /// RNG seed for deterministic simulation.
    pub seed: u64,
    /// Command queue for external commands (WASM interop).
    pub command_queue: Option<CommandQueue>,
    /// State stores for Yew integration.
    pub state_stores: Option<StateStores>,
}

impl Default for MarbleGamePlugin {
    fn default() -> Self {
        Self {
            seed: 12345,
            command_queue: None,
            state_stores: None,
        }
    }
}

impl MarbleGamePlugin {
    /// Create a new game plugin with shared handles.
    pub fn new(command_queue: CommandQueue, state_stores: StateStores) -> Self {
        Self {
            seed: 12345,
            command_queue: Some(command_queue),
            state_stores: Some(state_stores),
        }
    }

    /// Create a new game plugin with the given command queue (legacy).
    pub fn with_command_queue(command_queue: CommandQueue) -> Self {
        Self {
            seed: 12345,
            command_queue: Some(command_queue),
            state_stores: None,
        }
    }
}

impl Plugin for MarbleGamePlugin {
    fn build(&self, app: &mut App) {
        // Add core plugin
        app.add_plugins(MarbleCorePlugin {
            seed: self.seed,
            command_queue: self.command_queue.clone(),
            state_stores: self.state_stores.clone(),
        });

        // Add gizmo config for rendering
        app.insert_resource(systems::ShapeGizmoConfig::default());

        // Add camera resources
        app.insert_resource(crate::bevy::LocalPlayerId::default());

        // Add game-specific systems
        app.add_systems(Startup, setup_game_camera);

        // Add rendering systems
        app.add_systems(
            Update,
            (
                systems::render_map_objects,
                systems::render_marbles,
            ),
        );

        // Add camera systems (run after physics for accurate marble positions)
        app.add_systems(
            Update,
            (
                systems::update_follow_target,
                systems::update_follow_leader,
                systems::update_overview_camera,
                systems::apply_camera_smoothing,
            )
                .chain(),
        );
    }
}

/// Editor plugin for the map editor.
///
/// Includes `MarbleCorePlugin` plus editor-specific features:
/// - Gizmo rendering
/// - Object selection
/// - Keyframe preview
/// - Editor state machine
pub struct MarbleEditorPlugin {
    /// RNG seed for deterministic simulation.
    pub seed: u64,
    /// Command queue for external commands (WASM interop).
    pub command_queue: Option<CommandQueue>,
    /// State stores for Yew integration.
    pub state_stores: Option<StateStores>,
}

impl Default for MarbleEditorPlugin {
    fn default() -> Self {
        Self {
            seed: 12345,
            command_queue: None,
            state_stores: None,
        }
    }
}

impl MarbleEditorPlugin {
    /// Create a new editor plugin with shared handles.
    pub fn new(command_queue: CommandQueue, state_stores: StateStores) -> Self {
        Self {
            seed: 12345,
            command_queue: Some(command_queue),
            state_stores: Some(state_stores),
        }
    }

    /// Create a new editor plugin with the given command queue (legacy).
    pub fn with_command_queue(command_queue: CommandQueue) -> Self {
        Self {
            seed: 12345,
            command_queue: Some(command_queue),
            state_stores: None,
        }
    }
}

/// Editor states.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EditorState {
    #[default]
    Editing,
    Simulating,
    Preview,
}

impl Plugin for MarbleEditorPlugin {
    fn build(&self, app: &mut App) {
        // Add core plugin
        app.add_plugins(MarbleCorePlugin {
            seed: self.seed,
            command_queue: self.command_queue.clone(),
            state_stores: self.state_stores.clone(),
        });

        // Add gizmo config for rendering
        app.insert_resource(systems::ShapeGizmoConfig::default());

        // Add camera resources for editor
        app.insert_resource(crate::bevy::CameraInputState::default());

        // Add editor state
        app.init_state::<EditorState>();

        // Add editor resources
        app.insert_resource(systems::EditorStateRes::default());
        app.insert_resource(systems::EditorStateStore::new());
        app.insert_resource(systems::SnapConfig::default());

        // Add editor messages
        app.add_message::<systems::SelectObjectEvent>();
        app.add_message::<systems::UpdateObjectEvent>();
        app.add_message::<AddObjectEvent>();
        app.add_message::<DeleteObjectEvent>();
        app.add_message::<StartSimulationEvent>();
        app.add_message::<StopSimulationEvent>();
        app.add_message::<ResetSimulationEvent>();
        app.add_message::<PreviewSequenceEvent>();
        app.add_message::<UpdateKeyframeEvent>();

        // Add editor-specific systems
        app.add_systems(Startup, setup_editor_camera);

        // Add editor command processing (runs after process_commands)
        app.add_systems(
            Update,
            (
                systems::process_editor_commands.after(systems::process_commands),
                systems::handle_add_object,
                systems::handle_delete_object,
            ),
        );

        // Add rendering systems
        app.init_resource::<systems::GridConfig>();
        app.init_resource::<systems::GridLabelState>();
        app.init_resource::<systems::GridMeshState>();

        app.add_systems(
            Update,
            (
                systems::render_grid,
                systems::manage_grid_labels,
                systems::render_map_objects,
                systems::render_marbles,
                systems::render_guidelines,
            ),
        );

        // Add editor camera systems
        app.add_systems(
            Update,
            (
                systems::handle_editor_camera_input,
                systems::apply_camera_smoothing,
            )
                .chain(),
        );

        // Editor input and selection systems (Editing state only)
        app.add_systems(
            Update,
            (
                systems::track_mouse_position,
                systems::update_gizmo_hover,
                systems::update_keyframe_gizmo_hover,
                systems::sync_editor_state_from_store,
                systems::handle_mouse_click,
                systems::handle_mouse_drag,
                systems::handle_keyframe_gizmo_click,
                systems::handle_keyframe_drag,
                systems::handle_selection_events,
                systems::handle_object_updates,
                systems::validate_selection,
            )
                .chain()
                .run_if(in_state(EditorState::Editing)),
        );

        // Editor gizmo rendering (always visible in Editing state)
        app.add_systems(
            Update,
            (
                systems::render_editor_gizmos,
                systems::render_sequence_targets,
                systems::render_keyframe_gizmos,
                systems::render_guideline_gizmo,
                systems::render_distance_lines,
            )
                .run_if(in_state(EditorState::Editing)),
        );

        // Sync editor state to stores (for Yew UI)
        // mark_map_loaded_on_event must run before sync systems to set the flag
        app.add_systems(
            PostUpdate,
            (
                systems::mark_map_loaded_on_event,
                systems::sync_editor_state_to_store,
                systems::sync_editor_to_stores,
                systems::sync_snap_config_to_stores,
            )
                .chain(),
        );

        // Simulation control systems
        app.add_systems(
            Update,
            (
                systems::handle_start_simulation,
                systems::handle_stop_simulation,
                systems::handle_reset_simulation,
                systems::clear_executors_on_map_load,
            ),
        );

        // Preview systems
        app.add_systems(Update, systems::handle_preview_sequence);
        app.add_systems(
            Update,
            systems::update_preview_transforms.run_if(in_state(EditorState::Preview)),
        );
        app.add_systems(OnExit(EditorState::Preview), systems::on_exit_preview);
    }
}

/// Sets up the game camera.
fn setup_game_camera(mut commands: Commands) {
    tracing::info!("[marble] setup_game_camera called");

    commands.spawn((
        Camera2d,
        crate::bevy::MainCamera,
        crate::bevy::GameCamera::game(), // Overview mode by default
    ));

    tracing::info!("[marble] game camera spawned (Overview mode)");
}

/// Sets up the editor camera.
fn setup_editor_camera(mut commands: Commands) {
    tracing::info!("[marble] setup_editor_camera called");

    commands.spawn((
        Camera2d,
        crate::bevy::MainCamera,
        crate::bevy::GameCamera::editor(), // Editor mode for manual pan/zoom
    ));

    tracing::info!("[marble] editor camera spawned (Editor mode)");
}
