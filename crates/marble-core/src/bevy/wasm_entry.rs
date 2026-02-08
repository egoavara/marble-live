//! WASM entry points for the marble game.
//!
//! Provides JavaScript-callable functions to initialize and control the game.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};
use matchbox_socket::WebRtcSocket;
use wasm_bindgen::prelude::*;

use crate::bevy::{CameraMode, CommandQueue, GameCommand, MarbleUnifiedPlugin, StateStores};
use crate::map::RouletteConfig;
use crate::marble::Color;

// ============================================================================
// Global State
// ============================================================================

/// Atomic flag for signaling app shutdown (checked every frame by Bevy system).
/// Using AtomicBool for lock-free access from Bevy systems.
static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

/// Atomic flag indicating whether the Bevy App has been started.
/// In WASM, the EventLoop can only be created once, so we track this to prevent
/// RecreationAttempt errors on room transitions.
static BEVY_APP_STARTED: AtomicBool = AtomicBool::new(false);

/// Global state that can be reset on page reload.
struct GlobalState {
    command_queue: CommandQueue,
    state_stores: StateStores,
}

impl GlobalState {
    fn new() -> Self {
        Self {
            command_queue: CommandQueue::new(),
            state_stores: StateStores::new(),
        }
    }
}

/// Global state protected by Mutex for thread-safe access.
/// Using Option to allow resetting on page reload.
static GLOBAL_STATE: Mutex<Option<GlobalState>> = Mutex::new(None);

// ============================================================================
// P2P Socket Pending Init
// ============================================================================

/// Pending P2P socket initialization data.
/// Stored in a global slot and picked up by Bevy's `pickup_pending_p2p` system.
pub struct PendingP2pInit {
    pub socket: WebRtcSocket,
    pub mesh_group: u32,
    pub is_bridge: bool,
    pub player_id: String,
    pub is_host: bool,
}

// SAFETY: WASM is single-threaded; no data races possible.
unsafe impl Send for PendingP2pInit {}
unsafe impl Sync for PendingP2pInit {}

/// Global pending P2P init slot.
static PENDING_P2P: parking_lot::Mutex<Option<PendingP2pInit>> = parking_lot::Mutex::new(None);

/// Atomic flag for P2P disconnect request.
static P2P_DISCONNECT: AtomicBool = AtomicBool::new(false);

/// Pending peer_id → player_id updates (from Yew gRPC calls).
static PENDING_PEER_UPDATES: parking_lot::Mutex<Vec<(String, String)>> =
    parking_lot::Mutex::new(Vec::new());

fn ensure_global_state() {
    let mut guard = GLOBAL_STATE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(GlobalState::new());
    }
}

fn get_command_queue() -> CommandQueue {
    ensure_global_state();
    let guard = GLOBAL_STATE.lock().unwrap();
    guard.as_ref().unwrap().command_queue.clone()
}

fn get_state_stores() -> StateStores {
    ensure_global_state();
    let guard = GLOBAL_STATE.lock().unwrap();
    guard.as_ref().unwrap().state_stores.clone()
}

fn is_shutdown_requested() -> bool {
    SHOULD_EXIT.load(Ordering::SeqCst)
}

/// Request Bevy app to exit. Called before page unload.
/// The app will exit on the next frame when the exit_system runs.
#[wasm_bindgen]
pub fn request_bevy_exit() {
    tracing::info!("[marble] request_bevy_exit called - signaling app to exit");
    SHOULD_EXIT.store(true, Ordering::SeqCst);
}

/// Reset global state. Called after app exits or before page unload.
#[wasm_bindgen]
pub fn reset_bevy_state() {
    tracing::info!("[marble] reset_bevy_state called - clearing global state");

    // Signal exit (in case not already done)
    SHOULD_EXIT.store(true, Ordering::SeqCst);

    // Clear global state
    let mut guard = GLOBAL_STATE.lock().unwrap();
    if let Some(ref state) = *guard {
        state.command_queue.clear();
    }
    *guard = None;

    // Reset exit flag for next app instance
    SHOULD_EXIT.store(false, Ordering::SeqCst);
}

