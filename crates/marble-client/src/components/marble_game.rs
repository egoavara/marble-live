//! MarbleGame component - Bevy-based marble roulette game view.
//!
//! Uses BevyProvider for game initialization and hooks for state access.

use yew::prelude::*;

use crate::hooks::{
    use_bevy, use_bevy_game, use_bevy_players, BevyContext, BevyProvider, GameStateSummary,
    PlayerInfo,
};

/// Canvas ID for the game.
pub const GAME_CANVAS_ID: &str = "marble-game-canvas";

/// Props for MarbleGame component.
#[derive(Properties, PartialEq)]
pub struct MarbleGameProps {
    /// Map configuration JSON.
    pub config_json: String,
    /// Callback when game ends.
    #[prop_or_default]
    pub on_game_end: Callback<Vec<PlayerInfo>>,
}

/// MarbleGame component - renders the marble roulette game.
#[function_component(MarbleGame)]
pub fn marble_game(props: &MarbleGameProps) -> Html {
    html! {
        <BevyProvider canvas_id={GAME_CANVAS_ID}>
            <MarbleGameInner on_game_end={props.on_game_end.clone()} />
        </BevyProvider>
    }
}

/// Props for inner game component.
#[derive(Properties, PartialEq)]
struct MarbleGameInnerProps {
    on_game_end: Callback<Vec<PlayerInfo>>,
}

/// Inner component that uses Bevy hooks.
#[function_component(MarbleGameInner)]
fn marble_game_inner(props: &MarbleGameInnerProps) -> Html {
    let bevy = use_bevy();
    let game_state = use_bevy_game();
    let (players, arrival_order) = use_bevy_players();

    // Check for game end
    {
        let on_game_end = props.on_game_end.clone();
        let players = players.clone();
        let arrival_order = arrival_order.clone();

        use_effect_with(arrival_order.clone(), move |order| {
            // Game ends when all players have arrived
            if !order.is_empty() && order.len() == players.len() && !players.is_empty() {
                // Sort players by arrival order
                let mut sorted_players = players.clone();
                sorted_players.sort_by_key(|p| {
                    order.iter().position(|&id| id == p.id).unwrap_or(usize::MAX)
                });
                on_game_end.emit(sorted_players);
            }
        });
    }

    html! {
        <div class="marble-game">
            // Canvas container - Bevy renders here
            <div class="marble-game__canvas-container">
                <canvas
                    id={GAME_CANVAS_ID}
                    class="marble-game__canvas"
                />
            </div>

            // Game UI overlay
            <div class="marble-game__overlay">
                // Status indicator
                <GameStatusBar
                    initialized={bevy.initialized}
                    game_state={game_state.clone()}
                />

                // Player list / leaderboard
                <PlayerLeaderboard
                    players={players}
                    arrival_order={arrival_order}
                />

                // Game controls (when initialized)
                if bevy.initialized {
                    <GameControls bevy={bevy.clone()} game_state={game_state} />
                }
            </div>
        </div>
    }
}

/// Props for status bar.
#[derive(Properties, PartialEq)]
struct GameStatusBarProps {
    initialized: bool,
    game_state: GameStateSummary,
}

/// Game status bar component.
#[function_component(GameStatusBar)]
fn game_status_bar(props: &GameStatusBarProps) -> Html {
    let status_text = if !props.initialized {
        "Initializing...".to_string()
    } else if props.game_state.is_running {
        format!("Frame: {}", props.game_state.frame)
    } else {
        "Ready".to_string()
    };

    let status_class = if !props.initialized {
        "marble-game__status--loading"
    } else if props.game_state.is_running {
        "marble-game__status--running"
    } else {
        "marble-game__status--ready"
    };

    html! {
        <div class={classes!("marble-game__status", status_class)}>
            <span class="marble-game__status-text">{ status_text }</span>
            if !props.game_state.map_name.is_empty() {
                <span class="marble-game__map-name">{ &props.game_state.map_name }</span>
            }
        </div>
    }
}

/// Props for player leaderboard.
#[derive(Properties, PartialEq)]
struct PlayerLeaderboardProps {
    players: Vec<PlayerInfo>,
    arrival_order: Vec<u32>,
}

/// Player leaderboard component.
#[function_component(PlayerLeaderboard)]
fn player_leaderboard(props: &PlayerLeaderboardProps) -> Html {
    // Sort players: arrived first (by rank), then by live_rank
    let mut sorted_players = props.players.clone();
    sorted_players.sort_by(|a, b| {
        match (a.rank, b.rank) {
            (Some(ra), Some(rb)) => ra.cmp(&rb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => {
                // Both not arrived - sort by live_rank
                match (a.live_rank, b.live_rank) {
                    (Some(la), Some(lb)) => la.cmp(&lb),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.id.cmp(&b.id),
                }
            }
        }
    });

    html! {
        <div class="marble-game__leaderboard">
            <h3 class="marble-game__leaderboard-title">{"Players"}</h3>
            <ul class="marble-game__player-list">
                { for sorted_players.iter().map(|player| {
                    let color = format!(
                        "rgb({}, {}, {})",
                        player.color[0], player.color[1], player.color[2]
                    );
                    let arrived_class = if player.arrived {
                        "marble-game__player--arrived"
                    } else {
                        ""
                    };

                    html! {
                        <li class={classes!("marble-game__player", arrived_class)}>
                            <span
                                class="marble-game__player-color"
                                style={format!("background-color: {}", color)}
                            />
                            <span class="marble-game__player-name">{ &player.name }</span>
                            if let Some(rank) = player.rank {
                                <span class="marble-game__player-rank">{ format!("#{}", rank) }</span>
                            } else if let Some(live_rank) = player.live_rank {
                                <span class="marble-game__player-live-rank">
                                    { format!("~{}", live_rank) }
                                </span>
                            }
                        </li>
                    }
                })}
            </ul>
        </div>
    }
}

/// Props for game controls.
#[derive(Properties, PartialEq)]
struct GameControlsProps {
    bevy: BevyContext,
    game_state: GameStateSummary,
}

/// Game controls component.
#[function_component(GameControls)]
fn game_controls(props: &GameControlsProps) -> Html {
    let bevy = props.bevy.clone();

    let on_spawn = {
        let bevy = bevy.clone();
        Callback::from(move |_| {
            if let Err(e) = bevy.spawn_marbles() {
                tracing::error!("Failed to spawn marbles: {}", e);
            }
        })
    };

    let on_clear = {
        let bevy = bevy.clone();
        Callback::from(move |_| {
            if let Err(e) = bevy.clear_marbles() {
                tracing::error!("Failed to clear marbles: {}", e);
            }
        })
    };

    // Only show controls if host
    if !props.game_state.is_host {
        return html! {};
    }

    html! {
        <div class="marble-game__controls">
            if !props.game_state.is_running {
                <button
                    class="marble-game__btn marble-game__btn--spawn"
                    onclick={on_spawn}
                >
                    {"Spawn Marbles"}
                </button>
            } else {
                <button
                    class="marble-game__btn marble-game__btn--clear"
                    onclick={on_clear}
                >
                    {"Clear"}
                </button>
            }
        </div>
    }
}
