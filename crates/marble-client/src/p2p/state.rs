//! P2P game state management.

use marble_core::{Color, GamePhase, GameState, RouletteConfig, SyncSnapshot};
use matchbox_socket::PeerId;
use std::collections::HashMap;
use std::rc::Rc;
use yew::prelude::*;

/// P2P game phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P2PPhase {
    /// Not connected to any room.
    Disconnected,
    /// Connecting to the signaling server.
    Connecting,
    /// Waiting for other peers to join.
    WaitingForPeers,
    /// In lobby, waiting for all players to be ready.
    Lobby,
    /// Host is starting the game.
    Starting,
    /// Countdown before the game starts.
    Countdown { remaining_frames: u32 },
    /// Game is running.
    Running,
    /// Desync detected, resyncing.
    Resyncing,
    /// Reconnecting to an in-progress game (waiting for state sync from peers).
    Reconnecting,
    /// Game finished.
    Finished,
}

impl Default for P2PPhase {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// Information about a peer (may be connected or disconnected).
/// Note: Host status is determined by server_players, not stored here.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub name: String,
    pub hash_code: String,
    pub color: Color,
    pub rtt_ms: Option<u32>,
    /// Whether the peer is currently connected.
    pub connected: bool,
}

impl PeerInfo {
    pub fn new(peer_id: PeerId, name: String, color: Color) -> Self {
        Self {
            peer_id,
            name,
            hash_code: String::new(),
            color,
            rtt_ms: None,
            connected: true,
        }
    }
}

/// Server-authoritative player information.
/// This data comes from the server and is the single source of truth.
/// Note: Host status is determined by P2PGameState.host_player_id, not stored per-player.
#[derive(Debug, Clone)]
pub struct ServerPlayerInfo {
    pub player_id: String,
    pub name: String,
    pub color: Color,
    pub is_connected: bool,
    pub join_order: u32,
}

/// P2P runtime peer information (connection-related only).
#[derive(Debug, Clone)]
pub struct PeerRuntimeInfo {
    pub peer_id: PeerId,
    pub player_id: String,  // Links to ServerPlayerInfo
    pub rtt_ms: Option<u32>,
    pub p2p_connected: bool,
}

/// P2P game state containing network and game state.
#[derive(Clone)]
pub struct P2PGameState {
    /// Current P2P phase.
    pub phase: P2PPhase,
    /// Network manager.
    pub network: SharedNetworkManager,
    /// My peer ID (assigned after connection).
    pub my_peer_id: Option<PeerId>,
    /// My player ID (assigned by server).
    pub my_player_id: String,
    /// My player name.
    pub my_name: String,
    /// My player hash code (e.g., "1A2B").
    pub my_hash_code: String,
    /// My player color.
    pub my_color: Color,
    /// Connected peers (P2P runtime info).
    pub peers: HashMap<PeerId, PeerInfo>,
    /// Whether I am the host (from server).
    pub is_host: bool,
    /// Current game state.
    pub game_state: Rc<GameState>,
    /// Room ID (for display).
    pub room_id: String,
    /// Whether desync has been detected.
    pub desync_detected: bool,
    /// Frame hashes received from peers.
    pub peer_hashes: HashMap<PeerId, (u64, u64)>,
    /// Event log for debugging.
    pub logs: Vec<String>,
    /// Seed used for the game.
    pub game_seed: u64,
    /// Mapping from PeerId to PlayerId (set when game starts).
    pub peer_player_map: HashMap<PeerId, u32>,

    // --- Server-authoritative data ---
    /// Server-authoritative player information, keyed by player_id.
    pub server_players: HashMap<String, ServerPlayerInfo>,
    /// The player_id of the current host (from server).
    pub host_player_id: String,
    /// Mapping from player_id to peer_id (built via PeerAnnounce).
    pub player_to_peer: HashMap<String, PeerId>,
    /// Mapping from peer_id to player_id (built via PeerAnnounce).
    pub peer_to_player: HashMap<PeerId, String>,
}