/// Bevy system that checks if exit was requested and sends AppExit event.
/// Add this system to your Bevy app to enable clean shutdown.
pub fn check_exit_system(mut exit: MessageWriter<bevy::app::AppExit>) {
    if SHOULD_EXIT.load(Ordering::SeqCst) {
        tracing::info!("[marble] check_exit_system: sending AppExit");
        exit.write(bevy::app::AppExit::Success);
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Starts the unified Bevy app in Idle mode.
///
/// The app starts without any game or editor mode active.
/// Use `init_game_mode()` or `init_editor_mode()` to switch modes via commands.
#[wasm_bindgen]
pub fn start_bevy_app(canvas_id: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    tracing::info!("[marble] start_bevy_app called");

    if BEVY_APP_STARTED.load(Ordering::SeqCst) {
        tracing::info!("[marble] App already running, skipping creation");
        return Ok(());
    }

    let command_queue = get_command_queue().clone();
    let state_stores = get_state_stores().clone();

    tracing::info!(
        "[marble] creating unified Bevy app for canvas: #{}",
        canvas_id
    );

    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    canvas: Some(format!("#{}", canvas_id)),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    ..default()
                }),
                ..default()
            })
            .disable::<bevy::log::LogPlugin>(),
    );

    app.insert_resource(WinitSettings {
        focused_mode: UpdateMode::Continuous,
        unfocused_mode: UpdateMode::Continuous,
    });

    tracing::info!("[marble] adding MarbleUnifiedPlugin");
    app.add_plugins(MarbleUnifiedPlugin::new(command_queue, state_stores));

    BEVY_APP_STARTED.store(true, Ordering::SeqCst);

    tracing::info!("[marble] calling app.run()");
    app.run();
    tracing::info!("[marble] app.run() returned");

    Ok(())
}

