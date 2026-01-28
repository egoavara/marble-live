//! Marble-Live Core Library
//!
//! Physics simulation and game logic using `Rapier2D` with deterministic behavior.

#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

pub mod context_game;
pub mod context_keyframe;
pub mod dsl;
pub mod engine;
pub mod executor_cache;
pub mod expr;
pub mod game;
pub mod keyframe;
pub mod map;
pub mod marble;
pub mod physics;
pub mod sync;
pub mod util;

pub use dsl::{DslError, GameContext, NumberOrExpr, Vec2OrExpr};
pub use game::{GameState, Player};
pub use keyframe::KeyframeExecutor;
pub use map::{
    BlackholeData, EasingType, EvaluatedShape, Keyframe, KeyframeSequence, MapMeta, MapObject,
    MapWorldData, ObjectProperties, ObjectRole, RollDirection, RollProperties, RouletteConfig,
    Shape, SpawnerData,
};
pub use marble::{Color, DEFAULT_MARBLE_RADIUS, Marble, MarbleId, MarbleManager, PlayerId};
pub use physics::{PHYSICS_DT, PhysicsWorld, default_gravity};
pub use sync::SyncSnapshot;
