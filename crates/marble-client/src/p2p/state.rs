//! P2P game state management.

use crate::network::create_shared_network_manager;
use crate::network::manager::SharedNetworkManager;
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
    /// Game finished.
    Finished,
}

impl Default for P2PPhase {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// Information about a peer (may be connected or disconnected).
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub name: String,
    pub color: Color,
    pub ready: bool,
    pub rtt_ms: Option<u32>,
    /// Whether the peer is currently connected.
    pub connected: bool,
}

impl PeerInfo {
    pub fn new(peer_id: PeerId, name: String, color: Color) -> Self {
        Self {
            peer_id,
            name,
            color,
            ready: false,
            rtt_ms: None,
            connected: true,
        }
    }
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
    /// My player name.
    pub my_name: String,
    /// My player color.
    pub my_color: Color,
    /// Whether I'm ready.
    pub my_ready: bool,
    /// Connected peers.
    pub peers: HashMap<PeerId, PeerInfo>,
    /// Whether I am the host.
    pub is_host: bool,
    /// Current game state.
    pub game_state: Rc<GameState>,
    /// Room ID (for display).
    pub room_id: String,
    /// Whether desync has been detected.
    pub desync_detected: bool,
    /// Frame hashes received from peers.
    pub peer_hashes: HashMap<PeerId, (u64, u64)>,
    /// Last ping sent timestamps.
    pub ping_sent: HashMap<PeerId, f64>,
    /// Event log for debugging.
    pub logs: Vec<String>,
    /// Seed used for the game.
    pub game_seed: u64,
    /// Mapping from PeerId to PlayerId (set when game starts).
    pub peer_player_map: HashMap<PeerId, u32>,
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
            my_name: String::new(),
            my_color: Color::RED,
            my_ready: false,
            peers: HashMap::new(),
            is_host: false,
            game_state: Rc::new(game_state),
            room_id: String::new(),
            desync_detected: false,
            peer_hashes: HashMap::new(),
            ping_sent: HashMap::new(),
            logs: Vec::new(),
            game_seed: 42,
            peer_player_map: HashMap::new(),
        }
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

    /// Get all peers including self as a sorted list for host election.
    pub fn all_peer_ids(&self) -> Vec<PeerId> {
        let mut ids: Vec<PeerId> = self.peers.keys().copied().collect();
        if let Some(my_id) = self.my_peer_id {
            ids.push(my_id);
        }
        ids.sort();
        ids
    }

    /// Determine if I am the host (lowest peer ID).
    pub fn update_host_status(&mut self) {
        if let Some(my_id) = self.my_peer_id {
            let all_ids = self.all_peer_ids();
            self.is_host = all_ids.first() == Some(&my_id);
        }
    }

    /// Check if all connected peers are ready.
    pub fn all_peers_ready(&self) -> bool {
        let connected_peers: Vec<_> = self.peers.values().filter(|p| p.connected).collect();
        if connected_peers.is_empty() {
            return false;
        }
        self.my_ready && connected_peers.iter().all(|p| p.ready)
    }

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
    SetConnected { room_id: String },
    SetDisconnected,
    SetError(String),

    // Peer management
    PeerJoined(PeerId),
    PeerLeft(PeerId),
    SetMyPeerId(PeerId),
    UpdatePeerReady { peer_id: PeerId, ready: bool },
    UpdatePeerRtt { peer_id: PeerId, rtt_ms: u32 },
    UpdatePeerInfo { peer_id: PeerId, name: String, color: Color },

    // Player setup
    SetMyName(String),
    SetMyColor(Color),
    SetMyReady(bool),

    // Game flow
    TransitionToLobby,
    StartGame { seed: u64, players: Vec<(PeerId, String, Color)> },
    StartCountdown,
    Tick,
    GameFinished,
    ResetToLobby,

    // Synchronization
    ReceiveFrameHash { peer_id: PeerId, frame: u64, hash: u64 },
    DetectDesync,
    StartResync,
    ApplySyncState { frame: u64, state_data: Vec<u8> },

    // Ping
    RecordPingSent { peer_id: PeerId, timestamp: f64 },

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
            P2PAction::SetConnected { room_id } => {
                new_state.phase = P2PPhase::WaitingForPeers;
                new_state.room_id = room_id.clone();
                new_state.add_log(&format!("Connected to room: {room_id}"));
            }
            P2PAction::SetDisconnected => {
                new_state.phase = P2PPhase::Disconnected;
                new_state.peers.clear();
                new_state.my_peer_id = None;
                new_state.is_host = false;
                new_state.my_ready = false;
                new_state.room_id.clear();
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

                new_state.update_host_status();

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
                    if let Some(peer) = new_state.peers.get_mut(&peer_id) {
                        peer.connected = false;
                        peer.rtt_ms = None;
                    }

                    // Eliminate the player's marble
                    let mut eliminated_player: Option<u32> = None;
                    let mut game_finished = false;

                    if matches!(new_state.phase, P2PPhase::Countdown { .. } | P2PPhase::Running) {
                        if let Some(&player_id) = new_state.peer_player_map.get(&peer_id) {
                            let game = Rc::make_mut(&mut new_state.game_state);
                            if game.eliminate_player(player_id) {
                                eliminated_player = Some(player_id);
                                game_finished = matches!(game.current_phase(), GamePhase::Finished { .. });
                            }
                        }
                    }

                    // Log and update phase after borrow ends
                    if let Some(player_id) = eliminated_player {
                        new_state.add_log(&format!(
                            "Player {} eliminated (peer disconnected)",
                            player_id
                        ));
                        if game_finished {
                            new_state.phase = P2PPhase::Finished;
                            new_state.add_log("Game finished!");
                        }
                    }
                } else {
                    // In lobby/waiting: remove peer completely
                    new_state.peers.remove(&peer_id);
                    new_state.update_host_status();

                    // Go back to waiting if no peers
                    let connected_count = new_state.peers.values().filter(|p| p.connected).count();
                    if connected_count == 0
                        && matches!(new_state.phase, P2PPhase::Lobby | P2PPhase::WaitingForPeers)
                    {
                        new_state.phase = P2PPhase::WaitingForPeers;
                    }
                }

                new_state.add_log(&format!("Peer left: {}", &peer_id.0.to_string()[..8]));
            }
            P2PAction::SetMyPeerId(peer_id) => {
                new_state.my_peer_id = Some(peer_id);
                new_state.update_host_status();
                new_state.add_log(&format!("My peer ID: {}", &peer_id.0.to_string()[..8]));
            }
            P2PAction::UpdatePeerReady { peer_id, ready } => {
                if let Some(peer) = new_state.peers.get_mut(&peer_id) {
                    peer.ready = ready;
                    new_state.add_log(&format!(
                        "Peer {} is {}",
                        &peer_id.0.to_string()[..8],
                        if ready { "ready" } else { "not ready" }
                    ));
                }
            }
            P2PAction::UpdatePeerRtt { peer_id, rtt_ms } => {
                if let Some(peer) = new_state.peers.get_mut(&peer_id) {
                    peer.rtt_ms = Some(rtt_ms);
                }
            }
            P2PAction::UpdatePeerInfo { peer_id, name, color } => {
                if let Some(peer) = new_state.peers.get_mut(&peer_id) {
                    peer.name = name;
                    peer.color = color;
                }
            }
            P2PAction::SetMyName(name) => {
                new_state.my_name = name;
            }
            P2PAction::SetMyColor(color) => {
                new_state.my_color = color;
            }
            P2PAction::SetMyReady(ready) => {
                new_state.my_ready = ready;
                new_state.add_log(if ready {
                    "You are ready"
                } else {
                    "You are not ready"
                });
            }
            P2PAction::TransitionToLobby => {
                new_state.phase = P2PPhase::Lobby;
                new_state.add_log("Entered lobby");
            }
            P2PAction::StartGame { seed, players } => {
                new_state.game_seed = seed;
                new_state.phase = P2PPhase::Starting;

                // Initialize game state with players
                let mut game = GameState::new(seed);
                game.load_map(RouletteConfig::default_classic());

                // Clear and rebuild peer_player_map
                new_state.peer_player_map.clear();
                for (i, (peer_id, name, color)) in players.iter().enumerate() {
                    game.add_player(name.clone(), *color);
                    game.set_player_ready(i as u32, true);
                    new_state.peer_player_map.insert(*peer_id, i as u32);
                }

                new_state.game_state = Rc::new(game);
                new_state.add_log(&format!("Game starting with seed {} and {} players", seed, players.len()));
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
            P2PAction::ResetToLobby => {
                let mut game = GameState::new(42);
                game.load_map(RouletteConfig::default_classic());
                new_state.game_state = Rc::new(game);

                // Remove disconnected peers, reset ready status for connected ones
                new_state.peers.retain(|_, peer| peer.connected);
                for peer in new_state.peers.values_mut() {
                    peer.ready = false;
                }

                let has_connected_peers = !new_state.peers.is_empty();
                new_state.phase = if has_connected_peers {
                    P2PPhase::Lobby
                } else {
                    P2PPhase::WaitingForPeers
                };
                new_state.my_ready = false;
                new_state.desync_detected = false;
                new_state.peer_hashes.clear();
                new_state.peer_player_map.clear();
                new_state.add_log("Reset to lobby");
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
            P2PAction::RecordPingSent { peer_id, timestamp } => {
                new_state.ping_sent.insert(peer_id, timestamp);
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
