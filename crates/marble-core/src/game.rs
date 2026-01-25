//! Game state machine and physics simulation management.

use std::collections::HashMap;

use rapier2d::prelude::{ColliderHandle, RigidBodyHandle, Vector};
use serde::{Deserialize, Serialize};

use crate::dsl::GameContext;
use crate::keyframe::KeyframeExecutor;
use crate::map::{BlackholeData, EvaluatedShape, LiveRankingConfig, RollDirection, RouletteConfig, SpawnerData};
use crate::marble::{Color, MarbleManager, PlayerId};
use crate::physics::{PhysicsWorld, PHYSICS_DT};

/// Player information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub color: Color,
}

impl Player {
    pub fn new(id: PlayerId, name: String, color: Color) -> Self {
        Self { id, name, color }
    }
}

/// Complete game state containing all game data.
///
/// This represents a sandbox physics simulation that runs continuously.
/// Players and marbles can be added/removed at any time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub players: Vec<Player>,
    /// Order in which marbles arrived at triggers.
    pub arrival_order: Vec<PlayerId>,
    pub rng_seed: u64,
    /// Selected gamerule (e.g., "top_n", "last_n").
    pub selected_gamerule: String,
    #[serde(skip)]
    pub physics_world: PhysicsWorld,
    pub marble_manager: MarbleManager,
    #[serde(skip)]
    pub map_config: Option<RouletteConfig>,
    /// Trigger (hole) handles for arrival detection.
    #[serde(skip)]
    pub trigger_handles: Vec<ColliderHandle>,
    /// Trigger actions corresponding to each trigger handle.
    /// "gamerule" triggers will completely remove marbles from memory.
    #[serde(skip)]
    pub trigger_actions: Vec<String>,
    /// Spawner data from the map.
    #[serde(skip)]
    pub spawners: Vec<SpawnerData>,
    /// Blackhole data for force application.
    #[serde(skip)]
    pub blackholes: Vec<BlackholeData>,
    /// Cached game context for CEL expression evaluation.
    #[serde(skip)]
    game_context: GameContext,
    /// Kinematic body handles for animated objects.
    #[serde(skip)]
    pub kinematic_bodies: HashMap<String, RigidBodyHandle>,
    /// Initial transforms for kinematic bodies (for keyframe animations).
    #[serde(skip)]
    pub kinematic_initial_transforms: HashMap<String, ([f32; 2], f32)>,
    /// Active keyframe animation executors.
    pub keyframe_executors: Vec<KeyframeExecutor>,
}

impl GameState {
    /// Creates a new game state with the given RNG seed.
    pub fn new(seed: u64) -> Self {
        Self {
            players: Vec::new(),
            arrival_order: Vec::new(),
            rng_seed: seed,
            selected_gamerule: String::new(),
            physics_world: PhysicsWorld::new(),
            marble_manager: MarbleManager::new(seed),
            map_config: None,
            trigger_handles: Vec::new(),
            trigger_actions: Vec::new(),
            spawners: Vec::new(),
            blackholes: Vec::new(),
            game_context: GameContext::with_cache_and_seed(seed),
            kinematic_bodies: HashMap::new(),
            kinematic_initial_transforms: HashMap::new(),
            keyframe_executors: Vec::new(),
        }
    }

    /// Loads a map configuration and initializes the physics world.
    pub fn load_map(&mut self, config: RouletteConfig) {
        // Reset physics world
        self.physics_world.reset();

        // Apply map to world
        let map_data = config.apply_to_world(&mut self.physics_world);
        self.trigger_handles = map_data.trigger_handles;
        self.trigger_actions = map_data.trigger_actions;
        self.spawners = map_data.spawners;
        self.blackholes = map_data.blackholes;
        self.kinematic_bodies = map_data.kinematic_bodies;
        self.kinematic_initial_transforms = map_data.kinematic_initial_transforms;

        // Initialize keyframe executors for autoplay sequences
        self.keyframe_executors.clear();
        for seq in &config.keyframes {
            if seq.autoplay {
                self.keyframe_executors.push(KeyframeExecutor::new(seq.name.clone()));
            }
        }

        self.map_config = Some(config);
    }

    /// Adds a player to the game.
    /// Returns the player ID.
    pub fn add_player(&mut self, name: String, color: Color) -> PlayerId {
        #[allow(clippy::cast_possible_truncation)]
        let id = self.players.len() as PlayerId;
        self.players.push(Player::new(id, name, color));
        id
    }

