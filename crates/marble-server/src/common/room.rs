use chrono::{DateTime, Utc};
use marble_proto::room::{PeerConnectionStatus, PeerTopology, RoomState};

use crate::common::player::Player;
use crate::topology::{TopologyManager, TopologyManagerConfig};

#[derive(Debug, Clone)]
pub struct Room {
    id: uuid::Uuid,
    max_players: u32,
    host: Player,
    other_players: Vec<Player>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    topology_manager: TopologyManager,

    // Game state (set when game starts via StartGame RPC)
    game_start_frame: Option<u64>,
    game_rng_seed: Option<u64>,
    game_results: Vec<PlayerResult>,
}

/// Player arrival result
#[derive(Debug, Clone)]
pub struct PlayerResult {
    pub player_id: String,
    pub rank: u32,
    pub arrival_frame: u64,
}

#[derive(thiserror::Error, Debug)]
pub enum RoomError {
    #[error("Room is full")]
    RoomFull,

    #[error("Player not found in the room")]
    PlayerNotFound,

    #[error("Room host can not kick")]
    HostCanNotKick,

    #[error("Only the room host can perform this action: {0}")]
    RoomHostOnly(&'static str),

    #[error("Game already started")]
    GameAlreadyStarted,

    #[error("Game not started yet")]
    GameNotStarted,

    #[error("Player already arrived: {0}")]
    PlayerAlreadyArrived(String),
}

impl RoomError {
    pub fn to_code(&self) -> tonic::Code {
        match self {
            RoomError::RoomFull => tonic::Code::ResourceExhausted,
            RoomError::PlayerNotFound => tonic::Code::NotFound,
            RoomError::HostCanNotKick => tonic::Code::InvalidArgument,
            RoomError::RoomHostOnly(_) => tonic::Code::PermissionDenied,
            RoomError::GameAlreadyStarted => tonic::Code::AlreadyExists,
            RoomError::GameNotStarted => tonic::Code::FailedPrecondition,
            RoomError::PlayerAlreadyArrived(_) => tonic::Code::AlreadyExists,
        }
    }
}

impl From<RoomError> for tonic::Status {
    fn from(err: RoomError) -> Self {
        tonic::Status::new(err.to_code(), err.to_string())
    }
}

impl Room {
    pub fn new(id: uuid::Uuid, max_players: u32, host: Player) -> Self {
        // Calculate optimal topology config based on max_players
        let config = TopologyManagerConfig {
            mesh_group_size: (max_players / 3).max(10).min(40),
            peer_connections: 5,
            bridges_per_group: 2,
            gossip_ttl: 10,
            lockstep_delay_frames: 6,
        };

        let mut topology_manager = TopologyManager::new(config);

        // Add host to topology (peer_id will be updated on actual connection)
        topology_manager.add_player(&host.id, &format!("pending_{}", host.id));

        Self {
            id,
            max_players,
            host,
            other_players: Vec::new(),
            created_at: Utc::now(),
            started_at: None,
            topology_manager,
            game_start_frame: None,
            game_rng_seed: None,
            game_results: Vec::new(),
        }
    }

    pub fn id(&self) -> &uuid::Uuid {
        &self.id
    }

    pub fn max_players(&self) -> u32 {
        self.max_players
    }

    pub fn count_players(&self) -> u32 {
        1 + self.other_players.len() as u32
    }

    pub fn stated_at(&self) -> Option<DateTime<Utc>> {
        self.started_at
    }

    pub fn assert_host(
        &self,
        player_id: &str,
        player_secret: &str,
        feature: &'static str,
    ) -> Result<(), RoomError> {
        if self.host.id == player_id && self.host.secret == player_secret {
            Ok(())
        } else {
            Err(RoomError::RoomHostOnly(feature))
        }
    }

    pub fn once_started_at(&mut self, started_at: DateTime<Utc>) -> bool {
        if self.started_at.is_none() {
            self.started_at = Some(started_at);
            true
        } else {
            false
        }
    }