/// Initialize game mode with the given map configuration.
///
/// Command sequence: ClearMode → Yield → InitGame → Yield → ClearMarbles → ClearPlayers → LoadMap
#[wasm_bindgen]
pub fn init_game_mode(config_json: &str) -> Result<(), JsValue> {
    if is_shutdown_requested() {
        return Err(JsValue::from_str("Bevy app is shutting down"));
    }

    let config: RouletteConfig = serde_json::from_str(config_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse config: {}", e)))?;

    let queue = get_command_queue();
    let stores = get_state_stores();

    tracing::info!("[marble] init_game_mode: switching to Game mode");

    stores.reset_for_new_room();

    queue.push(GameCommand::ClearMode);
    queue.push(GameCommand::Yield);
    queue.push(GameCommand::InitGame);
    queue.push(GameCommand::Yield);
    queue.push(GameCommand::ClearMarbles);
    queue.push(GameCommand::ClearPlayers);
    queue.push(GameCommand::LoadMap { config });

    Ok(())
}

/// Initialize editor mode with the given map configuration.
///
/// Command sequence: ClearMode → Yield → InitEditor → Yield → ClearMarbles → ClearPlayers → LoadMap
#[wasm_bindgen]
pub fn init_editor_mode(config_json: &str) -> Result<(), JsValue> {
    if is_shutdown_requested() {
        return Err(JsValue::from_str("Bevy app is shutting down"));
    }

    let config: RouletteConfig = serde_json::from_str(config_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse config: {}", e)))?;

    let queue = get_command_queue();
    let stores = get_state_stores();

    tracing::info!("[marble] init_editor_mode: switching to Editor mode");

    stores.reset_for_new_room();

    queue.push(GameCommand::ClearMode);
    queue.push(GameCommand::Yield);
    queue.push(GameCommand::InitEditor);
    queue.push(GameCommand::Yield);
    queue.push(GameCommand::ClearMarbles);
    queue.push(GameCommand::ClearPlayers);
    queue.push(GameCommand::LoadMap { config });

    Ok(())
}

/// Legacy wrapper: starts the marble game.
///
/// If the Bevy App is already running, delegates to `init_game_mode`.
/// Otherwise, starts the unified app and then initializes game mode.
#[wasm_bindgen]
pub fn start_marble_game(canvas_id: &str, config_json: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    tracing::info!("[marble] start_marble_game called (legacy wrapper)");

    if !BEVY_APP_STARTED.load(Ordering::SeqCst) {
        start_bevy_app(canvas_id)?;
    }

    init_game_mode(config_json)
}

/// Legacy wrapper: starts the marble editor.
///
/// If the Bevy App is already running, delegates to `init_editor_mode`.
/// Otherwise, starts the unified app and then initializes editor mode.
#[wasm_bindgen]
pub fn start_marble_editor(canvas_id: &str, config_json: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    tracing::info!("[marble] start_marble_editor called (legacy wrapper)");

    if !BEVY_APP_STARTED.load(Ordering::SeqCst) {
        start_bevy_app(canvas_id)?;
    }

    init_editor_mode(config_json)
}

// ============================================================================
// Commands
// ============================================================================

/// Check if Bevy app is initialized and ready.
#[wasm_bindgen]
pub fn is_bevy_ready() -> bool {
    let guard = GLOBAL_STATE.lock().unwrap();
    guard.is_some() && !SHOULD_EXIT.load(Ordering::SeqCst)
}

/// Check if Bevy app is currently running.
///
/// Returns true if the app has been started and is not shutting down.
/// Used to determine whether to reuse the existing app or create a new one.
#[wasm_bindgen]
pub fn is_bevy_app_running() -> bool {
    BEVY_APP_STARTED.load(Ordering::SeqCst) && !SHOULD_EXIT.load(Ordering::SeqCst)
}

/// Prepare for a new room by resetting state and loading a new map.
///
/// Delegates to `init_game_mode` for proper mode transition.
#[wasm_bindgen]
pub fn prepare_new_room(config_json: &str) -> Result<(), JsValue> {
    tracing::info!("[marble] prepare_new_room: delegating to init_game_mode");
    init_game_mode(config_json)
}

/// Sends a command to the running game/editor.
#[wasm_bindgen]
pub fn send_command(command_json: &str) -> Result<(), JsValue> {
    // Check if shutdown was requested
    if is_shutdown_requested() {
        return Err(JsValue::from_str("Bevy app is shutting down"));
    }

    let value: serde_json::Value = serde_json::from_str(command_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid command JSON: {}", e)))?;

    let command_type = value["type"]
        .as_str()
        .ok_or_else(|| JsValue::from_str("Missing 'type' field"))?;

    let command = match command_type {
        "spawn_marbles" => GameCommand::SpawnMarbles,
        "spawn_marbles_at" => {
            let positions = value["positions"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            let x = v[0].as_f64()? as f32;
                            let y = v[1].as_f64()? as f32;
                            Some([x, y])
                        })
                        .collect()
                })
                .unwrap_or_default();
            GameCommand::SpawnMarblesAt { positions }
        }
        "clear_marbles" => GameCommand::ClearMarbles,
        "clear_players" => GameCommand::ClearPlayers,
        "yield" => GameCommand::Yield,
        "add_player" => {
            let name = value["name"]
                .as_str()
                .ok_or_else(|| JsValue::from_str("Missing 'name' field"))?
                .to_string();

            let color_arr = value["color"]
                .as_array()
                .ok_or_else(|| JsValue::from_str("Missing 'color' field"))?;

            let color = Color::new(
                color_arr.first().and_then(|v| v.as_u64()).unwrap_or(255) as u8,
                color_arr.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                color_arr.get(2).and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                color_arr.get(3).and_then(|v| v.as_u64()).unwrap_or(255) as u8,
            );

            GameCommand::AddPlayer { name, color }
        }
        "remove_player" => {
            let player_id = value["player_id"]
                .as_u64()
                .ok_or_else(|| JsValue::from_str("Missing 'player_id' field"))?
                as u32;

            GameCommand::RemovePlayer { player_id }
        }
        "load_map" => {
            let config: RouletteConfig = serde_json::from_value(value["config"].clone())
                .map_err(|e| JsValue::from_str(&format!("Invalid map config: {}", e)))?;

            GameCommand::LoadMap { config }
        }

        // Editor commands
        "select_object" => {
            let index = value["index"].as_u64().map(|i| i as usize);
            GameCommand::SelectObject { index }
        }
        "select_sequence" => {
            let index = value["index"].as_u64().map(|i| i as usize);
            GameCommand::SelectSequence { index }
        }
        "select_keyframe" => {
            let index = value["index"].as_u64().map(|i| i as usize);
            GameCommand::SelectKeyframe { index }
        }
        "update_object" => {
            let index = value["index"]
                .as_u64()
                .ok_or_else(|| JsValue::from_str("Missing 'index' field"))?
                as usize;
            let object: crate::map::MapObject = serde_json::from_value(value["object"].clone())
                .map_err(|e| JsValue::from_str(&format!("Invalid object: {}", e)))?;

            GameCommand::UpdateObject { index, object }
        }
        "add_object" => {
            let object: crate::map::MapObject = serde_json::from_value(value["object"].clone())
                .map_err(|e| JsValue::from_str(&format!("Invalid object: {}", e)))?;

            GameCommand::AddObject { object }
        }
        "delete_object" => {
            let index = value["index"]
                .as_u64()
                .ok_or_else(|| JsValue::from_str("Missing 'index' field"))?
                as usize;

            GameCommand::DeleteObject { index }
        }
        "update_keyframe" => {
            let sequence_index = value["sequence_index"]
                .as_u64()
                .ok_or_else(|| JsValue::from_str("Missing 'sequence_index' field"))?
                as usize;
            let keyframe_index = value["keyframe_index"]
                .as_u64()
                .ok_or_else(|| JsValue::from_str("Missing 'keyframe_index' field"))?
                as usize;
            let keyframe: crate::map::Keyframe = serde_json::from_value(value["keyframe"].clone())
                .map_err(|e| JsValue::from_str(&format!("Invalid keyframe: {}", e)))?;

            GameCommand::UpdateKeyframe {
                sequence_index,
                keyframe_index,
                keyframe,
            }
        }
        "start_simulation" => GameCommand::StartSimulation,
        "stop_simulation" => GameCommand::StopSimulation,
        "reset_simulation" => GameCommand::ResetSimulation,
        "preview_sequence" => {
            let start = value["start"].as_bool().unwrap_or(true);
            GameCommand::PreviewSequence { start }
        }
        "update_snap_config" => {
            let grid_snap_enabled = value["grid_snap_enabled"].as_bool();
            let grid_snap_interval = value["grid_snap_interval"].as_f64().map(|v| v as f32);
            let angle_snap_enabled = value["angle_snap_enabled"].as_bool();
            let angle_snap_interval = value["angle_snap_interval"].as_f64().map(|v| v as f32);
            GameCommand::UpdateSnapConfig {
                grid_snap_enabled,
                grid_snap_interval,
                angle_snap_enabled,
                angle_snap_interval,
            }
        }

        // Camera commands
        "set_camera_mode" => {
            let mode_str = value["mode"]
                .as_str()
                .ok_or_else(|| JsValue::from_str("Missing 'mode' field"))?;

            let mode = match mode_str {
                "overview" => CameraMode::Overview,
                "follow_leader" => CameraMode::FollowLeader,
                "editor" => CameraMode::Editor,
                "follow_target" => {
                    let player_id = value["player_id"].as_u64().ok_or_else(|| {
                        JsValue::from_str("Missing 'player_id' for follow_target mode")
                    })? as u32;
                    CameraMode::FollowTarget(player_id)
                }
                _ => {
                    return Err(JsValue::from_str(&format!(
                        "Unknown camera mode: {}",
                        mode_str
                    )));
                }
            };

            GameCommand::SetCameraMode { mode }
        }
        "set_local_player_id" => {
            let player_id = value["player_id"].as_u64().map(|id| id as u32);
            GameCommand::SetLocalPlayerId { player_id }
        }

        // Mode commands
        "init_game" => GameCommand::InitGame,
        "init_editor" => GameCommand::InitEditor,
        "clear_mode" => GameCommand::ClearMode,

        // P2P sync commands
        "set_seed" => {
            let seed = value["seed"]
                .as_u64()
                .ok_or_else(|| JsValue::from_str("Missing 'seed' field"))?;
            GameCommand::SetSeed { seed }
        }
        "set_sync_host" => {
            let is_host = value["is_host"]
                .as_bool()
                .ok_or_else(|| JsValue::from_str("Missing 'is_host' field"))?;
            GameCommand::SetSyncHost { is_host }
        }
        "set_gamerule" => {
            let gamerule = value["gamerule"]
                .as_str()
                .ok_or_else(|| JsValue::from_str("Missing 'gamerule' field"))?
                .to_string();
            GameCommand::SetGamerule { gamerule }
        }
        "broadcast_game_start" => GameCommand::BroadcastGameStart,

        // P2P chat/reaction commands
        "send_chat" => {
            let content = value["content"]
                .as_str()
                .ok_or_else(|| JsValue::from_str("Missing 'content' field"))?
                .to_string();
            GameCommand::SendChat { content }
        }
        "send_reaction" => {
            let emoji = value["emoji"]
                .as_str()
                .ok_or_else(|| JsValue::from_str("Missing 'emoji' field"))?
                .to_string();
            GameCommand::SendReaction { emoji }
        }
        "send_ping" => GameCommand::SendPing,
        "send_ping_to" => {
            let peer_id = value["peer_id"]
                .as_str()
                .ok_or_else(|| JsValue::from_str("Missing 'peer_id' field"))?
                .to_string();
            GameCommand::SendPingTo { peer_id }
        }

        _ => {
            return Err(JsValue::from_str(&format!(
                "Unknown command type: {}",
                command_type
            )));
        }
    };

    get_command_queue().push(command);
    Ok(())
}

