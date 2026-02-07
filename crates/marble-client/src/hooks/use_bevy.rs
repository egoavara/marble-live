//! Bevy integration hooks for Yew.
//!
//! Provides hooks to access Bevy game state from Yew components.
//! Each hook polls its specific state store and triggers re-renders
//! only when that slice of state changes.

use gloo::timers::callback::Interval;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use yew::prelude::*;

// ============================================================================
// WASM Bindings
// ============================================================================

// ============================================================================
// Direct calls to marble-core WASM functions
// These are re-exported from marble_core::bevy::wasm_entry
// ============================================================================

pub use marble_core::bevy::{
    get_arrival_order, get_chat_messages, get_chat_version, get_connection_state,
    get_editor_keyframes, get_editor_keyframes_version, get_editor_objects, get_editor_state,
    get_editor_state_version, get_game_state, get_game_version, get_peers, get_peers_version,
    get_players, get_players_version, get_reactions, get_reactions_version, get_recent_reactions,
    get_snap_config, get_snap_config_version, init_editor_mode, init_game_mode,
    is_bevy_app_running, is_bevy_ready, prepare_new_room, request_bevy_exit, reset_bevy_state,
    send_command, start_bevy_app, start_marble_editor, start_marble_game,
};

// ============================================================================
// Types (mirroring marble-core state_store types)
// ============================================================================

/// P2P connection state.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// Connection info.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
pub struct ConnectionInfo {
    pub state: String,
    pub my_player_id: String,
    pub room_id: String,
}

/// Peer information.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub player_id: Option<String>,
    pub is_host: bool,
}

/// Player information.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct PlayerInfo {
    pub id: u32,
    pub name: String,
    pub color: [u8; 4],
    pub arrived: bool,
    pub rank: Option<u32>,
    pub live_rank: Option<u32>,
}

/// Chat message.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ChatMessage {
    pub id: u64,
    pub sender_id: String,
    pub content: String,
    pub timestamp: f64,
}

/// Reaction.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Reaction {
    pub id: u64,
    pub sender_id: String,
    pub emoji: String,
    pub timestamp: f64,
}

/// Game state summary.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
pub struct GameStateSummary {
    pub is_running: bool,
    pub is_host: bool,
    pub frame: u64,
    pub gamerule: String,
    pub map_name: String,
}

/// Editor state summary.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
pub struct EditorStateSummary {
    pub selected_object: Option<usize>,
    pub selected_sequence: Option<usize>,
    pub selected_keyframe: Option<usize>,
    pub is_simulating: bool,
    pub is_previewing: bool,
    /// 모든 실행 중인 키프레임 시퀀스의 현재 인덱스
    /// key: 시퀀스 이름, value: current_index (다음 처리할 인덱스)
    #[serde(default)]
    pub executing_keyframes: std::collections::HashMap<String, usize>,
}

/// Snap configuration summary.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
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

// ============================================================================
// Context
// ============================================================================

/// Bevy context for sharing command sender across components.
#[derive(Clone, PartialEq)]
pub struct BevyContext {
    /// Whether Bevy has been initialized.
    pub initialized: bool,
}

impl BevyContext {
    /// Send a command to Bevy.
    pub fn send_command(&self, command: &str) -> Result<(), String> {
        if !self.initialized {
            return Err("Bevy not initialized".to_string());
        }
        send_command(command).map_err(|e| format!("{:?}", e))
    }

