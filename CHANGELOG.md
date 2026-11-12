# Changelog

## 1.0.0

First stable release after the Processes → Quality roadmap.

- Process/thread enumeration with inspection target and consented register capture
- Hardware cache counters when available; honest UNAVAILABLE otherwise
- Live storage IO rates and read-only file HEX view
- Simulation seeded from measured cache topology, plus disk model
- Deeper macOS memory maps and disk discovery
- Cross-panel selection with provenance-aware JSON export
- Wider FFI (cache topology, memory samples) and ctypes package
- Tracing with optional file sink; fuzz/jitter quality gates

## 0.1.0

Initial development release of U-Monitor.

- GUI panels: Overview, CPU, RAM, Memory, Storage, Registers, Cache, Simulation
- Provenance labels: MEASURED / CALCULATED / SIMULATED
- Platform backends for Windows and Linux (macOS partial via sysinfo)
- Background telemetry worker for live CPU/RAM charts
- Educational L1→L2→RAM simulation
- Stable C ABI with C and Python examples
- CI on Ubuntu / Windows / macOS and tagged release builds