    pub fn iter_players(&self) -> impl Iterator<Item = &Player> {
        std::iter::once(&self.host).chain(self.other_players.iter())
    }
    pub fn iter_other_players(&self) -> impl Iterator<Item = &Player> {
        self.other_players.iter()
    }
    pub fn host_player(&self) -> &Player {
        &self.host
    }

    /// Add a player to the room, or return existing topology if already joined.
    /// This is idempotent - calling multiple times with the same player is safe.
    pub fn add_player(&mut self, player: Player) -> Result<PeerTopology, RoomError> {
        // If player already exists, return their existing topology (idempotent)
        if let Some(topology) = self.get_topology(&player.id) {
            return Ok(topology);
        }

        if self.count_players() >= self.max_players {
            return Err(RoomError::RoomFull);
        }

        // Add to topology (peer_id will be updated on actual connection)
        let peer_id = format!("pending_{}", player.id);
        let topology = self.topology_manager.add_player(&player.id, &peer_id);

        self.other_players.push(player);
        Ok(topology)
    }

    pub fn kick_player(&mut self, player_id: &str) -> Result<(), RoomError> {
        if self.host.id == player_id {
            return Err(RoomError::HostCanNotKick);
        }
        let initial_len = self.other_players.len();
        self.other_players.retain(|p| p.id != player_id);
        if self.other_players.len() == initial_len {
            return Err(RoomError::PlayerNotFound);
        }
        self.topology_manager.remove_player(player_id);
        Ok(())
    }

    pub fn state(&self) -> RoomState {
        // WAITING → STARTED (game spawn) → ENDED (all players arrived)
        if self.started_at.is_none() {
            return RoomState::Waiting;
        }

        // Check if game has started (spawn happened)
        if self.game_start_frame.is_none() {
            return RoomState::Started;
        }

        // Check if all players have arrived
        let total_players = self.count_players() as usize;
        if self.game_results.len() >= total_players {
            return RoomState::Ended;
        }

        RoomState::Started
    }
    pub fn started_at(&self) -> Option<DateTime<Utc>> {
        self.started_at.clone()
    }

    /// Get topology for a player
    pub fn get_topology(&self, player_id: &str) -> Option<PeerTopology> {
        self.topology_manager.get_topology(player_id)
    }

    /// Update connection status and return new topology if changed
    pub fn update_connection_status(
        &mut self,
        player_id: &str,
        statuses: &[PeerConnectionStatus],
    ) -> Option<PeerTopology> {
        self.topology_manager
            .update_connection_status(player_id, statuses)
    }

    /// Get topology config
    pub fn topology_config(&self) -> &TopologyManagerConfig {
        &self.topology_manager.config
    }

    /// Check if a player is in the room
    pub fn has_player(&self, player_id: &str) -> bool {
        self.host.id == player_id || self.other_players.iter().any(|p| p.id == player_id)
    }

    /// Verify player credentials (returns true if player exists and secret matches)
    pub fn verify_player(&self, player_id: &str, player_secret: &str) -> bool {
        if self.host.id == player_id && self.host.secret == player_secret {
            return true;
        }
        self.other_players
            .iter()
            .any(|p| p.id == player_id && p.secret == player_secret)
    }

    /// Update peer_id for a player and return updated topology if successful
    pub fn update_peer_id(&mut self, player_id: &str, peer_id: &str) -> Option<PeerTopology> {
        if self.topology_manager.update_peer_id(player_id, peer_id) {
            self.get_topology(player_id)
        } else {
            None
        }
    }

    /// Get all players' topologies
    pub fn get_all_topologies(&self) -> Vec<(String, PeerTopology)> {
        self.iter_players()
            .filter_map(|player| {
                self.get_topology(&player.id)
                    .map(|topo| (player.id.clone(), topo))
            })
            .collect()
    }

    /// Resolve peer_ids to player_ids
    pub fn resolve_peer_ids(
        &self,
        peer_ids: &[String],
    ) -> std::collections::HashMap<String, String> {
        self.topology_manager.resolve_peer_ids(peer_ids)
    }

    // ========================================
    // Game state methods
    // ========================================

