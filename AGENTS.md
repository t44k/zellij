# Zellij Agent Guide

## Build/Test Commands
- Build: `cargo xtask build` (main + plugins) or `cargo x build`
- Quick run (no plugin rebuild): `cargo xtask run --quick-run` or `cargo q`
- Test all: `cargo xtask test`
- Test single: `cargo test -p <package> <test_name_filter>` (e.g., `cargo test -p zellij-server alternate_screen_change_size`)
- E2E tests: `docker-compose up -d && cargo xtask ci e2e --build && cargo xtask ci e2e --test`
- Format: `cargo xtask format` (run before committing)
- Lint: `cargo xtask clippy`
- All-in-one: `cargo xtask make` (format, build, test, clippy)

## Code Style
- **Imports**: Group by `std` → external crates → internal crates; use `zellij_utils::errors::prelude::*` for error handling
- **Formatting**: Run `cargo fmt` (enforces trailing commas in match blocks via `.rustfmt.toml`); 4 spaces, LF line endings
- **Types**: Return `Result<T>` instead of unwrap; use `.context("error description")` on Results; generate ad-hoc errors with `anyhow!("message")`
- **Error handling**: Use `.non_fatal()` for logging errors instead of `log::error!`; attach context before logging
- **Logging**: Use `log::info!()`, logs go to `/tmp/zellij-<UID>/zellij-log/zellij.log`; truncated at 100KB
- **Naming**: Follow Rust conventions (snake_case functions, PascalCase types)
- **Dependencies**: Requires `protoc` for building protocol buffers; plugins compile to wasm32-wasip1
- **Toolchain**: Rust 1.90.0 with clippy and rustfmt components
