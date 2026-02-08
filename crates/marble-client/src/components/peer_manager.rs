//! PeerManager — central peer lifecycle management.
//!
//! Pure data structure with no async/WASM dependencies.
//! Tracks peer resolution, liveness (ping/pong), and user presence.

use std::collections::HashMap;

/// Resolution / liveness status for a single P2P peer.
#[derive(Debug, Clone)]
pub enum PeerStatus {
    /// RPC resolve in progress; `attempts` counts consecutive failures.
    Resolving { attempts: u32 },
    /// Successfully resolved to a user_id.
    Resolved { user_id: String },
    /// A liveness ping was sent; waiting for pong.
    PingSent { sent_at_ms: f64, attempts: u32 },
    /// Peer failed liveness check — should be ignored.
    Stale,
}

/// Presence of a known user_id.
#[derive(Debug, Clone)]
pub enum UserPresence {
    /// Present in GetRoomUsers but no active P2P connection.
    InRoom,
    /// Has an active P2P connection via `peer_id`.
    Connected { peer_id: String },
}

/// Manages the mapping between peer_ids, user_ids, and display names.
pub struct PeerManager {
    /// peer_id → status
    peers: HashMap<String, PeerStatus>,
    /// user_id → presence (authoritative source: GetRoomUsers)
    users: HashMap<String, UserPresence>,
    /// user_id → display_name
    display_names: HashMap<String, String>,
    /// Maximum resolve attempts before triggering a ping.
    max_resolve_attempts: u32,
    /// Ping timeout in milliseconds.
    ping_timeout_ms: f64,
}

