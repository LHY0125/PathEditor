pub mod bootstrap;
pub mod db;
pub mod disabled;
pub mod domain;
pub mod platform;
pub mod service;

pub use domain::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, RevealedValue};
pub use domain::error::{CoreError, ErrorCode};
pub use domain::path_entry::{PathEntry, PathSnapshot};
pub use platform::backup::BackupOutcome;
pub use platform::registry::WriteOutcome;
pub use platform::scanner::{ConflictEntry, ConflictLocation, ToolGroup};