    /// Removes a player from the game.
    pub fn remove_player(&mut self, player_id: PlayerId) -> bool {
        if let Some(pos) = self.players.iter().position(|p| p.id == player_id) {
            self.players.remove(pos);
            true
        } else {
            false
        }
    }

    /// Spawns marbles for all players.
    /// Clears existing marbles first.
    /// Returns false if no spawners are available or no players.
    pub fn spawn_marbles(&mut self) -> bool {
        // Clear existing marbles
        self.marble_manager.clear(&mut self.physics_world);
        self.arrival_order.clear();

        if self.spawners.is_empty() || self.players.is_empty() {
            return false;
        }

        let spawner = &self.spawners[0];

        for player in &self.players {
            self.marble_manager.spawn_from_spawner(
                &mut self.physics_world,
                player.id,
                player.color,
                spawner,
            );
        }
        true
    }

    /// Advances the game by one frame.
    /// Returns a list of newly arrived player IDs.
    pub fn update(&mut self) -> Vec<PlayerId> {
        // If no map is loaded, do nothing
        if self.map_config.is_none() {
            return Vec::new();
        }

        // Update game context for CEL expressions
        let time = self.physics_world.current_frame() as f32 / 60.0;
        self.game_context.update(time, self.physics_world.current_frame());

        // Apply roll rotations to animated objects
        self.apply_roll_rotations();

        // Update keyframe animations
        self.update_keyframes();

        // Apply blackhole forces before physics step
        self.apply_blackhole_forces();

        // Step physics
        self.physics_world.step();

        // Check for arrivals at triggers
        self.check_arrivals()
    }

