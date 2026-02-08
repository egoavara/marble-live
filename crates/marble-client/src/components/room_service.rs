//! RoomServiceProvider — single entry point for all room.proto gRPC calls.
//!
//! Provides `RoomServiceHandle` via Yew context so any component can access
//! room lifecycle, peer resolution, and game operations without direct gRPC calls.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gloo::timers::callback::Interval;
use marble_proto::room::room_service_client::RoomServiceClient;
use marble_proto::room::{
    CreateRoomRequest, JoinRoomRequest, PlayerResult, RegisterPeerIdRequest,
    ReportArrivalRequest, ResolvePeerIdsRequest, StartGameRequest,
};
use marble_proto::user::user_service_client::UserServiceClient;
use marble_proto::user::{login_request, AnonymousLogin, GetUsersRequest, LoginRequest, UserInfo};
use tonic_web_wasm_client::Client;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::hooks::{use_auth_token, use_config_secret, use_config_username, use_fingerprint};

// ---------------------------------------------------------------------------
// RoomState
// ---------------------------------------------------------------------------

/// Room lifecycle state.
#[derive(Clone, PartialEq, Debug)]
pub enum RoomState {
    /// Not in any room.
    Idle,
    /// JoinRoom gRPC in progress.
    Joining { room_id: String },
    /// Successfully joined — P2P ready.
    Active {
        room_id: String,
        signaling_url: String,
        is_host: bool,
    },
    /// Error during join / create.
    Error { room_id: String, message: String },
}

// ---------------------------------------------------------------------------
// RoomServiceInner
// ---------------------------------------------------------------------------

struct RoomServiceInner {
    // Auth
    player_id: String,
    auth_token: Option<String>,
    auth_token_setter: Option<UseStateHandle<Option<String>>>,

    // Login credentials (for automatic re-login on token expiry)
    login_display_name: String,
    login_salt: String,
    login_fingerprint: String,

    // Room lifecycle
    room_state: RoomState,

    // peer_id → player_id cache (Yew-side, no Bevy round-trip)
    peer_cache: HashMap<String, String>,
    resolve_in_flight: bool,

    // user_id → display_name cache
    display_name_cache: HashMap<String, String>,
    get_users_in_flight: bool,

    // RegisterPeerId state
    peer_registered: bool,
    peer_register_confirmed: bool,
    register_in_flight: bool,

    // Bevy polling state
    last_peers_version: u64,

    // Server game state (from JoinRoom / ReportArrival responses)
    server_room_state: Option<i32>,  // proto RoomState (1=WAITING, 2=PLAYING, 3=ENDED)
    server_game_results: Vec<PlayerResult>,
    server_game_ended: bool,

    // Version setter — bumped on every state change to trigger re-render
    version_setter: Option<UseStateHandle<u32>>,
}

impl RoomServiceInner {
    fn new(player_id: String) -> Self {
        Self {
            player_id,
            auth_token: None,
            auth_token_setter: None,
            login_display_name: String::new(),
            login_salt: String::new(),
            login_fingerprint: String::new(),
            room_state: RoomState::Idle,
            peer_cache: HashMap::new(),
            resolve_in_flight: false,
            display_name_cache: HashMap::new(),
            get_users_in_flight: false,
            peer_registered: false,
            peer_register_confirmed: false,
            register_in_flight: false,
            last_peers_version: 0,
            server_room_state: None,
            server_game_results: Vec::new(),
            server_game_ended: false,
            version_setter: None,
        }
    }

    /// Bump version to trigger Yew re-render.
    fn bump_version(&self) {
        if let Some(ref setter) = self.version_setter {
            setter.set(**setter + 1);
        }
    }
}

// ---------------------------------------------------------------------------
// RoomServiceHandle (context value)
// ---------------------------------------------------------------------------

/// Handle exposed via Yew context. Clone-cheap (Rc + version).
#[derive(Clone)]
pub struct RoomServiceHandle {
    inner: Rc<RefCell<RoomServiceInner>>,
    version: u32,
}

impl PartialEq for RoomServiceHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner) && self.version == other.version
    }
}

impl RoomServiceHandle {
    // =======================================================================
    // Room lifecycle
    // =======================================================================

    /// Current room state.
    pub fn room_state(&self) -> RoomState {
        self.inner.borrow().room_state.clone()
    }

