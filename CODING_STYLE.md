To maintain quality code, strictly follow:
- Encapsulation: keep each file small do not make structs, fields or functions public when not needed.
- Use functional programming style code whenever possible, use idiomatic Rust
- Fix antipatterns from cargo clippy
- Do not use meaningless types e.g. i32, u64 to represent meaningful things, e.g. Score and Distance should be wrapped with different structs
- Do not use hacks, find common patterns between logic and implement common logic
- Write doc comments for every struct, write normal comments for sections of code that are not obvious to understand
- Before submitting code, run NO_COLOR=true trunk build --release on client, and cargo build --release on server to test for errors

Do not ask for confirmation, just do it.
