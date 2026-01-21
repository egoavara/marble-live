//! Game state machine and round management.

use rapier2d::prelude::ColliderHandle;
use serde::{Deserialize, Serialize};

use crate::map::RouletteConfig;
use crate::marble::{Color, MarbleManager, PlayerId};
use crate::physics::PhysicsWorld;

/// Countdown duration in frames (3 seconds at 60Hz).
pub const COUNTDOWN_FRAMES: u32 = 180;

/// Game phase representing the current state of the game.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GamePhase {
    /// Waiting for players to join.
    Lobby,
    /// Countdown before the round starts.
    Countdown { remaining_frames: u32 },
    /// Game is actively running.
    Running,
    /// Round has finished with a winner.
    Finished { winner: Option<PlayerId> },
}

impl Default for GamePhase {
    fn default() -> Self {
        Self::Lobby
    }
}

/// Player information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub color: Color,
    pub ready: bool,
}

impl Player {
    pub fn new(id: PlayerId, name: String, color: Color) -> Self {
        Self {
            id,
            name,
            color,
            ready: false,
        }
    }
}

/// Complete game state containing all game data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub phase: GamePhase,
    pub players: Vec<Player>,
    pub eliminated_order: Vec<PlayerId>,
    pub rng_seed: u64,
    #[serde(skip)]
    pub physics_world: PhysicsWorld,
    pub marble_manager: MarbleManager,
    #[serde(skip)]
    pub map_config: Option<RouletteConfig>,
    #[serde(skip)]
    pub hole_handles: Vec<ColliderHandle>,
}

impl GameState {
    /// Creates a new game state with the given RNG seed.
    pub fn new(seed: u64) -> Self {
        Self {
            phase: GamePhase::Lobby,
            players: Vec::new(),
            eliminated_order: Vec::new(),
            rng_seed: seed,
            physics_world: PhysicsWorld::new(),
            marble_manager: MarbleManager::new(seed),
            map_config: None,
            hole_handles: Vec::new(),
        }
    }

    /// Loads a map configuration.
    pub fn load_map(&mut self, config: RouletteConfig) {
        // Reset physics world
        self.physics_world.reset();

        // Apply map to world
        self.hole_handles = config.apply_to_world(&mut self.physics_world);
        self.map_config = Some(config);
    }

    /// Adds a player to the game.
    /// Returns false if the game is not in Lobby phase.
    pub fn add_player(&mut self, name: String, color: Color) -> Option<PlayerId> {
        if self.phase != GamePhase::Lobby {
            return None;
        }

        #[allow(clippy::cast_possible_truncation)]
        let id = self.players.len() as PlayerId;
        self.players.push(Player::new(id, name, color));
        Some(id)
    }

    /// Removes a player from the game.
    pub fn remove_player(&mut self, player_id: PlayerId) -> bool {
        if self.phase != GamePhase::Lobby {
            return false;
        }

        if let Some(pos) = self.players.iter().position(|p| p.id == player_id) {
            self.players.remove(pos);
            true
        } else {
            false
        }
    }

