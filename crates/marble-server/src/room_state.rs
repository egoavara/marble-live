//! Room state management

use parking_lot::RwLock;
use std::{
    collections::HashMap,
    sync::Arc,
};
use uuid::Uuid;

/// Player color (RGB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub color: Color,
    pub is_host: bool,
    pub is_connected: bool,
    pub join_order: u32,
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
    pub seed: u64,
    /// Counter for deterministic join order.
    next_join_order: u32,
}

/// Convert room ID (UUID) to a u64 seed for deterministic game initialization.
pub fn room_id_to_seed(room_id: &str) -> u64 {
    let uuid = Uuid::parse_str(room_id).unwrap_or_else(|_| Uuid::nil());
    let bytes = uuid.as_bytes();
    u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

impl Room {
    pub fn new(name: String, max_players: u32) -> Self {
        let id = Uuid::new_v4().to_string();
        let seed = room_id_to_seed(&id);
        Self {
            id,
            name,
            max_players,
            status: RoomStatus::Waiting,
            players: Vec::new(),
            seed,
            next_join_order: 0,
        }
    }

    pub fn add_player(&mut self, name: String, fingerprint: String, color: Color) -> Option<Player> {
        if self.players.len() >= self.max_players as usize {
            return None;
        }

        let is_host = self.players.is_empty();
        let join_order = self.next_join_order;
        self.next_join_order += 1;

        let player = Player {
            id: Uuid::new_v4().to_string(),
            name,
            fingerprint,
            color,
            is_host,
            is_connected: true,
            join_order,
        };
        self.players.push(player.clone());
        Some(player)
    }

    /// Reassign host if the current host is disconnected.
    /// Only applies in Waiting status.
    pub fn reassign_host_if_needed(&mut self) {
        if self.status != RoomStatus::Waiting {
            return;
        }

        let has_connected_host = self.players.iter()
            .any(|p| p.is_host && p.is_connected);

        if !has_connected_host {
            // Reset all host flags
            self.players.iter_mut().for_each(|p| p.is_host = false);
            // Assign host to first connected player (by join_order)
            if let Some(new_host) = self.players.iter_mut()
                .filter(|p| p.is_connected)
                .min_by_key(|p| p.join_order)
            {
                new_host.is_host = true;
            }
        }
    }

    /// Get players sorted by join_order for deterministic game initialization.
    pub fn players_by_join_order(&self) -> Vec<&Player> {
        let mut players: Vec<&Player> = self.players.iter().collect();
        players.sort_by_key(|p| p.join_order);
        players
    }

    /// Find a player by fingerprint.
    pub fn find_player_by_fingerprint(&self, fingerprint: &str) -> Option<&Player> {
        self.players.iter().find(|p| p.fingerprint == fingerprint)
    }

    /// Find a player by fingerprint (mutable).
    pub fn find_player_by_fingerprint_mut(&mut self, fingerprint: &str) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.fingerprint == fingerprint)
    }

    /// Reconnect a player by fingerprint.
    pub fn reconnect_player(&mut self, fingerprint: &str, new_name: String, color: Color) -> Option<Player> {
        if let Some(player) = self.find_player_by_fingerprint_mut(fingerprint) {
            player.is_connected = true;
            player.name = new_name;
            player.color = color;
            Some(player.clone())
        } else {
            None
        }
    }

    /// Disconnect a player (mark as disconnected, don't remove during game).
    pub fn disconnect_player(&mut self, player_id: &str) -> bool {
        if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
            player.is_connected = false;
            true
        } else {
            false
        }
    }

    /// Check if all players are disconnected.
    pub fn all_disconnected(&self) -> bool {
        self.players.iter().all(|p| !p.is_connected)
    }

    /// Get count of connected players.
    pub fn connected_player_count(&self) -> usize {
        self.players.iter().filter(|p| p.is_connected).count()
    }

    /// Get the host player.
    pub fn get_host(&self) -> Option<&Player> {
        self.players.iter().find(|p| p.is_host)
    }

    /// Start the game (changes status to Playing).
    /// Host can start anytime when there are 2+ connected players.
    pub fn start_game(&mut self) -> bool {
        if self.status != RoomStatus::Waiting {
            return false;
        }
        if self.connected_player_count() < 2 {
            return false;
        }
        self.status = RoomStatus::Playing;
        true
    }

    pub fn remove_player(&mut self, player_id: &str) -> bool {
        let initial_len = self.players.len();
        self.players.retain(|p| p.id != player_id);

        if self.players.len() < initial_len {
            // If host left, reassign host to the first connected player by join_order
            if !self.players.is_empty() && !self.players.iter().any(|p| p.is_host && p.is_connected) {
                // Reset all host flags first
                self.players.iter_mut().for_each(|p| p.is_host = false);
                // Assign host to first connected player by join_order
                if let Some(new_host) = self.players.iter_mut()
                    .filter(|p| p.is_connected)
                    .min_by_key(|p| p.join_order)
                {
                    new_host.is_host = true;
                }
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

    pub fn join_room(&self, room_id: &str, player_name: String, fingerprint: String, color: Color) -> Option<(Room, Player, bool)> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id)?;

        // Check for reconnection by fingerprint
        if let Some(existing_player) = room.find_player_by_fingerprint(&fingerprint) {
            if !existing_player.is_connected {
                // Reconnection - allowed even during gameplay
                let player = room.reconnect_player(&fingerprint, player_name, color)?;
                // Reassign host if needed after reconnection
                room.reassign_host_if_needed();
                return Some((room.clone(), player, true)); // true = reconnection
            } else {
                // Player already connected (same browser different tab scenario)
                // Return existing player info
                return Some((room.clone(), existing_player.clone(), true));
            }
        }

        // New player - only allowed in Waiting status
        if room.status != RoomStatus::Waiting {
            return None;
        }

        let player = room.add_player(player_name, fingerprint, color)?;
        Some((room.clone(), player, false)) // false = new player
    }

    pub fn leave_room(&self, room_id: &str, player_id: &str) -> bool {
        let mut rooms = self.rooms.write();
        if let Some(room) = rooms.get_mut(room_id) {
            match room.status {
                RoomStatus::Waiting => {
                    // In waiting room, remove player completely
                    let removed = room.remove_player(player_id);
                    // Reassign host if the leaving player was the host
                    room.reassign_host_if_needed();
                    if room.players.is_empty() {
                        rooms.remove(room_id);
                    }
                    removed
                }
                RoomStatus::Playing | RoomStatus::Finished => {
                    // During game, mark as disconnected but keep in list
                    let disconnected = room.disconnect_player(player_id);
                    // If all players disconnected, remove room
                    if room.all_disconnected() {
                        rooms.remove(room_id);
                    }
                    disconnected
                }
            }
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

    /// Start the game (host only).
    pub fn start_game(&self, room_id: &str, player_id: &str) -> Result<Room, String> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id).ok_or("Room not found")?;

        // Check if player is host
        let is_host = room.get_host().map(|h| h.id == player_id).unwrap_or(false);
        if !is_host {
            return Err("Only the host can start the game".to_string());
        }

        if room.start_game() {
            tracing::info!(room_id = %room.id, "Game started");
            Ok(room.clone())
        } else {
            if room.status != RoomStatus::Waiting {
                Err("Game already started".to_string())
            } else {
                Err("Need at least 2 players".to_string())
            }
        }
    }
}
