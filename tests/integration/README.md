# Nothing lives here

The integration tests are in `integration_tests/` at the repository root, because
they need to be a workspace member to run under `cargo test --workspace`. A
`tests/` directory inside the workspace root would not be compiled as its own crate.

Kept as a signpost so the path does not silently mislead.
