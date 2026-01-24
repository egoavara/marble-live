use std::{
    cell::LazyCell,
    collections::HashMap,
    f32::consts::E,
    sync::{Arc, LazyLock, OnceLock},
};

use chrono::{DateTime, Utc};
use http::status;
use marble_proto::room::PlayerAuth;
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

    #[error("Room has already started")]
    AlreadyStarted,

    #[error("Unauthorized to start the room, only host can start the room")]
    UnauthorizedStartRequest,
}

impl DatabaseError {
    fn to_code(&self) -> tonic::Code {
        match self {
            DatabaseError::RoomError(err) => err.to_code(),
            DatabaseError::RoomNotFound => tonic::Code::NotFound,
            DatabaseError::AlreadyStarted => tonic::Code::FailedPrecondition,
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
        let started_at = Utc::now();
        let is_setted = room.once_started_at(started_at.clone());
        if !is_setted {
            return Err(DatabaseError::AlreadyStarted);
        }
        Ok(started_at)
    }

    pub fn join_room(&self, room_id: &uuid::Uuid, player: Player) -> Result<Room, DatabaseError> {
        let mut rooms = self.rooms.write();
        let Some(room) = rooms.get_mut(room_id) else {
            return Err(DatabaseError::RoomNotFound);
        };
        room.add_player(player)?;
        Ok(room.clone())
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
}