    /// Join an existing room. No-op if already Active in the same room.
    pub fn join(&self, room_id: &str) {
        // Already in this room?
        {
            let inner = self.inner.borrow();
            if let RoomState::Active {
                room_id: ref id, ..
            } = inner.room_state
            {
                if id == room_id {
                    return;
                }
            }
        }

        let inner = self.inner.clone();
        let room_id = room_id.to_string();

        // Transition → Joining
        {
            let mut inner_mut = inner.borrow_mut();
            inner_mut.room_state = RoomState::Joining {
                room_id: room_id.clone(),
            };
            inner_mut.bump_version();
        }

        spawn_local(async move {
            let player_id;
            let token;
            {
                let inner_ref = inner.borrow();
                player_id = inner_ref.player_id.clone();
                token = inner_ref.auth_token.clone();
            }

            let Some(mut grpc) = create_grpc_client() else {
                let mut inner_mut = inner.borrow_mut();
                inner_mut.room_state = RoomState::Error {
                    room_id: room_id.clone(),
                    message: "Failed to create gRPC client".to_string(),
                };
                inner_mut.bump_version();
                return;
            };

            // 1. JoinRoom
            let mut token = token;
            let join_resp = grpc
                .join_room(attach_auth(
                    JoinRoomRequest {
                        room_id: room_id.clone(),
                        role: None,
                    },
                    &token,
                ))
                .await;

            // Retry on authentication failure (e.g. server restart invalidated token)
            let join_resp = match join_resp {
                Err(e) if is_unauthenticated(&e) => {
                    tracing::info!("RoomService: JoinRoom auth failed, attempting re-login");
                    if let Some(new_token) = relogin(&inner).await {
                        token = Some(new_token);
                        grpc.join_room(attach_auth(
                            JoinRoomRequest {
                                room_id: room_id.clone(),
                                role: None,
                            },
                            &token,
                        ))
                        .await
                    } else {
                        Err(e)
                    }
                }
                other => other,
            };

            let (signaling_url, is_host, server_state, game_results) = match join_resp {
                Ok(resp) => {
                    let resp = resp.into_inner();
                    let sig_url = resp
                        .topology
                        .as_ref()
                        .map(|t| t.signaling_url.clone())
                        .unwrap_or_default();
                    let host = resp
                        .room
                        .as_ref()
                        .map(|r| r.host_user_id == player_id)
                        .unwrap_or(false);
                    let state = resp.room.as_ref().map(|r| r.state).unwrap_or(0);
                    let results = resp.room.as_ref()
                        .and_then(|r| r.game_state.as_ref())
                        .map(|gs| gs.results.clone())
                        .unwrap_or_default();
                    (sig_url, host, state, results)
                }
                Err(e) => {
                    let mut inner_mut = inner.borrow_mut();
                    inner_mut.room_state = RoomState::Error {
                        room_id: room_id.clone(),
                        message: e.message().to_string(),
                    };
                    inner_mut.bump_version();
                    return;
                }
            };

            // 2. Transition → Active
            {
                let game_ended = server_state == 3; // ROOM_STATE_ENDED
                let mut inner_mut = inner.borrow_mut();
                inner_mut.room_state = RoomState::Active {
                    room_id,
                    signaling_url,
                    is_host,
                };
                inner_mut.peer_registered = false;
                inner_mut.peer_register_confirmed = false;
                inner_mut.register_in_flight = false;
                inner_mut.peer_cache.clear();
                inner_mut.get_users_in_flight = false;
                inner_mut.last_peers_version = 0;
                inner_mut.server_room_state = Some(server_state);
                inner_mut.server_game_results = game_results;
                inner_mut.server_game_ended = game_ended;
                inner_mut.bump_version();
            }

            tracing::info!("RoomService: joined room, is_host={}", is_host);
        });
    }

    /// Create a new room and then join it.
    pub fn create_and_join(&self, max_players: u32) {
        let inner = self.inner.clone();

        // Transition → Joining (with placeholder room_id)
        {
            let mut inner_mut = inner.borrow_mut();
            inner_mut.room_state = RoomState::Joining {
                room_id: String::new(),
            };
            inner_mut.bump_version();
        }

        let handle = self.clone();
        spawn_local(async move {
            let token;
            {
                let inner_ref = inner.borrow();
                token = inner_ref.auth_token.clone();
            }

            let Some(mut grpc) = create_grpc_client() else {
                let mut inner_mut = inner.borrow_mut();
                inner_mut.room_state = RoomState::Error {
                    room_id: String::new(),
                    message: "Failed to create gRPC client".to_string(),
                };
                inner_mut.bump_version();
                return;
            };

            // CreateRoom
            let mut token = token;
            let create_resp = grpc
                .create_room(attach_auth(
                    CreateRoomRequest {
                        map_id: String::new(),
                        max_players,
                        room_name: String::new(),
                        is_public: true,
                    },
                    &token,
                ))
                .await;

            // Retry on authentication failure
            let create_resp = match create_resp {
                Err(e) if is_unauthenticated(&e) => {
                    tracing::info!("RoomService: CreateRoom auth failed, attempting re-login");
                    if let Some(new_token) = relogin(&inner).await {
                        token = Some(new_token);
                        grpc.create_room(attach_auth(
                            CreateRoomRequest {
                                map_id: String::new(),
                                max_players,
                                room_name: String::new(),
                                is_public: true,
                            },
                            &token,
                        ))
                        .await
                    } else {
                        Err(e)
                    }
                }
                other => other,
            };

            match create_resp {
                Ok(resp) => {
                    let room_id = resp
                        .into_inner()
                        .room
                        .map(|r| r.room_id)
                        .unwrap_or_default();
                    tracing::info!("RoomService: created room {}", room_id);
                    // Chain → join
                    handle.join(&room_id);
                }
                Err(e) => {
                    let mut inner_mut = inner.borrow_mut();
                    inner_mut.room_state = RoomState::Error {
                        room_id: String::new(),
                        message: e.message().to_string(),
                    };
                    inner_mut.bump_version();
                }
            }
        });
    }