impl PartialEq for P2PGameState {
    fn eq(&self, _other: &Self) -> bool {
        // Always return false to ensure re-renders
        false
    }
}

impl Default for P2PGameState {
    fn default() -> Self {
        Self::new()
    }
}

impl P2PGameState {
    pub fn new() -> Self {
        let mut game_state = GameState::new(42);
        game_state.load_map(RouletteConfig::default_classic());

        Self {
            phase: P2PPhase::Disconnected,
            network: create_shared_network_manager("/grpc"),
            my_peer_id: None,
            my_player_id: String::new(),
            my_name: String::new(),
            my_hash_code: String::new(),
            my_color: Color::RED,
            peers: HashMap::new(),
            is_host: false,
            game_state: Rc::new(game_state),
            room_id: String::new(),
            desync_detected: false,
            peer_hashes: HashMap::new(),
            logs: Vec::new(),
            game_seed: 42,
            peer_player_map: HashMap::new(),
            // Server-authoritative data
            server_players: HashMap::new(),
            host_player_id: String::new(),
            player_to_peer: HashMap::new(),
            peer_to_player: HashMap::new(),
        }
    }

    /// Get server players sorted by join_order.
    pub fn server_players_by_order(&self) -> Vec<&ServerPlayerInfo> {
        let mut players: Vec<&ServerPlayerInfo> = self.server_players.values().collect();
        players.sort_by_key(|p| p.join_order);
        players
    }

    /// Add a log entry.
    pub fn add_log(&mut self, msg: &str) {
        self.logs.push(format!(
            "[{}] {}",
            js_sys::Date::new_0().to_locale_time_string("en-US"),
            msg
        ));
        if self.logs.len() > 50 {
            self.logs.remove(0);
        }
    }

    /// Get all peers including self as a sorted list.
    pub fn all_peer_ids(&self) -> Vec<PeerId> {
        let mut ids: Vec<PeerId> = self.peers.keys().copied().collect();
        if let Some(my_id) = self.my_peer_id {
            ids.push(my_id);
        }
        ids.sort();
        ids
    }

    // Note: Host status is now determined by the server (room creator),
    // not by P2P peer election. See SetConnected and UpdateServerPlayers actions.

    /// Get the total player count (all peers including disconnected).
    pub fn player_count(&self) -> usize {
        self.peers.len() + 1 // +1 for self
    }

    /// Get the connected player count.
    pub fn connected_player_count(&self) -> usize {
        self.peers.values().filter(|p| p.connected).count() + 1 // +1 for self
    }
}

/// Actions for P2P game state.
#[derive(Debug, Clone)]
pub enum P2PAction {
    // Connection actions
    SetConnecting,
    SetConnected { room_id: String, server_seed: u64, is_game_in_progress: bool, player_id: String, is_host: bool },
    SetDisconnected,
    SetError(String),

    // Peer management
    PeerJoined(PeerId),
    PeerLeft(PeerId),
    SetMyPeerId(PeerId),
    UpdatePeerRtt { peer_id: PeerId, rtt_ms: u32 },
    UpdatePeerInfo { peer_id: PeerId, name: String, color: Color, hash_code: String },

    // Server player management (authoritative)
    UpdateServerPlayers { players: Vec<ServerPlayerInfo>, host_player_id: String },
    /// Map a peer_id to a player_id via PeerAnnounce
    MapPeerToPlayer { peer_id: PeerId, player_id: String },

    // Player setup
    SetMyName(String),
    SetMyHashCode(String),
    SetMyColor(Color),

    // Game flow
    /// Start game using explicit player order from host
    StartGameFromServer { seed: u64, player_order: Vec<String> },
    StartCountdown,
    Tick,
    GameFinished,

    // Synchronization
    ReceiveFrameHash { peer_id: PeerId, frame: u64, hash: u64 },
    DetectDesync,
    StartResync,
    ApplySyncState { frame: u64, state_data: Vec<u8> },

