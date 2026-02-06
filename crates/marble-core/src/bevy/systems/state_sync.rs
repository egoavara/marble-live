//! State synchronization systems.
//!
//! Syncs Bevy ECS state to SharedStateStores for Yew UI access.

use bevy::prelude::*;

use crate::bevy::{
    EditorStateSummary, GameStateSummary, KeyframeExecutors, MapConfig, MapLoadedEvent, Marble,
    MarbleGameState, PlayerInfo, SnapConfigSummary, StateStores,
};
use crate::bevy::systems::editor::{EditorStateRes, SnapConfig};

/// Resource to store calculated live rankings.
#[derive(Resource, Default)]
pub struct LiveRankings {
    /// Map of player_id to live_rank (1-indexed).
    pub rankings: std::collections::HashMap<u32, u32>,
}

/// System to sync game state to state stores.
///
/// Runs every frame to keep UI state up-to-date.
/// Must run after sync_live_rankings to include live ranking data.
pub fn sync_game_state_to_stores(
    game_state: Res<MarbleGameState>,
    map_config: Option<Res<MapConfig>>,
    state_stores: Res<StateStores>,
    live_rankings: Option<Res<LiveRankings>>,
) {
    // Sync game state summary
    let summary = GameStateSummary {
        is_running: !game_state.arrival_order.is_empty() || game_state.frame > 0,
        is_host: false, // Set by P2P system
        frame: game_state.frame,
        gamerule: game_state.selected_gamerule.clone(),
        map_name: map_config
            .as_ref()
            .map(|c| c.0.meta.name.clone())
            .unwrap_or_default(),
    };
    state_stores.game.update(summary);

    // Sync players with live rankings
    let players: Vec<PlayerInfo> = game_state
        .players
        .iter()
        .map(|p| {
            let arrived = game_state.arrival_order.contains(&p.id);
            let rank = if arrived {
                game_state
                    .arrival_order
                    .iter()
                    .position(|&id| id == p.id)
                    .map(|pos| (pos + 1) as u32)
            } else {
                None
            };

            // Get live rank from calculated rankings
            let live_rank = if arrived {
                None // Already arrived, no live rank needed
            } else {
                live_rankings
                    .as_ref()
                    .and_then(|lr| lr.rankings.get(&p.id).copied())
            };

            PlayerInfo {
                id: p.id,
                name: p.name.clone(),
                color: [p.color.r, p.color.g, p.color.b, p.color.a],
                arrived,
                rank,
                live_rank,
            }
        })
        .collect();

    state_stores.players.set_players(players);
    state_stores
        .players
        .set_arrival_order(game_state.arrival_order.clone());
}

/// System to sync marble positions for live ranking.
///
/// Calculates live rankings based on marble positions and updates
/// the LiveRankings resource. Must run before sync_game_state_to_stores.
pub fn sync_live_rankings(
    game_state: Res<MarbleGameState>,
    marbles: Query<(&Marble, &Transform)>,
    map_config: Option<Res<MapConfig>>,
    mut live_rankings: ResMut<LiveRankings>,
) {
    // Clear previous rankings
    live_rankings.rankings.clear();

    // Only calculate live rankings if game is running
    if game_state.arrival_order.is_empty() && game_state.frame == 0 {
        return;
    }

    // Collect non-arrived marble positions
    let mut marble_scores: Vec<(u32, f32)> = marbles
        .iter()
        .filter(|(marble, _)| {
            !marble.eliminated && !game_state.arrival_order.contains(&marble.owner_id)
        })
        .map(|(marble, transform)| {
            let pos = transform.translation.truncate();
            let score = calculate_ranking_score(pos, map_config.as_deref());
            (marble.owner_id, score)
        })
        .collect();

    // Sort by score (lower is better/closer to goal)
    marble_scores.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Update live rankings resource
    for (rank, (player_id, _)) in marble_scores.iter().enumerate() {
        live_rankings.rankings.insert(*player_id, (rank + 1) as u32);
    }
}

