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
    CreateRoomRequest, JoinRoomRequest, RegisterPeerIdRequest,
    ReportArrivalRequest, ResolvePeerIdsRequest, StartGameRequest,
};
use tonic_web_wasm_client::Client;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::hooks::use_config_username;

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

    // Room lifecycle
    room_state: RoomState,

    // peer_id → player_id cache (Yew-side, no Bevy round-trip)
    peer_cache: HashMap<String, String>,
    resolve_in_flight: bool,

    // RegisterPeerId state
    peer_registered: bool,

    // Bevy polling state
    last_peers_version: u64,

    // Version setter — bumped on every state change to trigger re-render
    version_setter: Option<UseStateHandle<u32>>,
}

impl RoomServiceInner {
    fn new(player_id: String) -> Self {
        Self {
            player_id,
            room_state: RoomState::Idle,
            peer_cache: HashMap::new(),
            resolve_in_flight: false,
            peer_registered: false,
            last_peers_version: 0,
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
            {
                let inner_ref = inner.borrow();
                player_id = inner_ref.player_id.clone();
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
            let join_resp = grpc
                .join_room(JoinRoomRequest {
                    room_id: room_id.clone(),
                    role: None,
                })
                .await;

            let (signaling_url, is_host) = match join_resp {
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
                    (sig_url, host)
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
                let mut inner_mut = inner.borrow_mut();
                inner_mut.room_state = RoomState::Active {
                    room_id,
                    signaling_url,
                    is_host,
                };
                inner_mut.peer_registered = false;
                inner_mut.peer_cache.clear();
                inner_mut.last_peers_version = 0;
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
            let create_resp = grpc
                .create_room(CreateRoomRequest {
                    map_id: String::new(),
                    max_players,
                    room_name: String::new(),
                    is_public: true,
                })
                .await;

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
        inner.peer_registered = false;
        inner.resolve_in_flight = false;
        inner.last_peers_version = 0;
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

    // =======================================================================
    // Game operations (fire-and-forget gRPC)
    // =======================================================================

    /// Report game start to server (host only).
    pub fn start_game(&self, start_frame: u64) {
        let inner = self.inner.borrow();
        let room_id = match &inner.room_state {
            RoomState::Active { room_id, .. } => room_id.clone(),
            _ => return,
        };

        spawn_local(async move {
            let Some(mut grpc) = create_grpc_client() else {
                return;
            };
            let req = StartGameRequest {
                room_id: room_id.clone(),
                start_frame,
            };
            match grpc.start_game(req).await {
                Ok(resp) => {
                    let _resp = resp.into_inner();
                    tracing::info!(
                        room_id = %room_id,
                        start_frame,
                        "RoomService: game started on server"
                    );
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
        let inner = self.inner.borrow();
        let room_id = match &inner.room_state {
            RoomState::Active { room_id, .. } => room_id.clone(),
            _ => return,
        };
        let arrived_user_id = arrived_user_id.to_string();

        spawn_local(async move {
            let Some(mut grpc) = create_grpc_client() else {
                return;
            };
            let req = ReportArrivalRequest {
                room_id: room_id.clone(),
                arrived_user_id: arrived_user_id.clone(),
                arrival_frame,
                rank,
            };
            match grpc.report_arrival(req).await {
                Ok(resp) => {
                    let _resp = resp.into_inner();
                    tracing::info!(
                        user = %arrived_user_id,
                        rank,
                        frame = arrival_frame,
                        "RoomService: reported arrival"
                    );
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

    let player_id = (*config_username)
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let inner = use_mut_ref(|| RoomServiceInner::new(player_id.clone()));

    // Keep player credentials in sync with config changes
    {
        let inner = inner.clone();
        let pid = player_id.clone();
        use_effect_with(pid, move |pid| {
            let mut inner_mut = inner.borrow_mut();
            inner_mut.player_id = pid.clone();
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

                // --- RegisterPeerId ---
                let peer_registered = inner.borrow().peer_registered;
                if !peer_registered {
                    let my_peer_id = marble_core::bevy::wasm_entry::get_my_peer_id();
                    if !my_peer_id.is_empty() {
                        inner.borrow_mut().peer_registered = true; // optimistic
                        let inner_c = inner.clone();
                        let room_id = room_id.clone();

                        spawn_local(async move {
                            let success = register_peer_id_grpc(
                                &room_id,
                                &my_peer_id,
                            )
                            .await;
                            if !success {
                                inner_c.borrow_mut().peer_registered = false;
                            }
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
) -> bool {
    let Some(mut grpc) = create_grpc_client() else {
        return false;
    };

    for attempt in 0..=3u32 {
        let req = RegisterPeerIdRequest {
            room_id: room_id.to_string(),
            peer_id: peer_id.to_string(),
        };

        match grpc.register_peer_id(req).await {
            Ok(_) => {
                tracing::info!(
                    peer_id = %peer_id,
                    "RoomService: registered peer_id"
                );
                return true;
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
) -> Option<HashMap<String, String>> {
    if peer_ids.is_empty() {
        return Some(HashMap::new());
    }

    let Some(mut grpc) = create_grpc_client() else {
        return None;
    };

    let req = ResolvePeerIdsRequest {
        room_id: room_id.to_string(),
        peer_ids: peer_ids.to_vec(),
    };

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
        Err(e) => {
            tracing::warn!(error = %e, "RoomService: ResolvePeerIds failed");
            None
        }
    }
}
