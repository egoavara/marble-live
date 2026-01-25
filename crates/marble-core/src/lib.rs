//! Marble-Live Core Library
//!
//! Physics simulation and game logic using `Rapier2D` with deterministic behavior.

#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

pub mod dsl;
pub mod game;
pub mod keyframe;
pub mod map;
pub mod marble;
pub mod physics;
pub mod sync;

pub use dsl::{DslError, GameContext, NumberOrExpr, Vec2OrExpr};
pub use game::{GamePhase, GameState, Player, COUNTDOWN_FRAMES};
pub use keyframe::KeyframeExecutor;
pub use map::{
    BlackholeData, EasingType, EvaluatedShape, Keyframe, KeyframeSequence, MapMeta, MapObject,
    MapWorldData, ObjectProperties, ObjectRole, RollDirection, RollProperties, RouletteConfig,
    Shape, SpawnerData,
};
pub use marble::{Color, Marble, MarbleId, MarbleManager, PlayerId, DEFAULT_MARBLE_RADIUS};
pub use physics::{default_gravity, PhysicsWorld, PHYSICS_DT};
pub use sync::SyncSnapshot;