    /// Spawn marbles.
    pub fn spawn_marbles(&self) -> Result<(), String> {
        self.send_command(r#"{"type":"spawn_marbles"}"#)
    }

    /// Clear marbles.
    pub fn clear_marbles(&self) -> Result<(), String> {
        self.send_command(r#"{"type":"clear_marbles"}"#)
    }

    /// Add a player.
    pub fn add_player(&self, name: &str, color: [u8; 4]) -> Result<(), String> {
        let cmd = serde_json::json!({
            "type": "add_player",
            "name": name,
            "color": color
        });
        self.send_command(&cmd.to_string())
    }

    /// Remove a player.
    pub fn remove_player(&self, player_id: u32) -> Result<(), String> {
        let cmd = serde_json::json!({
            "type": "remove_player",
            "player_id": player_id
        });
        self.send_command(&cmd.to_string())
    }
}

/// Props for BevyProvider.
#[derive(Properties, PartialEq)]
pub struct BevyProviderProps {
    pub children: Children,
    /// Canvas element ID.
    pub canvas_id: String,
}

/// Provider component that initializes the unified Bevy app.
///
/// Starts the app in Idle mode. Pages are responsible for sending
/// `init_game_mode` or `init_editor_mode` commands to switch modes.
#[function_component(BevyProvider)]
pub fn bevy_provider(props: &BevyProviderProps) -> Html {
    let initialized = use_state(|| false);

    // Initialize Bevy on mount and register beforeunload handler
    {
        let initialized = initialized.clone();
        let canvas_id = props.canvas_id.clone();

        use_effect_with((), move |_| {
            // Register beforeunload handler to cleanup Bevy state on page reload
            let window = web_sys::window().expect("no global window");
            let beforeunload_closure = Closure::<dyn Fn()>::new(move || {
                tracing::info!("beforeunload: requesting Bevy exit and cleaning up state");
                request_bevy_exit();
                reset_bevy_state();
            });

            window
                .add_event_listener_with_callback(
                    "beforeunload",
                    beforeunload_closure.as_ref().unchecked_ref(),
                )
                .expect("failed to add beforeunload listener");

            // Small delay to ensure canvas is mounted
            let initialized = initialized.clone();
            let timeout = gloo::timers::callback::Timeout::new(100, move || {
                initialized.set(true);
                tracing::info!("Bevy initializing (unified app)...");

                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = start_bevy_app(&canvas_id) {
                        tracing::error!("Failed to initialize Bevy: {:?}", e);
                    }
                });
            });

            // Cleanup function - called on unmount
            let window_clone = web_sys::window().expect("no global window");
            move || {
                let _ = window_clone.remove_event_listener_with_callback(
                    "beforeunload",
                    beforeunload_closure.as_ref().unchecked_ref(),
                );
                tracing::info!(
                    "BevyProvider unmounting: keeping Bevy app alive for mode transition"
                );
                drop(timeout);
            }
        });
    }

    let context = BevyContext {
        initialized: *initialized,
    };

    let canvas_style = "position: fixed; top: 0; left: 0; width: 100%; height: 100%; z-index: 0;";

    html! {
        <ContextProvider<BevyContext> context={context}>
            <canvas
                id={props.canvas_id.clone()}
                class="bevy-canvas"
                style={canvas_style}
            />
            { props.children.clone() }
        </ContextProvider<BevyContext>>
    }
}

/// Hook to get Bevy context.
#[hook]
pub fn use_bevy() -> BevyContext {
    use_context::<BevyContext>().unwrap_or(BevyContext { initialized: false })
}

// ============================================================================
// Polling Hooks
// ============================================================================

/// Polling interval in milliseconds.
const POLL_INTERVAL_MS: u32 = 50; // 20 FPS for UI updates

/// Hook to get connection state.
#[hook]
pub fn use_bevy_connection() -> ConnectionInfo {
    let state = use_state(ConnectionInfo::default);

    {
        let state = state.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let js_value = get_connection_state();
                if let Ok(info) = serde_wasm_bindgen::from_value::<ConnectionInfo>(js_value) {
                    state.set(info);
                }
            });

            move || drop(interval)
        });
    }

    (*state).clone()
}

/// Hook to get peer list.
#[hook]
pub fn use_bevy_peers() -> Vec<PeerInfo> {
    let peers = use_state(Vec::new);
    let last_version = use_mut_ref(|| 0u64);

    {
        let peers = peers.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_peers_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;
                    let js_value = get_peers();
                    if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<PeerInfo>>(js_value) {
                        peers.set(list);
                    }
                }
            });

            move || drop(interval)
        });
    }

    (*peers).clone()
}

/// Hook to get player list.
#[hook]
pub fn use_bevy_players() -> (Vec<PlayerInfo>, Vec<u32>) {
    let players = use_state(Vec::new);
    let arrival_order = use_state(Vec::new);
    let last_version = use_mut_ref(|| 0u64);

    {
        let players = players.clone();
        let arrival_order = arrival_order.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_players_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;

                    let js_players = get_players();
                    if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<PlayerInfo>>(js_players)
                    {
                        players.set(list);
                    }

                    let js_order = get_arrival_order();
                    if let Ok(order) = serde_wasm_bindgen::from_value::<Vec<u32>>(js_order) {
                        arrival_order.set(order);
                    }
                }
            });

            move || drop(interval)
        });
    }

    ((*players).clone(), (*arrival_order).clone())
}

