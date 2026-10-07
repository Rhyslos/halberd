# layer-check

A development tool, not part of the editor. Its tests run with `cargo test` and fail if any crate in `crates/` breaks the architecture rules from `ARCHITECTURE.md`:

- Every crate declares its layer in `Cargo.toml` under `[package.metadata.halberd]`, as a number from 0 to 4.
- A crate only depends on Halberd crates in its own layer or a lower one.
- Every crate uses the workspace lints (`[lints] workspace = true`), so the safety and documentation rules apply everywhere.
- Every crate has a `README.md` with `## Purpose` and `## Must never` sections.

## Must never

- Ship inside the editor
- Be loosened to make a failing pull request pass: fix the crate instead, or change the rule through a decision record
