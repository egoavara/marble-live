use std::collections::HashMap;

use chrono::{DateTime, Utc};
use marble_proto::room::{PeerConnectionStatus, PeerTopology, RoomRole, RoomState};
use parking_lot::RwLock;
use std::sync::Arc;
use thiserror::Error;

use crate::common::room::{Room, RoomError};

// ========================================
// User storage
// ========================================

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StoredUser {
    pub user_id: String,
    pub display_name: String,
    pub auth_type: AuthType,
    pub salt: Option<String>,
    pub fingerprint: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    Anonymous,
    Sso,
}

// ========================================
// Map storage
// ========================================

#[derive(Debug, Clone)]
pub struct StoredMap {
    pub map_id: String,
    pub name: String,
    pub description: String,
    pub creator_id: String,
    pub tags: Vec<String>,
    pub data: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ========================================
// Database
// ========================================

#[derive(Clone)]
pub struct Database {
    rooms: Arc<RwLock<HashMap<uuid::Uuid, Room>>>,
    users: Arc<RwLock<HashMap<String, StoredUser>>>,
    /// (salt, fingerprint) -> `user_id` index for anonymous login lookup
    anon_index: Arc<RwLock<HashMap<(String, String), String>>>,
    maps: Arc<RwLock<HashMap<String, StoredMap>>>,
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error(transparent)]
    RoomError(#[from] RoomError),

    #[error("Room not found")]
    RoomNotFound,

    #[error("User not found")]
    UserNotFound,

    #[error("Map not found")]
    MapNotFound,

    #[error("Only the map owner can perform this action")]
    MapOwnerOnly,

    #[error("Map is in use by an active room")]
    MapInUse,

    #[error("Unauthorized: not a room member")]
    NotRoomMember,
}

impl DatabaseError {
    fn to_code(&self) -> tonic::Code {
        match self {
            Self::RoomError(err) => err.to_code(),
            Self::RoomNotFound | Self::UserNotFound | Self::MapNotFound => tonic::Code::NotFound,
            Self::MapOwnerOnly | Self::NotRoomMember => tonic::Code::PermissionDenied,
            Self::MapInUse => tonic::Code::FailedPrecondition,
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
            users: Arc::new(RwLock::new(HashMap::new())),
            anon_index: Arc::new(RwLock::new(HashMap::new())),
            maps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ========================================
    // User operations
    // ========================================

    /// Find or create anonymous user by (salt, fingerprint).
    /// Returns (user, `is_new`).
    pub fn find_or_create_anonymous_user(
        &self,
        display_name: &str,
        salt: &str,
        fingerprint: &str,
    ) -> (StoredUser, bool) {
        let key = (salt.to_string(), fingerprint.to_string());

        // Check if user already exists
        {
            let index = self.anon_index.read();
            if let Some(user_id) = index.get(&key) {
                let users = self.users.read();
                if let Some(user) = users.get(user_id) {
                    return (user.clone(), false);
                }
            }
        }

        // Create new user
        let user_id = uuid::Uuid::new_v4().to_string();
        let user = StoredUser {
            user_id: user_id.clone(),
            display_name: display_name.to_string(),
            auth_type: AuthType::Anonymous,
            salt: Some(salt.to_string()),
            fingerprint: Some(fingerprint.to_string()),
            created_at: Utc::now(),
        };

        {
            let mut users = self.users.write();
            users.insert(user_id.clone(), user.clone());
        }
        {
            let mut index = self.anon_index.write();
            index.insert(key, user_id);
        }

        (user, true)
    }

    pub fn get_user(&self, user_id: &str) -> Option<StoredUser> {
        let users = self.users.read();
        users.get(user_id).cloned()
    }

    pub fn get_users(&self, user_ids: &[String]) -> Vec<StoredUser> {
        let users = self.users.read();
        user_ids
            .iter()
            .filter_map(|id| users.get(id).cloned())
            .collect()
    }

    pub fn update_user_profile(
        &self,
        user_id: &str,
        display_name: &str,
    ) -> Result<StoredUser, DatabaseError> {
        let mut users = self.users.write();
        let user = users.get_mut(user_id).ok_or(DatabaseError::UserNotFound)?;
        user.display_name = display_name.to_string();
        Ok(user.clone())
    }

    // ========================================
    // Map operations
    // ========================================

    pub fn create_map(
        &self,
        creator_id: &str,
        name: &str,
        description: &str,
        tags: Vec<String>,
        data: &str,
    ) -> StoredMap {
        let map_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let map = StoredMap {
            map_id: map_id.clone(),
            name: name.to_string(),
            description: description.to_string(),
            creator_id: creator_id.to_string(),
            tags,
            data: data.to_string(),
            created_at: now,
            updated_at: now,
        };

        let mut maps = self.maps.write();
        maps.insert(map_id, map.clone());
        map
    }

    pub fn get_map(&self, map_id: &str) -> Option<StoredMap> {
        let maps = self.maps.read();
        maps.get(map_id).cloned()
    }

    pub fn update_map(
        &self,
        map_id: &str,
        user_id: &str,
        name: Option<&str>,
        description: Option<&str>,
        tags: Option<Vec<String>>,
        data: Option<&str>,
    ) -> Result<StoredMap, DatabaseError> {
        let mut maps = self.maps.write();
        let map = maps.get_mut(map_id).ok_or(DatabaseError::MapNotFound)?;

        if map.creator_id != user_id {
            return Err(DatabaseError::MapOwnerOnly);
        }

        if let Some(n) = name {
            map.name = n.to_string();
        }
        if let Some(d) = description {
            map.description = d.to_string();
        }
        if let Some(t) = tags {
            map.tags = t;
        }
        if let Some(d) = data {
            map.data = d.to_string();
        }
        map.updated_at = Utc::now();

        Ok(map.clone())
    }

    pub fn delete_map(&self, map_id: &str, user_id: &str) -> Result<StoredMap, DatabaseError> {
        // Check if map is in use by any active room
        {
            let rooms = self.rooms.read();
            for room in rooms.values() {
                // Only check non-ended rooms
                if room.state() != RoomState::Ended {
                    let info = room.to_room_info();
                    if info.map_id == map_id {
                        return Err(DatabaseError::MapInUse);
                    }
                }
            }
        }

        let mut maps = self.maps.write();
        let map = maps.get(map_id).ok_or(DatabaseError::MapNotFound)?;

        if map.creator_id != user_id {
            return Err(DatabaseError::MapOwnerOnly);
        }

        let map = maps.remove(map_id).unwrap();
        Ok(map)
    }

    pub fn list_maps(
        &self,
        page_size: u32,
        page_token: &str,
        creator_id: Option<&str>,
        name_query: Option<&str>,
        tags: &[String],
    ) -> (Vec<StoredMap>, String, u32) {
        let maps = self.maps.read();
        let page_size = page_size.clamp(1, 100) as usize;

        let mut filtered: Vec<&StoredMap> = maps
            .values()
            .filter(|m| {
                if let Some(cid) = creator_id
                    && m.creator_id != cid
                {
                    return false;
                }
                if let Some(query) = name_query
                    && !query.is_empty()
                    && !m.name.to_lowercase().contains(&query.to_lowercase())
                {
                    return false;
                }
                if !tags.is_empty() && !tags.iter().all(|t| m.tags.contains(t)) {
                    return false;
                }
                true
            })
            .collect();

        let total_count = u32::try_from(filtered.len()).unwrap_or(u32::MAX);

        // Sort by created_at descending
        filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Apply cursor
        let start = if page_token.is_empty() {
            0
        } else {
            filtered
                .iter()
                .position(|m| m.map_id == page_token)
                .map_or(0, |p| p + 1)
        };

        let page: Vec<StoredMap> = filtered
            .into_iter()
            .skip(start)
            .take(page_size)
            .cloned()
            .collect();

        let next_token = page.last().map(|m| m.map_id.clone()).unwrap_or_default();

        (page, next_token, total_count)
    }

    // ========================================
    // Room operations
    // ========================================

    pub fn get_room(&self, room_id: &uuid::Uuid) -> Option<Room> {
        let rooms = self.rooms.read();
        rooms.get(room_id).cloned()
    }

    pub fn add_room(&self, room: Room) {
        let mut rooms = self.rooms.write();
        rooms.insert(*room.id(), room);
    }

    pub fn join_room(
        &self,
        room_id: &uuid::Uuid,
        user_id: String,
        role: Option<RoomRole>,
    ) -> Result<(Room, PeerTopology), DatabaseError> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id).ok_or(DatabaseError::RoomNotFound)?;
        let topology = room.add_user(user_id, role)?;
        Ok((room.clone(), topology))
    }