/// Hook to get chat messages.
#[hook]
pub fn use_bevy_chat() -> Vec<ChatMessage> {
    let messages = use_state(Vec::new);
    let last_version = use_mut_ref(|| 0u64);

    {
        let messages = messages.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_chat_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;
                    let js_value = get_chat_messages();
                    if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<ChatMessage>>(js_value) {
                        messages.set(list);
                    }
                }
            });

            move || drop(interval)
        });
    }

    (*messages).clone()
}

/// Hook to get reactions.
#[hook]
pub fn use_bevy_reactions() -> Vec<Reaction> {
    let reactions = use_state(Vec::new);
    let last_version = use_mut_ref(|| 0u64);

    {
        let reactions = reactions.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_reactions_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;
                    let js_value = get_reactions();
                    if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<Reaction>>(js_value) {
                        reactions.set(list);
                    }
                }
            });

            move || drop(interval)
        });
    }

    (*reactions).clone()
}

/// Hook to get game state.
#[hook]
pub fn use_bevy_game() -> GameStateSummary {
    let state = use_state(GameStateSummary::default);
    let last_version = use_mut_ref(|| 0u64);

    {
        let state = state.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_game_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;
                    let js_value = get_game_state();
                    if let Ok(summary) =
                        serde_wasm_bindgen::from_value::<GameStateSummary>(js_value)
                    {
                        state.set(summary);
                    }
                }
            });

            move || drop(interval)
        });
    }

    (*state).clone()
}

/// Hook to get editor state summary.
#[hook]
pub fn use_bevy_editor_state() -> EditorStateSummary {
    let state = use_state(EditorStateSummary::default);
    let last_version = use_mut_ref(|| 0u64);

    {
        let state = state.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_editor_state_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;
                    let js_value = get_editor_state();
                    if let Ok(summary) =
                        serde_wasm_bindgen::from_value::<EditorStateSummary>(js_value)
                    {
                        state.set(summary);
                    }
                }
            });

            move || drop(interval)
        });
    }

    (*state).clone()
}

/// Hook to get editor objects (map objects) from Bevy.
///
/// Race condition prevention is handled at the state store level:
/// `get_editor_state_version()` returns 0 until the map is loaded,
/// so this hook won't fetch data until Bevy is ready.
#[hook]
pub fn use_bevy_editor_objects() -> Vec<marble_core::map::MapObject> {
    let objects = use_state(Vec::new);
    let last_version = use_mut_ref(|| 0u64);

    {
        let objects = objects.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_editor_state_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;
                    let js_value = get_editor_objects();
                    if let Ok(list) =
                        serde_wasm_bindgen::from_value::<Vec<marble_core::map::MapObject>>(js_value)
                    {
                        objects.set(list);
                    }
                }
            });

            move || drop(interval)
        });
    }

    (*objects).clone()
}

/// Hook to get editor keyframes (keyframe sequences) from Bevy.
///
/// Race condition prevention is handled at the state store level:
/// `get_editor_keyframes_version()` returns 0 until the map is loaded,
/// so this hook won't fetch data until Bevy is ready.
#[hook]
pub fn use_bevy_editor_keyframes() -> Vec<marble_core::map::KeyframeSequence> {
    let keyframes = use_state(Vec::new);
    let last_version = use_mut_ref(|| 0u64);

    {
        let keyframes = keyframes.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_editor_keyframes_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;
                    let js_value = get_editor_keyframes();
                    if let Ok(list) = serde_wasm_bindgen::from_value::<
                        Vec<marble_core::map::KeyframeSequence>,
                    >(js_value)
                    {
                        keyframes.set(list);
                    }
                }
            });

            move || drop(interval)
        });
    }

    (*keyframes).clone()
}

/// Hook to get snap configuration from Bevy.
#[hook]
pub fn use_bevy_snap_config() -> SnapConfigSummary {
    let config = use_state(SnapConfigSummary::default);
    let last_version = use_mut_ref(|| 0u64);

    {
        let config = config.clone();
        let last_version = last_version.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(POLL_INTERVAL_MS, move || {
                let version = get_snap_config_version();
                if version != *last_version.borrow() {
                    *last_version.borrow_mut() = version;
                    let js_value = get_snap_config();
                    if let Ok(summary) =
                        serde_wasm_bindgen::from_value::<SnapConfigSummary>(js_value)
                    {
                        config.set(summary);
                    }
                }
            });

            move || drop(interval)
        });
    }

    (*config).clone()
}
