//! Player dashboard component for displaying room players.

use crate::hooks::{use_config_username, use_room_player};
use yew::prelude::*;

/// Props for the PlayerDashboard component.
#[derive(Properties, PartialEq)]
pub struct PlayerDashboardProps {
    pub room_id: String,
}

/// Player dashboard component.
///
/// Displays a list of players in the room with their display names,
/// HOST badge for the host player, and YOU badge for the current user.
#[function_component(PlayerDashboard)]
pub fn player_dashboard(props: &PlayerDashboardProps) -> Html {
    let room_player_state = use_room_player(&props.room_id);
    let config_username = use_config_username();

    let current_user_id = (*config_username)
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let content = if room_player_state.loading {
        html! {
            <div class="dashboard-loading">
                <div class="dashboard-spinner"></div>
                <span>{"Loading..."}</span>
            </div>
        }
    } else if let Some(error) = &room_player_state.error {
        html! {
            <div class="dashboard-error">
                {format!("Error: {}", error)}
            </div>
        }
    } else if room_player_state.players.is_empty() {
        html! {
            <div class="dashboard-empty">
                {"No players"}
            </div>
        }
    } else {
        html! {
            <div class="dashboard-list">
                {room_player_state.players.iter().enumerate().map(|(idx, player)| {
                    let is_self = player.user_id == current_user_id;
                    let item_class = if is_self {
                        "dashboard-item self"
                    } else {
                        "dashboard-item"
                    };

                    html! {
                        <div class={item_class} key={player.user_id.clone()}>
                            <span class="dashboard-rank">{idx + 1}</span>
                            <span class="dashboard-name">{&player.user_id}</span>
                            <div class="dashboard-tags">
                                {if player.is_host {
                                    html! { <span class="tag host">{"HOST"}</span> }
                                } else {
                                    html! {}
                                }}
                                {if is_self {
                                    html! { <span class="tag you">{"YOU"}</span> }
                                } else {
                                    html! {}
                                }}
                            </div>
                        </div>
                    }
                }).collect::<Html>()}
            </div>
        }
    };

    html! {
        <div class="player-dashboard">
            <div class="dashboard-header">
                <span class="dashboard-header-icon">{"Players"}</span>
            </div>
            {content}
        </div>
    }
}