    pub fn kick_user(
        &self,
        room_id: &uuid::Uuid,
        host_user_id: &str,
        target_user_id: &str,
    ) -> Result<Room, DatabaseError> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id).ok_or(DatabaseError::RoomNotFound)?;
        room.assert_host(host_user_id, "kick_user")?;
        room.kick_user(target_user_id)?;
        Ok(room.clone())
    }

    pub fn start_game(
        &self,
        room_id: &uuid::Uuid,
        user_id: &str,
        start_frame: u64,
    ) -> Result<(bool, Room), DatabaseError> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id).ok_or(DatabaseError::RoomNotFound)?;
        let newly_started = room.start_game(user_id, start_frame)?;
        Ok((newly_started, room.clone()))
    }

    pub fn report_arrival(
        &self,
        room_id: &uuid::Uuid,
        user_id: &str,
        arrived_user_id: &str,
        arrival_frame: u64,
        rank: u32,
    ) -> Result<(bool, Room), DatabaseError> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id).ok_or(DatabaseError::RoomNotFound)?;
        let game_ended =
            room.report_arrival(user_id, arrived_user_id, arrival_frame, rank)?;
        Ok((game_ended, room.clone()))
    }

    pub fn report_connection(
        &self,
        room_id: &uuid::Uuid,
        user_id: &str,
        statuses: &[PeerConnectionStatus],
    ) -> Result<Option<PeerTopology>, DatabaseError> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id).ok_or(DatabaseError::RoomNotFound)?;

        if !room.has_member(user_id) {
            return Err(DatabaseError::NotRoomMember);
        }

        Ok(room.update_connection_status(user_id, statuses))
    }

    pub fn get_topology(
        &self,
        room_id: &uuid::Uuid,
        user_id: &str,
    ) -> Result<PeerTopology, DatabaseError> {
        let rooms = self.rooms.read();
        let room = rooms.get(room_id).ok_or(DatabaseError::RoomNotFound)?;

        if !room.has_member(user_id) {
            return Err(DatabaseError::NotRoomMember);
        }

        room.get_topology(user_id)
            .ok_or(DatabaseError::RoomNotFound)
    }

    pub fn register_peer_id(
        &self,
        room_id: &uuid::Uuid,
        user_id: &str,
        peer_id: &str,
    ) -> Result<Option<PeerTopology>, DatabaseError> {
        let mut rooms = self.rooms.write();
        let room = rooms.get_mut(room_id).ok_or(DatabaseError::RoomNotFound)?;

        if !room.has_member(user_id) {
            return Err(DatabaseError::NotRoomMember);
        }

        Ok(room.update_peer_id(user_id, peer_id))
    }

    pub fn get_room_topology(
        &self,
        room_id: &uuid::Uuid,
        user_id: &str,
    ) -> Result<Vec<(String, PeerTopology)>, DatabaseError> {
        let rooms = self.rooms.read();
        let room = rooms.get(room_id).ok_or(DatabaseError::RoomNotFound)?;

        if !room.has_member(user_id) {
            return Err(DatabaseError::NotRoomMember);
        }

        Ok(room.get_all_topologies())
    }

    pub fn resolve_peer_ids(
        &self,
        room_id: &uuid::Uuid,
        user_id: &str,
        peer_ids: &[String],
    ) -> Result<HashMap<String, String>, DatabaseError> {
        let rooms = self.rooms.read();
        let room = rooms.get(room_id).ok_or(DatabaseError::RoomNotFound)?;

        if !room.has_member(user_id) {
            return Err(DatabaseError::NotRoomMember);
        }

        Ok(room.resolve_peer_ids(peer_ids))
    }

    pub fn list_rooms(
        &self,
        page_size: u32,
        page_token: &str,
        states: &[RoomState],
        map_id: Option<&str>,
        name_query: Option<&str>,
        has_available_slots: bool,
    ) -> (Vec<Room>, String, u32) {
        let rooms = self.rooms.read();
        let page_size = page_size.clamp(1, 100) as usize;

        let mut filtered: Vec<&Room> = rooms
            .values()
            .filter(|r| {
                if !r.is_public() {
                    return false;
                }
                if !states.is_empty() && !states.contains(&r.state()) {
                    return false;
                }
                if let Some(mid) = map_id
                    && !mid.is_empty()
                {
                    let info = r.to_room_info();
                    if info.map_id != mid {
                        return false;
                    }
                }
                if let Some(query) = name_query
                    && !query.is_empty()
                {
                    let info = r.to_room_info();
                    if !info.room_name.to_lowercase().contains(&query.to_lowercase()) {
                        return false;
                    }
                }
                if has_available_slots && r.participant_count() >= r.max_players() {
                    return false;
                }
                true
            })
            .collect();

        let total_count = u32::try_from(filtered.len()).unwrap_or(u32::MAX);

        // Sort by created_at descending (newest first)
        filtered.sort_by(|a, b| {
            let a_info = a.to_room_info();
            let b_info = b.to_room_info();
            b_info.created_at.cmp(&a_info.created_at)
        });

        let start = if page_token.is_empty() {
            0
        } else {
            filtered
                .iter()
                .position(|r| r.id().to_string() == page_token)
                .map_or(0, |p| p + 1)
        };

        let page: Vec<Room> = filtered
            .into_iter()
            .skip(start)
            .take(page_size)
            .cloned()
            .collect();

        let next_token = page
            .last()
            .map(|r| r.id().to_string())
            .unwrap_or_default();

        (page, next_token, total_count)
    }
}