    /// Start the game (spawn marbles). Can only be called once.
    /// Returns Ok(true) if newly started, Ok(false) if already started (idempotent).
    pub fn start_game(
        &mut self,
        player_id: &str,
        player_secret: &str,
        start_frame: u64,
        rng_seed: u64,
    ) -> Result<bool, RoomError> {
        // Only host can start the game
        self.assert_host(player_id, player_secret, "start_game")?;

        // Check if game already started (idempotent behavior)
        if self.game_start_frame.is_some() {
            return Ok(false);
        }

        // Also mark room as started if not already
        if self.started_at.is_none() {
            self.started_at = Some(Utc::now());
        }

        self.game_start_frame = Some(start_frame);
        self.game_rng_seed = Some(rng_seed);
        self.game_results.clear();

        Ok(true)
    }

    /// Report a player's arrival at the hole.
    /// Returns Ok(true) if all players have arrived (game ended).
    pub fn report_arrival(
        &mut self,
        player_id: &str,
        player_secret: &str,
        arrived_player_id: &str,
        arrival_frame: u64,
        rank: u32,
    ) -> Result<bool, RoomError> {
        // Only host can report arrivals
        self.assert_host(player_id, player_secret, "report_arrival")?;

        // Game must be started
        if self.game_start_frame.is_none() {
            return Err(RoomError::GameNotStarted);
        }

        // Check if player already arrived
        if self
            .game_results
            .iter()
            .any(|r| r.player_id == arrived_player_id)
        {
            return Err(RoomError::PlayerAlreadyArrived(
                arrived_player_id.to_string(),
            ));
        }

        // Add result
        self.game_results.push(PlayerResult {
            player_id: arrived_player_id.to_string(),
            rank,
            arrival_frame,
        });

        // Check if all players have arrived
        let total_players = self.count_players() as usize;
        Ok(self.game_results.len() >= total_players)
    }

    /// Check if game has started (spawn happened)
    pub fn is_game_started(&self) -> bool {
        self.game_start_frame.is_some()
    }

    /// Get game start frame
    pub fn game_start_frame(&self) -> Option<u64> {
        self.game_start_frame
    }

    /// Get game RNG seed
    pub fn game_rng_seed(&self) -> Option<u64> {
        self.game_rng_seed
    }

    /// Get game results
    pub fn game_results(&self) -> &[PlayerResult] {
        &self.game_results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_room() -> Room {
        let host = Player::new("host_id".to_string(), "host_secret".to_string());
        Room::new(uuid::Uuid::new_v4(), 10, host)
    }

    #[test]
    fn test_add_player_idempotent() {
        let mut room = create_test_room();

        // Add a new player
        let player = Player::new("player1".to_string(), "secret1".to_string());
        let topology1 = room.add_player(player.clone()).unwrap();

        // Adding the same player again should succeed and return the same topology
        let topology2 = room.add_player(player.clone()).unwrap();

        assert_eq!(topology1.mesh_group, topology2.mesh_group);
        assert_eq!(topology1.is_bridge, topology2.is_bridge);

        // Player count should remain the same
        assert_eq!(room.count_players(), 2); // host + 1 player
    }

    #[test]
    fn test_host_join_idempotent() {
        let mut room = create_test_room();

        // Host trying to "join" should return their existing topology
        let host_player = Player::new("host_id".to_string(), "host_secret".to_string());
        let topology = room.add_player(host_player).unwrap();

        // Should succeed and return host's topology
        assert!(room.has_player("host_id"));
        assert_eq!(room.count_players(), 1); // only host, not duplicated
    }

    #[test]
    fn test_room_full_error() {
        let host = Player::new("host".to_string(), "secret".to_string());
        let mut room = Room::new(uuid::Uuid::new_v4(), 2, host); // max 2 players

        // Add one more player (room now full)
        let player1 = Player::new("player1".to_string(), "secret1".to_string());
        room.add_player(player1).unwrap();

        // Try to add another player - should fail
        let player2 = Player::new("player2".to_string(), "secret2".to_string());
        let result = room.add_player(player2);
        assert!(matches!(result, Err(RoomError::RoomFull)));
    }
}
