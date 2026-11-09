# Release

Version is centralized in `[workspace.package] version` (`Cargo.toml`).

## Local release build

```powershell
cargo build --workspace --release
cargo test --workspace --release
```

Artifacts:

| Artifact | Path |
| --- | --- |
| GUI | `target/release/u-monitor` (`.exe` on Windows) |
| Shared library | `target/release/u_monitor.dll` / `libu_monitor.so` / `libu_monitor.dylib` |
| C header | `crates/u-monitor-ffi/include/u_monitor.h` |

## Tag release (CI)

Push a version tag to trigger `.github/workflows/release.yml`:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

CI builds release binaries for Ubuntu, Windows, and macOS and uploads them as
workflow artifacts / GitHub Release assets.

## Pre-release checklist

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. Smoke the GUI: `cargo run -p u-monitor-ui --release`
5. Smoke FFI: `cargo build -p u-monitor-ffi --release` then
   `python examples/python/u_monitor.py`
6. Confirm Overview capability matrix matches the host OS
