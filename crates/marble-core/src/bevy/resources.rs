//! ECS Resources for the marble game.
//!
//! These resources hold shared game state and configuration.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

use bevy::prelude::*;
use parking_lot::Mutex;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::dsl::GameContext;
use crate::game::Player;
use crate::keyframe::KeyframeExecutor;
use crate::map::RouletteConfig;
use crate::marble::{Color, PlayerId};

/// Main game state resource.
#[derive(Resource, Debug, Clone)]
pub struct MarbleGameState {
    /// List of players in the game.
    pub players: Vec<Player>,
    /// Order in which marbles arrived at triggers.
    pub arrival_order: Vec<PlayerId>,
    /// Selected game rule (e.g., "top_n", "last_n").
    pub selected_gamerule: String,
    /// Current simulation frame number.
    pub frame: u64,
    /// RNG seed for deterministic behavior.
    pub rng_seed: u64,
}

impl MarbleGameState {
    pub fn new(seed: u64) -> Self {
        Self {
            players: Vec::new(),
            arrival_order: Vec::new(),
            selected_gamerule: String::new(),
            frame: 0,
            rng_seed: seed,
        }
    }

    /// Adds a player and returns their ID.
    pub fn add_player(&mut self, player: Player) -> PlayerId {
        let id = self.players.len() as PlayerId;
        self.players.push(player);
        id
    }

    /// Returns the leaderboard based on the selected gamerule.
    pub fn leaderboard(&self) -> Vec<PlayerId> {
        match self.selected_gamerule.as_str() {
            "last_n" => self.arrival_order.iter().copied().rev().collect(),
            _ => self.arrival_order.clone(),
        }
    }
}

impl Default for MarbleGameState {
    fn default() -> Self {
        Self::new(12345)
    }
}

/// Map configuration resource.
#[derive(Resource, Debug, Clone)]
pub struct MapConfig(pub RouletteConfig);

impl MapConfig {
    pub fn new(config: RouletteConfig) -> Self {
        Self(config)
    }
}

/// Deterministic RNG resource.
#[derive(Resource)]
pub struct DeterministicRng {
    pub rng: ChaCha8Rng,
    seed: u64,
}

impl DeterministicRng {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            seed,
        }
    }

    pub fn reset(&mut self) {
        self.rng = ChaCha8Rng::seed_from_u64(self.seed);
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }
}

impl Default for DeterministicRng {
    fn default() -> Self {
        Self::new(12345)
    }
}

/// Game context resource for CEL expression evaluation.
#[derive(Resource)]
pub struct GameContextRes {
    pub context: GameContext,
}

impl GameContextRes {
    pub fn new(seed: u64) -> Self {
        Self {
            context: GameContext::with_cache_and_seed(seed),
        }
    }

    pub fn update(&mut self, time: f32, frame: u64) {
        self.context.update(time, frame);
    }
}

impl Default for GameContextRes {
    fn default() -> Self {
        Self::new(12345)
    }
}

/// Specifies which keyframe sequences should be activated.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ActivatedKeyframes {
    /// No keyframes active (paused/stopped).
    #[default]
    None,
    /// All autoplay keyframes are active.
    All,
    /// Only specific sequences by name are active.
    Sequences(Vec<String>),
}

impl ActivatedKeyframes {
    /// Check if a sequence should be executed.
    pub fn should_execute(&self, sequence_name: &str) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Sequences(names) => names.iter().any(|n| n == sequence_name),
        }
    }
}

/// Collection of keyframe animation executors.
#[derive(Resource, Default)]
pub struct KeyframeExecutors {
    pub executors: Vec<KeyframeExecutor>,
    /// Which keyframes are currently activated.
    pub activated: ActivatedKeyframes,
}

impl KeyframeExecutors {
    pub fn new() -> Self {
        Self {
            executors: Vec::new(),
            activated: ActivatedKeyframes::None,
        }
    }

    pub fn add(&mut self, executor: KeyframeExecutor) {
        self.executors.push(executor);
    }

    pub fn clear(&mut self) {
        self.executors.clear();
        self.activated = ActivatedKeyframes::None;
    }

    /// Activate all keyframes.
    pub fn activate_all(&mut self) {
        self.activated = ActivatedKeyframes::All;
    }

