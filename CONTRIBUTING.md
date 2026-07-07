# Contributing to Disburse Protocol Contracts

Thanks for your interest in contributing to the Disburse Protocol. This repo holds the three Soroban smart contracts (Payroll, Org Registry, Vesting) that make up the on-chain core of the protocol. See [ARCHITECTURE.md](ARCHITECTURE.md) for the full system design.

## Branching Strategy

```
main      — production-ready, protected
develop   — active development, PRs merge here
feat/*    — feature branches (from develop)
fix/*     — bug fix branches (from develop)
```

## How to Contribute

1. Fork the repo.
2. Clone your fork.
3. Create a branch from `develop`: `git checkout -b feat/your-feature`.
4. Make changes, write tests.
5. Run linting and tests locally.
6. Push and open a PR against `develop`.
7. Fill out the PR template.
8. Wait for review.

## Code Standards

- Format with `cargo fmt` before committing.
- Lint with `cargo clippy --all-targets --all-features -- -D warnings` and fix all warnings.
- Public contract functions should have a doc comment explaining what they do, who can call them, and what they emit.
- New storage types or interface changes should be reflected in [ARCHITECTURE.md](ARCHITECTURE.md).

## Commit Convention

```
feat: add payment split configuration
fix: correct timelock calculation overflow
docs: update API route documentation
test: add batch approval unit tests
chore: update dependencies
```

## PR Requirements

- Must pass CI (fmt + clippy + build + test).
- Must include tests for new functionality.
- Must not break existing tests.
- Must reference the issue number: `Closes #42`.

## Wave Contributors

- Check the issues labeled `Stellar Wave` for available tasks.
- Apply via the Drips Wave app or comment on the issue.
- Do NOT start coding until you are officially assigned.
- PRs must be submitted before the Wave deadline.

## Local Setup

```bash
# Install Rust + the wasm32 target
rustup target add wasm32-unknown-unknown

# Install the Soroban CLI
cargo install --locked soroban-cli

# Build all contracts
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test --all
```