// ============================================================================
// State Getters (for Yew hooks)
// ============================================================================

/// Get connection state.
#[wasm_bindgen]
pub fn get_connection_state() -> JsValue {
    if is_shutdown_requested() {
        return JsValue::NULL;
    }
    let stores = get_state_stores();
    let state = serde_json::json!({
        "state": format!("{:?}", stores.connection.get_state()),
        "my_player_id": stores.connection.get_my_player_id(),
        "room_id": stores.connection.get_room_id(),
    });
    serde_wasm_bindgen::to_value(&state).unwrap_or(JsValue::NULL)
}

/// Get connection state version (for change detection).
#[wasm_bindgen]
pub fn get_connection_version() -> u64 {
    // Connection store doesn't have version, always return 0
    0
}

/// Get this socket's own peer ID (assigned by signaling server).
/// Returns empty string if not yet assigned.
#[wasm_bindgen]
pub fn get_my_peer_id() -> String {
    get_state_stores()
        .peers
        .get_my_peer_id()
        .unwrap_or_default()
}

/// Get peer list.
#[wasm_bindgen]
pub fn get_peers() -> JsValue {
    let stores = get_state_stores();
    let peers = stores.peers.get_peers();
    serde_wasm_bindgen::to_value(&peers).unwrap_or(JsValue::NULL)
}

