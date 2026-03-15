use std::path::PathBuf;

/// Resolve the built client asset directory from common run locations.
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
