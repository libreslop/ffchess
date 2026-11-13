//! Filesystem path helpers for locating data shipped next to the server.

use std::path::{Path, PathBuf};

/// Resolves `relative` against the working directory, then against the workspace root.
///
/// Falls back to `relative` itself when neither location exists.
pub fn resolve_data_dir(relative: &str) -> PathBuf {
    [
        PathBuf::from(relative),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative),
    ]
    .into_iter()
    .find(|path| path.is_dir())
    .unwrap_or_else(|| PathBuf::from(relative))
}

/// Returns the built client asset directory.
pub fn client_dist_dir() -> PathBuf {
    resolve_data_dir("client/dist")
}

/// Returns the shared static assets directory.
pub fn assets_dir() -> PathBuf {
    resolve_data_dir("assets")
}

/// Returns the runtime configuration directory.
pub fn config_dir() -> PathBuf {
    resolve_data_dir("config")
}
