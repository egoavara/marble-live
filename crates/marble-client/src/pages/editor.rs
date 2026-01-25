//! Map Editor page.

use std::cell::RefCell;
use std::rc::Rc;

use marble_core::marble::Color;
use marble_core::GameState;
use yew::prelude::*;

use crate::components::editor::{EditorCanvas, EditorToolbar, ObjectList, PropertyPanel};
use crate::components::Layout;
use crate::hooks::use_editor_state;

/// Map Editor page component.
#[function_component(EditorPage)]
pub fn editor_page() -> Html {
    let editor_state = use_editor_state();

    // Simulation states - use_mut_ref for immediate updates
    let is_simulating = use_state(|| false);
    let spawn_count = use_state(|| 5u32);
    // Use Rc<RefCell<Option<...>>> for game_state to allow immediate mutation
    let game_state_ref: Rc<RefCell<Option<Rc<RefCell<GameState>>>>> =
        use_mut_ref(|| None).clone();
    // Trigger for re-rendering when game_state changes
    let game_state_version = use_state(|| 0u32);


    // Toggle simulation (Play/Pause)
    let on_toggle_simulation = {
        let is_simulating = is_simulating.clone();
        let game_state_ref = game_state_ref.clone();
        let game_state_version = game_state_version.clone();
        let config = editor_state.config.clone();

        Callback::from(move |_| {
            if *is_simulating {
                // Pause
                is_simulating.set(false);
            } else {
                // Play: create GameState if needed
                if game_state_ref.borrow().is_none() {
                    let mut gs = GameState::new(0);
                    gs.load_map(config.clone());
                    *game_state_ref.borrow_mut() = Some(Rc::new(RefCell::new(gs)));
                    game_state_version.set(*game_state_version + 1);
                }
                is_simulating.set(true);
            }
        })
    };

    // Spawn marbles
    let on_spawn = {
        let game_state_ref = game_state_ref.clone();
        let game_state_version = game_state_version.clone();
        let spawn_count = spawn_count.clone();
        let is_simulating = is_simulating.clone();
        let config = editor_state.config.clone();

        Callback::from(move |_| {
            let palette = Color::palette();
            let count = *spawn_count as usize;

            // Create game_state if not exists
            let gs = {
                let mut gs_ref = game_state_ref.borrow_mut();
                if gs_ref.is_none() {
                    let mut new_gs = GameState::new(0);
                    new_gs.load_map(config.clone());
                    let gs_rc = Rc::new(RefCell::new(new_gs));
                    *gs_ref = Some(gs_rc.clone());
                    game_state_version.set(*game_state_version + 1);
                    gs_rc
                } else {
                    gs_ref.as_ref().unwrap().clone()
                }
            };

            // Add players and spawn marbles
            {
                let mut gs = gs.borrow_mut();
                for i in 0..count {
                    let color = palette[i % palette.len()];
                    gs.add_player(format!("Player{}", i + 1), color);
                }
                gs.spawn_marbles();
            }

            // Auto-start simulation if not already running
            if !*is_simulating {
                is_simulating.set(true);
            }
        })
    };

    // Reset simulation
    let on_reset = {
        let is_simulating = is_simulating.clone();
        let game_state_ref = game_state_ref.clone();
        let game_state_version = game_state_version.clone();

        Callback::from(move |_| {
            is_simulating.set(false);
            *game_state_ref.borrow_mut() = None;
            game_state_version.set(*game_state_version + 1);
        })
    };

    // Spawn count change
    let on_spawn_count_change = {
        let spawn_count = spawn_count.clone();
        Callback::from(move |count: u32| {
            spawn_count.set(count);
        })
    };

    // Physics update is handled in EditorCanvas simulation loop

    html! {
        <Layout show_settings={false}>
            <div class="editor-fullscreen">
                // Full-screen canvas with Blender-style unified gizmo
                <EditorCanvas
                    config={editor_state.config.clone()}
                    selected_index={editor_state.selected_object}
                    on_select={editor_state.on_select.clone()}
                    on_object_update={editor_state.on_update_object.clone()}
                    game_state_ref={Some(game_state_ref.clone())}
                    is_simulating={*is_simulating}
                    game_state_version={*game_state_version}
                    has_clipboard={editor_state.clipboard.is_some()}
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
                />

                // Floating Object List (left side) - always visible
                <div class="editor-floating-panel editor-panel-left">
                    <ObjectList
                        objects={editor_state.config.objects.clone()}
                        selected_index={editor_state.selected_object}
                        on_select={editor_state.on_select.clone()}
                        on_add={editor_state.on_add.clone()}
                        on_delete={editor_state.on_delete.clone()}
                    />
                </div>

                // Floating Property Panel (right side) - always visible
                <div class="editor-floating-panel editor-panel-right">
                    <PropertyPanel
                        config={editor_state.config.clone()}
                        selected_index={editor_state.selected_object}
                        on_update_meta={editor_state.on_update_meta.clone()}
                        on_update_object={editor_state.on_update_object.clone()}
                    />
                </div>
            </div>
        </Layout>
    }
}
