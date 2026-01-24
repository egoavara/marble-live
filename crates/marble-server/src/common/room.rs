use chrono::{DateTime, Utc};
use marble_proto::room::RoomState;

use crate::common::player::Player;

#[derive(Debug, Clone)]
pub struct Room {
    id: uuid::Uuid,
    max_players: u32,
    host: Player,
    other_players: Vec<Player>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
}

#[derive(thiserror::Error, Debug)]
pub enum RoomError {
    #[error("Room is full")]
    RoomFull,

    #[error("Player already exists in the room")]
    PlayerAlreadyExists,

    #[error("Player not found in the room")]
    PlayerNotFound,

    #[error("Room host can not kick")]
    HostCanNotKick,

    #[error("Only the room host can perform this action: {0}")]
    RoomHostOnly(&'static str),
}

impl RoomError {
    pub fn to_code(&self) -> tonic::Code {
        match self {
            RoomError::RoomFull => tonic::Code::ResourceExhausted,
            RoomError::PlayerAlreadyExists => tonic::Code::AlreadyExists,
            RoomError::PlayerNotFound => tonic::Code::NotFound,
            RoomError::HostCanNotKick => tonic::Code::InvalidArgument,
            RoomError::RoomHostOnly(_) => tonic::Code::PermissionDenied,
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
        Self {
            id,
            max_players,
            host,
            other_players: Vec::new(),
            created_at: Utc::now(),
            started_at: None,
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

    pub fn add_player(&mut self, player: Player) -> Result<(), RoomError> {
        if self.count_players() >= self.max_players {
            return Err(RoomError::RoomFull);
        }

        if self.iter_players().any(|p| p.id == player.id) {
            return Err(RoomError::PlayerAlreadyExists);
        }

        self.other_players.push(player);
        Ok(())
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
        Ok(())
    }

    pub fn state(&self) -> RoomState {
        if self.started_at.is_none() {
            return RoomState::Waiting;
        }
        RoomState::Started
    }
    pub fn started_at(&self) -> Option<DateTime<Utc>> {
        self.started_at.clone()
    }
}
