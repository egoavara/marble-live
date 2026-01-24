//! Custom hooks for the marble-client application.
//!
//! These hooks encapsulate reusable logic for:
//! - Room connection management
//! - Room state synchronization
//! - P2P network handling
//! - Game loop tick/render
//! - User settings

mod use_config;
mod use_fingerprint;
// mod use_game_loop;
mod use_grpc_room_service;
mod use_localstorage;
// mod use_p2p_network;
mod use_querystring;
// mod use_room_connection;
// mod use_room_sync;
mod use_userhash;

pub use use_config::*;
pub use use_fingerprint::use_fingerprint;
// pub use use_game_loop::use_game_loop;
pub use use_grpc_room_service::use_grpc_room_service;
pub use use_localstorage::use_localstorage;
// pub use use_p2p_network::use_p2p_network;
pub use use_querystring::use_querystring;
// pub use use_room_connection::use_room_connection;
// pub use use_room_sync::use_room_sync;
pub use use_userhash::*;