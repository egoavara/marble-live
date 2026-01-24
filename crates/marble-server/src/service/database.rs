use std::{
    cell::LazyCell,
    collections::HashMap,
    f32::consts::E,
    sync::{Arc, LazyLock, OnceLock},
};

use chrono::{DateTime, Utc};
use http::status;
use marble_proto::room::{PeerConnectionStatus, PeerTopology, PlayerAuth};
use parking_lot::RwLock;
use serde::de;
use thiserror::Error;

use crate::common::{
    player::{self, Player},
    room::{Room, RoomError},
};

pub struct Database {
    rooms: Arc<RwLock<HashMap<uuid::Uuid, Room>>>,
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error(transparent)]
    RoomError(#[from] RoomError),

    #[error("Room not found")]
    RoomNotFound,

    #[error("Unauthorized to start the room, only host can start the room")]
    UnauthorizedStartRequest,
}

impl DatabaseError {
    fn to_code(&self) -> tonic::Code {
        match self {
            DatabaseError::RoomError(err) => err.to_code(),
            DatabaseError::RoomNotFound => tonic::Code::NotFound,
            DatabaseError::UnauthorizedStartRequest => tonic::Code::PermissionDenied,
        }
    }
}

impl From<DatabaseError> for tonic::Status {
    fn from(err: DatabaseError) -> Self {
        tonic::Status::new(err.to_code(), err.to_string())
    }
}

impl Database {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_room(&self, room_id: &uuid::Uuid) -> Option<Room> {
        let rooms = self.rooms.read();
        rooms.get(room_id).cloned()
    }

    /// Start a room. This is idempotent - if already started, returns existing start time.
    pub fn start_room(
        &self,
        room_id: &uuid::Uuid,
        player: &PlayerAuth,
    ) -> Result<DateTime<Utc>, DatabaseError> {
        let mut rooms = self.rooms.write();
        let Some(room) = rooms.get_mut(room_id) else {
            return Err(DatabaseError::RoomNotFound);
        };

        room.assert_host(&player.id, &player.secret, "start_room")?;

        // Idempotent: if already started, return existing start time
        if let Some(existing_started_at) = room.started_at() {
            return Ok(existing_started_at);
        }

        let started_at = Utc::now();
        room.once_started_at(started_at.clone());
        Ok(started_at)
    }

    pub fn join_room(
        &self,
        room_id: &uuid::Uuid,
        player: Player,
    ) -> Result<(Room, PeerTopology), DatabaseError> {
        let mut rooms = self.rooms.write();
        let Some(room) = rooms.get_mut(room_id) else {
            return Err(DatabaseError::RoomNotFound);
        };
        let topology = room.add_player(player)?;
        Ok((room.clone(), topology))
    }

    pub fn kick_room(
        &self,
        room_id: &uuid::Uuid,
        player: &PlayerAuth,
        target_player: &str,
    ) -> Result<(), DatabaseError> {
        let mut rooms = self.rooms.write();
        let Some(room) = rooms.get_mut(room_id) else {
            return Err(DatabaseError::RoomNotFound);
        };
        room.assert_host(&player.id, &player.secret, "kick_room")?;
        room.kick_player(target_player)?;
        Ok(())
    }

    pub fn add_room(&self, room: Room) {
        let mut rooms = self.rooms.write();
        rooms.insert(room.id().clone(), room);
    }

    /// Report connection status and get updated topology if changed
    pub fn report_connection(
        &self,
        room_id: &uuid::Uuid,
        player_id: &str,
        statuses: Vec<PeerConnectionStatus>,
    ) -> Result<Option<PeerTopology>, DatabaseError> {
        let mut rooms = self.rooms.write();
        let Some(room) = rooms.get_mut(room_id) else {
            return Err(DatabaseError::RoomNotFound);
        };
        Ok(room.update_connection_status(player_id, &statuses))
    }

    /// Get topology for a player
    pub fn get_topology(
        &self,
        room_id: &uuid::Uuid,
        player_id: &str,
    ) -> Result<PeerTopology, DatabaseError> {
        let rooms = self.rooms.read();
        let Some(room) = rooms.get(room_id) else {
            return Err(DatabaseError::RoomNotFound);
        };
        room.get_topology(player_id)
            .ok_or(DatabaseError::RoomNotFound)
    }
}
