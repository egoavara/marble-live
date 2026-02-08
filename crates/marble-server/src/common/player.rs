use chrono::{DateTime, Utc};
use marble_proto::room::RoomRole;

/// A user in a room (identified by `user_id` from JWT)
#[derive(Debug, Clone)]
pub struct RoomMember {
    pub user_id: String,
    pub is_host: bool,
    pub role: RoomRole,
    pub joined_at: DateTime<Utc>,
}

impl RoomMember {
    pub fn new_host(user_id: String) -> Self {
        Self {
            user_id,
            is_host: true,
            role: RoomRole::Participant,
            joined_at: Utc::now(),
        }
    }

    pub fn new_participant(user_id: String) -> Self {
        Self {
            user_id,
            is_host: false,
            role: RoomRole::Participant,
            joined_at: Utc::now(),
        }
    }

    pub fn new_spectator(user_id: String) -> Self {
        Self {
            user_id,
            is_host: false,
            role: RoomRole::Spectator,
            joined_at: Utc::now(),
        }
    }
}
