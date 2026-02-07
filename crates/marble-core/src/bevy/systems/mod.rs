//! Systems for the marble game.
//!
//! Organized by functionality:
//! - camera: Camera control (pan, zoom, follow modes)
//! - command: Command queue processing from WASM
//! - physics: Physics configuration, blackhole forces
//! - marble: Marble spawning, elimination, position tracking
//! - keyframe: Keyframe animation updates (including continuous rotation)
//! - game_rules: Trigger detection, arrival handling, ranking
//! - map_loader: Map object spawning
//! - rendering: Shape and marble rendering
//! - state_sync: Sync ECS state to shared stores for UI
//! - editor: Editor-specific systems (gizmos, selection, input)

pub mod camera;
pub mod command;
pub mod editor;
pub mod game_rules;
pub mod keyframe;
pub mod map_loader;
pub mod marble;
#[cfg(target_arch = "wasm32")]
pub mod p2p_sync;
pub mod physics;
pub mod preview;
pub mod rendering;
pub mod simulation;
pub mod state_sync;

pub use camera::*;
pub use command::*;
pub use editor::*;
pub use game_rules::*;
pub use keyframe::*;
pub use map_loader::*;
pub use marble::*;
pub use physics::*;
pub use preview::*;
pub use rendering::*;
pub use simulation::*;
pub use state_sync::*;
