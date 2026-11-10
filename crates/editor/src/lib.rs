//! The map editor core shared by the CLI, the MCP server and (later) the in-game editor:
//! scenes and their op log, gameplay validation, and review overlays.
pub mod draw;
pub mod scene;
pub mod validate;

pub use scene::{Kind, Op, Scene, Store, Team};
pub use validate::{analyze, Analysis, Report, World};
