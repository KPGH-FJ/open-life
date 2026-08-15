# OpenLife

OpenLife is a local-first personal Agent OS built with Tauri, React, Rust, and
SQLite. It combines conversation, task execution, private context, tools, and
user-reviewed durable updates in one desktop product.

## Current Product

The desktop product exposes three routes:

- `/workspace`
- `/life-model`
- `/settings`

Workbench keeps conversations, results, and needs-attention items in one
surface. Task and review state remain explicit backend facts rather than
duplicated top-level pages.

Unknown and retired routes fail closed instead of redirecting to old product
surfaces.

OpenLife does not silently write LifeModel, long-term memory, files, external
services, or other durable state. Risky changes go through an explicit
confirmation or Review Center proposal flow.

See [PRODUCT.md](PRODUCT.md) for product scope and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the source map.

## Development

Requirements:

- Rust 1.89+
- Node.js 18+
- Corepack with pnpm 9.1.x
- Tauri 2 system dependencies

```sh
make setup
make dev
```

Common checks:

```sh
cargo fmt --check
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
```

Repository-specific development rules are in [AGENTS.md](AGENTS.md).
