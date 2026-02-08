//! Bevy plugins for the marble game.
//!
//! Provides:
//! - `MarbleHeadlessPlugin`: Logic-only plugin (no rendering/window dependencies) for headless testing
//! - `MarbleUnifiedPlugin`: Full plugin including `MarbleHeadlessPlugin` + rendering systems

use bevy::prelude::*;

use crate::bevy::events::*;
use crate::bevy::rapier_plugin::{MarblePhysicsPlugin, PhysicsSet};
use crate::bevy::resources::*;
use crate::bevy::state_store::StateStores;
use crate::bevy::systems;
use crate::physics::PHYSICS_DT;

/// Application mode state for dynamic mode switching.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppMode {
    #[default]
    Idle,
    Game,
    Editor,
}

/// Editor states (SubState of AppMode::Editor).
///
/// Only exists when AppMode is Editor. Automatically removed when
/// AppMode transitions away from Editor.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppMode = AppMode::Editor)]
pub enum EditorState {
    #[default]
    Editing,
    Simulating,
    Preview,
}

// ============================================================================
// Headless Plugin (logic only, no rendering/window dependencies)
// ============================================================================

/// Headless plugin containing all game logic without rendering or window dependencies.
///
/// Use this plugin in tests with `MinimalPlugins` to run ECS systems
/// without requiring a windowing or rendering backend.
///
/// Excluded systems (rendering-dependent):
/// - Gizmos-based rendering (render_map_objects, render_marbles, render_editor_gizmos, etc.)
/// - Grid rendering (render_grid, manage_grid_labels)
/// - Window-dependent systems (track_mouse_position, update_overview_camera, handle_editor_camera_input)
/// - Projection-dependent systems (apply_camera_smoothing)
/// - Camera2d spawning (setup_game_camera, setup_editor_camera)
pub struct MarbleHeadlessPlugin {
    pub seed: u64,
    pub command_queue: Option<CommandQueue>,
    pub state_stores: Option<StateStores>,
}

impl Default for MarbleHeadlessPlugin {
    fn default() -> Self {
        Self {
            seed: 12345,
            command_queue: None,
            state_stores: None,
        }
    }
}

impl Plugin for MarbleHeadlessPlugin {
    fn build(&self, app: &mut App) {
        // ====================================================================
        // States
        // ====================================================================
        app.init_state::<AppMode>();
        app.add_sub_state::<EditorState>();

        // ====================================================================
        // Physics
        // ====================================================================
        app.insert_resource(Time::<Fixed>::from_seconds(PHYSICS_DT as f64));
        app.add_plugins(MarblePhysicsPlugin);

        // ====================================================================
        // Resources (all registered upfront, systems gated by run_if)
        // ====================================================================

        // Core resources
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

        // Rendering resources (shared, needed by some logic systems for config)
        app.insert_resource(systems::ShapeGizmoConfig::default());

        // Game-specific resources
        app.insert_resource(crate::bevy::LocalPlayerId::default());

        // Editor-specific resources
        app.insert_resource(crate::bevy::CameraInputState::default());
        app.insert_resource(systems::EditorStateRes::default());
        app.insert_resource(systems::EditorStateStore::new());
        app.insert_resource(systems::SnapConfig::default());

        // Grid resources (editor)
        app.init_resource::<systems::GridConfig>();
        app.init_resource::<systems::GridLabelState>();
        app.init_resource::<systems::GridMeshState>();

        // ====================================================================
        // Messages (all registered upfront)
        // ====================================================================

        // Core messages
        app.add_message::<MarbleArrivedEvent>()
            .add_message::<SpawnMarblesEvent>()
            .add_message::<SpawnMarblesAtEvent>()
            .add_message::<MapLoadedEvent>()
            .add_message::<SyncSnapshotReceivedEvent>()
            .add_message::<LoadMapEvent>()
            .add_message::<ClearMarblesEvent>()
            .add_message::<AddPlayerEvent>()
            .add_message::<PlayerAddedEvent>()
            .add_message::<RemovePlayerEvent>()
            .add_message::<GameOverEvent>();

        // P2P sync messages
        app.add_message::<BroadcastGameStartEvent>()
            .add_message::<SyncSnapshotRequestEvent>();

        // Editor messages
        app.add_message::<systems::SelectObjectEvent>()
            .add_message::<systems::UpdateObjectEvent>()
            .add_message::<AddObjectEvent>()
            .add_message::<DeleteObjectEvent>()
            .add_message::<StartSimulationEvent>()
            .add_message::<StopSimulationEvent>()
            .add_message::<ResetSimulationEvent>()
            .add_message::<PreviewSequenceEvent>()
            .add_message::<UpdateKeyframeEvent>();

        // ====================================================================
        // Core systems (always active)
        // ====================================================================

        // Pre-physics (FixedUpdate)
        app.add_systems(
            FixedUpdate,
            (
                systems::clear_external_forces,
                systems::update_game_context,
                systems::apply_vector_field_forces,
                systems::update_keyframe_animations,
                systems::apply_keyframe_updates,
            )
                .chain()
                .before(PhysicsSet::SyncToRapier),
        );

        // Post-physics (FixedUpdate)
        app.add_systems(
            FixedUpdate,
            (
                systems::check_trigger_arrivals,
                systems::handle_marble_arrivals,
                systems::check_game_over,
            )
                .chain()
                .after(PhysicsSet::SyncFromRapier),
        );

        // Command processing and event handlers (always active)
        app.add_systems(
            Update,
            (
                systems::process_commands,
                systems::handle_load_map,
                systems::handle_clear_marbles,
                systems::handle_spawn_marbles,
                systems::handle_spawn_marbles_at,
            )
                .chain(),
        );

        // WASM exit system
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Update, crate::bevy::wasm_entry::check_exit_system);

