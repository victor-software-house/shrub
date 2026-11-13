use std::sync::Arc;

use shrub_core::extensions::DefaultShellExtensions;
use tokio::sync::Mutex;

/// A reference-counted, thread-safe reference to a `shrub_core::Shell`.
#[allow(type_alias_bounds)]
pub type ShellRef<SE: shrub_core::ShellExtensions = DefaultShellExtensions> =
    Arc<Mutex<shrub_core::Shell<SE>>>;
