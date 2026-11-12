# Release

Version is centralized in `[workspace.package] version` (`Cargo.toml`).

## Local release build

```powershell
cargo build --locked --release -p u-monitor-ui
cargo build --locked --release -p u-monitor-ffi
cargo test --workspace --locked --release
```

Artifacts:

| Artifact | Path |
| --- | --- |
| GUI | `target/release/u-monitor` (`.exe` on Windows) |
| Shared library | `target/release/u_monitor.dll` / `libu_monitor.so` / `libu_monitor.dylib` |
| C header | `crates/u-monitor-ffi/include/u_monitor.h` |

## Tag release (CI)

The CI and release workflows check out U-Monitor and U-Library into sibling
directories. Set the GitHub Actions repository variables
`U_LIBRARY_REPOSITORY` to the published library's `owner/repo` and
`U_LIBRARY_REVISION` to the full 40-character commit SHA to build. For a
private U-Library repository, set `U_LIBRARY_READ_TOKEN` to a token with
read access to its contents. GitHub withholds this secret from pull requests
from forks, so those runs require publicly readable library sources.
Record the selected library SHA for each release
so a rerun can restore it if the repository variable has since changed.

Push a version tag to trigger `.github/workflows/release.yml`:

```powershell
git tag v1.0.0
git push origin v1.0.0
```

CI builds release binaries for Ubuntu, Windows, and macOS and uploads them as
workflow artifacts / GitHub Release assets.
The macOS archive contains universal x86_64 and arm64 builds of both the GUI
and the shared library.

## Pre-release checklist

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. Smoke the GUI: `cargo run -p u-monitor-ui --release`
5. Smoke FFI: `cargo build -p u-monitor-ffi --release` then
   `python examples/python/u_monitor.py`
6. Confirm Overview capability matrix matches the host OS
