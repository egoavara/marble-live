//! Game loop hook for physics simulation and rendering.

use std::cell::RefCell;
use std::rc::Rc;

use marble_core::{GamePhase, GameState, RouletteConfig, SyncSnapshot};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

use crate::renderer::CanvasRenderer;
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
    /// Game has finished
    Finished,
}

/// Handle returned by use_game_loop
#[derive(Clone)]
pub struct GameLoopHandle {
    pub game_state: Rc<RefCell<GameState>>,
    pub loop_state: UseStateHandle<GameLoopState>,
    pub current_frame: UseStateHandle<u64>,
    is_host: bool,
    p2p: P2pRoomHandle,
}

impl GameLoopHandle {
    /// Start the game (host only)
    pub fn start_game(&self) {
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

        // Mark all players as ready
        for i in 0..game.players.len() {
            game.set_player_ready(i as u32, true);
        }

        // Start countdown
        if game.start_countdown() {
            // Create and broadcast game start message
            let snapshot = game.create_snapshot();
            if let Ok(state_bytes) = snapshot.to_bytes() {
                drop(game);
                self.p2p.send_game_start(snapshot.rng_seed, state_bytes);
                self.loop_state.set(GameLoopState::Running);
                tracing::info!("Game started, broadcasting to peers");
            }
        } else {
            tracing::warn!("Failed to start countdown - players: {}", game.players.len());
        }
    }

    /// Initialize game from received GameStart message (non-host)
    pub fn init_from_game_start(&self, seed: u64, initial_state: &[u8]) {
        match SyncSnapshot::from_bytes(initial_state) {
            Ok(snapshot) => {
                let mut game = self.game_state.borrow_mut();

                // Load map first
                if game.map_config.is_none() {
                    game.load_map(RouletteConfig::default_classic());
                }

                // Restore state
                game.restore_from_snapshot(snapshot);
                drop(game);

                self.loop_state.set(GameLoopState::Running);
                tracing::info!(seed = seed, "Game initialized from host state");
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to initialize game from host state");
            }
        }
    }

    /// Reset game to lobby
    pub fn reset_to_lobby(&self) {
        let mut game = self.game_state.borrow_mut();
        game.reset_to_lobby();
        drop(game);
        self.loop_state.set(GameLoopState::Idle);
        self.current_frame.set(0);
    }

    /// Get current game phase
    pub fn game_phase(&self) -> GamePhase {
        self.game_state.borrow().phase.clone()
    }

    /// Check if game is running
    pub fn is_running(&self) -> bool {
        matches!(*self.loop_state, GameLoopState::Running)
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
) -> GameLoopHandle {
    // Game state - shared with P2P layer
    let game_state = use_memo(seed, |seed| {
        let mut state = GameState::new(*seed);
        // Pre-load the map so it renders in lobby
        state.load_map(RouletteConfig::default_classic());
        Rc::new(RefCell::new(state))
    });

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

    // Renderer reference
    let renderer_ref: Rc<RefCell<Option<CanvasRenderer>>> = use_mut_ref(|| None);

    // Share game state with P2P layer (run once on mount)
    {
        let game_state = game_state.clone();
        let p2p = p2p.clone();
        use_effect_with(seed, move |_seed| {
            p2p.set_game_state((*game_state).clone());
            p2p.set_host_status(is_host);
        });
    }

    // Initialize renderer when canvas is ready (run once)
    {
        let canvas_ref = canvas_ref.clone();
        let renderer_ref = renderer_ref.clone();
        let renderer_version = renderer_version.clone();
        use_effect_with((), move |_| {
            // Use timeout to ensure canvas is mounted
            let canvas_ref = canvas_ref.clone();
            let renderer_ref = renderer_ref.clone();
            let renderer_version = renderer_version.clone();

            gloo::timers::callback::Timeout::new(100, move || {
                if renderer_ref.borrow().is_some() {
                    return; // Already initialized
                }
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    match CanvasRenderer::new(&canvas) {
                        Ok(r) => {
                            *renderer_ref.borrow_mut() = Some(r);
                            renderer_version.set(1);
                            tracing::debug!("Canvas renderer initialized");
                        }
                        Err(e) => {
                            tracing::error!("Failed to create renderer: {:?}", e);
                        }
                    }
                }
            }).forget();
        });
    }

    // Idle/Finished state rendering (single frame, no physics)
    {
        let game_state = game_state.clone();
        let loop_state = loop_state.clone();
        let renderer_ref = renderer_ref.clone();
        let renderer_version = *renderer_version;

        use_effect_with(
            ((*loop_state).clone(), renderer_version),
            move |(state, _version)| {
                // Render once when not running but renderer is ready
                if !matches!(state, GameLoopState::Running) {
                    if let Some(ref renderer) = *renderer_ref.borrow() {
                        let game = game_state.borrow();
                        renderer.render(&game);
                        tracing::debug!("Rendered idle/finished state");
                    }
                }
            },
        );
    }

    // Main game loop
    {
        let game_state = game_state.clone();
        let loop_state = loop_state.clone();
        let current_frame = current_frame.clone();
        let renderer_ref = renderer_ref.clone();
        let p2p = p2p.clone();
        let accumulated_time = accumulated_time.clone();
        let last_time = last_time.clone();
        let animation_frame_id = animation_frame_id.clone();

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
                    let current_frame = current_frame.clone();
                    let loop_state = loop_state.clone();
                    let p2p = p2p.clone();
                    let accumulated_time = accumulated_time.clone();
                    let last_time = last_time.clone();
                    let animation_frame_id = animation_frame_id.clone();
                    let renderer_ref = renderer_ref.clone();

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

                        // Fixed timestep physics updates
                        while *accumulated_time.borrow() >= PHYSICS_DT_MS {
                            let mut game = game_state.borrow_mut();

                            // Update physics
                            game.update();

                            let frame = game.current_frame();
                            let phase = game.phase.clone();
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

                            // Check for game finish
                            if matches!(phase, GamePhase::Finished { .. }) {
                                loop_state.set(GameLoopState::Finished);
                                break;
                            }
                        }

                        // Render
                        if let Some(ref renderer) = *renderer_ref.borrow() {
                            let game = game_state.borrow();
                            renderer.render(&game);
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
        loop_state,
        current_frame,
        is_host,
        p2p: p2p.clone(),
    }
}
