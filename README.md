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
```

## Current non-goals

- calendar integration;
- Git/CI adapters;
- synchronization;
- generalized workflow automation;
- automatic completion without an explicit verification record;
- a UI or dashboard.
