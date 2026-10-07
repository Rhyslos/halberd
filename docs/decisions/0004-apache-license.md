# 0004. Apache 2.0 license

- **Status:** Accepted
- **Date:** 2026-10-07

## Problem

Halberd is free and open source. The license decides what others may do with the code.

## Options

1. **GPL 3.0:** every derived version must stay open source; some contributors and companies avoid it.
2. **MIT:** maximally permissive, very short, no patent terms.
3. **Apache 2.0:** permissive like MIT, plus an explicit patent grant and a requirement to keep copyright and NOTICE files in derived works.

## Decision

Apache 2.0. The GMod mapping market is small, so the risk of someone profiting from a closed fork is low; a skilled developer building their own version is welcome. Apache keeps Halberd credited through the NOTICE file and is friendly to contributors.

## Consequences

- Anyone may fork, modify or sell their own version, but must keep the LICENSE and NOTICE.
- All dependencies must have compatible licenses; `cargo deny` checks this on every change (see `deny.toml`).
- The name "Halberd" and the official repository and release channels identify the official version.
