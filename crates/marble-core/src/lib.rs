//! Marble-Live Core Library
//!
//! Physics simulation and game logic using `Rapier2D` with deterministic behavior.

#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

pub mod game;
pub mod map;
pub mod marble;
pub mod physics;
pub mod sync;

pub use game::{GamePhase, GameState, Player, COUNTDOWN_FRAMES};
pub use map::RouletteConfig;
pub use marble::{Marble, MarbleId, MarbleManager, PlayerId, Color, DEFAULT_MARBLE_RADIUS};
pub use physics::{PhysicsWorld, PHYSICS_DT, default_gravity};
pub use sync::SyncSnapshot;
