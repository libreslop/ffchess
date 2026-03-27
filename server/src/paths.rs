//! Filesystem path helpers for locating client assets.

use std::path::PathBuf;

/// Resolve the built client asset directory from common run locations.
///
/// Returns a `PathBuf` pointing to the client build output directory.
pub fn client_dist_dir() -> PathBuf {
    let candidates = [
        PathBuf::from("client/dist"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../client/dist"),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("client/dist"))
}

/// Resolve the shared assets directory from common run locations.
///
/// Returns a `PathBuf` pointing to the static assets directory.
pub fn assets_dir() -> PathBuf {
    let candidates = [
        PathBuf::from("assets"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets"),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("assets"))
}
