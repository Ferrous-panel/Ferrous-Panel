# Contributing to FerrousPanel

Thank you for your interest in contributing to **FerrousPanel** — a 100% pure-Rust Ubuntu 22.04 / 24.04 LTS VPS Control Panel (`ferrous-panel` + `ferrousx-bin`).

## Development Setup

### Prerequisites
- **Operating System**: Ubuntu 22.04 / 24.04 LTS
- **Rust**: 1.85+ (`rustup default 1.85.0`)
- **Node.js**: v22+ (used strictly at build time to compile static `dist/` assets for `rust-embed`)

### Build & Verify
```bash
npm ci --legacy-peer-deps
npm run build
npx tsc --noEmit
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

## Engineering Rules
1. **No Fake Data**: Never fabricate CPU, RAM, PID, disk size, or hashes. Unimplemented routes must return `HTTP 501 Not Implemented`.
2. **No Panics or Unsafe**: `#![forbid(unsafe_code)]`; `.unwrap()`, `.expect()`, `panic!()`, `todo!()`, and `unimplemented!()` are denied by workspace lints. Use `Result` + `thiserror`.
3. **Allowlisted System Commands**: Execute system commands only via `system::cmd::run(bin, args, timeout)` with allowlisted binaries; `sh -c` is strictly forbidden.
4. **Validated Newtypes**: Validate all external inputs via `Domain`, `ServiceName`, `Port`, `SafePath`, `SqlIdentifier`, `PortOrRange`, `CronSchedule`, `SafeLinuxUser`, and `OctalMode`.
5. **No Plaintext Secrets**: Never hardcode or log secrets, and never accept passwords via CLI flags.