/// Get peer list version.
#[wasm_bindgen]
pub fn get_peers_version() -> u64 {
    get_state_stores().peers.get_version()
}

/// Get player list.
#[wasm_bindgen]
pub fn get_players() -> JsValue {
    let stores = get_state_stores();
    let players = stores.players.get_players();
    serde_wasm_bindgen::to_value(&players).unwrap_or(JsValue::NULL)
}

/// Get arrival order.
#[wasm_bindgen]
pub fn get_arrival_order() -> JsValue {
    let stores = get_state_stores();
    let order = stores.players.get_arrival_order();
    serde_wasm_bindgen::to_value(&order).unwrap_or(JsValue::NULL)
}

/// Get player list version.
#[wasm_bindgen]
pub fn get_players_version() -> u64 {
    get_state_stores().players.get_version()
}

/// Get chat messages.
#[wasm_bindgen]
pub fn get_chat_messages() -> JsValue {
    let stores = get_state_stores();
    let messages = stores.chat.get_messages();
    serde_wasm_bindgen::to_value(&messages).unwrap_or(JsValue::NULL)
}

/// Get chat version.
#[wasm_bindgen]
pub fn get_chat_version() -> u64 {
    get_state_stores().chat.get_version()
}

/// Get reactions.
#[wasm_bindgen]
pub fn get_reactions() -> JsValue {
    let stores = get_state_stores();
    let reactions = stores.reactions.get_reactions();
    serde_wasm_bindgen::to_value(&reactions).unwrap_or(JsValue::NULL)
}

