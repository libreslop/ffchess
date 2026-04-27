//! Config parsing coverage tests.

#[cfg(test)]
mod tests {
    use jsonc_parser::parse_to_serde_value;
    use std::path::PathBuf;
    use walkdir::WalkDir;

    #[test]
    /// Verifies every JSONC file in `config/*` parses successfully.
    fn all_jsonc_configs_parse() {
        let config_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config");
        assert!(
            config_root.exists(),
            "config dir missing: {:?}",
            config_root
        );

        let mut checked = 0usize;
        for path in WalkDir::new(&config_root)
            .into_iter()
            .filter_map(Result::ok)
            .map(|entry| entry.into_path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "jsonc"))
        {
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {:?}: {}", path, e));
            let parsed = parse_to_serde_value(&raw, &Default::default())
                .unwrap_or_else(|e| panic!("failed to parse {:?}: {}", path, e));
            assert!(parsed.is_some(), "empty jsonc document: {:?}", path);
            checked += 1;
        }

        assert!(checked > 0, "no jsonc files found under {:?}", config_root);
    }
}
