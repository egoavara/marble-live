//! Page components.

mod debug;
mod debug_grpc;
mod debug_p2p;
mod home;
mod not_found;
mod panic;
mod play;

pub use debug::DebugIndexPage;
pub use debug_grpc::DebugGrpcPage;
pub use debug_p2p::DebugP2pPage;
pub use home::HomePage;
pub use not_found::NotFoundPage;
pub use panic::{set_panic_hook, PanicPage};
pub use play::PlayPage;
