pub mod backup;
pub mod bootstrap;
pub mod capabilities;
pub mod db;
pub mod disabled;
pub mod domain;
pub mod fs;
pub(crate) mod persist;
pub mod registry;
pub mod scanner;
pub mod service;
pub mod system;

pub use backup::BackupOutcome;
pub use domain::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, RevealedValue};
pub use domain::error::{CoreError, ErrorCode};
pub use domain::path_entry::{PathEntry, PathSnapshot};
pub use registry::WriteOutcome;
pub use scanner::{ConflictEntry, ConflictLocation, ToolGroup};