/// Calculate ranking score based on map configuration.
fn calculate_ranking_score(pos: Vec2, config: Option<&MapConfig>) -> f32 {
    use crate::map::{EvaluatedShape, LiveRankingConfig};

    match config {
        Some(MapConfig(config)) => match &config.meta.live_ranking {
            LiveRankingConfig::YPosition => pos.y,
            LiveRankingConfig::Distance { target_id } => {
                if let Some(obj) = config
                    .objects
                    .iter()
                    .find(|o| o.id.as_deref() == Some(target_id))
                {
                    let ctx = crate::dsl::GameContext::new(0.0, 0);
                    let shape = obj.shape.evaluate(&ctx);
                    let target_center = match shape {
                        EvaluatedShape::Circle { center, .. } => Vec2::new(center[0], center[1]),
                        EvaluatedShape::Rect { center, .. } => Vec2::new(center[0], center[1]),
                        _ => return pos.y,
                    };
                    pos.distance(target_center)
                } else {
                    pos.y
                }
            }
        },
        None => pos.y,
    }
}

/// System to sync editor state to state stores.
///
/// Syncs EditorStateRes and MapConfig.objects to EditorStore for Yew UI access.
pub fn sync_editor_to_stores(
    editor_state: Option<Res<EditorStateRes>>,
    map_config: Option<Res<MapConfig>>,
    keyframe_executors: Option<Res<KeyframeExecutors>>,
    state_stores: Res<StateStores>,
) {
    let Some(editor_state) = editor_state else {
        return;
    };

    // 모든 실행 중인 executor의 상태를 맵으로 수집
    let executing_keyframes = keyframe_executors
        .map(|execs| {
            execs.executors.iter()
                .filter(|e| !e.is_finished())
                .map(|e| (e.sequence_name().to_string(), e.current_index()))
                .collect()
        })
        .unwrap_or_default();

    // Build editor state summary
    let summary = EditorStateSummary {
        selected_object: editor_state.selected_object,
        selected_sequence: editor_state.selected_sequence,
        selected_keyframe: editor_state.selected_keyframe,
        is_simulating: editor_state.is_simulating,
        is_previewing: editor_state.is_previewing,
        executing_keyframes,
    };

    // Get objects from MapConfig
    let objects = map_config
        .as_ref()
        .map(|c| c.0.objects.clone())
        .unwrap_or_default();

    // Get keyframes from MapConfig
    let keyframes = map_config
        .as_ref()
        .map(|c| c.0.keyframes.clone())
        .unwrap_or_default();

    // Update the store
    state_stores.editor.update_all(summary, objects, keyframes);
}

/// System to sync snap configuration to state stores.
///
/// Syncs SnapConfig to SnapConfigStore for Yew UI access.
pub fn sync_snap_config_to_stores(
    snap_config: Option<Res<SnapConfig>>,
    state_stores: Res<StateStores>,
) {
    let Some(snap_config) = snap_config else {
        return;
    };

    let summary = SnapConfigSummary {
        // In simplified snap system, snapping is always enabled when interval > 0
        grid_snap_enabled: snap_config.grid_interval > 0.0,
        grid_snap_interval: snap_config.grid_interval,
        angle_snap_enabled: snap_config.angle_interval > 0.0,
        angle_snap_interval: snap_config.angle_interval,
    };

    state_stores.snap_config.update(summary);
}

/// System to mark map as loaded when MapLoadedEvent is received.
///
/// This prevents the Yew-Bevy race condition where Yew starts polling
/// before Bevy has finished loading the map, causing objects to be lost.
pub fn mark_map_loaded_on_event(
    mut events: MessageReader<MapLoadedEvent>,
    state_stores: Res<StateStores>,
) {
    for _event in events.read() {
        state_stores.editor.set_map_loaded(true);
    }
}