    /// Activate specific sequences.
    pub fn activate_sequences(&mut self, names: Vec<String>) {
        self.activated = ActivatedKeyframes::Sequences(names);
    }

    /// Deactivate all keyframes.
    pub fn deactivate(&mut self) {
        self.activated = ActivatedKeyframes::None;
    }

    /// Check if a sequence should be executed.
    pub fn should_execute(&self, sequence_name: &str) -> bool {
        self.activated.should_execute(sequence_name)
    }

    /// Removes finished executors.
    pub fn retain_active(&mut self) {
        self.executors.retain(|e| !e.is_finished());
    }
}

/// Mapping from object IDs to their entities.
#[derive(Resource, Default)]
pub struct ObjectEntityMap {
    pub map: HashMap<String, Entity>,
}

impl ObjectEntityMap {
    pub fn insert(&mut self, id: String, entity: Entity) {
        self.map.insert(id, entity);
    }

    pub fn get(&self, id: &str) -> Option<Entity> {
        self.map.get(id).copied()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}

/// Initial transforms for animated objects (for keyframe animations).
#[derive(Resource, Default)]
pub struct InitialTransforms {
    pub transforms: HashMap<String, (Vec2, f32)>,
}

impl InitialTransforms {
    pub fn insert(&mut self, id: String, position: Vec2, rotation: f32) {
        self.transforms.insert(id, (position, rotation));
    }

    pub fn get(&self, id: &str) -> Option<(Vec2, f32)> {
        self.transforms.get(id).copied()
    }

    pub fn clear(&mut self) {
        self.transforms.clear();
    }
}

/// P2P synchronization state.
#[derive(Resource, Default)]
pub struct SyncState {
    /// Whether we are the host.
    pub is_host: bool,
    /// Last synchronized frame.
    pub last_sync_frame: u64,
    /// Pending snapshot to apply.
    pub pending_snapshot: Option<Vec<u8>>,
}

/// Local player ID for camera following.
///
/// In multiplayer, this identifies which player's marble to follow
/// when using `CameraMode::FollowTarget`.
#[derive(Resource, Default)]
pub struct LocalPlayerId(pub Option<PlayerId>);

impl LocalPlayerId {
    pub fn new(player_id: PlayerId) -> Self {
        Self(Some(player_id))
    }

    pub fn none() -> Self {
        Self(None)
    }

    pub fn set(&mut self, player_id: Option<PlayerId>) {
        self.0 = player_id;
    }

    pub fn get(&self) -> Option<PlayerId> {
        self.0
    }
}

/// Camera input state for editor mode.
///
/// Tracks drag operations for pan and zoom functionality.
#[derive(Resource, Default)]
pub struct CameraInputState {
    /// Screen position where drag started (for middle-click pan).
    pub drag_start_screen: Option<Vec2>,
    /// Camera position when drag started (for middle-click pan).
    pub drag_start_camera_pos: Option<Vec2>,
    /// Whether a drag operation is currently in progress.
    pub is_dragging: bool,
}

/// Commands that can be sent from JavaScript to the Bevy app.
#[derive(Debug, Clone)]
pub enum GameCommand {
    /// Spawn marbles for all players.
    SpawnMarbles,
    /// Clear all marbles.
    ClearMarbles,
    /// Clear all players.
    ClearPlayers,
    /// Add a new player.
    AddPlayer { name: String, color: Color },
    /// Remove a player.
    RemovePlayer { player_id: PlayerId },
    /// Load a new map.
    LoadMap { config: RouletteConfig },
    /// Frame boundary marker - commands after this are processed in the next frame.
    Yield,

    // ========== Camera Commands ==========
    /// Set the camera mode.
    SetCameraMode { mode: crate::bevy::CameraMode },
    /// Set the local player ID for camera following.
    SetLocalPlayerId { player_id: Option<PlayerId> },

