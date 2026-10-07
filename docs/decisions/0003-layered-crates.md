# 0003. Layered single-purpose crates

- **Status:** Accepted
- **Date:** 2026-10-07

## Problem

Large editors tend to tangle: UI code edits data directly, file parsers know about rendering, and every change risks breaking something unrelated.

## Options

1. **One big crate with modules:** simple to start, but nothing stops modules from reaching into each other.
2. **Many crates in fixed layers:** more files, but the build system itself refuses forbidden dependencies.

## Decision

17 crates in five layers (0 file formats, 1 core, 2 services, 3 front end, 4 app). A crate may only depend on its own layer or below. Each crate's README states its purpose and what it must never do. The `layer-check` tool's tests enforce the layer rule, the workspace lints and the README sections on every change.

## Consequences

- Each crate can be tested and replaced on its own; file-format crates can even be reused by other projects.
- Adding a feature means deciding which crate owns it, which is sometimes slower but keeps responsibilities clear.
- Rust compiles crates in parallel, which helps build times as the project grows.
