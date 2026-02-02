//! Map Editor page - Bevy-based version.
//!
//! Uses MarbleEditor (Bevy) for rendering and interaction,
//! with Yew UI panels for property editing.

use marble_core::RouletteConfig;
use yew::prelude::*;

use crate::components::editor::{EditorToolbar, ObjectList, PropertyPanel, TimelinePanel};
use crate::components::{Layout, MarbleEditor};
use crate::hooks::{
    get_shape_center, send_command, use_bevy_editor_state, use_bevy_snap_config, use_editor_state,
    use_keyboard_shortcuts, KeyboardShortcutsConfig,
};

/// Map Editor page component.
#[function_component(EditorPage)]
pub fn editor_page() -> Html {
    let editor_state = use_editor_state();
    let bevy_editor_state = use_bevy_editor_state();
    let snap_config = use_bevy_snap_config();

    // Simulation control states (these will be synced with Bevy)
    let is_simulating = use_state(|| false);
    let spawn_count = use_state(|| 5u32);

    // Keyframe preview state - 로컬 상태
    let is_previewing = use_state(|| false);

    // 현재 선택된 시퀀스의 실행 인덱스 가져오기 (Bevy에서 동기화)
    let preview_keyframe_index = editor_state.selected_sequence
        .and_then(|seq_idx| editor_state.config.keyframes.get(seq_idx))
        .and_then(|seq| bevy_editor_state.executing_keyframes.get(&seq.name))
        .copied();

    // Toggle simulation
    let on_toggle_simulation = {
        let is_simulating = is_simulating.clone();
        let currently_simulating = *is_simulating;
        Callback::from(move |_| {
            if currently_simulating {
                if let Err(e) = send_command(r#"{"type":"stop_simulation"}"#) {
                    tracing::error!("Failed to stop simulation: {:?}", e);
                }
                is_simulating.set(false);
            } else {
                if let Err(e) = send_command(r#"{"type":"start_simulation"}"#) {
                    tracing::error!("Failed to start simulation: {:?}", e);
                }
                is_simulating.set(true);
            }
        })
    };

    // Spawn marbles
    let on_spawn = {
        let spawn_count = spawn_count.clone();
        Callback::from(move |_| {
            let count = *spawn_count;
            tracing::info!("[editor] on_spawn called with spawn_count={}", count);

            // Frame 1: 기존 마블 제거
            tracing::info!("[editor] Sending clear_marbles");
            if let Err(e) = send_command(r#"{"type":"clear_marbles"}"#) {
                tracing::error!("Failed to clear marbles: {:?}", e);
            }

            // Frame boundary - 이후 명령은 다음 프레임에서 처리
            tracing::info!("[editor] Sending yield");
            if let Err(e) = send_command(r#"{"type":"yield"}"#) {
                tracing::error!("Failed to send yield: {:?}", e);
            }

            // Frame 2: 플레이어 초기화 + 스폰
            tracing::info!("[editor] Sending clear_players");
            if let Err(e) = send_command(r#"{"type":"clear_players"}"#) {
                tracing::error!("Failed to clear players: {:?}", e);
            }

            for i in 0..count {
                let color = get_test_player_color(i);
                let cmd = serde_json::json!({
                    "type": "add_player",
                    "name": format!("Player {}", i + 1),
                    "color": color
                });
                tracing::info!("[editor] Sending add_player: Player {}", i + 1);
                if let Err(e) = send_command(&cmd.to_string()) {
                    tracing::error!("Failed to add player {}: {:?}", i + 1, e);
                }
            }

            tracing::info!("[editor] Sending spawn_marbles");
            if let Err(e) = send_command(r#"{"type":"spawn_marbles"}"#) {
                tracing::error!("Failed to spawn marbles: {:?}", e);
            }
        })
    };

    // Reset simulation
    let on_reset = {
        let is_simulating = is_simulating.clone();
        Callback::from(move |_| {
            if let Err(e) = send_command(r#"{"type":"reset_simulation"}"#) {
                tracing::error!("Failed to reset simulation: {:?}", e);
            }
            if let Err(e) = send_command(r#"{"type":"clear_marbles"}"#) {
                tracing::error!("Failed to clear marbles: {:?}", e);
            }
            is_simulating.set(false);
        })
    };

    // Spawn count change
    let on_spawn_count_change = {
        let spawn_count = spawn_count.clone();
        Callback::from(move |count: u32| {
            spawn_count.set(count);
        })
    };

    // Preview sequence callback
    let on_preview_sequence = {
        let is_previewing = is_previewing.clone();
        let currently_previewing = *is_previewing;
        Callback::from(move |_: ()| {
            if currently_previewing {
                if let Err(e) = send_command(r#"{"type":"preview_sequence","start":false}"#) {
                    tracing::error!("Failed to stop preview: {:?}", e);
                }
                is_previewing.set(false);
            } else {
                if let Err(e) = send_command(r#"{"type":"preview_sequence","start":true}"#) {
                    tracing::error!("Failed to start preview: {:?}", e);
                }
                is_previewing.set(true);
            }
        })
    };

    // Serialize config for Bevy
    let config_json = serde_json::to_string(&editor_state.config)
        .unwrap_or_else(|_| serde_json::to_string(&RouletteConfig::default_classic()).unwrap());

    // Keyboard shortcuts
    let on_kb_copy = {
        let on_copy = editor_state.on_copy.clone();
        let selected = editor_state.selected_object;
        Callback::from(move |_: ()| {
            if let Some(idx) = selected {
                on_copy.emit(idx);
            }
        })
    };

    let on_kb_paste = {
        let on_paste = editor_state.on_paste.clone();
        let clipboard = editor_state.clipboard.clone();
        Callback::from(move |_: ()| {
            if let Some(ref obj) = clipboard {
                // Paste near the original object position with a small offset
                let center = get_shape_center(&obj.shape);
                let offset = 0.3; // Small offset to avoid exact overlap
                on_paste.emit((center[0] + offset, center[1] + offset));
            }
        })
    };

    let on_kb_delete = {
        let on_delete = editor_state.on_delete.clone();
        let selected = editor_state.selected_object;
        Callback::from(move |_: ()| {
            if let Some(idx) = selected {
                on_delete.emit(idx);
            }
        })
    };

    use_keyboard_shortcuts(KeyboardShortcutsConfig {
        on_copy: Some(on_kb_copy),
        on_paste: Some(on_kb_paste),
        on_delete: Some(on_kb_delete),
        on_undo: None, // TODO: Implement undo/redo
        on_redo: None,
        enabled: true,
    });

    html! {
        <Layout show_settings={false}>
            <div class="editor-fullscreen">
                // Bevy-rendered canvas with gizmos
                <MarbleEditor
                    config_json={config_json}
                    has_clipboard={editor_state.clipboard.is_some()}
                    selected_object={editor_state.selected_object}
                    on_copy={editor_state.on_copy.clone()}
                    on_paste={editor_state.on_paste.clone()}
                    on_delete={editor_state.on_delete.clone()}
                    on_mirror_x={editor_state.on_mirror_x.clone()}
                    on_mirror_y={editor_state.on_mirror_y.clone()}
                />

                // Toolbar (top-center)
                <EditorToolbar
                    config={editor_state.config.clone()}
                    is_dirty={editor_state.is_dirty}
                    on_new={editor_state.on_new.clone()}
                    on_load={editor_state.on_load.clone()}
                    on_save={editor_state.on_save.clone()}
                    is_simulating={*is_simulating}
                    on_toggle_simulation={on_toggle_simulation}
                    spawn_count={*spawn_count}
                    on_spawn_count_change={on_spawn_count_change}
                    on_spawn={on_spawn}
                    on_reset={on_reset}
                    snap_config={snap_config}
                />

                // Floating Object List (left side)
                <div class="editor-floating-panel editor-panel-left">
                    <ObjectList
                        objects={editor_state.config.objects.clone()}
                        selected_index={editor_state.selected_object}
                        on_select={editor_state.on_select.clone()}
                        on_add={editor_state.on_add.clone()}
                        on_delete={editor_state.on_delete.clone()}
                        sequences={editor_state.config.keyframes.clone()}
                        selected_sequence={editor_state.selected_sequence}
                        on_select_sequence={editor_state.on_select_sequence.clone()}
                        on_add_sequence={editor_state.on_add_sequence.clone()}
                        on_delete_sequence={editor_state.on_delete_sequence.clone()}
                        on_update_sequence={editor_state.on_update_sequence.clone()}
                    />
                </div>

                // Floating Property Panel (right side)
                <div class="editor-floating-panel editor-panel-right">
                    <PropertyPanel
                        config={editor_state.config.clone()}
                        selected_index={editor_state.selected_object}
                        on_update_meta={editor_state.on_update_meta.clone()}
                        on_update_object={editor_state.on_update_object.clone()}
                        sequence={editor_state.selected_sequence.and_then(|idx| editor_state.config.keyframes.get(idx).cloned())}
                        selected_keyframe={editor_state.selected_keyframe}
                        on_update_keyframe={{
                            let cb = editor_state.on_update_keyframe.clone();
                            let seq_idx = editor_state.selected_sequence;
                            Callback::from(move |(kf_idx, kf)| {
                                if let Some(idx) = seq_idx {
                                    cb.emit((idx, kf_idx, kf));
                                }
                            })
                        }}
                        shape_cache={editor_state.shape_cache.clone()}
                        on_cache_shape={editor_state.on_cache_shape.clone()}
                    />
                </div>

                // Timeline Panel (bottom)
                <div class="editor-floating-panel editor-panel-bottom">
                    <TimelinePanel
                        sequence={editor_state.selected_sequence.and_then(|idx| editor_state.config.keyframes.get(idx).cloned())}
                        sequence_index={editor_state.selected_sequence}
                        selected_keyframe={editor_state.selected_keyframe}
                        on_select_keyframe={editor_state.on_select_keyframe.clone()}
                        on_add_keyframe={{
                            let cb = editor_state.on_add_keyframe.clone();
                            let seq_idx = editor_state.selected_sequence;
                            Callback::from(move |kf| {
                                if let Some(idx) = seq_idx {
                                    cb.emit((idx, kf));
                                }
                            })
                        }}
                        on_update_keyframe={{
                            let cb = editor_state.on_update_keyframe.clone();
                            let seq_idx = editor_state.selected_sequence;
                            Callback::from(move |(kf_idx, kf)| {
                                if let Some(idx) = seq_idx {
                                    cb.emit((idx, kf_idx, kf));
                                }
                            })
                        }}
                        on_delete_keyframe={{
                            let cb = editor_state.on_delete_keyframe.clone();
                            let seq_idx = editor_state.selected_sequence;
                            Callback::from(move |kf_idx| {
                                if let Some(idx) = seq_idx {
                                    cb.emit((idx, kf_idx));
                                }
                            })
                        }}
                        on_move_keyframe={{
                            let cb = editor_state.on_move_keyframe.clone();
                            let seq_idx = editor_state.selected_sequence;
                            Callback::from(move |(from, to)| {
                                if let Some(idx) = seq_idx {
                                    cb.emit((idx, from, to));
                                }
                            })
                        }}
                        on_update_sequence={{
                            let cb = editor_state.on_update_sequence.clone();
                            let seq_idx = editor_state.selected_sequence;
                            Callback::from(move |seq| {
                                if let Some(idx) = seq_idx {
                                    cb.emit((idx, seq));
                                }
                            })
                        }}
                        available_object_ids={editor_state.config.objects.iter().filter_map(|o| o.id.clone()).collect::<Vec<_>>()}
                        on_preview_sequence={on_preview_sequence}
                        is_previewing={bevy_editor_state.is_previewing || bevy_editor_state.is_simulating}
                        preview_keyframe_index={preview_keyframe_index}
                        on_deselect_sequence={{
                            let cb = editor_state.on_select_sequence.clone();
                            Callback::from(move |_: ()| {
                                cb.emit(None);
                            })
                        }}
                    />
                </div>
            </div>
        </Layout>
    }
}

/// 테스트 플레이어 색상 (인덱스 기반)
fn get_test_player_color(index: u32) -> [u8; 4] {
    const COLORS: [[u8; 4]; 8] = [
        [255, 0, 0, 255],     // Red
        [0, 255, 0, 255],     // Green
        [0, 0, 255, 255],     // Blue
        [255, 255, 0, 255],   // Yellow
        [255, 0, 255, 255],   // Magenta
        [0, 255, 255, 255],   // Cyan
        [255, 128, 0, 255],   // Orange
        [128, 0, 255, 255],   // Purple
    ];
    COLORS[(index as usize) % COLORS.len()]
}
