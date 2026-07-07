# Disburse Protocol Contracts

Soroban smart contracts for Disburse Protocol — a decentralized payroll protocol on Stellar. This repo holds the three contracts that make up the on-chain core of the protocol: **Payroll** (schedules, splits, multi-sig disbursement), **Org Registry** (organizations, employees, signer roles), and **Vesting** (cliff + linear token grants). Fund custody and all payroll logic live here; the backend and frontend read from and write to these contracts but never hold funds themselves.

## Architecture

Full system design, storage layout, contract interfaces, and cross-contract call patterns are documented in [ARCHITECTURE.md](ARCHITECTURE.md).

## Tech Stack

- Rust
- Soroban SDK
- `wasm32-unknown-unknown` target
- Soroban CLI for deployment

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [Soroban CLI](https://developers.stellar.org/docs/tools/developer-tools#soroban-cli): `cargo install --locked soroban-cli`

## Local Setup

```bash
git clone https://github.com/Disburse-Protocol/DP-Contracts.git
cd contracts
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
```

## Testing

```bash
cargo test --all
```

## Deployment

Deployed to Stellar Testnet during development, promoted to Mainnet at release. See the deployment script under `scripts/` (once added) and [ARCHITECTURE.md](ARCHITECTURE.md#deployment) for target details.

## Related Repos

- [backend](https://github.com/Disburse-Protocol/DP-Backend) — chain indexer and query API
- [frontend](https://github.com/Disburse-Protocol/DP-Frontend) — employer dashboard and employee portal

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branching strategy, code standards, and how to pick up a Wave task.

## License

[MIT](LICENSE)
