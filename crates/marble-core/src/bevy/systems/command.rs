//! Command processing system.
//!
//! Processes commands from the JavaScript/WASM interface.

use bevy::prelude::*;

use crate::bevy::{
    AddObjectEvent, AddPlayerEvent, ClearMarblesEvent, CommandQueue, DeleteObjectEvent,
    GameCamera, GameCommand, LoadMapEvent, LocalPlayerId, MainCamera, MapConfig,
    MarbleGameState, PreviewSequenceEvent, RemovePlayerEvent, ResetSimulationEvent,
    SpawnMarblesEvent, StartSimulationEvent, StopSimulationEvent,
};
use crate::bevy::systems::editor::{EditorStateRes, SelectObjectEvent, SnapConfig, UpdateObjectEvent};
use crate::game::Player;

/// System to process all commands from the external command queue.
///
/// Handles game commands until a Yield is encountered.
/// Commands after Yield are processed in the next frame.
#[allow(clippy::too_many_arguments)]
pub fn process_commands(
    command_queue: Res<CommandQueue>,
    mut game_state: ResMut<MarbleGameState>,
    mut cameras: Query<&mut GameCamera, With<MainCamera>>,
    mut local_player: Option<ResMut<LocalPlayerId>>,
    mut spawn_events: MessageWriter<SpawnMarblesEvent>,
    mut clear_events: MessageWriter<ClearMarblesEvent>,
    mut add_player_events: MessageWriter<AddPlayerEvent>,
    mut remove_player_events: MessageWriter<RemovePlayerEvent>,
    mut load_map_events: MessageWriter<LoadMapEvent>,
) {
    // Use drain_until_yield() to process game commands until Yield or empty.
    // This allows frame-separated command processing.
    for command in command_queue.drain_until_yield() {
        match command {
            GameCommand::SpawnMarbles => {
                tracing::info!("[command] SpawnMarbles");
                spawn_events.write(SpawnMarblesEvent);
            }
            GameCommand::ClearMarbles => {
                tracing::info!("[command] ClearMarbles");
                clear_events.write(ClearMarblesEvent);
            }
            GameCommand::ClearPlayers => {
                tracing::info!("[command] ClearPlayers (had {} players)", game_state.players.len());
                game_state.players.clear();
            }
            GameCommand::AddPlayer { name, color } => {
                let id = game_state.players.len() as u32;
                tracing::info!("[command] AddPlayer: {} (id={}, total={})", name, id, id + 1);
                game_state.players.push(Player { id, name: name.clone(), color });
                add_player_events.write(AddPlayerEvent { name, color });
            }
            GameCommand::RemovePlayer { player_id } => {
                tracing::info!("[command] RemovePlayer: {}", player_id);
                game_state.players.retain(|p| p.id != player_id);
                remove_player_events.write(RemovePlayerEvent { player_id });
            }
            GameCommand::LoadMap { config } => {
                tracing::info!("[command] LoadMap with {} objects", config.objects.len());
                load_map_events.write(LoadMapEvent { config });
            }
            GameCommand::SetCameraMode { mode } => {
                tracing::info!("[command] SetCameraMode");
                for mut camera in cameras.iter_mut() {
                    camera.set_mode(mode);
                }
            }
            GameCommand::SetLocalPlayerId { player_id } => {
                tracing::info!("[command] SetLocalPlayerId: {:?}", player_id);
                if let Some(ref mut local) = local_player {
                    local.set(player_id);
                }
            }
            // Yield is consumed by drain_until_yield(), should not reach here
            GameCommand::Yield => {}
            // Editor commands should not reach here due to drain_until_yield()
            _ => {
                tracing::warn!("[command] Unexpected command in process_commands");
            }
        }
    }
}