        // ====================================================================
        // P2P Sync Systems (WASM only)
        // ====================================================================
        #[cfg(target_arch = "wasm32")]
        {
            use crate::bevy::systems::p2p_sync;

            // Socket lifecycle + message polling (always active)
            app.add_systems(
                Update,
                (
                    p2p_sync::pickup_pending_p2p,
                    p2p_sync::handle_p2p_disconnect,
                    p2p_sync::poll_p2p_socket,
                )
                    .chain(),
            );

            // Frame hash + desync detection (Game mode, FixedUpdate after physics)
            app.add_systems(
                FixedUpdate,
                (p2p_sync::broadcast_frame_hash, p2p_sync::check_desync)
                    .chain()
                    .after(PhysicsSet::SyncFromRapier)
                    .run_if(in_state(AppMode::Game)),
            );

            // Sync snapshot + game start (Game mode, Update)
            app.add_systems(
                Update,
                (
                    p2p_sync::handle_sync_request,
                    p2p_sync::apply_sync_snapshot,
                    p2p_sync::broadcast_game_start,
                )
                    .run_if(in_state(AppMode::Game)),
            );
        }

        // Core state sync (always active)
        app.add_systems(
            PostUpdate,
            (
                systems::sync_live_rankings,
                systems::sync_game_state_to_stores,
            )
                .chain(),
        );

        // ====================================================================
        // Game camera logic (Game only) — no Window/Projection dependency
        // ====================================================================

        app.add_systems(
            Update,
            (
                systems::update_follow_target,
                systems::update_follow_leader,
            )
                .chain()
                .run_if(in_state(AppMode::Game)),
        );

        // ====================================================================
        // Editor command processing (Editor only)
        // ====================================================================

        app.add_systems(
            Update,
            (
                systems::process_editor_commands.after(systems::process_commands),
                systems::handle_add_object,
                systems::handle_delete_object,
            )
                .run_if(in_state(AppMode::Editor)),
        );

        // ====================================================================
        // Editor input and selection (EditorState::Editing only)
        // — track_mouse_position excluded (needs Window/Camera)
        // ====================================================================

