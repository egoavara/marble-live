use chrono::{DateTime, Utc};
use marble_proto::room::{
    GameState as ProtoGameState, NetworkConfig, PeerConnectionStatus, PeerTopology, PlayerResult,
    RoomInfo, RoomRole, RoomState, RoomSummary, RoomUser,
};
use rand::Rng;

use crate::common::player::RoomMember;
use crate::topology::{TopologyManager, TopologyManagerConfig};

#[derive(Debug, Clone)]
pub struct Room {
    id: uuid::Uuid,
    name: String,
    map_id: String,
    max_players: u32,
    is_public: bool,
    host_user_id: String,
    members: Vec<RoomMember>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    topology_manager: TopologyManager,
    topology_version: u64,
    signaling_base_url: String,

    // Game state
    rng_seed: u64,
    game_start_frame: Option<u64>,
    game_results: Vec<GameResult>,
}

#[derive(Debug, Clone)]
pub struct GameResult {
    pub user_id: String,
    pub rank: u32,
    pub arrival_frame: u64,
}

#[derive(thiserror::Error, Debug)]
pub enum RoomError {
    #[error("Room is full")]
    RoomFull,

    #[error("User not found in the room")]
    UserNotFound,

    #[error("Cannot kick the room host")]
    HostCanNotKick,

    #[error("Only the room host can perform this action: {0}")]
    RoomHostOnly(&'static str),

    #[error("Game not started yet")]
    GameNotStarted,
}

impl RoomError {
    pub fn to_code(&self) -> tonic::Code {
        match self {
            Self::RoomFull => tonic::Code::ResourceExhausted,
            Self::UserNotFound => tonic::Code::NotFound,
            Self::HostCanNotKick => tonic::Code::InvalidArgument,
            Self::RoomHostOnly(_) => tonic::Code::PermissionDenied,
            Self::GameNotStarted => tonic::Code::FailedPrecondition,
        }
    }
}

impl From<RoomError> for tonic::Status {
    fn from(err: RoomError) -> Self {
        tonic::Status::new(err.to_code(), err.to_string())
    }
}

#[allow(dead_code)]
impl Room {
    pub fn new(
        id: uuid::Uuid,
        room_name: String,
        map_id: String,
        max_players: u32,
        is_public: bool,
        host_user_id: String,
        signaling_base_url: String,
    ) -> Self {
        let config = TopologyManagerConfig {
            mesh_group_size: (max_players / 3).clamp(10, 40),
            peer_connections: 5,
            bridges_per_group: 2,
            gossip_ttl: 10,
            lockstep_delay_frames: 6,
        };

        let mut topology_manager = TopologyManager::new(config);
        topology_manager.add_player(&host_user_id, &format!("pending_{host_user_id}"));

        let host = RoomMember::new_host(host_user_id.clone());
        let rng_seed = rand::rng().random::<u64>();

        Self {
            id,
            name: room_name,
            map_id,
            max_players,
            is_public,
            host_user_id,
            members: vec![host],
            created_at: Utc::now(),
            started_at: None,
            topology_manager,
            topology_version: 1,
            signaling_base_url,
            rng_seed,
            game_start_frame: None,
            game_results: Vec::new(),
        }
    }

    // === Accessors ===

    pub fn id(&self) -> &uuid::Uuid {
        &self.id
    }

    pub fn max_players(&self) -> u32 {
        self.max_players
    }

