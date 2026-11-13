//! Environment variable retrieval for Unix platforms.

use std::env;

/// Retrieves environment variables from the host process.
///
/// On Unix, this is a direct passthrough to [`std::env::vars()`].
pub(crate) fn get_host_env_vars() -> impl Iterator<Item = (String, String)> {
    env::vars()
}
