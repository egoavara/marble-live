//! Marble-Live Protocol Library
//!
//! Generated protobuf types and gRPC service definitions.
//!
//! # Features
//! - `server`: Enable gRPC server/client code generation (requires tokio runtime)

#[allow(clippy::pedantic)]
pub mod user {
    #[cfg(feature = "server")]
    tonic::include_proto!("marble.user");

    #[cfg(not(feature = "server"))]
    include!(concat!(env!("OUT_DIR"), "/marble.user.rs"));
}

#[allow(clippy::pedantic)]
pub mod map {
    #[cfg(feature = "server")]
    tonic::include_proto!("marble.map");

    #[cfg(not(feature = "server"))]
    include!(concat!(env!("OUT_DIR"), "/marble.map.rs"));
}

#[allow(clippy::pedantic)]
pub mod room {
    #[cfg(feature = "server")]
    tonic::include_proto!("marble.room");

    #[cfg(not(feature = "server"))]
    include!(concat!(env!("OUT_DIR"), "/marble.room.rs"));
}

#[allow(clippy::pedantic)]
pub mod play {
    #[cfg(feature = "server")]
    tonic::include_proto!("marble.play");

    #[cfg(not(feature = "server"))]
    include!(concat!(env!("OUT_DIR"), "/marble.play.rs"));
}
