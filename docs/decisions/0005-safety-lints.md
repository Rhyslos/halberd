# 0005. No unsafe, no panics in library code

- **Status:** Accepted
- **Date:** 2026-10-07

## Problem

Halberd opens files made by strangers. A crash loses the user's work; a memory bug can be a security hole.

## Options

1. **Guidelines only:** rely on review to catch problems.
2. **Compiler-enforced lints across the workspace:** some convenience lost, but violations fail the build.

## Decision

The workspace `Cargo.toml` sets, for every crate:

- `unsafe_code = "forbid"`: no unsafe Rust. An exception needs its own decision record.
- `missing_docs = "deny"`: every public item is documented.
- Clippy denies `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!` and `dbg!` outside tests (`clippy.toml` exempts tests).

## Consequences

- Errors must be handled and explained, which makes code a little longer but keeps the editor running when something goes wrong.
- Tests may still unwrap, so test code stays short and fails loudly.
