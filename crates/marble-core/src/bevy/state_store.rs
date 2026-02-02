//! Shared state stores for Bevy-Yew communication.
//!
//! Each store holds a specific slice of game state that can be
//! polled independently by Yew hooks, minimizing unnecessary re-renders.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use bevy::prelude::Resource;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::map::{KeyframeSequence, MapObject};
use crate::marble::Color;

/// Maximum number of chat messages to keep.
const MAX_CHAT_MESSAGES: usize = 100;

/// Maximum number of reactions to keep.
const MAX_REACTIONS: usize = 50;

// ============================================================================
// Data Types
// ============================================================================

/// P2P connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Peer information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub player_id: Option<String>,
    pub is_host: bool,
}

/// Player information for UI display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: u32,
    pub name: String,
    pub color: [u8; 4],
    pub arrived: bool,
    pub rank: Option<u32>,
    pub live_rank: Option<u32>,
}

impl PlayerInfo {
    pub fn new(id: u32, name: String, color: Color) -> Self {
        Self {
            id,
            name,
            color: [color.r, color.g, color.b, color.a],
            arrived: false,
            rank: None,
            live_rank: None,
        }
    }
}

/// Chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: u64,
    pub sender_id: String,
    pub content: String,
    pub timestamp: f64,
}

/// Reaction (floating emoji).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    pub id: u64,
    pub sender_id: String,
    pub emoji: String,
    pub timestamp: f64,
}

/// Game state summary for UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameStateSummary {
    pub is_running: bool,
    pub is_host: bool,
    pub frame: u64,
    pub gamerule: String,
    pub map_name: String,
}

// ============================================================================
// Individual Stores
// ============================================================================

/// Store for P2P connection state.
#[derive(Debug, Default)]
pub struct ConnectionStore {
    state: RwLock<ConnectionState>,
    my_player_id: RwLock<String>,
    room_id: RwLock<String>,
}

impl ConnectionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_state(&self) -> ConnectionState {
        *self.state.read()
    }

    pub fn set_state(&self, state: ConnectionState) {
        *self.state.write() = state;
    }

    pub fn get_my_player_id(&self) -> String {
        self.my_player_id.read().clone()
    }

    pub fn set_my_player_id(&self, id: String) {
        *self.my_player_id.write() = id;
    }

    pub fn get_room_id(&self) -> String {
        self.room_id.read().clone()
    }

    pub fn set_room_id(&self, id: String) {
        *self.room_id.write() = id;
    }
}

/// Store for peer list.
#[derive(Debug, Default)]
pub struct PeerStore {
    peers: RwLock<Vec<PeerInfo>>,
    version: RwLock<u64>,
}

impl PeerStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().clone()
    }

    pub fn get_version(&self) -> u64 {
        *self.version.read()
    }

    pub fn set_peers(&self, peers: Vec<PeerInfo>) {
        *self.peers.write() = peers;
        *self.version.write() += 1;
    }

    pub fn add_peer(&self, peer: PeerInfo) {
        self.peers.write().push(peer);
        *self.version.write() += 1;
    }

    pub fn remove_peer(&self, peer_id: &str) {
        self.peers.write().retain(|p| p.peer_id != peer_id);
        *self.version.write() += 1;
    }
}

/// Store for player list.
#[derive(Debug, Default)]
pub struct PlayerStore {
    players: RwLock<Vec<PlayerInfo>>,
    arrival_order: RwLock<Vec<u32>>,
    version: RwLock<u64>,
}

