# Security policy

## Reporting a vulnerability

Please report security problems **privately**, not in a public issue:

1. Go to the repository's **Security** tab.
2. Choose **Report a vulnerability**.
3. Describe the problem, how to reproduce it, and what an attacker could do with it.

Halberd is a hobby project, so responses are best effort. You can expect an acknowledgement within a week, and a fix or a plan as soon as practical. Reporters are credited in the release notes unless they prefer not to be.

## Supported versions

Only the latest release and the `main` branch receive security fixes.

## What counts as a vulnerability

Halberd opens files made by strangers (maps, models, textures, Workshop addons), so these are in scope:

- A file that crashes Halberd, hangs it, or makes it use unbounded memory
- A file that makes Halberd read or write outside where it should (for example, archive paths with `../`)
- Anything that makes Halberd run code it should not (for example, Lua from an addon or map)
- Personal data leaking into exported maps or logs (usernames, local paths)
- Weaknesses in how releases are built, signed or downloaded

Out of scope: bugs in Garry's Mod, Valve's compile tools or Steam themselves.

## How Halberd defends itself

See the Security section of [ARCHITECTURE.md](ARCHITECTURE.md): safe Rust, size limits and fuzz tests on every file parser, no shell when running compilers, no telemetry by default, and automatic dependency audits on every change.
