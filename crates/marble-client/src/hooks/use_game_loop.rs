//! Game loop hook for physics simulation and rendering.

use std::cell::RefCell;
use std::rc::Rc;

use marble_core::{GameState, PlayerId, RouletteConfig, SyncSnapshot};
use marble_proto::room::room_service_client::RoomServiceClient;
use marble_proto::room::{PlayerAuth, ReportArrivalRequest, StartGameRequest};
use tonic_web_wasm_client::Client;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

use crate::camera::{CameraMode, CameraState};
use crate::renderer::WgpuRenderer;
use crate::services::p2p::{should_broadcast_hash, P2pRoomHandle};

/// Fixed timestep for physics simulation (60 FPS)
const PHYSICS_DT_MS: f64 = 1000.0 / 60.0;

/// Game loop state
#[derive(Clone, PartialEq)]
pub enum GameLoopState {
    /// Waiting to start
    Idle,
    /// Game is running
    Running,
    /// Game is paused (e.g., waiting for sync)
    Paused,
}

/// Handle returned by use_game_loop
#[derive(Clone)]
pub struct GameLoopHandle {
    pub game_state: Rc<RefCell<GameState>>,
    pub camera_state: Rc<RefCell<CameraState>>,
    pub loop_state: UseStateHandle<GameLoopState>,
    pub current_frame: UseStateHandle<u64>,
    is_host: bool,
    p2p: P2pRoomHandle,
    my_player_id: Option<String>,
    room_id: String,
    player_secret: Option<String>,
    /// Track which players have been reported as arrived
    reported_arrivals: Rc<RefCell<Vec<PlayerId>>>,
    /// Track if game start has been reported to server (only report once)
    server_game_started: Rc<RefCell<bool>>,
}

impl GameLoopHandle {
    /// Start the game (host only)
    pub fn start_game(&self) {
        self.start_game_with_gamerule(String::new())
    }

    /// Start the game with a specific gamerule (host only)
    pub fn start_game_with_gamerule(&self, gamerule: String) {
        use marble_core::Color;

        if !self.is_host {
            tracing::warn!("Only host can start the game");
            return;
        }

        let mut game = self.game_state.borrow_mut();

        // Load map if not loaded
        if game.map_config.is_none() {
            game.load_map(RouletteConfig::default_classic());
        }

        // Set gamerule
        game.set_gamerule(gamerule.clone());

        // Add players from P2P peers if not already added
        if game.players.is_empty() {
            let peers = self.p2p.peers();
            let my_player_id = self.p2p.my_player_id();

            // Predefined colors for players
            let colors = [
                Color::RED,
                Color::BLUE,
                Color::GREEN,
                Color::ORANGE,
                Color::PURPLE,
                Color::PINK,
                Color::CYAN,
                Color::YELLOW,
            ];

            // Add self first
            game.add_player(my_player_id, colors[0]);

            // Add peers
            for (i, peer) in peers.iter().enumerate() {
                if let Some(player_id) = &peer.player_id {
                    let color = colors.get(i + 1).copied().unwrap_or(Color::RED);
                    game.add_player(player_id.clone(), color);
                }
            }
        }

        // Create and broadcast game start message
        let snapshot = game.create_snapshot();
        if let Ok(state_bytes) = snapshot.to_bytes() {
            drop(game);
            self.p2p.send_game_start(snapshot.rng_seed, state_bytes, gamerule);
            self.loop_state.set(GameLoopState::Running);
            tracing::info!("Game started, broadcasting to peers");
        }
    }

    /// Initialize game from received GameStart message (non-host)
    pub fn init_from_game_start(&self, seed: u64, initial_state: &[u8], gamerule: &str) {
        match SyncSnapshot::from_bytes(initial_state) {
            Ok(snapshot) => {
                let mut game = self.game_state.borrow_mut();

                // Load map first
                if game.map_config.is_none() {
                    game.load_map(RouletteConfig::default_classic());
                }

                // Restore state (includes gamerule from snapshot)
                game.restore_from_snapshot(snapshot);

                // Ensure gamerule is set (in case snapshot doesn't have it)
                if game.gamerule().is_empty() && !gamerule.is_empty() {
                    game.set_gamerule(gamerule.to_string());
                }

                drop(game);

                self.loop_state.set(GameLoopState::Running);
                tracing::info!(seed = seed, gamerule = gamerule, "Game initialized from host state");
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to initialize game from host state");
            }
        }
    }