/// Get recent reactions (since timestamp).
#[wasm_bindgen]
pub fn get_recent_reactions(since_timestamp: f64) -> JsValue {
    let stores = get_state_stores();
    let reactions = stores.reactions.get_recent_reactions(since_timestamp);
    serde_wasm_bindgen::to_value(&reactions).unwrap_or(JsValue::NULL)
}

/// Get reactions version.
#[wasm_bindgen]
pub fn get_reactions_version() -> u64 {
    get_state_stores().reactions.get_version()
}

/// Get game state summary.
#[wasm_bindgen]
pub fn get_game_state() -> JsValue {
    let stores = get_state_stores();
    let summary = stores.game.get_summary();
    serde_wasm_bindgen::to_value(&summary).unwrap_or(JsValue::NULL)
}

/// Get game state version.
#[wasm_bindgen]
pub fn get_game_version() -> u64 {
    get_state_stores().game.get_version()
}

// ============================================================================
// Pong Store Getters (for PeerManager liveness checks)
// ============================================================================

/// Get and consume all recorded pongs (peer_id → timestamp).
#[wasm_bindgen]
pub fn get_pongs() -> JsValue {
    let stores = get_state_stores();
    let pongs = stores.pongs.take_pongs();
    serde_wasm_bindgen::to_value(&pongs).unwrap_or(JsValue::NULL)
}

/// Get pong store version (for change detection).
#[wasm_bindgen]
pub fn get_pongs_version() -> u64 {
    get_state_stores().pongs.get_version()
}

// ============================================================================
// Editor State Getters (for Yew hooks)
// ============================================================================

/// Get editor state summary.
#[wasm_bindgen]
pub fn get_editor_state() -> JsValue {
    let stores = get_state_stores();
    let summary = stores.editor.get_summary();
    serde_wasm_bindgen::to_value(&summary).unwrap_or(JsValue::NULL)
}

/// Get editor state version.
#[wasm_bindgen]
pub fn get_editor_state_version() -> u64 {
    get_state_stores().editor.get_version()
}

/// Get editor objects (map objects).
#[wasm_bindgen]
pub fn get_editor_objects() -> JsValue {
    let stores = get_state_stores();
    let objects = stores.editor.get_objects();
    serde_wasm_bindgen::to_value(&objects).unwrap_or(JsValue::NULL)
}

