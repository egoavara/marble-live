//! P2P Debug page for testing Partial Mesh + Gossip communication.
//! Supports multiple peer instances in a single browser tab.

use marble_proto::room::{CreateRoomRequest, PlayerAuth};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::components::{NetworkVisualization, PeerConfig, PeerInstanceCard};
use crate::hooks::{use_config_secret, use_config_username, use_grpc_room_service};

/// P2P Debug Page Component with multi-instance support
#[function_component(DebugP2pPage)]
pub fn debug_p2p_page() -> Html {
    let grpc = use_grpc_room_service();
    let config_username = use_config_username();
    let config_secret = use_config_secret();

    // Username must be set before using P2P debug page
    let base_player_id = (*config_username)
        .clone()
        .expect("Username must be set before using P2P debug page");
    let base_player_secret = (*config_secret).to_string();

    // Multi-instance state
    let peer_id_counter = use_state(|| 1u32);
    let peer_configs = use_state(|| Vec::<PeerConfig>::new());
    let global_room_id = use_state(|| String::new());
    let max_players_input = use_state(|| "10".to_string());

    // Join all trigger state
    let join_all_trigger = use_state(|| 0u32);

    // Visualization refresh trigger (incremented when peer joins)
    let viz_refresh_trigger = use_state(|| 0u32);

    // Add new peer instance
    let on_add_peer = {
        let peer_id_counter = peer_id_counter.clone();
        let peer_configs = peer_configs.clone();
        let base_player_id = base_player_id.clone();
        let base_player_secret = base_player_secret.clone();

        Callback::from(move |_| {
            let id = *peer_id_counter;
            let new_config = PeerConfig {
                id,
                player_id: format!("{}_{}", base_player_id, id),
                player_secret: base_player_secret.clone(),
            };

            let mut configs = (*peer_configs).clone();
            configs.push(new_config);
            peer_configs.set(configs);
            peer_id_counter.set(id + 1);
        })
    };

    // Remove peer instance
    let on_remove_peer = {
        let peer_configs = peer_configs.clone();

        Callback::from(move |id: u32| {
            let configs: Vec<_> = (*peer_configs)
                .iter()
                .filter(|c| c.id != id)
                .cloned()
                .collect();
            peer_configs.set(configs);
        })
    };

    // Create Room handler
    let on_create_room = {
        let grpc = grpc.clone();
        let base_player_id = base_player_id.clone();
        let base_player_secret = base_player_secret.clone();
        let global_room_id = global_room_id.clone();
        let max_players_input = max_players_input.clone();
        let peer_id_counter = peer_id_counter.clone();
        let peer_configs = peer_configs.clone();

        Callback::from(move |_| {
            let grpc = grpc.clone();
            let player_id = base_player_id.clone();
            let player_secret = base_player_secret.clone();
            let global_room_id = global_room_id.clone();
            let max_players = max_players_input.parse::<u32>().unwrap_or(10);
            let peer_id_counter = peer_id_counter.clone();
            let peer_configs = peer_configs.clone();

            spawn_local(async move {
                let req = CreateRoomRequest {
                    host: Some(PlayerAuth {
                        id: player_id.clone(),
                        secret: player_secret.clone(),
                    }),
                    max_players,
                };

                match grpc.borrow_mut().create_room(req).await {
                    Ok(resp) => {
                        let resp = resp.into_inner();
                        global_room_id.set(resp.room_id.clone());

                        // Auto-add host as the first peer instance
                        let host_id = *peer_id_counter;
                        let host_config = PeerConfig {
                            id: host_id,
                            player_id: player_id.clone(),
                            player_secret: player_secret.clone(),
                        };
                        let mut configs = (*peer_configs).clone();
                        configs.push(host_config);
                        peer_configs.set(configs);
                        peer_id_counter.set(host_id + 1);

                        web_sys::console::log_1(
                            &format!(
                                "Room created: {} (host added as instance #{})",
                                resp.room_id, host_id
                            )
                            .into(),
                        );
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to create room: {e}").into());
                    }
                }
            });
        })
    };

    // Input handlers
    let on_room_id_change = {
        let global_room_id = global_room_id.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            global_room_id.set(input.value());
        })
    };

    let on_max_players_change = {
        let max_players_input = max_players_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            max_players_input.set(input.value());
        })
    };

    // Join all peers at once
    let on_join_all = {
        let join_all_trigger = join_all_trigger.clone();
        Callback::from(move |_| {
            join_all_trigger.set(*join_all_trigger + 1);
        })
    };

    // Callback when a peer successfully joins - refresh visualization
    let on_peer_joined = {
        let viz_refresh_trigger = viz_refresh_trigger.clone();
        Callback::from(move |_: u32| {
            viz_refresh_trigger.set(*viz_refresh_trigger + 1);
        })
    };

    html! {
        <main class="page debug-p2p-page" style="padding: 20px; font-family: monospace; color: #e0e0e0;">
            <h1 style="color: #fff; margin-bottom: 20px;">{ "P2P Debug - Multi-Instance" }</h1>

            // Global Controls
            <section style="margin-bottom: 20px; padding: 15px; background: #1a1a2e; border: 1px solid #333; border-radius: 8px;">
                <h3 style="color: #667eea; margin-top: 0; margin-bottom: 10px; border-bottom: 1px solid #333; padding-bottom: 8px;">{ "Global Controls" }</h3>

                // Base player info
                <div style="margin-bottom: 10px; color: #aaa; font-size: 12px;">
                    <span>{ format!("Base Player: {}", base_player_id) }</span>
                    <span style="margin-left: 20px;">{ format!("Secret: {}****", &base_player_secret[..4.min(base_player_secret.len())]) }</span>
                </div>

                // Room creation / ID input
                <div style="display: flex; gap: 10px; flex-wrap: wrap; align-items: center; margin-bottom: 10px;">
                    <div style="display: flex; align-items: center; gap: 4px;">
                        <label style="color: #aaa; font-size: 12px;">{ "Max Players:" }</label>
                        <input
                            type="number"
                            value={(*max_players_input).clone()}
                            oninput={on_max_players_change}
                            style="width: 50px; padding: 4px; background: #2a2a3e; border: 1px solid #444; border-radius: 4px; color: #fff; font-size: 12px;"
                        />
                    </div>
                    <button
                        onclick={on_create_room}
                        style="padding: 6px 12px; background: #4caf50; border: none; border-radius: 4px; color: #fff; cursor: pointer;"
                    >{ "Create Room" }</button>
                </div>

                <div style="display: flex; gap: 10px; flex-wrap: wrap; align-items: center;">
                    <div style="display: flex; align-items: center; gap: 4px;">
                        <label style="color: #aaa; font-size: 12px;">{ "Global Room ID:" }</label>
                        <input
                            type="text"
                            value={(*global_room_id).clone()}
                            oninput={on_room_id_change}
                            placeholder="Enter or create room ID"
                            style="width: 250px; padding: 6px; background: #2a2a3e; border: 1px solid #444; border-radius: 4px; color: #fff;"
                        />
                    </div>
                    <button
                        onclick={on_add_peer}
                        style="padding: 6px 12px; background: #667eea; border: none; border-radius: 4px; color: #fff; cursor: pointer;"
                    >{ "+ Add Peer Instance" }</button>
                    <button
                        onclick={on_join_all}
                        disabled={peer_configs.is_empty() || (*global_room_id).is_empty()}
                        style="padding: 6px 12px; background: #ff9800; border: none; border-radius: 4px; color: #fff; cursor: pointer;"
                    >{ "Join All" }</button>
                    <span style="color: #aaa; font-size: 12px;">
                        { format!("Total: {} instances", peer_configs.len()) }
                    </span>
                </div>
            </section>

            // Network Visualization (self-fetching from server)
            <section style="margin-bottom: 20px; padding: 15px; background: #1a1a2e; border: 1px solid #333; border-radius: 8px;">
                <h3 style="color: #9c27b0; margin-top: 0; margin-bottom: 10px; border-bottom: 1px solid #333; padding-bottom: 8px;">{ "Network Topology" }</h3>
                <NetworkVisualization
                    room_id={(*global_room_id).clone()}
                    player_id={base_player_id.clone()}
                    player_secret={base_player_secret.clone()}
                    auto_refresh_ms={5000}
                    refresh_trigger={*viz_refresh_trigger}
                />
            </section>

            // Peer Grid
            <section style="
                display: grid;
                grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
                gap: 16px;
            ">
                { for (*peer_configs).iter().map(|config| {
                    html! {
                        <PeerInstanceCard
                            key={config.id}
                            config={config.clone()}
                            room_id={(*global_room_id).clone()}
                            on_remove={on_remove_peer.clone()}
                            on_joined={on_peer_joined.clone()}
                            join_trigger={*join_all_trigger}
                        />
                    }
                }) }
            </section>

            // Help text when no peers
            if peer_configs.is_empty() {
                <div style="
                    text-align: center;
                    padding: 40px;
                    color: #666;
                    border: 2px dashed #333;
                    border-radius: 8px;
                    margin-top: 20px;
                ">
                    <p style="font-size: 16px; margin-bottom: 10px;">{ "No peer instances yet" }</p>
                    <p style="font-size: 12px;">{ "Click \"+ Add Peer Instance\" to create a new P2P peer." }</p>
                    <p style="font-size: 12px;">{ "Each peer will have an independent P2P connection with a unique player ID." }</p>
                </div>
            }
        </main>
    }
}
