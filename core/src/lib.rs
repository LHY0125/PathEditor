pub mod backup;
pub mod capabilities;
pub mod disabled;
pub mod env_var;
pub mod fs;
pub mod path_entry;
pub mod profiles;
pub mod reg_store;
pub mod registry;
pub mod scanner;
pub mod system;

pub use env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot};
pub use path_entry::{PathEntry, PathSnapshot};
pub use profiles::{ProfileData, ProfileMeta, ProfilePathEntry};
pub use scanner::{ConflictEntry, ConflictLocation, ToolGroup};
