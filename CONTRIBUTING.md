# Contributing to Halberd

Thanks for your interest. Halberd is a hobby project with high standards for stability and performance; these rules keep it that way.

## Setting up

1. Install Rust with [rustup](https://rustup.rs). The exact Rust version is pinned in `rust-toolchain.toml` and installs automatically.
2. Clone the repository and run:

```
cargo run -p halberd-app        # build and start Halberd
cargo test --workspace          # run all tests
cargo clippy --workspace --all-targets -- -D warnings   # lint
cargo fmt --all                 # format
cargo doc --workspace --no-deps --open                  # read the API docs
```

Optional, matching CI: `cargo install cargo-deny` and run `cargo deny check`.

## Ground rules

A pull request that breaks one of these is sent back, however good the feature.

**Structure**

1. **One crate, one job.** New code goes in the crate whose job it is (see each crate's README). If none fits, propose a new crate first.
2. **Dependencies point down only.** Formats know nothing of the editor; the core knows nothing of the GPU or UI; the UI changes the document only through commands. `tools/layer-check` enforces this.
3. **Small units.** As a guide: files under about 500 lines, functions under about 60.
4. **Narrow public surface.** Expose as little as possible from each crate.
5. **Reuse before writing.** Check whether a crate already does it.

**Safety**

6. **No `unsafe` code** without a decision record. The workspace forbids it.
7. **No crashes on bad input.** Library code never uses `unwrap`, `expect` or `panic!` outside tests (enforced by clippy). Return errors that explain the problem in plain words.
8. **Untrusted files are hostile.** Every parser checks sizes and counts before allocating, and gets a fuzz test.
9. **One owner for the document.** Only the interface thread changes it; background jobs send messages.

**Working with existing code**

10. **Read before changing:** the crate's README and tests first.
11. **Change the smallest thing that works.** Refactors get their own pull request.
12. **Tests move with the code.** Bug fixes add a test that reproduces the bug; features add tests for their behaviour.
13. **Documentation changes with the code.** See below.

## Documentation rule

A change is not done until its documentation is.

| Document | Update when |
| --- | --- |
| Code comments (`///` and `//!`) | The item changes. Missing docs fail the build. |
| Crate `README.md` | The crate's purpose or limits change |
| `ARCHITECTURE.md` | A crate is added, removed or moved |
| `CONTRIBUTING.md` | Tooling or rules change |
| `docs/decisions/` | A major choice is made or reversed |
| `CHANGELOG.md` | Any change a user would notice |
| README progress tracker | A milestone or gate is completed |

## Testing: two sign-offs per pull request

1. **Code tests** by the author: normal cases, edge cases, failure cases (bad input gives an error, never a crash), a regression test for every bug fixed, round trips for anything that reads and writes a format, fuzz targets for new parsers, benchmarks for code on a performance path. Run the full suite and report the results in the pull request.
2. **Functional test** by the project lead: the "how to test" list in the pull request, tried in the editor and in GMod.

Both must pass before merging.

## Pull requests

- One milestone or one fix per pull request, on its own branch (for example `phase0/viewport-camera`).
- Fill in the pull request template: what changed, test results, docs updated, how to test.
- All CI checks must be green.
- Commit messages: a short summary line in the imperative ("Add VMF parser"), then details if needed.

## AI assistance

Much of Halberd's code is written with AI assistance (Claude by Anthropic), and commits say so with a `Co-Authored-By` line. AI-written changes follow exactly the same rules and reviews as anyone else's.

## Licensing

By contributing, you agree that your contribution is licensed under the Apache License 2.0. Write your own code: no code from Hammer, leaked Source code, or closed tools such as Hammer++. File formats are implemented from public documentation.