    /// Sets a player's ready status.
    pub fn set_player_ready(&mut self, player_id: PlayerId, ready: bool) -> bool {
        if self.phase != GamePhase::Lobby {
            return false;
        }

        if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
            player.ready = ready;
            true
        } else {
            false
        }
    }

    /// Checks if all players are ready.
    pub fn all_players_ready(&self) -> bool {
        !self.players.is_empty() && self.players.iter().all(|p| p.ready)
    }

    /// Starts the countdown phase.
    /// Returns false if conditions are not met.
    pub fn start_countdown(&mut self) -> bool {
        if self.phase != GamePhase::Lobby {
            return false;
        }

        if !self.all_players_ready() {
            return false;
        }

        if self.map_config.is_none() {
            return false;
        }

        self.phase = GamePhase::Countdown {
            remaining_frames: COUNTDOWN_FRAMES,
        };
        true
    }

    /// Starts the running phase and spawns marbles.
    fn start_running(&mut self) {
        let spawn_area = self
            .map_config
            .as_ref()
            .map(|c| &c.spawn_area)
            .expect("Map config required");

        // Clear any existing marbles
        self.marble_manager.clear(&mut self.physics_world);
        self.eliminated_order.clear();

        // Spawn a marble for each player
        for player in &self.players {
            self.marble_manager.spawn_marble(
                &mut self.physics_world,
                player.id,
                player.color,
                spawn_area,
            );
        }

        self.phase = GamePhase::Running;
    }

    /// Advances the game by one frame.
    /// Returns a list of newly eliminated player IDs.
    pub fn update(&mut self) -> Vec<PlayerId> {
        match &self.phase {
            GamePhase::Lobby | GamePhase::Finished { .. } => Vec::new(),
            GamePhase::Countdown { remaining_frames } => {
                let remaining = *remaining_frames;
                if remaining <= 1 {
                    self.start_running();
                } else {
                    self.phase = GamePhase::Countdown {
                        remaining_frames: remaining - 1,
                    };
                }
                Vec::new()
            }
            GamePhase::Running => {
                // Step physics
                self.physics_world.step();

                // Check for eliminations
                let eliminated_marble_ids =
                    self.marble_manager.check_hole_collisions(&self.physics_world, &self.hole_handles);

                // Map marble IDs to player IDs
                let mut newly_eliminated = Vec::new();
                for marble_id in eliminated_marble_ids {
                    if let Some(marble) = self.marble_manager.get_marble(marble_id) {
                        let player_id = marble.owner_id;
                        if !self.eliminated_order.contains(&player_id) {
                            self.eliminated_order.push(player_id);
                            newly_eliminated.push(player_id);
                        }
                    }
                }

                // Check for game end
                let active_count = self.marble_manager.active_count();
                if active_count <= 1 {
                    // Find the winner (last remaining or none if all eliminated)
                    let winner = self
                        .marble_manager
                        .active_marbles()
                        .first()
                        .map(|m| m.owner_id);

                    self.phase = GamePhase::Finished { winner };
                }

                newly_eliminated
            }
        }
    }

    /// Returns the current game phase.
    pub fn current_phase(&self) -> &GamePhase {
        &self.phase
    }

    /// Returns the winner if the game is finished.
    pub fn winner(&self) -> Option<PlayerId> {
        match &self.phase {
            GamePhase::Finished { winner } => *winner,
            _ => None,
        }
    }

    /// Resets the game to lobby state.
    pub fn reset_to_lobby(&mut self) {
        self.phase = GamePhase::Lobby;
        self.eliminated_order.clear();
        self.marble_manager.clear(&mut self.physics_world);

        // Mark all players as not ready
        for player in &mut self.players {
            player.ready = false;
        }

        // Re-apply map if exists
        if let Some(config) = self.map_config.take() {
            self.load_map(config);
        }
    }

    /// Returns the current frame number.
    pub fn current_frame(&self) -> u64 {
        self.physics_world.current_frame()
    }

    /// Computes a hash of the current game state for synchronization.
    pub fn compute_hash(&self) -> u64 {
        self.physics_world.compute_hash()
    }

    /// Gets a player by ID.
    pub fn get_player(&self, player_id: PlayerId) -> Option<&Player> {
        self.players.iter().find(|p| p.id == player_id)
    }

    /// Gets the ranking of eliminated players (first eliminated = last place).
    pub fn get_rankings(&self) -> Vec<PlayerId> {
        let mut rankings: Vec<PlayerId> = self.eliminated_order.clone();

        // Add active players at the end (if any)
        for marble in self.marble_manager.active_marbles() {
            if !rankings.contains(&marble.owner_id) {
                rankings.push(marble.owner_id);
            }
        }

        // Reverse so winner is first
        rankings.reverse();
        rankings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_game() -> GameState {
        let mut game = GameState::new(12345);
        game.load_map(RouletteConfig::default_classic());
        game
    }

    #[test]
    fn test_game_state_creation() {
        let game = setup_game();
        assert_eq!(game.phase, GamePhase::Lobby);
        assert!(game.players.is_empty());
    }

    #[test]
    fn test_add_remove_players() {
        let mut game = setup_game();

        let p1 = game.add_player("Player 1".to_string(), Color::RED);
        let p2 = game.add_player("Player 2".to_string(), Color::BLUE);

        assert!(p1.is_some());
        assert!(p2.is_some());
        assert_eq!(game.players.len(), 2);

        game.remove_player(p1.unwrap());
        assert_eq!(game.players.len(), 1);
    }

    #[test]
    fn test_ready_and_countdown() {
        let mut game = setup_game();

        game.add_player("Player 1".to_string(), Color::RED);
        game.add_player("Player 2".to_string(), Color::BLUE);

        // Can't start without ready
        assert!(!game.start_countdown());

        // Set ready
        game.set_player_ready(0, true);
        game.set_player_ready(1, true);

        assert!(game.all_players_ready());
        assert!(game.start_countdown());

        assert!(matches!(game.phase, GamePhase::Countdown { .. }));
    }

    #[test]
    fn test_countdown_to_running() {
        let mut game = setup_game();

        game.add_player("Player 1".to_string(), Color::RED);
        game.add_player("Player 2".to_string(), Color::BLUE);
        game.set_player_ready(0, true);
        game.set_player_ready(1, true);
        game.start_countdown();

        // Fast-forward countdown
        for _ in 0..COUNTDOWN_FRAMES {
            game.update();
        }

        assert_eq!(game.phase, GamePhase::Running);
        assert_eq!(game.marble_manager.marbles().len(), 2);
    }

    #[test]
    fn test_game_finish() {
        let mut game = setup_game();

        game.add_player("Player 1".to_string(), Color::RED);
        game.add_player("Player 2".to_string(), Color::BLUE);
        game.set_player_ready(0, true);
        game.set_player_ready(1, true);
        game.start_countdown();

        // Fast-forward to running
        for _ in 0..COUNTDOWN_FRAMES {
            game.update();
        }

        // Manually eliminate one player
        game.marble_manager.get_marble_mut(0).unwrap().eliminate();
        game.eliminated_order.push(0);

        // Update should detect finish
        game.update();

        assert!(matches!(game.phase, GamePhase::Finished { winner: Some(1) }));
        assert_eq!(game.winner(), Some(1));
    }

    #[test]
    fn test_reset_to_lobby() {
        let mut game = setup_game();

        game.add_player("Player 1".to_string(), Color::RED);
        game.set_player_ready(0, true);

        game.reset_to_lobby();

        assert_eq!(game.phase, GamePhase::Lobby);
        assert!(!game.players[0].ready);
    }

    #[test]
    fn test_rankings() {
        let mut game = setup_game();

        game.add_player("P1".to_string(), Color::RED);
        game.add_player("P2".to_string(), Color::BLUE);
        game.add_player("P3".to_string(), Color::GREEN);
        game.set_player_ready(0, true);
        game.set_player_ready(1, true);
        game.set_player_ready(2, true);
        game.start_countdown();

        for _ in 0..COUNTDOWN_FRAMES {
            game.update();
        }

        // Simulate elimination order: P1, then P3
        game.marble_manager.get_marble_mut(0).unwrap().eliminate();
        game.eliminated_order.push(0);
        game.marble_manager.get_marble_mut(2).unwrap().eliminate();
        game.eliminated_order.push(2);

        let rankings = game.get_rankings();
        // Winner (P2) first, then P3 (2nd eliminated), then P1 (1st eliminated)
        assert_eq!(rankings, vec![1, 2, 0]);
    }

    #[test]
    fn test_deterministic_game() {
        // Create two identical games
        let mut game1 = GameState::new(42);
        let mut game2 = GameState::new(42);

        game1.load_map(RouletteConfig::default_classic());
        game2.load_map(RouletteConfig::default_classic());

        // Add same players
        game1.add_player("P1".to_string(), Color::RED);
        game1.add_player("P2".to_string(), Color::BLUE);
        game1.set_player_ready(0, true);
        game1.set_player_ready(1, true);

        game2.add_player("P1".to_string(), Color::RED);
        game2.add_player("P2".to_string(), Color::BLUE);
        game2.set_player_ready(0, true);
        game2.set_player_ready(1, true);

        // Start both
        game1.start_countdown();
        game2.start_countdown();

        // Run for same number of frames
        for _ in 0..300 {
            game1.update();
            game2.update();
        }

        // Hashes should be identical
        assert_eq!(game1.compute_hash(), game2.compute_hash());
    }
}
