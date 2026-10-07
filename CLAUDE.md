# Notes for Claude sessions

Halberd is developed largely by Claude, steered by the project lead (Rhyslos), who is not a programmer. Read this before any work.

## Start of every session

1. Read `README.md` (progress tracker: what is next), `ARCHITECTURE.md` and `CONTRIBUTING.md`.
2. Read the README of every crate you will touch.
3. Check `docs/decisions/` before changing anything a decision covers.

## Rules that are easy to forget

- One milestone per branch and pull request. Never push to `main`.
- Follow every ground rule in `CONTRIBUTING.md`; CI enforces many of them, review enforces the rest.
- Write tests for every batch (normal, edge, failure, regression, round trip, fuzz, benchmarks as relevant), run the whole suite, and report results in the pull request.
- Update documentation in the same pull request: code comments, crate READMEs, `ARCHITECTURE.md`, `CHANGELOG.md`, and the README progress tracker and history when a milestone completes.
- The pull request's "How to test" list covers what automated tests cannot: feel, visuals, in-game behaviour. Write it for a non-programmer.
- Explain decisions to the project lead in plain words.

## Local environment notes

- If rustup cannot download the pinned toolchain, run commands with `RUSTUP_TOOLCHAIN=stable` when the installed stable version matches `rust-toolchain.toml`.
