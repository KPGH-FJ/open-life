# Contributing to OpenLife

## Setup

Requirements:

- Rust 1.75+
- Node.js 18+
- Corepack and pnpm 9.1.x
- Tauri 2 system dependencies

```sh
git clone https://github.com/KPGH-FJ/open-life.git
cd open-life
make setup
make dev
```

## Branches

`main` is the only long-lived branch. Use a short-lived `codex/`, `feat/`,
`fix/`, `refactor/`, `test/`, or `docs/` branch and open a PR to `main`.

Do not create additional worktrees or sibling OpenLife checkouts for ordinary
development.

## Before a Pull Request

Run checks proportional to the change:

```sh
git diff --check
cargo fmt --check
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
```

For product-flow changes, also run the Workbench browser smoke and an
appropriate native Tauri check.

## Change Boundaries

- Keep secrets, raw personal data, private prompts, and product data out of Git.
- Do not broaden durable-write, provider, tool, or privacy authority implicitly.
- Add tests for product behavior and failure boundaries.
- Keep documents concise and current.
- Do not add a second planning or evidence platform to the repository.

See [PRODUCT.md](PRODUCT.md), [AGENTS.md](AGENTS.md), and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
