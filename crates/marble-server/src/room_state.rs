//! Room state management

use parking_lot::RwLock;
use std::{
    collections::HashMap,
    sync::Arc,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub is_host: bool,
    pub is_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomStatus {
    Waiting,
    Playing,
    Finished,
}

#[derive(Debug, Clone)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub max_players: u32,
    pub status: RoomStatus,
    pub players: Vec<Player>,
}

impl Room {
    pub fn new(name: String, max_players: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            max_players,
            status: RoomStatus::Waiting,
            players: Vec::new(),
        }
    }

    pub fn add_player(&mut self, name: String) -> Option<Player> {
        if self.players.len() >= self.max_players as usize {
            return None;
        }

        let is_host = self.players.is_empty();
        let player = Player {
            id: Uuid::new_v4().to_string(),
            name,
            is_host,
            is_ready: false,
        };
        self.players.push(player.clone());
        Some(player)
    }

    pub fn remove_player(&mut self, player_id: &str) -> bool {
        let initial_len = self.players.len();
        self.players.retain(|p| p.id != player_id);

        if self.players.len() < initial_len {
            // If host left, assign new host
            if !self.players.is_empty() && !self.players.iter().any(|p| p.is_host) {
                self.players[0].is_host = true;
            }
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RoomStore {
    rooms: Arc<RwLock<HashMap<String, Room>>>,
}

impl RoomStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_room(&self, name: String, max_players: u32) -> Room {
        let room = Room::new(name, max_players);
        let mut rooms = self.rooms.write();
        rooms.insert(room.id.clone(), room.clone());
        room
    }

    pub fn get_room(&self, room_id: &str) -> Option<Room> {
        let rooms = self.rooms.read();
        rooms.get(room_id).cloned()
    }

    pub fn join_room(&self, room_id: &str, player_name: String) -> Option<(Room, Player)> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id)?;

        if room.status != RoomStatus::Waiting {
            return None;
        }

        let player = room.add_player(player_name)?;
        Some((room.clone(), player))
    }

    pub fn leave_room(&self, room_id: &str, player_id: &str) -> bool {
        let mut rooms = self.rooms.write();
        if let Some(room) = rooms.get_mut(room_id) {
            let removed = room.remove_player(player_id);
            if room.players.is_empty() {
                rooms.remove(room_id);
            }
            removed
        } else {
            false
        }
    }

    pub fn list_rooms(&self, offset: u32, limit: u32) -> (Vec<Room>, u32) {
        let rooms = self.rooms.read();
        let total = rooms.len() as u32;
        let room_list: Vec<Room> = rooms
            .values()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect();
        (room_list, total)
    }
}
