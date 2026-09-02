# mg-plan

mg-plan owns the commitment-to-proof path for the suite:

- plans and work items;
- prerequisite dependencies;
- acceptance criteria;
- producer-owned evidence references;
- revision-pinned verification attempts;
- completion only after every criterion passes.

The current slice persists each complete plan aggregate in SQLite and exposes a narrow CLI.
SQLite is the local durable authority; JSON is the portable export/import envelope. mg-plan
does not own calendar events, repositories, CI runs, raw evidence, or cross-application
databases. Those integrate through explicit versioned references and receipts in later slices.

## Verify

```text
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## CLI

```text
cargo run -- create <db> <plan-id> <title>
cargo run -- show <db> <plan-id>
cargo run -- export <db> <plan-id>
cargo run -- import <db> <json-file>
cargo run -- add-work <db> <plan-id> <work-id> <title>
cargo run -- add-dependency <db> <plan-id> <dependent-id> <prerequisite-id>
cargo run -- add-criterion <db> <plan-id> <work-id> <criterion-id> <statement>
cargo run -- start|block|unblock <db> <plan-id> <work-id>
cargo run -- revise <db> <plan-id> <work-id> <title>
cargo run -- verify <db> <plan-id> <work-id> <verification-id> <criterion-id> <subject-revision> <evidence-id> <producer> <source-record> <evidence-revision> <digest> <pass|fail|inconclusive|waived> <verifier>
cargo run -- complete <db> <plan-id> <work-id>
cargo run -- list-work <db> <plan-id>
cargo run -- blocked <db> <plan-id>
cargo run -- verification-gaps <db> <plan-id>
```

Mutation commands load the authoritative aggregate and its SQLite revision, apply domain
validation, and save only through an optimistic revision check. A stale writer receives a
revision conflict and cannot overwrite newer state. Every successful create or mutation adds
an immutable aggregate snapshot to `mutation_history`. Evidence remains producer-owned; the
CLI stores references and revision-pinned verification records rather than raw evidence.

The query commands are read-only projections. `list-work` returns stable summaries ordered by
identifier; `blocked` filters explicit blocked state; `verification-gaps` reports one typed gap
per criterion whose latest proof is missing, non-passing, stale, or missing evidence.

## Current non-goals

- calendar integration;
- Git/CI adapters;
- synchronization;
- generalized workflow automation;
- automatic completion without an explicit verification record;
- a UI or dashboard.