    // Reconnection
    ApplyReconnectState {
        seed: u64,
        frame: u64,
        state_data: Vec<u8>,
        players: Vec<(PeerId, String, Color)>,
    },

    // Logging
    AddLog(String),
}

impl Reducible for P2PGameState {
    type Action = P2PAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut new_state = (*self).clone();

        match action {
            P2PAction::SetConnecting => {
                new_state.phase = P2PPhase::Connecting;
                new_state.add_log("Connecting...");
            }
            P2PAction::SetConnected { room_id, server_seed, is_game_in_progress, player_id, is_host } => {
                new_state.room_id = room_id.clone();
                new_state.game_seed = server_seed;
                new_state.my_player_id = player_id.clone();
                new_state.is_host = is_host;  // Host status from server (room creator)
                if is_game_in_progress {
                    // Game is already in progress - enter reconnecting state
                    new_state.phase = P2PPhase::Reconnecting;
                    new_state.add_log(&format!("Reconnecting to room: {room_id} (game in progress)"));
                } else {
                    new_state.phase = P2PPhase::WaitingForPeers;
                    new_state.add_log(&format!("Connected to room: {room_id} (seed: {server_seed}, host: {is_host})"));
                }
            }
            P2PAction::SetDisconnected => {
                new_state.phase = P2PPhase::Disconnected;
                new_state.peers.clear();
                new_state.my_peer_id = None;
                new_state.is_host = false;
                new_state.room_id.clear();
                new_state.server_players.clear();
                new_state.host_player_id.clear();
                new_state.player_to_peer.clear();
                new_state.peer_to_player.clear();
                new_state.desync_detected = false;  // Clear desync state for new session
                new_state.peer_hashes.clear();  // Clear stale hash data
                new_state.add_log("Disconnected");
            }
            P2PAction::SetError(msg) => {
                new_state.add_log(&format!("Error: {msg}"));
            }
            P2PAction::PeerJoined(peer_id) => {
                // Check if this is a reconnection (peer already exists)
                if let Some(peer) = new_state.peers.get_mut(&peer_id) {
                    // Reconnection - mark as connected again
                    peer.connected = true;
                    new_state.add_log(&format!("Peer reconnected: {}", &peer_id.0.to_string()[..8]));
                } else {
                    // New peer
                    let color = match new_state.peers.len() % 4 {
                        0 => Color::BLUE,
                        1 => Color::GREEN,
                        2 => Color::ORANGE,
                        _ => Color::PURPLE,
                    };
                    new_state.peers.insert(
                        peer_id,
                        PeerInfo::new(peer_id, format!("Peer-{}", &peer_id.0.to_string()[..8]), color),
                    );
                    new_state.add_log(&format!("Peer joined: {}", &peer_id.0.to_string()[..8]));
                }

                // Host status is fixed by server (room creator), no need to update here

                // Transition to lobby when peers are connected
                if new_state.phase == P2PPhase::WaitingForPeers && !new_state.peers.is_empty() {
                    new_state.phase = P2PPhase::Lobby;
                }
            }
            P2PAction::PeerLeft(peer_id) => {
                let is_in_game = matches!(
                    new_state.phase,
                    P2PPhase::Starting | P2PPhase::Countdown { .. } | P2PPhase::Running | P2PPhase::Finished
                );

                if is_in_game {
                    // During gameplay: mark as disconnected but keep in list
                    // DO NOT eliminate the player's marble - physics simulation continues
                    // The marble keeps participating and the game result is preserved
                    if let Some(peer) = new_state.peers.get_mut(&peer_id) {
                        peer.connected = false;
                        peer.rtt_ms = None;
                    }
                    new_state.add_log(&format!(
                        "Peer {} disconnected (marble continues in game)",
                        &peer_id.0.to_string()[..8]
                    ));
                } else {
                    // In lobby/waiting: remove peer completely
                    new_state.peers.remove(&peer_id);

                    // Clean up peer_player_map to prevent stale data on reconnection
                    if let Some(player_id) = new_state.peer_to_player.remove(&peer_id) {
                        new_state.player_to_peer.remove(&player_id);
                    }

                    // Go back to waiting if no peers
                    let connected_count = new_state.peers.values().filter(|p| p.connected).count();
                    if connected_count == 0
                        && matches!(new_state.phase, P2PPhase::Lobby | P2PPhase::WaitingForPeers)
                    {
                        new_state.phase = P2PPhase::WaitingForPeers;
                    }
                    new_state.add_log(&format!("Peer left: {}", &peer_id.0.to_string()[..8]));
                }
            }
            P2PAction::SetMyPeerId(peer_id) => {
                // Only log if peer ID is being set for the first time
                if new_state.my_peer_id.is_none() {
                    new_state.add_log(&format!("My peer ID: {}", &peer_id.0.to_string()[..8]));
                }
                new_state.my_peer_id = Some(peer_id);
            }
            P2PAction::UpdatePeerRtt { peer_id, rtt_ms } => {
                if let Some(peer) = new_state.peers.get_mut(&peer_id) {
                    peer.rtt_ms = Some(rtt_ms);
                }
            }
            P2PAction::UpdatePeerInfo { peer_id, name, color, hash_code } => {
                if let Some(peer) = new_state.peers.get_mut(&peer_id) {
                    peer.name = name;
                    peer.color = color;
                    peer.hash_code = hash_code;
                }
            }
            P2PAction::UpdateServerPlayers { players, host_player_id } => {
                // Update server_players from server response
                new_state.server_players.clear();
                new_state.host_player_id = host_player_id.clone();

                for player in players {
                    new_state.server_players.insert(player.player_id.clone(), player);
                }

                // Update my host status based on host_player_id
                new_state.is_host = new_state.my_player_id == host_player_id;

                new_state.add_log(&format!(
                    "Updated server players: {} players (host: {})",
                    new_state.server_players.len(),
                    &host_player_id[..8.min(host_player_id.len())]
                ));
            }
            P2PAction::MapPeerToPlayer { peer_id, player_id } => {
                new_state.player_to_peer.insert(player_id.clone(), peer_id);
                new_state.peer_to_player.insert(peer_id, player_id.clone());
                new_state.add_log(&format!(
                    "Mapped peer {} to player {}",
                    &peer_id.0.to_string()[..8],
                    &player_id[..8.min(player_id.len())]
                ));
            }
            P2PAction::SetMyName(name) => {
                new_state.my_name = name;
            }
            P2PAction::SetMyHashCode(hash_code) => {
                new_state.my_hash_code = hash_code;
            }
            P2PAction::SetMyColor(color) => {
                new_state.my_color = color;
            }
            P2PAction::StartGameFromServer { seed, player_order } => {
                new_state.game_seed = seed;
                new_state.phase = P2PPhase::Starting;

                // Initialize game state using explicit player order from host
                let mut game = GameState::new(seed);
                game.load_map(RouletteConfig::default_classic());

                // Use the player_order from host (authoritative order)
                // Build player data in the exact order received
                let player_data: Vec<(String, String, Color)> = player_order
                    .iter()
                    .filter_map(|player_id| {
                        new_state.server_players.get(player_id).map(|p| {
                            (p.player_id.clone(), p.name.clone(), p.color)
                        })
                    })
                    .collect();

                let player_count = player_data.len();

                // Clear and rebuild peer_player_map using host's order
                new_state.peer_player_map.clear();
                for (i, (player_id, name, color)) in player_data.iter().enumerate() {
                    game.add_player(name.clone(), *color);
                    game.set_player_ready(i as u32, true);

                    // Map peer_id to game player index if we have the mapping
                    if let Some(&peer_id) = new_state.player_to_peer.get(player_id) {
                        new_state.peer_player_map.insert(peer_id, i as u32);
                    } else if player_id == &new_state.my_player_id {
                        // This is our player
                        if let Some(my_peer_id) = new_state.my_peer_id {
                            new_state.peer_player_map.insert(my_peer_id, i as u32);
                        }
                    }
                }

                new_state.game_state = Rc::new(game);
                new_state.add_log(&format!(
                    "Game starting from host with seed {} and {} players (by host order)",
                    seed,
                    player_count
                ));
            }
            P2PAction::StartCountdown => {
                if new_state.phase == P2PPhase::Starting || new_state.phase == P2PPhase::Lobby {
                    let game = Rc::make_mut(&mut new_state.game_state);
                    game.start_countdown();
                    new_state.phase = P2PPhase::Countdown {
                        remaining_frames: marble_core::COUNTDOWN_FRAMES,
                    };
                    new_state.add_log("Countdown started");
                }
            }
            P2PAction::Tick => {
                match new_state.phase {
                    P2PPhase::Countdown { remaining_frames } => {
                        if remaining_frames <= 1 {
                            new_state.phase = P2PPhase::Running;
                            new_state.add_log("Game running!");
                        } else {
                            new_state.phase = P2PPhase::Countdown {
                                remaining_frames: remaining_frames - 1,
                            };
                        }
                        let game = Rc::make_mut(&mut new_state.game_state);
                        game.update();
                    }
                    P2PPhase::Running => {
                        let game = Rc::make_mut(&mut new_state.game_state);
                        game.update();

                        // Check for game finish
                        if matches!(game.current_phase(), GamePhase::Finished { .. }) {
                            new_state.phase = P2PPhase::Finished;
                            new_state.add_log("Game finished!");
                        }
                    }
                    _ => {}
                }
            }
            P2PAction::GameFinished => {
                new_state.phase = P2PPhase::Finished;
                new_state.add_log("Game finished!");
            }
            P2PAction::ReceiveFrameHash { peer_id, frame, hash } => {
                new_state.peer_hashes.insert(peer_id, (frame, hash));
            }
            P2PAction::DetectDesync => {
                new_state.desync_detected = true;
                new_state.add_log("Desync detected!");
            }
            P2PAction::StartResync => {
                new_state.phase = P2PPhase::Resyncing;
                new_state.add_log("Starting resync...");
            }
            P2PAction::ApplySyncState { frame, state_data } => {
                match SyncSnapshot::from_bytes(&state_data) {
                    Ok(snapshot) => {
                        let game = Rc::make_mut(&mut new_state.game_state);
                        game.restore_from_snapshot(snapshot);
                        new_state.add_log(&format!("Applied sync state at frame {}", frame));
                        new_state.desync_detected = false;
                        new_state.peer_hashes.clear();
                        new_state.phase = P2PPhase::Running;
                    }
                    Err(e) => {
                        new_state.add_log(&format!("Failed to apply sync state: {}", e));
                    }
                }
            }
            P2PAction::ApplyReconnectState { seed, frame, state_data, players } => {
                new_state.game_seed = seed;

                // Rebuild peer_player_map from player list
                new_state.peer_player_map.clear();
                for (i, (peer_id, _, _)) in players.iter().enumerate() {
                    new_state.peer_player_map.insert(*peer_id, i as u32);
                }

                match SyncSnapshot::from_bytes(&state_data) {
                    Ok(snapshot) => {
                        let game = Rc::make_mut(&mut new_state.game_state);
                        game.restore_from_snapshot(snapshot);
                        new_state.add_log(&format!(
                            "Reconnected! Restored game state at frame {} with {} players",
                            frame, players.len()
                        ));
                        new_state.desync_detected = false;
                        new_state.peer_hashes.clear();
                        new_state.phase = P2PPhase::Running;
                    }
                    Err(e) => {
                        new_state.add_log(&format!("Failed to apply reconnect state: {}", e));
                        // Fall back to lobby if reconnection fails
                        new_state.phase = P2PPhase::Lobby;
                    }
                }
            }
            P2PAction::AddLog(msg) => {
                new_state.add_log(&msg);
            }
        }

        Rc::new(new_state)
    }
}

/// Context type for P2P game state.
pub type P2PStateContext = UseReducerHandle<P2PGameState>;
