# Security model

U-Monitor is a local diagnostic tool. It does not elevate privileges and does
not inspect other processes in the current release.

## Trust boundaries

- **Current process only.** Virtual memory maps and HEX samples come from the
  U-Monitor process (`VirtualQuery` / `/proc/self/*`). Foreign-process reads
  remain `NOT_IMPLEMENTED`.
- **Read-only memory samples.** HEX reads are capped (512 bytes) and require
  committed + readable protection. The adapter never writes into process
  memory.
- **Registers.** Only the calling UI thread is captured where implemented
  (Windows x86-64). Other threads need debug APIs and are out of scope.
- **Simulation.** Cache hierarchy models never claim to be hardware state and
  are always labelled `SIMULATED`.
- **C ABI.** Null arguments are rejected. Opaque handles must be destroyed
  exactly once by the caller. Buffer outputs require a non-zero capacity and
  are NUL-terminated on success.

## What this release does not do

- No kernel drivers
- No credential or secret scanning
- No remote telemetry
- No automatic privilege elevation

Report security issues privately to the repository maintainers.
