//! Marble-Live Core Library
//!
//! Physics simulation and game logic using `Rapier2D` with deterministic behavior.

#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

pub mod dsl;
pub mod game;
pub mod map;
pub mod marble;
pub mod physics;
pub mod sync;

pub use dsl::{DslError, GameContext, NumberOrExpr, Vec2OrExpr};
pub use game::{GamePhase, GameState, Player, COUNTDOWN_FRAMES};
pub use map::{
    BlackholeData, EvaluatedShape, MapMeta, MapObject, MapWorldData, ObjectProperties, ObjectRole,
    RouletteConfig, Shape, SpawnerData,
};
pub use marble::{Color, Marble, MarbleId, MarbleManager, PlayerId, DEFAULT_MARBLE_RADIUS};
pub use physics::{default_gravity, PhysicsWorld, PHYSICS_DT};
pub use sync::SyncSnapshot;