impl PlayerStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_players(&self) -> Vec<PlayerInfo> {
        self.players.read().clone()
    }

    pub fn get_arrival_order(&self) -> Vec<u32> {
        self.arrival_order.read().clone()
    }

    pub fn get_version(&self) -> u64 {
        *self.version.read()
    }

    pub fn set_players(&self, players: Vec<PlayerInfo>) {
        *self.players.write() = players;
        *self.version.write() += 1;
    }

    pub fn set_arrival_order(&self, order: Vec<u32>) {
        *self.arrival_order.write() = order;
        *self.version.write() += 1;
    }

    pub fn update_player_rank(&self, player_id: u32, rank: Option<u32>, live_rank: Option<u32>) {
        let mut players = self.players.write();
        if let Some(player) = players.iter_mut().find(|p| p.id == player_id) {
            player.rank = rank;
            player.live_rank = live_rank;
        }
        *self.version.write() += 1;
    }

    pub fn mark_arrived(&self, player_id: u32) {
        let mut players = self.players.write();
        if let Some(player) = players.iter_mut().find(|p| p.id == player_id) {
            player.arrived = true;
        }
        self.arrival_order.write().push(player_id);
        *self.version.write() += 1;
    }
}

/// Store for chat messages.
#[derive(Debug, Default)]
pub struct ChatStore {
    messages: RwLock<VecDeque<ChatMessage>>,
    next_id: RwLock<u64>,
    version: RwLock<u64>,
}

impl ChatStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_messages(&self) -> Vec<ChatMessage> {
        self.messages.read().iter().cloned().collect()
    }

    pub fn get_version(&self) -> u64 {
        *self.version.read()
    }

    pub fn add_message(&self, sender_id: String, content: String, timestamp: f64) {
        let id = {
            let mut next = self.next_id.write();
            let id = *next;
            *next += 1;
            id
        };

        let mut messages = self.messages.write();
        messages.push_back(ChatMessage {
            id,
            sender_id,
            content,
            timestamp,
        });

        // Trim old messages
        while messages.len() > MAX_CHAT_MESSAGES {
            messages.pop_front();
        }

        *self.version.write() += 1;
    }

    pub fn clear(&self) {
        self.messages.write().clear();
        *self.version.write() += 1;
    }
}

/// Store for reactions.
#[derive(Debug, Default)]
pub struct ReactionStore {
    reactions: RwLock<VecDeque<Reaction>>,
    next_id: RwLock<u64>,
    version: RwLock<u64>,
}

impl ReactionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_reactions(&self) -> Vec<Reaction> {
        self.reactions.read().iter().cloned().collect()
    }

    pub fn get_recent_reactions(&self, since_timestamp: f64) -> Vec<Reaction> {
        self.reactions
            .read()
            .iter()
            .filter(|r| r.timestamp >= since_timestamp)
            .cloned()
            .collect()
    }

    pub fn get_version(&self) -> u64 {
        *self.version.read()
    }

    pub fn add_reaction(&self, sender_id: String, emoji: String, timestamp: f64) {
        let id = {
            let mut next = self.next_id.write();
            let id = *next;
            *next += 1;
            id
        };

        let mut reactions = self.reactions.write();
        reactions.push_back(Reaction {
            id,
            sender_id,
            emoji,
            timestamp,
        });

        // Trim old reactions
        while reactions.len() > MAX_REACTIONS {
            reactions.pop_front();
        }

        *self.version.write() += 1;
    }
}

/// Store for game state summary.
#[derive(Debug, Default)]
pub struct GameStateStore {
    summary: RwLock<GameStateSummary>,
    version: RwLock<u64>,
}

impl GameStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_summary(&self) -> GameStateSummary {
        self.summary.read().clone()
    }

    pub fn get_version(&self) -> u64 {
        *self.version.read()
    }

    pub fn update(&self, summary: GameStateSummary) {
        *self.summary.write() = summary;
        *self.version.write() += 1;
    }

    pub fn set_running(&self, running: bool) {
        self.summary.write().is_running = running;
        *self.version.write() += 1;
    }

    pub fn set_frame(&self, frame: u64) {
        self.summary.write().frame = frame;
        // Don't bump version for every frame (too frequent)
    }
}

/// Editor state summary for UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EditorStateSummary {
    pub selected_object: Option<usize>,
    pub selected_sequence: Option<usize>,
    pub selected_keyframe: Option<usize>,
    pub is_simulating: bool,
    pub is_previewing: bool,
    /// 모든 실행 중인 키프레임 시퀀스의 현재 인덱스
    /// key: 시퀀스 이름, value: current_index (다음 처리할 인덱스)
    #[serde(default)]
    pub executing_keyframes: HashMap<String, usize>,
}

