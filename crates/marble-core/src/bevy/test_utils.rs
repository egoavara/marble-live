//! Test utilities for headless Bevy integration tests.
//!
//! Provides `TestApp`, a wrapper around `bevy::app::App` that uses
//! `MinimalPlugins` + `MarbleHeadlessPlugin` for testing game logic
//! without a rendering or windowing backend.

use bevy::prelude::*;

use crate::bevy::plugin::MarbleHeadlessPlugin;
use crate::bevy::resources::{CommandQueue, GameCommand, MarbleGameState};
use crate::map::RouletteConfig;
use crate::marble::Color;
use crate::physics::PHYSICS_DT;

/// A headless Bevy app wrapper for testing.
///
/// Provides convenience methods for common test operations like
/// loading maps, adding players, spawning marbles, and advancing
/// the physics simulation.
pub(crate) struct TestApp {
    pub app: App,
}

impl TestApp {
    /// Create a new test app with default seed.
    pub fn new() -> Self {
        Self::with_seed(12345)
    }

    /// Create a new test app with a specific RNG seed.
    pub fn with_seed(seed: u64) -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.add_plugins(bevy::input::InputPlugin);
        app.add_plugins(MarbleHeadlessPlugin {
            seed,
            command_queue: None,
            state_stores: None,
        });
        // Pause virtual time so that only explicit advance_by calls
        // advance the simulation — ensures deterministic behavior.
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .pause();
        // Run one update to initialize all resources and state
        app.update();
        Self { app }
    }

    /// Run a single frame update.
    pub fn update(&mut self) {
        self.app.update();
    }

    /// Advance the physics simulation by exactly `n` fixed timesteps.
    ///
    /// Uses `Time<Fixed>::accumulate_overstep` to feed time directly into
    /// the fixed-timestep accumulator, bypassing virtual time. Combined
    /// with paused virtual time this gives fully deterministic physics.
    pub fn step_physics(&mut self, n: usize) {
        let dt = std::time::Duration::from_secs_f32(PHYSICS_DT);
        for _ in 0..n {
            self.app
                .world_mut()
                .resource_mut::<Time<Fixed>>()
                .accumulate_overstep(dt);
            self.app.update();
        }
    }

    /// Transition to Game mode and run an update to apply the state change.
    pub fn enter_game_mode(&mut self) {
        self.push_command(GameCommand::InitGame);
        self.update();
        // Extra update to process OnEnter systems
        self.update();
    }

    /// Transition to Editor mode and run an update to apply the state change.
    #[allow(dead_code)]
    pub fn enter_editor_mode(&mut self) {
        self.push_command(GameCommand::InitEditor);
        self.update();
        // Extra update to process OnEnter systems
        self.update();
    }

    /// Push a command to the command queue.
    pub fn push_command(&mut self, cmd: GameCommand) {
        self.app.world().resource::<CommandQueue>().push(cmd);
    }

    /// Load a map configuration and run updates until it's processed.
    pub fn load_map(&mut self, config: RouletteConfig) {
        self.push_command(GameCommand::LoadMap { config });
        self.update();
    }

    /// Add a player with the given name and color.
    pub fn add_player(&mut self, name: &str, color: Color) {
        self.push_command(GameCommand::AddPlayer {
            name: name.to_string(),
            color,
        });
        self.update();
    }

    /// Spawn marbles for all registered players.
    pub fn spawn_marbles(&mut self) {
        self.push_command(GameCommand::SpawnMarbles);
        self.update();
    }

    /// Get a reference to the current game state.
    #[allow(dead_code)]
    pub fn game_state(&self) -> &MarbleGameState {
        self.app.world().resource::<MarbleGameState>()
    }

    /// Get a reference to the World.
    pub fn world(&self) -> &World {
        self.app.world()
    }

    /// Get a mutable reference to the World.
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }
}
