//! Page components.

mod debug;
mod debug_grpc;
mod home;
mod not_found;
mod panic;
mod play;

pub use debug::DebugIndexPage;
pub use debug_grpc::DebugGrpcPage;
pub use home::HomePage;
pub use not_found::NotFoundPage;
pub use panic::{set_panic_hook, PanicPage};
pub use play::PlayPage;