/// Snap configuration summary for UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapConfigSummary {
    pub grid_snap_enabled: bool,
    pub grid_snap_interval: f32,
    pub angle_snap_enabled: bool,
    pub angle_snap_interval: f32,
}

impl Default for SnapConfigSummary {
    fn default() -> Self {
        Self {
            grid_snap_enabled: true,
            grid_snap_interval: 0.05,
            angle_snap_enabled: true,
            angle_snap_interval: 0.5,
        }
    }
}

/// Store for editor state.
#[derive(Debug, Default)]
pub struct EditorStore {
    summary: RwLock<EditorStateSummary>,
    objects: RwLock<Vec<MapObject>>,
    keyframes: RwLock<Vec<KeyframeSequence>>,
    version: RwLock<u64>,
    keyframes_version: RwLock<u64>,
}

impl EditorStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_summary(&self) -> EditorStateSummary {
        self.summary.read().clone()
    }

    pub fn get_objects(&self) -> Vec<MapObject> {
        self.objects.read().clone()
    }

    pub fn get_keyframes(&self) -> Vec<KeyframeSequence> {
        self.keyframes.read().clone()
    }

    pub fn get_version(&self) -> u64 {
        *self.version.read()
    }

    pub fn get_keyframes_version(&self) -> u64 {
        *self.keyframes_version.read()
    }

    pub fn update_summary(&self, summary: EditorStateSummary) {
        *self.summary.write() = summary;
        *self.version.write() += 1;
    }

    pub fn update_objects(&self, objects: Vec<MapObject>) {
        *self.objects.write() = objects;
        *self.version.write() += 1;
    }

    pub fn update_keyframes(&self, keyframes: Vec<KeyframeSequence>) {
        *self.keyframes.write() = keyframes;
        *self.keyframes_version.write() += 1;
    }

    pub fn update(&self, summary: EditorStateSummary, objects: Vec<MapObject>) {
        *self.summary.write() = summary;
        *self.objects.write() = objects;
        *self.version.write() += 1;
    }

    pub fn update_all(&self, summary: EditorStateSummary, objects: Vec<MapObject>, keyframes: Vec<KeyframeSequence>) {
        *self.summary.write() = summary;
        *self.objects.write() = objects;
        *self.keyframes.write() = keyframes;
        *self.version.write() += 1;
        *self.keyframes_version.write() += 1;
    }
}

/// Store for snap configuration.
#[derive(Debug, Default)]
pub struct SnapConfigStore {
    summary: RwLock<SnapConfigSummary>,
    version: RwLock<u64>,
}

impl SnapConfigStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_summary(&self) -> SnapConfigSummary {
        self.summary.read().clone()
    }

    pub fn get_version(&self) -> u64 {
        *self.version.read()
    }

    pub fn update(&self, summary: SnapConfigSummary) {
        *self.summary.write() = summary;
        *self.version.write() += 1;
    }
}

// ============================================================================
// Combined State Stores
// ============================================================================

/// All state stores combined for easy sharing.
#[derive(Debug, Clone, Resource)]
pub struct StateStores {
    pub connection: Arc<ConnectionStore>,
    pub peers: Arc<PeerStore>,
    pub players: Arc<PlayerStore>,
    pub chat: Arc<ChatStore>,
    pub reactions: Arc<ReactionStore>,
    pub game: Arc<GameStateStore>,
    pub editor: Arc<EditorStore>,
    pub snap_config: Arc<SnapConfigStore>,
}

impl StateStores {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(ConnectionStore::new()),
            peers: Arc::new(PeerStore::new()),
            players: Arc::new(PlayerStore::new()),
            chat: Arc::new(ChatStore::new()),
            reactions: Arc::new(ReactionStore::new()),
            game: Arc::new(GameStateStore::new()),
            editor: Arc::new(EditorStore::new()),
            snap_config: Arc::new(SnapConfigStore::new()),
        }
    }
}

impl Default for StateStores {
    fn default() -> Self {
        Self::new()
    }
}
