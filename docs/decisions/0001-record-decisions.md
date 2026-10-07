# 0001. Record major decisions

- **Status:** Accepted
- **Date:** 2026-10-07

## Problem

Halberd will be built over months, across many development sessions, mostly by an AI assistant guided by a non-programmer. Without written reasons, past choices get second-guessed or quietly undone.

## Options

1. **Rely on memory and commit messages:** no overhead, but reasons get lost.
2. **Short numbered decision records in the repository:** a little overhead per major choice.

## Decision

Write a decision record for every major choice: technology, architecture, file formats, licensing, and any exception to the ground rules.

## Consequences

Every session can see why things are as they are. Reversing a decision means writing a new record, which forces the trade-off to be stated.