        app.add_systems(
            Update,
            (
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

        // ====================================================================
        // Editor state sync (Editor only, PostUpdate)
        // ====================================================================

        app.add_systems(
            PostUpdate,
            (
                systems::mark_map_loaded_on_event,
                systems::sync_editor_state_to_store,
                systems::sync_editor_to_stores,
                systems::sync_snap_config_to_stores,
            )
                .chain()
                .run_if(in_state(AppMode::Editor)),
        );

        // ====================================================================
        // Simulation control (Editor only)
        // ====================================================================

        app.add_systems(
            Update,
            (
                systems::handle_start_simulation,
                systems::handle_stop_simulation,
                systems::handle_reset_simulation,
                systems::clear_executors_on_map_load,
            )
                .run_if(in_state(AppMode::Editor)),
        );

        // ====================================================================
        // Preview systems (Editor only)
        // ====================================================================

        app.add_systems(
            Update,
            systems::handle_preview_sequence.run_if(in_state(AppMode::Editor)),
        );
        app.add_systems(
            Update,
            systems::update_preview_transforms.run_if(in_state(EditorState::Preview)),
        );
        app.add_systems(OnExit(EditorState::Preview), systems::on_exit_preview);

        // ====================================================================
        // OnExit transition systems (cleanup only, no camera spawning)
        // ====================================================================

        app.add_systems(OnExit(AppMode::Game), cleanup_game_mode);
        app.add_systems(OnExit(AppMode::Editor), cleanup_editor_mode);
    }
}

// ============================================================================
// Unified Plugin (headless + rendering)
// ============================================================================

/// Unified plugin that supports dynamic mode switching via `AppMode` state.
///
/// Includes `MarbleHeadlessPlugin` for all game logic, plus rendering systems
/// that require `Gizmos`, `Mesh2d`, `Window`, `Projection`, and `Camera2d`.
pub struct MarbleUnifiedPlugin {
    pub seed: u64,
    pub command_queue: Option<CommandQueue>,
    pub state_stores: Option<StateStores>,
}

impl Default for MarbleUnifiedPlugin {
    fn default() -> Self {
        Self {
            seed: 12345,
            command_queue: None,
            state_stores: None,
        }
    }
}

impl MarbleUnifiedPlugin {
    pub fn new(command_queue: CommandQueue, state_stores: StateStores) -> Self {
        Self {
            seed: 12345,
            command_queue: Some(command_queue),
            state_stores: Some(state_stores),
        }
    }
}

impl Plugin for MarbleUnifiedPlugin {
    fn build(&self, app: &mut App) {
        // ====================================================================
        // Headless logic (all game systems without rendering)
        // ====================================================================
        app.add_plugins(MarbleHeadlessPlugin {
            seed: self.seed,
            command_queue: self.command_queue.clone(),
            state_stores: self.state_stores.clone(),
        });

        // ====================================================================
        // Rendering systems (Game | Editor)
        // ====================================================================

        let in_game_or_editor = in_state(AppMode::Game).or(in_state(AppMode::Editor));

        app.add_systems(
            Update,
            (systems::render_map_objects, systems::render_marbles)
                .run_if(in_game_or_editor.clone()),
        );

        // ====================================================================
        // Window-dependent game camera systems (Game only)
        // ====================================================================

        app.add_systems(
            Update,
            systems::update_overview_camera.run_if(in_state(AppMode::Game)),
        );

        // ====================================================================
        // Editor camera systems (Editor only) — needs Window/Projection
        // ====================================================================

        app.add_systems(
            Update,
            systems::handle_editor_camera_input.run_if(in_state(AppMode::Editor)),
        );

        // Camera smoothing (Game | Editor) — needs Projection
        app.add_systems(
            Update,
            systems::apply_camera_smoothing.run_if(in_game_or_editor.clone()),
        );

        // ====================================================================
        // Editor grid/guidelines (Editor only) — needs Mesh2d/Text2d/Gizmos
        // ====================================================================

        app.add_systems(
            Update,
            (
                systems::render_grid,
                systems::manage_grid_labels,
                systems::render_guidelines,
            )
                .run_if(in_state(AppMode::Editor)),
        );

        // ====================================================================
        // Editor input: track_mouse_position (needs Window/Camera)
        // ====================================================================

        app.add_systems(
            Update,
            systems::track_mouse_position
                .before(systems::update_gizmo_hover)
                .run_if(in_state(EditorState::Editing)),
        );

        // ====================================================================
        // Editor gizmo rendering (EditorState::Editing only) — needs Gizmos
        // ====================================================================

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

        // ====================================================================
        // OnEnter camera setup (needs Camera2d)
        // ====================================================================

        app.add_systems(OnEnter(AppMode::Game), setup_game_camera);
        app.add_systems(OnEnter(AppMode::Editor), setup_editor_camera);
    }
}

/// Sets up or reconfigures the camera for Game mode.
///
/// Reuses existing camera entity to avoid destroying GPU textures
/// mid-frame (which causes "Destroyed texture used in a submit" errors).
fn setup_game_camera(
    mut commands: Commands,
    mut existing: Query<&mut crate::bevy::GameCamera, With<crate::bevy::MainCamera>>,
) {
    tracing::info!("[marble] setup_game_camera called");

    if let Ok(mut cam) = existing.single_mut() {
        // Reuse existing camera — just reconfigure it
        *cam = crate::bevy::GameCamera::game();
        tracing::info!("[marble] game camera reconfigured (Overview mode)");
    } else {
        // No camera yet — spawn a fresh one
        commands.spawn((
            Camera2d,
            crate::bevy::MainCamera,
            crate::bevy::GameCamera::game(),
        ));
        tracing::info!("[marble] game camera spawned (Overview mode)");
    }
}

/// Sets up or reconfigures the camera for Editor mode.
fn setup_editor_camera(
    mut commands: Commands,
    mut existing: Query<&mut crate::bevy::GameCamera, With<crate::bevy::MainCamera>>,
) {
    tracing::info!("[marble] setup_editor_camera called");

    if let Ok(mut cam) = existing.single_mut() {
        *cam = crate::bevy::GameCamera::editor();
        tracing::info!("[marble] editor camera reconfigured (Editor mode)");
    } else {
        commands.spawn((
            Camera2d,
            crate::bevy::MainCamera,
            crate::bevy::GameCamera::editor(),
        ));
        tracing::info!("[marble] editor camera spawned (Editor mode)");
    }
}

/// Cleanup when exiting Game mode.
///
/// NOTE: Camera entities are NOT despawned here to avoid destroying GPU
/// textures while render commands still reference them. The OnEnter system
/// of the next mode reconfigures the existing camera instead.
fn cleanup_game_mode(
    mut commands: Commands,
    map_objects: Query<Entity, With<crate::bevy::MapObjectMarker>>,
    marbles: Query<Entity, With<crate::bevy::Marble>>,
    marble_visuals: Query<Entity, With<crate::bevy::MarbleVisual>>,
    mut object_map: ResMut<ObjectEntityMap>,
    mut initial_transforms: ResMut<InitialTransforms>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    mut game_state: ResMut<MarbleGameState>,
    mut physics: ResMut<crate::bevy::rapier_plugin::PhysicsWorldRes>,
) {
    tracing::info!("[marble] cleanup_game_mode");

    for entity in map_objects.iter() {
        commands.entity(entity).despawn();
    }
    for entity in marbles.iter() {
        commands.entity(entity).despawn();
    }
    for entity in marble_visuals.iter() {
        commands.entity(entity).despawn();
    }

    object_map.clear();
    initial_transforms.clear();
    keyframe_executors.clear();
    game_state.players.clear();
    game_state.arrival_order.clear();
    game_state.frame = 0;
    physics.world.reset();
}

/// Cleanup when exiting Editor mode.
///
/// NOTE: Camera entities are NOT despawned here (see cleanup_game_mode).
fn cleanup_editor_mode(
    mut commands: Commands,
    map_objects: Query<Entity, With<crate::bevy::MapObjectMarker>>,
    marbles: Query<Entity, With<crate::bevy::Marble>>,
    marble_visuals: Query<Entity, With<crate::bevy::MarbleVisual>>,
    grid_meshes: Query<Entity, With<systems::GridMesh>>,
    grid_labels: Query<Entity, With<systems::GridLabel>>,
    mut object_map: ResMut<ObjectEntityMap>,
    mut initial_transforms: ResMut<InitialTransforms>,
    mut keyframe_executors: ResMut<KeyframeExecutors>,
    mut editor_state: ResMut<systems::EditorStateRes>,
    mut grid_mesh_state: ResMut<systems::GridMeshState>,
    mut grid_label_state: ResMut<systems::GridLabelState>,
    mut physics: ResMut<crate::bevy::rapier_plugin::PhysicsWorldRes>,
) {
    tracing::info!("[marble] cleanup_editor_mode");

    for entity in map_objects.iter() {
        commands.entity(entity).despawn();
    }
    for entity in marbles.iter() {
        commands.entity(entity).despawn();
    }
    for entity in marble_visuals.iter() {
        commands.entity(entity).despawn();
    }
    for entity in grid_meshes.iter() {
        commands.entity(entity).despawn();
    }
    for entity in grid_labels.iter() {
        commands.entity(entity).despawn();
    }

    object_map.clear();
    initial_transforms.clear();
    keyframe_executors.clear();
    *editor_state = systems::EditorStateRes::default();
    *grid_mesh_state = systems::GridMeshState::default();
    *grid_label_state = systems::GridLabelState::default();
    physics.world.reset();
}
