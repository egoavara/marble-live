//! Page components.

mod debug_conntest;
mod debug_index;
mod debug_p2p_play;
mod debug_simple;
mod home;
mod not_found;
mod play;

pub use debug_conntest::DebugConnTestPage;
pub use debug_index::DebugIndexPage;
pub use debug_p2p_play::DebugP2PPlayPage;
pub use debug_simple::DebugSimplePage;
pub use home::HomePage;
pub use not_found::NotFoundPage;
pub use play::PlayPage;
