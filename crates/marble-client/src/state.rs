//! Application state management.

use marble_core::{GameState, RouletteConfig};
use std::rc::Rc;
use yew::prelude::*;

/// Application state containing game state and UI state.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Current game state
    pub game_state: Rc<GameState>,
    /// Whether the simulation is running
    pub is_running: bool,
    /// Current FPS
    pub fps: f64,
    /// Last frame time in milliseconds
    pub last_frame_time: f64,
}

impl PartialEq for AppState {
    fn eq(&self, _other: &Self) -> bool {
        // Always return false to ensure re-renders on every state change
        // This is necessary because game state changes every tick
        false
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Creates a new application state with default game configuration.
    pub fn new() -> Self {
        let mut game_state = GameState::new(42);
        game_state.load_map(RouletteConfig::default_classic());

        Self {
            game_state: Rc::new(game_state),
            is_running: false,
            fps: 0.0,
            last_frame_time: 0.0,
        }
    }

    /// Returns the current frame number.
    pub fn frame(&self) -> u64 {
        self.game_state.current_frame()
    }
}

/// Actions that can be dispatched to update the application state.
#[derive(Debug, Clone)]
pub enum AppAction {
    /// Start the simulation
    Start,
    /// Stop the simulation
    Stop,
    /// Reset the game to initial state
    Reset,
    /// Advance the game by one frame
    Tick,
    /// Update FPS counter
    UpdateFps(f64),
    /// Update last frame time
    UpdateFrameTime(f64),
}

/// Reducer for application state.
impl Reducible for AppState {
    type Action = AppAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            AppAction::Start => {
                let mut new_state = (*self).clone();
                new_state.is_running = true;

                // Add test players and spawn marbles if not already done
                if new_state.game_state.players.is_empty() {
                    let game = Rc::make_mut(&mut new_state.game_state);
                    game.add_player("Player 1".to_string(), marble_core::Color::RED);
                    game.add_player("Player 2".to_string(), marble_core::Color::BLUE);
                    game.spawn_marbles();
                }

                Rc::new(new_state)
            }
            AppAction::Stop => {
                let mut new_state = (*self).clone();
                new_state.is_running = false;
                Rc::new(new_state)
            }
            AppAction::Reset => {
                let mut new_state = Self::new();
                new_state.is_running = false;
                Rc::new(new_state)
            }
            AppAction::Tick => {
                if !self.is_running {
                    return self;
                }

                let mut new_state = (*self).clone();
                let game = Rc::make_mut(&mut new_state.game_state);
                game.update();
                Rc::new(new_state)
            }
            AppAction::UpdateFps(fps) => {
                let mut new_state = (*self).clone();
                new_state.fps = fps;
                Rc::new(new_state)
            }
            AppAction::UpdateFrameTime(time) => {
                let mut new_state = (*self).clone();
                new_state.last_frame_time = time;
                Rc::new(new_state)
            }
        }
    }
}

/// Context type for the application state.
pub type AppStateContext = UseReducerHandle<AppState>;