impl PeerManager {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            users: HashMap::new(),
            display_names: HashMap::new(),
            max_resolve_attempts: 5,
            ping_timeout_ms: 3000.0,
        }
    }

    // ========================================================================
    // Peer lifecycle
    // ========================================================================

    /// Called when Bevy reports a new peer connection.
    pub fn on_peer_connected(&mut self, peer_id: &str) {
        if !self.peers.contains_key(peer_id) {
            self.peers.insert(
                peer_id.to_string(),
                PeerStatus::Resolving { attempts: 0 },
            );
        }
    }

    /// Called when Bevy reports a peer disconnection.
    pub fn on_peer_disconnected(&mut self, peer_id: &str) {
        // If this peer was resolved, update UserPresence
        let resolved_user = match self.peers.get(peer_id) {
            Some(PeerStatus::Resolved { user_id }) => Some(user_id.clone()),
            _ => None,
        };
        if let Some(uid) = resolved_user {
            if let Some(presence) = self.users.get_mut(&uid) {
                let is_this_peer = match presence {
                    UserPresence::Connected { peer_id: pid } => pid == peer_id,
                    _ => false,
                };
                if is_this_peer {
                    *presence = UserPresence::InRoom;
                }
            }
        }
        self.peers.remove(peer_id);
    }

    /// Called when ResolvePeerIds succeeds for a peer.
    pub fn on_peer_resolved(&mut self, peer_id: &str, user_id: &str) {
        self.peers.insert(
            peer_id.to_string(),
            PeerStatus::Resolved {
                user_id: user_id.to_string(),
            },
        );
        // Update user presence to Connected
        self.users.insert(
            user_id.to_string(),
            UserPresence::Connected {
                peer_id: peer_id.to_string(),
            },
        );
    }

    /// Called when ResolvePeerIds fails for a peer.
    /// Returns `true` if max attempts reached (caller should trigger ping).
    pub fn on_resolve_failed(&mut self, peer_id: &str) -> bool {
        if let Some(status) = self.peers.get_mut(peer_id) {
            match status {
                PeerStatus::Resolving { attempts } => {
                    *attempts += 1;
                    *attempts >= self.max_resolve_attempts
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Called after sending a ping to a peer for liveness check.
    pub fn on_ping_sent(&mut self, peer_id: &str, now_ms: f64) {
        if let Some(status) = self.peers.get(peer_id) {
            let attempts = match status {
                PeerStatus::PingSent { attempts, .. } => *attempts + 1,
                _ => 1,
            };
            self.peers.insert(
                peer_id.to_string(),
                PeerStatus::PingSent {
                    sent_at_ms: now_ms,
                    attempts,
                },
            );
        }
    }

    /// Called when a pong is received from a peer.
    /// Resets the peer to Resolving { attempts: 0 } so resolve is retried.
    pub fn on_pong_received(&mut self, peer_id: &str) {
        if self.peers.contains_key(peer_id) {
            self.peers.insert(
                peer_id.to_string(),
                PeerStatus::Resolving { attempts: 0 },
            );
        }
    }

    /// Check for ping timeouts. Returns peer_ids that timed out → marked Stale.
    pub fn check_ping_timeouts(&mut self, now_ms: f64) -> Vec<String> {
        let mut stale = Vec::new();
        for (peer_id, status) in &self.peers {
            if let PeerStatus::PingSent { sent_at_ms, .. } = status {
                if now_ms - sent_at_ms >= self.ping_timeout_ms {
                    stale.push(peer_id.clone());
                }
            }
        }
        for pid in &stale {
            self.peers.insert(pid.clone(), PeerStatus::Stale);
        }
        stale
    }

    // ========================================================================
    // Room users (authoritative)
    // ========================================================================

    /// Update the authoritative user list from GetRoomUsers.
    pub fn update_room_users(&mut self, user_ids: Vec<String>) {
        // Remove users no longer in the room
        self.users.retain(|uid, _| user_ids.contains(uid));

        // Add new users (preserving existing Connected state)
        for uid in &user_ids {
            if !self.users.contains_key(uid) {
                self.users.insert(uid.clone(), UserPresence::InRoom);
            }
        }
    }

    // ========================================================================
    // Query methods
    // ========================================================================

    /// Peer IDs that need resolution (status = Resolving).
    pub fn unresolved_peer_ids(&self) -> Vec<String> {
        self.peers
            .iter()
            .filter(|(_, status)| matches!(status, PeerStatus::Resolving { .. }))
            .map(|(pid, _)| pid.clone())
            .collect()
    }

    /// Peer IDs that need a ping (resolve attempts maxed out, not yet pinged).
    pub fn peers_needing_ping(&self) -> Vec<String> {
        self.peers
            .iter()
            .filter(|(_, status)| {
                matches!(status, PeerStatus::Resolving { attempts } if *attempts >= self.max_resolve_attempts)
            })
            .map(|(pid, _)| pid.clone())
            .collect()
    }

    /// User IDs that have no display name cached yet.
    pub fn unresolved_user_ids(&self) -> Vec<String> {
        self.users
            .keys()
            .filter(|uid| !self.display_names.contains_key(*uid))
            .cloned()
            .collect()
    }

    /// Resolve peer_id → user_id.
    pub fn peer_to_user(&self, peer_id: &str) -> Option<String> {
        match self.peers.get(peer_id)? {
            PeerStatus::Resolved { user_id } => Some(user_id.clone()),
            _ => None,
        }
    }

    /// Get display name for a user_id.
    pub fn display_name(&self, user_id: &str) -> Option<String> {
        self.display_names.get(user_id).cloned()
    }

    /// Store display name for a user_id.
    pub fn set_display_name(&mut self, user_id: &str, name: String) {
        self.display_names.insert(user_id.to_string(), name);
    }

    /// Get the authoritative room users map.
    pub fn room_users(&self) -> &HashMap<String, UserPresence> {
        &self.users
    }

    /// Get the peer status.
    pub fn peer_status(&self, peer_id: &str) -> Option<&PeerStatus> {
        self.peers.get(peer_id)
    }

    /// Reset all state (e.g., when leaving a room).
    pub fn reset(&mut self) {
        self.peers.clear();
        self.users.clear();
        self.display_names.clear();
    }

    /// All currently tracked peer_ids.
    pub fn all_peer_ids(&self) -> Vec<String> {
        self.peers.keys().cloned().collect()
    }
}

impl Default for PeerManager {
    fn default() -> Self {
        Self::new()
    }
}
