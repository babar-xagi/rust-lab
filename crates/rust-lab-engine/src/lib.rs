//! Phase 1 execution baseline. Session history is replayed, not live Rust memory.
pub mod backend;
pub mod error;
pub mod session;
pub mod timing;

pub use backend::{Backend, CargoBackend, Execution};
pub use error::{EngineError, Stage};
pub use session::{Cell, Evaluation, Session};
pub use timing::Timings;
