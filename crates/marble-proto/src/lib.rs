//! Marble-Live Protocol Library
//!
//! Generated protobuf types and gRPC service definitions.
//!
//! # Features
//! - `server`: Enable gRPC server/client code generation (requires tokio runtime)

#[allow(clippy::pedantic)]
pub mod room {
    #[cfg(feature = "server")]
    tonic::include_proto!("room");

    #[cfg(not(feature = "server"))]
    include!(concat!(env!("OUT_DIR"), "/room.rs"));
}

#[allow(clippy::pedantic)]
pub mod play {
    #[cfg(feature = "server")]
    tonic::include_proto!("marble.play");

    #[cfg(not(feature = "server"))]
    include!(concat!(env!("OUT_DIR"), "/marble.play.rs"));
}