    /// Spawn marbles for all connected players (host only).
    /// Call this after all peers have joined.
    /// This also calls StartGame RPC to register the spawn with the server.
    pub fn spawn_marbles(&self) {
        use marble_core::Color;

        if !self.is_host {
            tracing::warn!("Only host can spawn marbles");
            return;
        }

        let mut game = self.game_state.borrow_mut();

        // Sync players from current peers
        let peers = self.p2p.peers();
        let my_player_id = self.p2p.my_player_id();

        // Predefined colors for players
        let colors = [
            Color::RED,
            Color::BLUE,
            Color::GREEN,
            Color::ORANGE,
            Color::PURPLE,
            Color::PINK,
            Color::CYAN,
            Color::YELLOW,
        ];

        // Clear existing players and re-add from current peer list
        game.players.clear();

        // Add self first
        game.add_player(my_player_id.clone(), colors[0]);

        // Add peers
        for (i, peer) in peers.iter().enumerate() {
            if let Some(player_id) = &peer.player_id {
                let color = colors.get(i + 1).copied().unwrap_or(Color::RED);
                game.add_player(player_id.clone(), color);
            }
        }

        // Spawn marbles
        if !game.spawn_marbles() {
            tracing::warn!("No spawners available");
            return;
        }

        // Clear reported arrivals for new spawn (in-game respawn)
        self.reported_arrivals.borrow_mut().clear();

        // Check if this is the first spawn (need to report to server)
        let should_report_to_server = !*self.server_game_started.borrow();

        // Get game state info for RPC
        let start_frame = game.current_frame();
        let rng_seed = game.rng_seed;
        let gamerule = game.gamerule().to_string();
        let snapshot = game.create_snapshot();

        // Broadcast updated state to peers (always do this for P2P sync)
        if let Ok(state_bytes) = snapshot.to_bytes() {
            drop(game);
            self.p2p.send_game_start(snapshot.rng_seed, state_bytes, gamerule);
            tracing::info!("Marbles spawned for {} players", peers.len() + 1);

            // Call StartGame RPC to register with server (only first time)
            if should_report_to_server {
                if let Some(ref player_secret) = self.player_secret {
                    // Mark as reported before async call
                    *self.server_game_started.borrow_mut() = true;

                    let room_id = self.room_id.clone();
                    let player_id = my_player_id;
                    let player_secret = player_secret.clone();

                    spawn_local(async move {
                        let Some(window) = web_sys::window() else {
                            tracing::warn!("No window object available for StartGame RPC");
                            return;
                        };
                        let Ok(origin) = window.location().origin() else {
                            tracing::warn!("Failed to get origin for StartGame RPC");
                            return;
                        };
                        let client = Client::new(format!("{}/grpc", origin));
                        let mut grpc = RoomServiceClient::new(client);

                        let req = StartGameRequest {
                            room_id: room_id.clone(),
                            player: Some(PlayerAuth {
                                id: player_id.clone(),
                                secret: player_secret,
                            }),
                            start_frame,
                            rng_seed,
                        };

                        match grpc.start_game(req).await {
                            Ok(resp) => {
                                let resp = resp.into_inner();
                                if resp.already_started {
                                    tracing::info!(room_id = %room_id, "Game was already started on server");
                                } else {
                                    tracing::info!(
                                        room_id = %room_id,
                                        start_frame = start_frame,
                                        rng_seed = rng_seed,
                                        "Game started on server"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    room_id = %room_id,
                                    error = %e,
                                    "Failed to call StartGame RPC"
                                );
                            }
                        }
                    });
                }
            } else {
                tracing::info!("Respawn (server already notified, P2P only)");
            }
        }
    }

    /// Check if game is running
    pub fn is_running(&self) -> bool {
        matches!(*self.loop_state, GameLoopState::Running)
    }

    /// Check if game has been spawned (reported to server)
    /// Once spawned, no more spawns are allowed in this room.
    pub fn is_spawned(&self) -> bool {
        *self.server_game_started.borrow()
    }

    /// Get camera state for external access (e.g., keyboard handlers)
    pub fn camera(&self) -> Rc<RefCell<CameraState>> {
        self.camera_state.clone()
    }

    /// Get local player's numeric ID for camera tracking
    pub fn my_numeric_player_id(&self) -> Option<PlayerId> {
        let game = self.game_state.borrow();
        let my_player_id = self.my_player_id.as_ref()?;

        game.players
            .iter()
            .find(|p| &p.name == my_player_id)
            .map(|p| p.id)
    }

    /// Get available gamerules from the loaded map
    pub fn available_gamerules(&self) -> Vec<String> {
        self.game_state.borrow().available_gamerules()
    }

    /// Get current gamerule
    pub fn current_gamerule(&self) -> String {
        self.game_state.borrow().gamerule().to_string()
    }
}

impl PartialEq for GameLoopHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.game_state, &other.game_state)
    }
}

