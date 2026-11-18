## Summary


## Verification Checklist
- [ ] Zero fake/random metrics; unimplemented routes return `HTTP 501`
- [ ] Zero `.unwrap()`, `.expect()`, `panic!()`, `todo!()`, `unimplemented!()`, or `unsafe`
- [ ] All host commands executed strictly via `system::cmd::run` (no `sh -c`)
- [ ] All external inputs validated through newtypes (`Domain`, `ServiceName`, `Port`, `SafePath`, etc.)
- [ ] `npm run build && npx tsc --noEmit` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