    /// Checks for marbles arriving at triggers.
    /// Handles marbles based on trigger action:
    /// - "gamerule": completely removes marble from memory
    /// - other: disables physics but keeps in memory
    fn check_arrivals(&mut self) -> Vec<PlayerId> {
        let arrived = self
            .marble_manager
            .check_hole_collisions(&self.physics_world, &self.trigger_handles);

        // Collect marbles to remove (for "gamerule" triggers)
        let mut marbles_to_remove = Vec::new();
        let mut marbles_to_disable = Vec::new();

        // Map marble IDs to player IDs and determine action
        let mut newly_arrived = Vec::new();
        for (marble_id, trigger_idx) in &arrived {
            if let Some(marble) = self.marble_manager.get_marble(*marble_id) {
                let player_id = marble.owner_id;
                if !self.arrival_order.contains(&player_id) {
                    self.arrival_order.push(player_id);
                    newly_arrived.push(player_id);
                }

                // Check trigger action
                let action = self.trigger_actions
                    .get(*trigger_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("gamerule");

                if action == "gamerule" {
                    marbles_to_remove.push(*marble_id);
                } else {
                    marbles_to_disable.push(*marble_id);
                }
            }
        }

        // Remove marbles for "gamerule" triggers (completely from memory)
        for marble_id in marbles_to_remove {
            self.marble_manager.remove_marble(&mut self.physics_world, marble_id);
        }

        // Disable physics for other triggers (keep in memory)
        for marble_id in marbles_to_disable {
            self.marble_manager.disable_marble_physics(&mut self.physics_world, marble_id);
        }

        newly_arrived
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

    /// Eliminates a player's marble (e.g., when they disconnect).
    /// Returns true if the player was eliminated, false if already eliminated or not found.
    pub fn eliminate_player(&mut self, player_id: PlayerId) -> bool {
        // Check if already in arrival order
        if self.arrival_order.contains(&player_id) {
            return false;
        }

        // Find and eliminate the marble
        if let Some(marble) = self.marble_manager.get_marble_by_owner_mut(player_id) {
            if !marble.eliminated {
                marble.eliminate();
                self.arrival_order.push(player_id);
                return true;
            }
        }

        false
    }

    /// Returns the current arrival order.
    pub fn arrival_order(&self) -> &[PlayerId] {
        &self.arrival_order
    }

    /// Returns the leaderboard based on the selected gamerule.
    ///
    /// - `top_n`: First to arrive = highest rank (arrival_order as-is)
    /// - `last_n`: Last to arrive = highest rank (arrival_order reversed)
    pub fn leaderboard(&self) -> Vec<PlayerId> {
        match self.selected_gamerule.as_str() {
            "last_n" => self.arrival_order.iter().copied().rev().collect(),
            _ => self.arrival_order.clone(), // "top_n" or default
        }
    }

    /// Returns the available gamerules from the loaded map.
    pub fn available_gamerules(&self) -> Vec<String> {
        self.map_config
            .as_ref()
            .map(|c| c.meta.gamerule.clone())
            .unwrap_or_default()
    }

    /// Sets the selected gamerule.
    pub fn set_gamerule(&mut self, gamerule: String) {
        self.selected_gamerule = gamerule;
    }

    /// Returns the selected gamerule.
    pub fn gamerule(&self) -> &str {
        &self.selected_gamerule
    }

    /// Applies blackhole forces to all active marbles.
    fn apply_blackhole_forces(&mut self) {
        if self.blackholes.is_empty() {
            return;
        }

        for blackhole in &self.blackholes {
            // Evaluate the force using the current game context (supports CEL expressions)
            let force = blackhole.force.evaluate(&self.game_context);
            if force.abs() < f32::EPSILON {
                continue;
            }

            // Get blackhole center
            let shape = blackhole.shape.evaluate(&self.game_context);
            let center = match shape {
                EvaluatedShape::Circle { center, .. } => center,
                EvaluatedShape::Rect { center, .. } => center,
                EvaluatedShape::Line { .. } => continue,
                EvaluatedShape::Bezier { .. } => continue,
            };

            // Apply force to all active marbles
            for marble in self.marble_manager.marbles() {
                if marble.eliminated {
                    continue;
                }

                if let Some(body) = self.physics_world.get_rigid_body_mut(marble.body_handle) {
                    let pos = body.translation();
                    let dx = center[0] - pos.x;
                    let dy = center[1] - pos.y;
                    let dist_sq = dx * dx + dy * dy;
                    let dist = dist_sq.sqrt().max(1.0); // Prevent division by zero

                    // Force magnitude inversely proportional to distance
                    let force_magnitude = force * 1000.0 / dist;
                    let force_vec = Vector::new(dx / dist * force_magnitude, dy / dist * force_magnitude);

                    body.add_force(force_vec, true);
                }
            }
        }
    }

    /// Applies roll rotation to objects with the roll property.
    fn apply_roll_rotations(&mut self) {
        let config = match &self.map_config {
            Some(c) => c,
            None => return,
        };

        for obj in &config.objects {
            let roll = match &obj.properties.roll {
                Some(r) => r,
                None => continue,
            };

            let obj_id = match &obj.id {
                Some(id) => id,
                None => continue,
            };

            let body_handle = match self.kinematic_bodies.get(obj_id) {
                Some(h) => *h,
                None => continue,
            };

            // Calculate rotation speed in radians per frame
            let speed_deg_per_sec = roll.speed;
            let speed_rad_per_frame = speed_deg_per_sec.to_radians() * PHYSICS_DT;

            let direction_mult = match roll.direction {
                RollDirection::Clockwise => 1.0,
                RollDirection::Counterclockwise => -1.0,
            };

            // Get current position and rotation
            if let Some((pos, current_rot)) = self.physics_world.get_body_position(body_handle) {
                let new_rot = current_rot + speed_rad_per_frame * direction_mult;
                self.physics_world.set_kinematic_target(
                    body_handle,
                    Vector::new(pos[0], pos[1]),
                    new_rot,
                );
            }
        }
    }

    /// Updates keyframe animations.
    fn update_keyframes(&mut self) {
        let config = match &self.map_config {
            Some(c) => c.clone(),
            None => return,
        };

        // Collect current positions of kinematic bodies
        let mut current_positions = HashMap::new();
        for (id, handle) in &self.kinematic_bodies {
            if let Some(pos_rot) = self.physics_world.get_body_position(*handle) {
                current_positions.insert(id.clone(), pos_rot);
            }
        }

        // Update each executor
        for executor in &mut self.keyframe_executors {
            let updates = executor.update(
                PHYSICS_DT,
                &config.keyframes,
                &current_positions,
                &self.kinematic_initial_transforms,
                &mut self.game_context,
            );

            // Apply updates to kinematic bodies
            for (id, pos, rot) in updates {
                if let Some(&handle) = self.kinematic_bodies.get(&id) {
                    self.physics_world.set_kinematic_target(
                        handle,
                        Vector::new(pos[0], pos[1]),
                        rot,
                    );
                }
            }
        }

        // Remove finished executors (but keep infinite loops running)
        self.keyframe_executors.retain(|e| !e.is_finished());
    }

    /// Returns the current game context for CEL expression evaluation.
    pub fn game_context(&self) -> &GameContext {
        &self.game_context
    }

    /// Calculates ranking score based on live_ranking configuration.
    /// Lower score = higher rank.
    pub fn calculate_ranking_score(&self, marble_pos: (f32, f32)) -> f32 {
        match &self.map_config {
            Some(config) => match &config.meta.live_ranking {
                LiveRankingConfig::YPosition => marble_pos.1,
                LiveRankingConfig::Distance { target_id } => {
                    self.get_object_center(target_id)
                        .map(|(tx, ty)| {
                            let dx = marble_pos.0 - tx;
                            let dy = marble_pos.1 - ty;
                            (dx * dx + dy * dy).sqrt()
                        })
                        .unwrap_or(marble_pos.1) // fallback to y
                }
            },
            None => marble_pos.1,
        }
    }

    /// Gets the center position of an object by its ID.
    fn get_object_center(&self, object_id: &str) -> Option<(f32, f32)> {
        let config = self.map_config.as_ref()?;
        let obj = config.objects.iter().find(|o| o.id.as_deref() == Some(object_id))?;

        // Evaluate shape to get center
        let shape = obj.shape.evaluate(&self.game_context);
        match shape {
            EvaluatedShape::Circle { center, .. } => Some((center[0], center[1])),
            EvaluatedShape::Rect { center, .. } => Some((center[0], center[1])),
            EvaluatedShape::Line { start, end } => {
                // For Line, use midpoint
                Some(((start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0))
            }
            EvaluatedShape::Bezier { start, end, .. } => {
                // For Bezier, use midpoint of start/end
                Some(((start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0))
            }
        }
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
        assert!(game.players.is_empty());
        assert!(game.arrival_order.is_empty());
    }

    #[test]
    fn test_add_remove_players() {
        let mut game = setup_game();

        let p1 = game.add_player("Player 1".to_string(), Color::RED);
        let p2 = game.add_player("Player 2".to_string(), Color::BLUE);

        assert_eq!(p1, 0);
        assert_eq!(p2, 1);
        assert_eq!(game.players.len(), 2);

        game.remove_player(p1);
        assert_eq!(game.players.len(), 1);
    }

    #[test]
    fn test_spawn_marbles() {
        let mut game = setup_game();

        game.add_player("Player 1".to_string(), Color::RED);
        game.add_player("Player 2".to_string(), Color::BLUE);

        assert!(game.spawn_marbles());
        assert_eq!(game.marble_manager.marbles().len(), 2);
    }

    #[test]
    fn test_physics_runs_immediately() {
        let mut game = setup_game();

        game.add_player("Player 1".to_string(), Color::RED);
        game.add_player("Player 2".to_string(), Color::BLUE);

        game.spawn_marbles();

        // Physics should update without any phase transitions
        for _ in 0..60 {
            game.update();
        }

        assert_eq!(game.current_frame(), 60);
    }

    #[test]
    fn test_leaderboard_top_n() {
        let mut game = setup_game();
        game.set_gamerule("top_n".to_string());

        game.arrival_order = vec![0, 1, 2];

        let leaderboard = game.leaderboard();
        // top_n: first to arrive = first in leaderboard
        assert_eq!(leaderboard, vec![0, 1, 2]);
    }

    #[test]
    fn test_leaderboard_last_n() {
        let mut game = setup_game();
        game.set_gamerule("last_n".to_string());

        game.arrival_order = vec![0, 1, 2];

        let leaderboard = game.leaderboard();
        // last_n: last to arrive = first in leaderboard
        assert_eq!(leaderboard, vec![2, 1, 0]);
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

        game2.add_player("P1".to_string(), Color::RED);
        game2.add_player("P2".to_string(), Color::BLUE);

        // Spawn marbles
        game1.spawn_marbles();
        game2.spawn_marbles();

        // Run for same number of frames
        for _ in 0..120 {
            game1.update();
            game2.update();
        }

        // Hashes should be identical
        assert_eq!(game1.compute_hash(), game2.compute_hash());
    }
}