    /// Leave current room. Resets to Idle.
    pub fn leave(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.room_state = RoomState::Idle;
        inner.peer_cache.clear();
        inner.get_users_in_flight = false;
        inner.peer_registered = false;
        inner.peer_register_confirmed = false;
        inner.register_in_flight = false;
        inner.resolve_in_flight = false;
        inner.last_peers_version = 0;
        inner.server_room_state = None;
        inner.server_game_results = Vec::new();
        inner.server_game_ended = false;
        inner.bump_version();
        tracing::info!("RoomService: left room");
    }

    // =======================================================================
    // Peer resolution (synchronous, cache-based)
    // =======================================================================

    /// Resolve a peer_id to a player name. Returns `None` if not yet cached.
    pub fn player_name(&self, peer_id: &str) -> Option<String> {
        let inner = self.inner.borrow();
        // Check if it's our own peer_id
        let my_peer_id = marble_core::bevy::wasm_entry::get_my_peer_id();
        if !my_peer_id.is_empty() && my_peer_id == peer_id {
            return Some(inner.player_id.clone());
        }
        inner.peer_cache.get(peer_id).cloned()
    }

    /// Resolve a peer_id to a player name, falling back to a short peer prefix.
    pub fn player_name_or_fallback(&self, peer_id: &str) -> String {
        self.player_name(peer_id)
            .unwrap_or_else(|| format!("Peer-{}", &peer_id[..peer_id.len().min(8)]))
    }

    /// Resolve a user_id to a display name from cache.
    pub fn display_name(&self, user_id: &str) -> Option<String> {
        self.inner.borrow().display_name_cache.get(user_id).cloned()
    }

    /// Resolve a user_id to a display name, falling back to "User-{8chars}".
    pub fn display_name_or_fallback(&self, user_id: &str) -> String {
        self.display_name(user_id)
            .unwrap_or_else(|| format!("User-{}", &user_id[..user_id.len().min(8)]))
    }

    // =======================================================================
    // Game operations (fire-and-forget gRPC)
    // =======================================================================