    pub fn participant_count(&self) -> u32 {
        u32::try_from(
            self.members
                .iter()
                .filter(|m| m.role == RoomRole::Participant)
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    pub fn member_count(&self) -> u32 {
        u32::try_from(self.members.len()).unwrap_or(u32::MAX)
    }

    pub fn host_user_id(&self) -> &str {
        &self.host_user_id
    }

    pub fn is_public(&self) -> bool {
        self.is_public
    }

    pub fn started_at(&self) -> Option<DateTime<Utc>> {
        self.started_at
    }

    pub fn topology_config(&self) -> &TopologyManagerConfig {
        &self.topology_manager.config
    }

    pub fn rng_seed(&self) -> u64 {
        self.rng_seed
    }

    pub fn game_start_frame(&self) -> Option<u64> {
        self.game_start_frame
    }

    pub fn topology_version(&self) -> u64 {
        self.topology_version
    }

    fn signaling_url(&self) -> String {
        format!("{}/{}", self.signaling_base_url, self.id)
    }

    // === State ===

    pub fn state(&self) -> RoomState {
        if self.started_at.is_none() {
            return RoomState::Waiting;
        }
        if self.game_start_frame.is_none() {
            return RoomState::Playing;
        }
        let participant_count = self.participant_count() as usize;
        if self.game_results.len() >= participant_count {
            return RoomState::Ended;
        }
        RoomState::Playing
    }

    // === Auth ===

    pub fn assert_host(&self, user_id: &str, feature: &'static str) -> Result<(), RoomError> {
        if self.host_user_id == user_id {
            Ok(())
        } else {
            Err(RoomError::RoomHostOnly(feature))
        }
    }

    pub fn has_member(&self, user_id: &str) -> bool {
        self.members.iter().any(|m| m.user_id == user_id)
    }

    // === Room management ===

    /// Add a user to the room. Idempotent.
    pub fn add_user(
        &mut self,
        user_id: String,
        role: Option<RoomRole>,
    ) -> Result<PeerTopology, RoomError> {
        // Idempotent: if user already exists, return their topology
        if let Some(topology) = self.get_topology(&user_id) {
            return Ok(topology);
        }

        let actual_role = match role {
            Some(RoomRole::Spectator) => RoomRole::Spectator,
            Some(RoomRole::Participant | RoomRole::Unspecified) | None => {
                if self.participant_count() >= self.max_players {
                    RoomRole::Spectator
                } else {
                    RoomRole::Participant
                }
            }
        };

        if actual_role == RoomRole::Participant && self.participant_count() >= self.max_players {
            return Err(RoomError::RoomFull);
        }

        let peer_id = format!("pending_{user_id}");
        let mut topology = self.topology_manager.add_player(&user_id, &peer_id);
        topology.signaling_url = self.signaling_url();

        let member = match actual_role {
            RoomRole::Spectator => RoomMember::new_spectator(user_id),
            _ => RoomMember::new_participant(user_id),
        };
        self.members.push(member);

        Ok(topology)
    }

    pub fn kick_user(&mut self, target_user_id: &str) -> Result<(), RoomError> {
        if self.host_user_id == target_user_id {
            return Err(RoomError::HostCanNotKick);
        }
        let initial_len = self.members.len();
        self.members.retain(|m| m.user_id != target_user_id);
        if self.members.len() == initial_len {
            return Err(RoomError::UserNotFound);
        }
        self.topology_manager.remove_player(target_user_id);
        self.topology_version += 1;
        Ok(())
    }

    pub fn get_room_users(&self) -> Vec<RoomUser> {
        self.members
            .iter()
            .map(|m| RoomUser {
                user_id: m.user_id.clone(),
                is_host: m.is_host,
                role: m.role.into(),
                joined_at: m.joined_at.to_rfc3339(),
            })
            .collect()
    }

    // === Game lifecycle ===

    /// Start the game. Can only be called once. Also marks room as started.
    pub fn start_game(
        &mut self,
        user_id: &str,
        start_frame: u64,
    ) -> Result<bool, RoomError> {
        self.assert_host(user_id, "start_game")?;

        if self.game_start_frame.is_some() {
            return Ok(false); // Idempotent
        }

        if self.started_at.is_none() {
            self.started_at = Some(Utc::now());
        }

        self.game_start_frame = Some(start_frame);
        self.game_results.clear();

        Ok(true)
    }

    /// Report a player's arrival. Host only. Idempotent — duplicate arrivals are ignored.
    pub fn report_arrival(
        &mut self,
        user_id: &str,
        arrived_user_id: &str,
        arrival_frame: u64,
        rank: u32,
    ) -> Result<bool, RoomError> {
        self.assert_host(user_id, "report_arrival")?;

        if self.game_start_frame.is_none() {
            return Err(RoomError::GameNotStarted);
        }

        // Idempotent: skip if already reported
        if !self
            .game_results
            .iter()
            .any(|r| r.user_id == arrived_user_id)
        {
            self.game_results.push(GameResult {
                user_id: arrived_user_id.to_string(),
                rank,
                arrival_frame,
            });
        }

        let participant_count = self.participant_count() as usize;
        Ok(self.game_results.len() >= participant_count)
    }

    // === Topology ===

    pub fn get_topology(&self, user_id: &str) -> Option<PeerTopology> {
        self.topology_manager.get_topology(user_id).map(|mut t| {
            t.signaling_url = self.signaling_url();
            t
        })
    }

    pub fn update_connection_status(
        &mut self,
        user_id: &str,
        statuses: &[PeerConnectionStatus],
    ) -> Option<PeerTopology> {
        let result = self
            .topology_manager
            .update_connection_status(user_id, statuses);
        if result.is_some() {
            self.topology_version += 1;
        }
        result.map(|mut t| {
            t.signaling_url = self.signaling_url();
            t
        })
    }

    pub fn update_peer_id(&mut self, user_id: &str, peer_id: &str) -> Option<PeerTopology> {
        if self.topology_manager.update_peer_id(user_id, peer_id) {
            self.get_topology(user_id)
        } else {
            None
        }
    }

    pub fn get_all_topologies(&self) -> Vec<(String, PeerTopology)> {
        self.members
            .iter()
            .filter_map(|m| {
                self.get_topology(&m.user_id)
                    .map(|topo| (m.user_id.clone(), topo))
            })
            .collect()
    }

    pub fn resolve_peer_ids(
        &self,
        peer_ids: &[String],
    ) -> std::collections::HashMap<String, String> {
        self.topology_manager.resolve_peer_ids(peer_ids)
    }

    // === Proto conversion ===

    pub fn to_room_info(&self) -> RoomInfo {
        let config = self.topology_config();
        let results: Vec<PlayerResult> = self
            .game_results
            .iter()
            .map(|r| PlayerResult {
                user_id: r.user_id.clone(),
                rank: r.rank,
                arrival_frame: r.arrival_frame,
            })
            .collect();

        RoomInfo {
            room_id: self.id.to_string(),
            room_name: self.name.clone(),
            map_id: self.map_id.clone(),
            host_user_id: self.host_user_id.clone(),
            max_players: self.max_players,
            current_players: self.participant_count(),
            state: self.state().into(),
            is_public: self.is_public,
            created_at: self.created_at.to_rfc3339(),
            started_at: self
                .started_at
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
            network_config: Some(NetworkConfig {
                lockstep_delay_frames: config.lockstep_delay_frames,
                gossip_ttl: config.gossip_ttl,
                mesh_group_size: config.mesh_group_size,
                peer_connections: config.peer_connections,
            }),
            game_state: Some(ProtoGameState {
                rng_seed: self.rng_seed,
                start_frame: self.game_start_frame.unwrap_or(0),
                results,
            }),
            topology_version: self.topology_version,
        }
    }

    pub fn to_room_summary(&self) -> RoomSummary {
        RoomSummary {
            room_id: self.id.to_string(),
            room_name: self.name.clone(),
            map_id: self.map_id.clone(),
            host_user_id: self.host_user_id.clone(),
            max_players: self.max_players,
            current_players: self.participant_count(),
            state: self.state().into(),
            created_at: self.created_at.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_room() -> Room {
        Room::new(
            uuid::Uuid::new_v4(),
            "Test Room".to_string(),
            "map_123".to_string(),
            10,
            true,
            "host_user".to_string(),
            "ws://localhost:3000/signaling".to_string(),
        )
    }

    #[test]
    fn test_add_user_idempotent() {
        let mut room = create_test_room();

        let topology1 = room.add_user("user1".to_string(), None).unwrap();
        let topology2 = room.add_user("user1".to_string(), None).unwrap();

        assert_eq!(topology1.mesh_group, topology2.mesh_group);
        assert_eq!(topology1.is_bridge, topology2.is_bridge);
        assert_eq!(room.member_count(), 2); // host + 1 user
    }

    #[test]
    fn test_host_join_idempotent() {
        let mut room = create_test_room();
        let _topology = room.add_user("host_user".to_string(), None).unwrap();
        assert_eq!(room.member_count(), 1);
    }

    #[test]
    fn test_room_full_error() {
        let mut room = Room::new(
            uuid::Uuid::new_v4(),
            "Small Room".to_string(),
            "map_123".to_string(),
            2,
            true,
            "host".to_string(),
            "ws://localhost:3000/signaling".to_string(),
        );

        room.add_user("user1".to_string(), None).unwrap();
        let result = room.add_user("user2".to_string(), Some(RoomRole::Participant));
        assert!(matches!(result, Err(RoomError::RoomFull)));
    }

    #[test]
    fn test_auto_spectator_when_full() {
        let mut room = Room::new(
            uuid::Uuid::new_v4(),
            "Small Room".to_string(),
            "map_123".to_string(),
            2,
            true,
            "host".to_string(),
            "ws://localhost:3000/signaling".to_string(),
        );

        room.add_user("user1".to_string(), None).unwrap();
        // Room full for participants, but unspecified role -> auto spectator
        let _topology = room.add_user("user2".to_string(), None).unwrap();
        assert_eq!(room.participant_count(), 2);
    }

    #[test]
    fn test_rng_seed_generated() {
        let room = create_test_room();
        assert_ne!(room.rng_seed(), 0);
    }

    #[test]
    fn test_room_info_conversion() {
        let room = create_test_room();
        let info = room.to_room_info();
        assert_eq!(info.room_name, "Test Room");
        assert_eq!(info.map_id, "map_123");
        assert_eq!(info.host_user_id, "host_user");
        assert!(info.game_state.is_some());
        assert_ne!(info.game_state.as_ref().unwrap().rng_seed, 0);
    }
}
