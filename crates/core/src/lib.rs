//! Privacy Panel core: finds apps, reads and writes their privacy settings, keeps history.
//! No GUI code lives here, so the root helper and tests can use it too.

pub mod about;
pub mod admin;
pub mod backends;
pub mod describe;
pub mod discovery;
pub mod engine;
pub mod env;
pub mod history;
pub mod model;
pub mod providers;
pub mod target;

pub use env::Env;
pub use model::*;