    /// Report game start to server (host only).
    pub fn start_game(&self, start_frame: u64) {
        let inner_rc = self.inner.clone();
        let room_id;
        let token;
        {
            let inner = inner_rc.borrow();
            room_id = match &inner.room_state {
                RoomState::Active { room_id, .. } => room_id.clone(),
                _ => return,
            };
            token = inner.auth_token.clone();
        }

        spawn_local(async move {
            let Some(mut grpc) = create_grpc_client() else {
                return;
            };
            let mut token = token;
            let req = attach_auth(
                StartGameRequest {
                    room_id: room_id.clone(),
                    start_frame,
                },
                &token,
            );
            match grpc.start_game(req).await {
                Ok(resp) => {
                    let _resp = resp.into_inner();
                    tracing::info!(
                        room_id = %room_id,
                        start_frame,
                        "RoomService: game started on server"
                    );
                }
                Err(e) if is_unauthenticated(&e) => {
                    tracing::info!("RoomService: StartGame auth failed, attempting re-login");
                    if let Some(new_token) = relogin(&inner_rc).await {
                        token = Some(new_token);
                        let req = attach_auth(
                            StartGameRequest {
                                room_id: room_id.clone(),
                                start_frame,
                            },
                            &token,
                        );
                        match grpc.start_game(req).await {
                            Ok(_) => {
                                tracing::info!(
                                    room_id = %room_id,
                                    start_frame,
                                    "RoomService: game started on server (after re-login)"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    room_id = %room_id,
                                    error = %e,
                                    "RoomService: StartGame RPC failed after re-login"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        room_id = %room_id,
                        error = %e,
                        "RoomService: StartGame RPC failed"
                    );
                }
            }
        });
    }

    /// Report player arrival to server (host only).
    pub fn report_arrival(&self, arrived_user_id: &str, arrival_frame: u64, rank: u32) {
        let inner_rc = self.inner.clone();
        let room_id;
        let token;
        {
            let inner = inner_rc.borrow();
            room_id = match &inner.room_state {
                RoomState::Active { room_id, .. } => room_id.clone(),
                _ => return,
            };
            token = inner.auth_token.clone();
        }
        let arrived_user_id = arrived_user_id.to_string();

        spawn_local(async move {
            let Some(mut grpc) = create_grpc_client() else {
                return;
            };
            let mut token = token;
            let req = attach_auth(
                ReportArrivalRequest {
                    room_id: room_id.clone(),
                    arrived_user_id: arrived_user_id.clone(),
                    arrival_frame,
                    rank,
                },
                &token,
            );
            match grpc.report_arrival(req).await {
                Ok(resp) => {
                    let resp = resp.into_inner();
                    // Check if game ended from server response
                    if let Some(room_info) = resp.room.as_ref() {
                        if room_info.state == 3 {  // ROOM_STATE_ENDED
                            let results = room_info.game_state.as_ref()
                                .map(|gs| gs.results.clone())
                                .unwrap_or_default();
                            let mut inner_mut = inner_rc.borrow_mut();
                            inner_mut.server_game_ended = true;
                            inner_mut.server_room_state = Some(3);
                            inner_mut.server_game_results = results;
                            inner_mut.bump_version();
                        }
                    }
                    tracing::info!(
                        user = %arrived_user_id,
                        rank,
                        frame = arrival_frame,
                        "RoomService: reported arrival"
                    );
                }
                Err(e) if is_unauthenticated(&e) => {
                    tracing::info!("RoomService: ReportArrival auth failed, attempting re-login");
                    if let Some(new_token) = relogin(&inner_rc).await {
                        token = Some(new_token);
                        let req = attach_auth(
                            ReportArrivalRequest {
                                room_id: room_id.clone(),
                                arrived_user_id: arrived_user_id.clone(),
                                arrival_frame,
                                rank,
                            },
                            &token,
                        );
                        match grpc.report_arrival(req).await {
                            Ok(resp) => {
                                let resp = resp.into_inner();
                                // Check if game ended from server response (after re-login)
                                if let Some(room_info) = resp.room.as_ref() {
                                    if room_info.state == 3 {
                                        let results = room_info.game_state.as_ref()
                                            .map(|gs| gs.results.clone())
                                            .unwrap_or_default();
                                        let mut inner_mut = inner_rc.borrow_mut();
                                        inner_mut.server_game_ended = true;
                                        inner_mut.server_room_state = Some(3);
                                        inner_mut.server_game_results = results;
                                        inner_mut.bump_version();
                                    }
                                }
                                tracing::info!(
                                    user = %arrived_user_id,
                                    rank,
                                    frame = arrival_frame,
                                    "RoomService: reported arrival (after re-login)"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    user = %arrived_user_id,
                                    error = %e,
                                    "RoomService: ReportArrival RPC failed after re-login"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        user = %arrived_user_id,
                        error = %e,
                        "RoomService: ReportArrival RPC failed"
                    );
                }
            }
        });
    }

    // =======================================================================
    // Accessors
    // =======================================================================

    /// Current room ID (if Active or Joining).
    pub fn room_id(&self) -> Option<String> {
        match &self.inner.borrow().room_state {
            RoomState::Active { room_id, .. } | RoomState::Joining { room_id } => {
                Some(room_id.clone())
            }
            _ => None,
        }
    }

    /// Local player ID.
    pub fn player_id(&self) -> String {
        self.inner.borrow().player_id.clone()
    }

    /// Signaling URL (Active only).
    pub fn signaling_url(&self) -> Option<String> {
        match &self.inner.borrow().room_state {
            RoomState::Active { signaling_url, .. } => Some(signaling_url.clone()),
            _ => None,
        }
    }

    /// Whether local player is host.
    pub fn is_host(&self) -> bool {
        matches!(
            self.inner.borrow().room_state,
            RoomState::Active { is_host: true, .. }
        )
    }

    // =======================================================================
    // Server game state accessors
    // =======================================================================

    /// Server room state (proto RoomState: 1=WAITING, 2=PLAYING, 3=ENDED).
    pub fn server_room_state(&self) -> Option<i32> {
        self.inner.borrow().server_room_state
    }

    /// Whether the server has indicated the game is ended.
    pub fn is_game_ended(&self) -> bool {
        self.inner.borrow().server_game_ended
    }

    /// Server game results (sorted by rank).
    pub fn game_results(&self) -> Vec<PlayerResult> {
        self.inner.borrow().server_game_results.clone()
    }

    /// Mark game as ended from client-side detection (e.g. all marbles arrived).
    pub fn set_game_ended(&self, results: Vec<PlayerResult>) {
        let mut inner = self.inner.borrow_mut();
        inner.server_game_ended = true;
        inner.server_game_results = results;
        inner.bump_version();
    }
}

// ---------------------------------------------------------------------------
// gRPC helpers
// ---------------------------------------------------------------------------

fn create_grpc_client() -> Option<RoomServiceClient<Client>> {
    let origin = web_sys::window()?.location().origin().ok()?;
    Some(RoomServiceClient::new(Client::new(format!(
        "{}/grpc",
        origin
    ))))
}

fn create_user_grpc_client() -> Option<UserServiceClient<Client>> {
    let origin = web_sys::window()?.location().origin().ok()?;
    Some(UserServiceClient::new(Client::new(format!(
        "{}/grpc",
        origin
    ))))
}

fn attach_auth<T>(msg: T, token: &Option<String>) -> tonic::Request<T> {
    let mut req = tonic::Request::new(msg);
    if let Some(token) = token {
        if let Ok(val) = format!("Bearer {token}").parse() {
            req.metadata_mut().insert("authorization", val);
        }
    }
    req
}

/// Check if a tonic error indicates authentication failure.
fn is_unauthenticated(status: &tonic::Status) -> bool {
    status.code() == tonic::Code::Unauthenticated
}

/// Attempt re-login using stored credentials. Updates inner state and Yew hook on success.
async fn relogin(inner: &Rc<RefCell<RoomServiceInner>>) -> Option<String> {
    let (display_name, salt, fingerprint) = {
        let inner_ref = inner.borrow();
        (
            inner_ref.login_display_name.clone(),
            inner_ref.login_salt.clone(),
            inner_ref.login_fingerprint.clone(),
        )
    };

    if display_name.is_empty() || fingerprint.is_empty() {
        tracing::warn!("Cannot re-login: missing credentials");
        return None;
    }

    let mut grpc = create_user_grpc_client()?;
    let login_req = LoginRequest {
        method: Some(login_request::Method::Anonymous(AnonymousLogin {
            display_name: display_name.clone(),
            salt,
            fingerprint,
        })),
    };

    match grpc.login(login_req).await {
        Ok(resp) => {
            let resp = resp.into_inner();
            let token = resp.token;
            let user_id = resp
                .user
                .as_ref()
                .map(|u| u.user_id.clone())
                .unwrap_or_default();
            let dn = resp
                .user
                .as_ref()
                .map(|u| u.display_name.clone())
                .unwrap_or_default();

            tracing::info!(
                user_id = %user_id,
                display_name = %dn,
                "Re-login successful, token refreshed"
            );

            let mut inner_mut = inner.borrow_mut();
            inner_mut.auth_token = Some(token.clone());
            inner_mut.player_id = user_id.clone();
            if !dn.is_empty() {
                inner_mut.display_name_cache.insert(user_id, dn);
            }
            // Update Yew hook → triggers localStorage persistence
            if let Some(ref setter) = inner_mut.auth_token_setter {
                setter.set(Some(token.clone()));
            }

            Some(token)
        }
        Err(e) => {
            tracing::error!(error = %e, "Re-login failed");
            None
        }
    }
}

/// Extract `user_id` (the `sub` claim) from our custom JWT token.
/// Token format: `{base64url_payload}.{signature}`.
fn extract_user_id_from_token(token: &str) -> Option<String> {
    let payload_b64 = token.split('.').next()?;
    let decoded = base64url_decode(payload_b64)?;
    let payload: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    payload.get("sub")?.as_str().map(|s| s.to_string())
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    fn val(c: u8) -> Option<u32> {
        TABLE.iter().position(|&ch| ch == c).map(|p| p as u32)
    }

    let bytes = input.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let b0 = val(bytes[i])?;
        let b1 = if i + 1 < bytes.len() { val(bytes[i + 1])? } else { 0 };
        let b2 = if i + 2 < bytes.len() { val(bytes[i + 2])? } else { 0 };
        let b3 = if i + 3 < bytes.len() { val(bytes[i + 3])? } else { 0 };

        let n = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        result.push(((n >> 16) & 0xFF) as u8);
        if i + 2 < bytes.len() {
            result.push(((n >> 8) & 0xFF) as u8);
        }
        if i + 3 < bytes.len() {
            result.push((n & 0xFF) as u8);
        }

        i += 4;
    }

    Some(result)
}

// ---------------------------------------------------------------------------
// RoomServiceProvider component
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct RoomServiceProviderProps {
    pub children: Children,
}

#[function_component(RoomServiceProvider)]
pub fn room_service_provider(props: &RoomServiceProviderProps) -> Html {
    let config_username = use_config_username();
    let config_secret = use_config_secret();
    let fingerprint = use_fingerprint();
    let auth_token = use_auth_token();

    let player_id = (*config_username)
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let inner = use_mut_ref(|| RoomServiceInner::new(player_id.clone()));

    // Keep player credentials in sync with config changes (only pre-login fallback)
    {
        let inner = inner.clone();
        let pid = player_id.clone();
        use_effect_with(pid, move |pid| {
            let mut inner_mut = inner.borrow_mut();
            // Only use config_username when no token (pre-login state).
            // Once a token exists, player_id is the UUID extracted from the token.
            if inner_mut.auth_token.is_none() {
                inner_mut.player_id = pid.clone();
            }
        });
    }

    // Sync auth_token from hook → inner, and extract user_id from JWT
    {
        let inner = inner.clone();
        let token = (*auth_token).clone();
        use_effect_with(token, move |token| {
            let mut inner_mut = inner.borrow_mut();
            inner_mut.auth_token = token.clone();
            // Extract user_id (UUID) from JWT so player_id is always correct
            if let Some(t) = token.as_deref() {
                if let Some(uid) = extract_user_id_from_token(t) {
                    inner_mut.player_id = uid;
                }
            }
        });
    }

    // Store auth_token setter in inner for re-login
    {
        let inner = inner.clone();
        let auth_token = auth_token.clone();
        use_effect_with((), move |_| {
            inner.borrow_mut().auth_token_setter = Some(auth_token);
        });
    }

    // Sync login credentials to inner for automatic re-login on token expiry
    {
        let inner = inner.clone();
        let username = (*config_username).clone();
        let secret_val = (*config_secret).to_string();
        let fp = (*fingerprint).clone();
        use_effect_with(
            (username.clone(), secret_val.clone(), fp.clone()),
            move |_| {
                let mut inner_mut = inner.borrow_mut();
                inner_mut.login_display_name = username.unwrap_or_default();
                inner_mut.login_salt = secret_val;
                inner_mut.login_fingerprint = fp.unwrap_or_default();
            },
        );
    }

    // Auto-login: when username + fingerprint are ready and no token exists, perform Login RPC
    {
        let inner = inner.clone();
        let auth_token = auth_token.clone();
        let username = (*config_username).clone();
        let secret = (*config_secret).clone();
        let fp = (*fingerprint).clone();

        use_effect_with((username.clone(), fp.clone(), (*auth_token).clone()), move |_| {
            // Need username and fingerprint to be ready, and no existing token
            let Some(display_name) = username else {
                return;
            };
            if display_name.is_empty() {
                return;
            }
            let Some(fp_value) = fp else {
                return;
            };
            if auth_token.is_some() {
                return;
            }

            let salt = secret.to_string();
            let inner = inner.clone();
            let auth_token = auth_token.clone();

            spawn_local(async move {
                let Some(mut grpc) = create_user_grpc_client() else {
                    tracing::warn!("Failed to create UserService gRPC client for login");
                    return;
                };

                let login_req = LoginRequest {
                    method: Some(login_request::Method::Anonymous(AnonymousLogin {
                        display_name: display_name.clone(),
                        salt,
                        fingerprint: fp_value,
                    })),
                };

                match grpc.login(login_req).await {
                    Ok(resp) => {
                        let resp = resp.into_inner();
                        let token = resp.token;
                        let user_id = resp.user.as_ref().map(|u| u.user_id.clone()).unwrap_or_default();
                        let dn = resp.user.as_ref().map(|u| u.display_name.clone()).unwrap_or_default();

                        tracing::info!(user_id = %user_id, display_name = %dn, "Auto-login successful");

                        // Store token in hook (persists to LocalStorage)
                        auth_token.set(Some(token.clone()));

                        // Update inner state
                        let mut inner_mut = inner.borrow_mut();
                        inner_mut.auth_token = Some(token);
                        inner_mut.player_id = user_id.clone();
                        // Cache own display name so lobby shows it immediately
                        if !dn.is_empty() {
                            inner_mut.display_name_cache.insert(user_id, dn);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Auto-login failed");
                    }
                }
            });
        });
    }

    let version = use_state(|| 0u32);

    // Store version setter inside inner so methods can bump it
    {
        let inner = inner.clone();
        let version = version.clone();
        use_effect_with(*version, move |_| {
            inner.borrow_mut().version_setter = Some(version);
        });
    }

    // 200ms polling interval — only active when room is Active
    {
        let inner = inner.clone();
        let _version = version.clone(); // dependency so effect reruns on version change

        use_effect_with((), move |_| {
            let interval = Interval::new(200, move || {
                let is_active;
                let room_id;
                {
                    let inner_ref = inner.borrow();
                    is_active = matches!(inner_ref.room_state, RoomState::Active { .. });
                    room_id = match &inner_ref.room_state {
                        RoomState::Active { room_id, .. } => room_id.clone(),
                        _ => String::new(),
                    };
                }

                if !is_active {
                    return;
                }

                // --- RegisterPeerId (등록 + 검증) ---
                let peer_register_confirmed = inner.borrow().peer_register_confirmed;
                let register_in_flight = inner.borrow().register_in_flight;

                if !peer_register_confirmed && !register_in_flight {
                    let my_peer_id = marble_core::bevy::wasm_entry::get_my_peer_id();
                    if !my_peer_id.is_empty() {
                        inner.borrow_mut().register_in_flight = true;
                        let inner_c = inner.clone();
                        let room_id = room_id.clone();

                        spawn_local(async move {
                            // 1) RegisterPeerId 호출
                            let registered = register_peer_id_grpc(
                                &room_id,
                                &my_peer_id,
                                &inner_c,
                            ).await;

                            if !registered {
                                inner_c.borrow_mut().register_in_flight = false;
                                return;
                            }

                            // 2) ResolvePeerIds로 자신의 peer_id 검증
                            if let Some(resolved) = resolve_peer_ids_grpc(
                                &room_id,
                                &[my_peer_id.clone()],
                                &inner_c,
                            ).await {
                                if resolved.contains_key(&my_peer_id) {
                                    // 서버에 매핑 확인됨 → 완료
                                    let mut inner_mut = inner_c.borrow_mut();
                                    inner_mut.peer_registered = true;
                                    inner_mut.peer_register_confirmed = true;
                                    inner_mut.register_in_flight = false;
                                    tracing::info!(
                                        peer_id = %my_peer_id,
                                        "RoomService: peer_id registration confirmed"
                                    );
                                    return;
                                }
                            }

                            // 검증 실패 → 다음 틱에 재시도
                            inner_c.borrow_mut().register_in_flight = false;
                            tracing::debug!("RoomService: peer_id registration not yet confirmed, will retry");
                        });
                    }
                }

                // --- ResolvePeerIds ---
                let current_peers_version = marble_core::bevy::wasm_entry::get_peers_version();
                let last_version = inner.borrow().last_peers_version;
                let resolve_in_flight = inner.borrow().resolve_in_flight;

                if current_peers_version != last_version || !resolve_in_flight {
                    inner.borrow_mut().last_peers_version = current_peers_version;

                    let peers_js = marble_core::bevy::wasm_entry::get_peers();
                    if let Ok(bevy_peers) = serde_wasm_bindgen::from_value::<
                        Vec<marble_core::bevy::state_store::PeerInfo>,
                    >(peers_js)
                    {
                        let unresolved: Vec<String> = {
                            let inner_ref = inner.borrow();
                            bevy_peers
                                .iter()
                                .filter(|bp| {
                                    // Not in cache and not self
                                    let my_peer =
                                        marble_core::bevy::wasm_entry::get_my_peer_id();
                                    bp.peer_id != my_peer
                                        && !inner_ref.peer_cache.contains_key(&bp.peer_id)
                                })
                                .map(|bp| bp.peer_id.clone())
                                .collect()
                        };

                        if !unresolved.is_empty() && !resolve_in_flight {
                            inner.borrow_mut().resolve_in_flight = true;
                            let inner_c = inner.clone();
                            let room_id = room_id.clone();

                            spawn_local(async move {
                                if let Some(resolved) = resolve_peer_ids_grpc(
                                    &room_id,
                                    &unresolved,
                                    &inner_c,
                                )
                                .await
                                {
                                    let mut inner_mut = inner_c.borrow_mut();
                                    for (peer_id, user_id) in &resolved {
                                        inner_mut
                                            .peer_cache
                                            .insert(peer_id.clone(), user_id.clone());
                                        // Also update Bevy's state store
                                        marble_core::bevy::wasm_entry::update_peer_player_id(
                                            peer_id, user_id,
                                        );
                                    }
                                    if !resolved.is_empty() {
                                        inner_mut.bump_version();
                                    }
                                }
                                inner_c.borrow_mut().resolve_in_flight = false;
                            });
                        }
                    }
                }

                // --- GetUsers (display name resolution) ---
                let get_users_in_flight = inner.borrow().get_users_in_flight;
                if !get_users_in_flight {
                    let unresolved_user_ids: Vec<String> = {
                        let inner_ref = inner.borrow();
                        let mut ids: Vec<String> = inner_ref
                            .peer_cache
                            .values()
                            .filter(|user_id| !inner_ref.display_name_cache.contains_key(user_id.as_str()))
                            .cloned()
                            .collect();
                        // Also include self if not yet cached
                        if !inner_ref.player_id.is_empty()
                            && !inner_ref.display_name_cache.contains_key(&inner_ref.player_id)
                        {
                            ids.push(inner_ref.player_id.clone());
                        }
                        // Also include user_ids from server game results
                        for result in &inner_ref.server_game_results {
                            if !result.user_id.is_empty()
                                && !inner_ref.display_name_cache.contains_key(&result.user_id)
                                && !ids.contains(&result.user_id)
                            {
                                ids.push(result.user_id.clone());
                            }
                        }
                        ids
                    };

                    if !unresolved_user_ids.is_empty() {
                        inner.borrow_mut().get_users_in_flight = true;
                        let inner_c = inner.clone();

                        spawn_local(async move {
                            if let Some(users) = get_users_grpc(&unresolved_user_ids, &inner_c).await {
                                let mut inner_mut = inner_c.borrow_mut();
                                for user in users {
                                    inner_mut
                                        .display_name_cache
                                        .insert(user.user_id.clone(), user.display_name.clone());
                                }
                                inner_mut.bump_version();
                            }
                            inner_c.borrow_mut().get_users_in_flight = false;
                        });
                    }
                }
            });

            move || drop(interval)
        });
    }

    let handle = RoomServiceHandle {
        inner: inner.clone(),
        version: *version,
    };

    html! {
        <ContextProvider<RoomServiceHandle> context={handle}>
            {props.children.clone()}
        </ContextProvider<RoomServiceHandle>>
    }
}

// ---------------------------------------------------------------------------
// use_room_service hook
// ---------------------------------------------------------------------------

/// Access `RoomServiceHandle` from context. Panics if no `RoomServiceProvider` ancestor.
#[hook]
pub fn use_room_service() -> RoomServiceHandle {
    use_context::<RoomServiceHandle>().expect("use_room_service: RoomServiceProvider not found")
}

// ---------------------------------------------------------------------------
// Async gRPC helpers
// ---------------------------------------------------------------------------

async fn register_peer_id_grpc(
    room_id: &str,
    peer_id: &str,
    inner: &Rc<RefCell<RoomServiceInner>>,
) -> bool {
    let Some(mut grpc) = create_grpc_client() else {
        return false;
    };

    let mut token = inner.borrow().auth_token.clone();
    let mut relogin_attempted = false;

    for attempt in 0..=3u32 {
        let req = attach_auth(
            RegisterPeerIdRequest {
                room_id: room_id.to_string(),
                peer_id: peer_id.to_string(),
            },
            &token,
        );

        match grpc.register_peer_id(req).await {
            Ok(_) => {
                tracing::info!(
                    peer_id = %peer_id,
                    "RoomService: registered peer_id"
                );
                return true;
            }
            Err(e) if is_unauthenticated(&e) && !relogin_attempted => {
                relogin_attempted = true;
                tracing::info!("RoomService: RegisterPeerId auth failed, attempting re-login");
                if let Some(new_token) = relogin(inner).await {
                    token = Some(new_token);
                    continue;
                }
                return false;
            }
            Err(e) => {
                if attempt < 3 {
                    tracing::warn!(
                        error = %e,
                        attempt = attempt + 1,
                        "RoomService: RegisterPeerId retry in 500ms"
                    );
                    gloo::timers::future::TimeoutFuture::new(500).await;
                } else {
                    tracing::error!(
                        error = %e,
                        "RoomService: RegisterPeerId failed after 4 attempts"
                    );
                }
            }
        }
    }

    false
}

async fn resolve_peer_ids_grpc(
    room_id: &str,
    peer_ids: &[String],
    inner: &Rc<RefCell<RoomServiceInner>>,
) -> Option<HashMap<String, String>> {
    if peer_ids.is_empty() {
        return Some(HashMap::new());
    }

    let Some(mut grpc) = create_grpc_client() else {
        return None;
    };

    let mut token = inner.borrow().auth_token.clone();
    let req = attach_auth(
        ResolvePeerIdsRequest {
            room_id: room_id.to_string(),
            peer_ids: peer_ids.to_vec(),
        },
        &token,
    );

    match grpc.resolve_peer_ids(req).await {
        Ok(resp) => {
            let resolved = resp.into_inner().peer_to_user;
            tracing::debug!(
                resolved = resolved.len(),
                requested = peer_ids.len(),
                "RoomService: resolved peer_ids"
            );
            Some(resolved)
        }
        Err(e) if is_unauthenticated(&e) => {
            tracing::info!("RoomService: ResolvePeerIds auth failed, attempting re-login");
            if let Some(new_token) = relogin(inner).await {
                token = Some(new_token);
                let req = attach_auth(
                    ResolvePeerIdsRequest {
                        room_id: room_id.to_string(),
                        peer_ids: peer_ids.to_vec(),
                    },
                    &token,
                );
                match grpc.resolve_peer_ids(req).await {
                    Ok(resp) => {
                        let resolved = resp.into_inner().peer_to_user;
                        tracing::debug!(
                            resolved = resolved.len(),
                            requested = peer_ids.len(),
                            "RoomService: resolved peer_ids (after re-login)"
                        );
                        Some(resolved)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "RoomService: ResolvePeerIds failed after re-login");
                        None
                    }
                }
            } else {
                None
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "RoomService: ResolvePeerIds failed");
            None
        }
    }
}

async fn get_users_grpc(
    user_ids: &[String],
    inner: &Rc<RefCell<RoomServiceInner>>,
) -> Option<Vec<UserInfo>> {
    if user_ids.is_empty() {
        return Some(Vec::new());
    }

    let Some(mut grpc) = create_user_grpc_client() else {
        return None;
    };

    let mut token = inner.borrow().auth_token.clone();
    let req = attach_auth(
        GetUsersRequest {
            user_ids: user_ids.to_vec(),
        },
        &token,
    );

    match grpc.get_users(req).await {
        Ok(resp) => {
            let users = resp.into_inner().users;
            tracing::debug!(
                resolved = users.len(),
                requested = user_ids.len(),
                "RoomService: resolved display names"
            );
            Some(users)
        }
        Err(e) if is_unauthenticated(&e) => {
            tracing::info!("RoomService: GetUsers auth failed, attempting re-login");
            if let Some(new_token) = relogin(inner).await {
                token = Some(new_token);
                let req = attach_auth(
                    GetUsersRequest {
                        user_ids: user_ids.to_vec(),
                    },
                    &token,
                );
                match grpc.get_users(req).await {
                    Ok(resp) => {
                        let users = resp.into_inner().users;
                        tracing::debug!(
                            resolved = users.len(),
                            requested = user_ids.len(),
                            "RoomService: resolved display names (after re-login)"
                        );
                        Some(users)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "RoomService: GetUsers failed after re-login");
                        None
                    }
                }
            } else {
                None
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "RoomService: GetUsers failed");
            None
        }
    }
}