/// System to process editor-specific commands.
///
/// Handles editor commands like selection, simulation control, and object updates.
/// NOTE: This must run on a separate drain pass, so commands are collected first.
#[allow(clippy::too_many_arguments)]
pub fn process_editor_commands(
    command_queue: Res<CommandQueue>,
    mut editor_state: Option<ResMut<EditorStateRes>>,
    mut map_config: Option<ResMut<MapConfig>>,
    mut snap_config: Option<ResMut<SnapConfig>>,
    mut select_events: MessageWriter<SelectObjectEvent>,
    mut update_events: MessageWriter<UpdateObjectEvent>,
    mut add_object_events: MessageWriter<AddObjectEvent>,
    mut delete_object_events: MessageWriter<DeleteObjectEvent>,
    mut start_sim_events: MessageWriter<StartSimulationEvent>,
    mut stop_sim_events: MessageWriter<StopSimulationEvent>,
    mut reset_sim_events: MessageWriter<ResetSimulationEvent>,
    mut preview_events: MessageWriter<PreviewSequenceEvent>,
) {
    let Some(ref mut editor_state) = editor_state else {
        return;
    };

    // Process editor commands from the queue
    // Note: We use drain_editor() which only takes editor-specific commands
    for command in command_queue.drain_editor() {
        match command {
            GameCommand::SelectObject { index } => {
                editor_state.selected_object = index;
                select_events.write(SelectObjectEvent(index));
            }
            GameCommand::SelectSequence { index } => {
                editor_state.selected_sequence = index;
                if index.is_none() {
                    editor_state.selected_keyframe = None;
                }
            }
            GameCommand::SelectKeyframe { index } => {
                editor_state.selected_keyframe = index;
            }
            GameCommand::UpdateObject { index, object } => {
                if let Some(ref mut config) = map_config {
                    if let Some(obj) = config.0.objects.get_mut(index) {
                        *obj = object.clone();
                    }
                }
                update_events.write(UpdateObjectEvent { index, object });
            }
            GameCommand::AddObject { object } => {
                tracing::info!("[command] AddObject");
                let new_index = if let Some(ref mut config) = map_config {
                    config.0.objects.push(object.clone());
                    let idx = config.0.objects.len() - 1;
                    editor_state.selected_object = Some(idx);
                    idx
                } else {
                    0
                };
                add_object_events.write(AddObjectEvent { object, index: new_index });
            }
            GameCommand::DeleteObject { index } => {
                tracing::info!("[command] DeleteObject index={}", index);
                if let Some(ref mut config) = map_config {
                    if index < config.0.objects.len() {
                        config.0.objects.remove(index);
                        // Adjust selection
                        if let Some(selected) = editor_state.selected_object {
                            if selected == index {
                                editor_state.selected_object = if config.0.objects.is_empty() {
                                    None
                                } else {
                                    Some(selected.min(config.0.objects.len() - 1))
                                };
                            } else if selected > index {
                                editor_state.selected_object = Some(selected - 1);
                            }
                        }
                    }
                }
                delete_object_events.write(DeleteObjectEvent { index });
            }
            GameCommand::UpdateKeyframe {
                sequence_index,
                keyframe_index,
                keyframe,
            } => {
                if let Some(ref mut config) = map_config {
                    if let Some(seq) = config.0.keyframes.get_mut(sequence_index) {
                        if let Some(kf) = seq.keyframes.get_mut(keyframe_index) {
                            *kf = keyframe;
                        }
                    }
                }
            }
            GameCommand::StartSimulation => {
                tracing::info!("[command] StartSimulation");
                start_sim_events.write(StartSimulationEvent);
            }
            GameCommand::StopSimulation => {
                tracing::info!("[command] StopSimulation");
                stop_sim_events.write(StopSimulationEvent);
            }
            GameCommand::ResetSimulation => {
                tracing::info!("[command] ResetSimulation");
                reset_sim_events.write(ResetSimulationEvent);
            }
            GameCommand::PreviewSequence { start } => {
                tracing::info!("[command] PreviewSequence start={}", start);
                preview_events.write(PreviewSequenceEvent { start });
            }
            GameCommand::UpdateSnapConfig {
                grid_snap_interval,
                angle_snap_interval,
                ..
            } => {
                if let Some(ref mut snap_cfg) = snap_config {
                    if let Some(interval) = grid_snap_interval {
                        snap_cfg.grid_interval = interval;
                    }
                    if let Some(interval) = angle_snap_interval {
                        snap_cfg.angle_interval = interval;
                    }
                    tracing::info!(
                        "[command] UpdateSnapConfig: grid={}, angle={}",
                        snap_cfg.grid_interval,
                        snap_cfg.angle_interval
                    );
                }
            }
            _ => {}
        }
    }
}