/// Hook for managing the game loop
#[hook]
pub fn use_game_loop(
    p2p: &P2pRoomHandle,
    canvas_ref: NodeRef,
    is_host: bool,
    seed: u64,
    initial_camera_mode: CameraMode,
) -> GameLoopHandle {
    // Game state - shared with P2P layer
    let game_state = use_memo(seed, |seed| {
        let mut state = GameState::new(*seed);
        // Pre-load the map so it renders in lobby
        state.load_map(RouletteConfig::default_classic());
        Rc::new(RefCell::new(state))
    });

    // Camera state - created once with initial mode
    let camera_state = {
        let initial_mode = initial_camera_mode;
        use_memo((), move |_| {
            // Default map size from classic config
            let mut camera = CameraState::new((800.0, 600.0), (800.0, 600.0));
            camera.set_mode(initial_mode);
            Rc::new(RefCell::new(camera))
        })
    };

    // Loop state
    let loop_state = use_state(|| GameLoopState::Idle);
    let current_frame = use_state(|| 0u64);

    // Renderer version (to track when canvas is ready)
    let renderer_version = use_state(|| 0u32);

    // Accumulated time for fixed timestep
    let accumulated_time = use_mut_ref(|| 0.0f64);
    let last_time = use_mut_ref(|| 0.0f64);

    // Animation frame ID for cleanup
    let animation_frame_id = use_mut_ref(|| None::<i32>);

    // Renderer reference (wgpu)
    let renderer_ref: Rc<RefCell<Option<WgpuRenderer>>> = use_mut_ref(|| None);

    // Store my_player_id for the handle
    let my_player_id = p2p.my_player_id();

    // Store room_id and player_secret for RPC calls
    let room_id = p2p.room_id();
    let player_secret = p2p.player_secret();

    // Track reported arrivals (to avoid duplicate RPC calls)
    let reported_arrivals: Rc<RefCell<Vec<PlayerId>>> = use_mut_ref(Vec::new);

    // Track if game start has been reported to server
    let server_game_started: Rc<RefCell<bool>> = use_mut_ref(|| false);

    // Share game state with P2P layer (run once on mount)
    {
        let game_state = game_state.clone();
        let p2p = p2p.clone();
        use_effect_with(seed, move |_seed| {
            p2p.set_game_state((*game_state).clone());
            p2p.set_host_status(is_host);
        });
    }

    // Initialize wgpu renderer when canvas is ready (run once)
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();
        let renderer_version = renderer_version.clone();
        let camera_state = camera_state.clone();

        use_effect_with((), move |_| {
            // Use timeout to ensure canvas is mounted
            let canvas_ref = canvas_ref.clone();
            let renderer_ref = renderer_ref.clone();
            let renderer_version = renderer_version.clone();
            let camera_state = camera_state.clone();

            gloo::timers::callback::Timeout::new(100, move || {
                if renderer_ref.borrow().is_some() {
                    return; // Already initialized
                }
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    // Update camera viewport
                    {
                        let mut camera = camera_state.borrow_mut();
                        camera.set_viewport(canvas.width() as f32, canvas.height() as f32);
                    }

                    // Initialize wgpu renderer asynchronously
                    let renderer_ref = renderer_ref.clone();
                    let renderer_version = renderer_version.clone();

                    spawn_local(async move {
                        match WgpuRenderer::new(canvas).await {
                            Ok(r) => {
                                *renderer_ref.borrow_mut() = Some(r);
                                renderer_version.set(1);
                                tracing::info!("wgpu renderer initialized");
                            }
                            Err(e) => {
                                tracing::error!("Failed to create wgpu renderer: {}", e);
                            }
                        }
                    });
                }
            })
            .forget();
        });
    }

    // Idle state rendering (single frame, no physics)
    {
        let game_state = game_state.clone();
        let camera_state = camera_state.clone();
        let loop_state = loop_state.clone();
        let renderer_ref = renderer_ref.clone();
        let renderer_version = *renderer_version;

        use_effect_with(
            ((*loop_state).clone(), renderer_version),
            move |(state, _version)| {
                // Render once when not running but renderer is ready
                if !matches!(state, GameLoopState::Running) {
                    if let Some(ref mut renderer) = *renderer_ref.borrow_mut() {
                        let game = game_state.borrow();
                        let camera = camera_state.borrow();
                        renderer.render(&game, &camera);
                        tracing::debug!("Rendered idle state");
                    }
                }
            },
        );
    }

    // Main game loop
    {
        let game_state = game_state.clone();
        let camera_state = camera_state.clone();
        let loop_state = loop_state.clone();
        let current_frame = current_frame.clone();
        let renderer_ref = renderer_ref.clone();
        let p2p = p2p.clone();
        let accumulated_time = accumulated_time.clone();
        let last_time = last_time.clone();
        let animation_frame_id = animation_frame_id.clone();
        let my_player_id_for_camera = my_player_id.clone();
        let reported_arrivals_for_loop = reported_arrivals.clone();
        let room_id_for_loop = room_id.clone();
        let player_secret_for_loop = player_secret.clone();
        let my_player_id_for_rpc = my_player_id.clone();

        use_effect_with(
            ((*loop_state).clone(), *renderer_version),
            move |(state, _version)| {
                let animation_frame_id_cleanup = animation_frame_id.clone();

                // Only run if in Running state and renderer is ready
                if matches!(state, GameLoopState::Running) && renderer_ref.borrow().is_some() {
                    // Create the animation loop closure
                    let closure: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> =
                        Rc::new(RefCell::new(None));
                    let closure_clone = closure.clone();

                    let game_state = game_state.clone();
                    let camera_state = camera_state.clone();
                    let current_frame = current_frame.clone();
                    let loop_state = loop_state.clone();
                    let p2p = p2p.clone();
                    let accumulated_time = accumulated_time.clone();
                    let last_time = last_time.clone();
                    let animation_frame_id = animation_frame_id.clone();
                    let renderer_ref = renderer_ref.clone();
                    let my_player_id_for_camera = my_player_id_for_camera.clone();
                    let reported_arrivals = reported_arrivals_for_loop.clone();
                    let room_id = room_id_for_loop.clone();
                    let player_secret = player_secret_for_loop.clone();
                    let my_player_id_for_rpc = my_player_id_for_rpc.clone();

                    *closure.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
                        // Check if still running
                        if !matches!(*loop_state, GameLoopState::Running) {
                            return;
                        }

                        let last = *last_time.borrow();
                        let delta = if last == 0.0 {
                            PHYSICS_DT_MS
                        } else {
                            (timestamp - last).min(100.0) // Cap delta to prevent spiral of death
                        };
                        *last_time.borrow_mut() = timestamp;

                        // Accumulate time
                        *accumulated_time.borrow_mut() += delta;

                        // Get my numeric player ID for camera tracking
                        let my_numeric_id: Option<PlayerId> = {
                            let game = game_state.borrow();
                            game.players
                                .iter()
                                .find(|p| p.name == my_player_id_for_camera)
                                .map(|p| p.id)
                        };

                        // Fixed timestep physics updates
                        while *accumulated_time.borrow() >= PHYSICS_DT_MS {
                            let mut game = game_state.borrow_mut();

                            // Update physics and get newly arrived players
                            let newly_arrived = game.update();

                            let frame = game.current_frame();

                            // Host: report arrivals to server
                            if is_host && !newly_arrived.is_empty() {
                                let mut reported = reported_arrivals.borrow_mut();
                                for &player_id in &newly_arrived {
                                    // Skip if already reported
                                    if reported.contains(&player_id) {
                                        continue;
                                    }
                                    reported.push(player_id);

                                    // Get player name and rank
                                    let player_name = game
                                        .players
                                        .iter()
                                        .find(|p| p.id == player_id)
                                        .map(|p| p.name.clone())
                                        .unwrap_or_default();
                                    let rank = reported.len() as u32;

                                    // Call ReportArrival RPC
                                    if let Some(ref secret) = player_secret {
                                        let room_id = room_id.clone();
                                        let my_player_id = my_player_id_for_rpc.clone();
                                        let secret = secret.clone();
                                        let arrival_frame = frame;

                                        spawn_local(async move {
                                            let Some(window) = web_sys::window() else {
                                                return;
                                            };
                                            let Ok(origin) = window.location().origin() else {
                                                return;
                                            };
                                            let client = Client::new(format!("{}/grpc", origin));
                                            let mut grpc = RoomServiceClient::new(client);

                                            let req = ReportArrivalRequest {
                                                room_id: room_id.clone(),
                                                player: Some(PlayerAuth {
                                                    id: my_player_id,
                                                    secret,
                                                }),
                                                arrived_player_id: player_name.clone(),
                                                arrival_frame,
                                                rank,
                                            };

                                            match grpc.report_arrival(req).await {
                                                Ok(resp) => {
                                                    let resp = resp.into_inner();
                                                    tracing::info!(
                                                        player = %player_name,
                                                        rank = rank,
                                                        frame = arrival_frame,
                                                        game_ended = resp.game_ended,
                                                        "Reported player arrival"
                                                    );
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        player = %player_name,
                                                        error = %e,
                                                        "Failed to report arrival"
                                                    );
                                                }
                                            }
                                        });
                                    }
                                }
                            }

                            drop(game);

                            current_frame.set(frame);
                            *accumulated_time.borrow_mut() -= PHYSICS_DT_MS;

                            // Host: broadcast hash periodically
                            if is_host && should_broadcast_hash(frame, p2p.last_hash_frame()) {
                                let game = game_state.borrow();
                                let hash = game.compute_hash();
                                drop(game);
                                p2p.send_frame_hash(frame, hash);
                            }
                        }

                        // Update camera
                        {
                            let game = game_state.borrow();
                            let mut camera = camera_state.borrow_mut();
                            camera.update(&game, my_numeric_id);
                        }

                        // Render
                        if let Some(ref mut renderer) = *renderer_ref.borrow_mut() {
                            let game = game_state.borrow();
                            let camera = camera_state.borrow();
                            renderer.render(&game, &camera);
                        }

                        // Request next frame
                        if matches!(*loop_state, GameLoopState::Running) {
                            if let Some(window) = web_sys::window() {
                                if let Some(ref closure) = *closure_clone.borrow() {
                                    let id = window
                                        .request_animation_frame(closure.as_ref().unchecked_ref())
                                        .ok();
                                    *animation_frame_id.borrow_mut() = id;
                                }
                            }
                        }
                    }));

                    // Start the loop
                    if let Some(window) = web_sys::window() {
                        if let Some(ref closure) = *closure.borrow() {
                            let id = window
                                .request_animation_frame(closure.as_ref().unchecked_ref())
                                .ok();
                            *animation_frame_id_cleanup.borrow_mut() = id;
                        }
                    }
                }

                // Cleanup - always return same closure type
                move || {
                    if let Some(id) = *animation_frame_id_cleanup.borrow() {
                        if let Some(window) = web_sys::window() {
                            let _ = window.cancel_animation_frame(id);
                        }
                    }
                }
            },
        );
    }

    GameLoopHandle {
        game_state: (*game_state).clone(),
        camera_state: (*camera_state).clone(),
        loop_state,
        current_frame,
        is_host,
        p2p: p2p.clone(),
        my_player_id: Some(my_player_id),
        room_id,
        player_secret,
        reported_arrivals,
        server_game_started,
    }
}
