//! Networking module for P2P game synchronization

pub mod grpc_web;
pub mod manager;

pub use manager::{create_shared_network_manager, ConnectionState, NetworkEvent};