    // ========== Editor Commands ==========
    /// Select an object in the editor.
    SelectObject { index: Option<usize> },
    /// Select a sequence in the editor.
    SelectSequence { index: Option<usize> },
    /// Select a keyframe in the editor.
    SelectKeyframe { index: Option<usize> },
    /// Update a map object.
    UpdateObject { index: usize, object: crate::map::MapObject },
    /// Add a new map object.
    AddObject { object: crate::map::MapObject },
    /// Delete a map object.
    DeleteObject { index: usize },
    /// Update a keyframe in a sequence.
    UpdateKeyframe {
        sequence_index: usize,
        keyframe_index: usize,
        keyframe: crate::map::Keyframe,
    },
    /// Start simulation in editor.
    StartSimulation,
    /// Stop simulation in editor.
    StopSimulation,
    /// Reset simulation in editor.
    ResetSimulation,
    /// Preview keyframe sequence.
    PreviewSequence { start: bool },
    /// Update snap configuration.
    UpdateSnapConfig {
        grid_snap_enabled: Option<bool>,
        grid_snap_interval: Option<f32>,
        angle_snap_enabled: Option<bool>,
        angle_snap_interval: Option<f32>,
    },
}

impl GameCommand {
    /// Returns true if this is an editor-specific command.
    pub fn is_editor_command(&self) -> bool {
        matches!(
            self,
            Self::SelectObject { .. }
                | Self::SelectSequence { .. }
                | Self::SelectKeyframe { .. }
                | Self::UpdateObject { .. }
                | Self::AddObject { .. }
                | Self::DeleteObject { .. }
                | Self::UpdateKeyframe { .. }
                | Self::StartSimulation
                | Self::StopSimulation
                | Self::ResetSimulation
                | Self::PreviewSequence { .. }
                | Self::UpdateSnapConfig { .. }
        )
    }
}

/// Thread-safe command queue for WASM interop.
///
/// This allows JavaScript to push commands that will be processed
/// by Bevy systems on the next frame.
#[derive(Resource, Clone)]
pub struct CommandQueue {
    inner: Arc<Mutex<VecDeque<GameCommand>>>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Push a command to be processed.
    pub fn push(&self, command: GameCommand) {
        self.inner.lock().push_back(command);
    }

    /// Drain all pending commands.
    pub fn drain(&self) -> Vec<GameCommand> {
        self.inner.lock().drain(..).collect()
    }

    /// Drain only game-specific commands, leaving editor commands in the queue.
    pub fn drain_game(&self) -> Vec<GameCommand> {
        let mut guard = self.inner.lock();
        let mut game_commands = Vec::new();
        let mut remaining = VecDeque::new();

        for cmd in guard.drain(..) {
            if cmd.is_editor_command() {
                remaining.push_back(cmd);
            } else {
                game_commands.push(cmd);
            }
        }

        *guard = remaining;
        game_commands
    }

    /// Drain game commands until Yield or empty.
    ///
    /// Returns commands up to (not including) Yield.
    /// Yield itself is consumed but not returned.
    /// Editor commands are skipped and left in the queue.
    pub fn drain_until_yield(&self) -> Vec<GameCommand> {
        let mut guard = self.inner.lock();
        let mut commands = Vec::new();
        let mut remaining = VecDeque::new();
        let mut hit_yield = false;

        while let Some(cmd) = guard.pop_front() {
            if hit_yield {
                // After yield, keep everything for next frame
                remaining.push_back(cmd);
                continue;
            }

            if cmd.is_editor_command() {
                // Editor commands are left for drain_editor
                remaining.push_back(cmd);
                continue;
            }

            if matches!(cmd, GameCommand::Yield) {
                // Yield encountered - consume it and stop processing game commands
                tracing::debug!("[command] Yield - deferring remaining commands to next frame");
                hit_yield = true;
                continue;
            }

            commands.push(cmd);
        }

        *guard = remaining;
        commands
    }

    /// Drain only editor-specific commands, leaving others in the queue.
    pub fn drain_editor(&self) -> Vec<GameCommand> {
        let mut guard = self.inner.lock();
        let mut editor_commands = Vec::new();
        let mut remaining = VecDeque::new();

        for cmd in guard.drain(..) {
            if cmd.is_editor_command() {
                editor_commands.push(cmd);
            } else {
                remaining.push_back(cmd);
            }
        }

        *guard = remaining;
        editor_commands
    }

    /// Check if there are pending commands.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    /// Clear all pending commands.
    pub fn clear(&self) {
        self.inner.lock().clear();
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}