/// Get editor keyframes (keyframe sequences).
#[wasm_bindgen]
pub fn get_editor_keyframes() -> JsValue {
    let stores = get_state_stores();
    let keyframes = stores.editor.get_keyframes();
    serde_wasm_bindgen::to_value(&keyframes).unwrap_or(JsValue::NULL)
}

/// Get editor keyframes version (for change detection).
#[wasm_bindgen]
pub fn get_editor_keyframes_version() -> u64 {
    get_state_stores().editor.get_keyframes_version()
}

// ============================================================================
// Snap Config State Getters
// ============================================================================

/// Get snap configuration summary.
#[wasm_bindgen]
pub fn get_snap_config() -> JsValue {
    let stores = get_state_stores();
    let summary = stores.snap_config.get_summary();
    serde_wasm_bindgen::to_value(&summary).unwrap_or(JsValue::NULL)
}

/// Get snap configuration version (for change detection).
#[wasm_bindgen]
pub fn get_snap_config_version() -> u64 {
    get_state_stores().snap_config.get_version()
}

/// Check if the editor map has been loaded.
///
/// Used to prevent the Yew-Bevy race condition where Yew starts polling
/// before Bevy has finished loading the map.
#[wasm_bindgen]
pub fn get_editor_map_loaded() -> bool {
    if is_shutdown_requested() {
        return false;
    }
    get_state_stores().editor.is_map_loaded()
}

// ============================================================================
// P2P Socket Management
// ============================================================================

/// Initialize a P2P socket connection.
///
/// Creates a `WebRtcSocket` and stores it in a pending slot.
/// The Bevy `pickup_pending_p2p` system will insert it as a Resource next frame.
#[wasm_bindgen]
pub fn init_p2p_socket(
    signaling_url: &str,
    mesh_group: u32,
    is_bridge: bool,
    player_id: &str,
    is_host: bool,
) {
    tracing::info!(
        "[marble] init_p2p_socket: url={}, group={}, bridge={}, player={}, host={}",
        signaling_url,
        mesh_group,
        is_bridge,
        player_id,
        is_host
    );

    let (socket, loop_fut) = WebRtcSocket::new_reliable(signaling_url);
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = loop_fut.await {
            tracing::error!("[marble] P2P signaling error: {:?}", e);
        }
    });

    PENDING_P2P.lock().replace(PendingP2pInit {
        socket,
        mesh_group,
        is_bridge,
        player_id: player_id.to_string(),
        is_host,
    });
}

/// Request P2P socket disconnection.
///
/// The Bevy `handle_p2p_disconnect` system will remove the Resource next frame.
#[wasm_bindgen]
pub fn disconnect_p2p() {
    tracing::info!("[marble] disconnect_p2p called");
    P2P_DISCONNECT.store(true, Ordering::SeqCst);
}

/// Update peer_id → player_id mapping from Yew (resolved via gRPC).
///
/// The Bevy `poll_p2p_socket` system will apply these updates.
#[wasm_bindgen]
pub fn update_peer_player_id(peer_id_str: &str, player_id: &str) {
    PENDING_PEER_UPDATES
        .lock()
        .push((peer_id_str.to_string(), player_id.to_string()));
}

// --- Internal accessors for Bevy systems ---

/// Take the pending P2P init data (called by `pickup_pending_p2p` system).
pub fn take_pending_p2p() -> Option<PendingP2pInit> {
    PENDING_P2P.lock().take()
}

/// Check and reset the P2P disconnect flag.
pub fn take_p2p_disconnect() -> bool {
    P2P_DISCONNECT.swap(false, Ordering::SeqCst)
}

/// Take pending peer_id → player_id updates.
pub fn take_pending_peer_updates() -> Vec<(String, String)> {
    std::mem::take(&mut *PENDING_PEER_UPDATES.lock())
}
