# Architecture

U-Monitor is a Cargo workspace with a strict separation between measured
platform data, calculated fields, and simulated models.

```text
u-monitor-ui          egui application (composition root)
u-monitor-telemetry   background sampling worker + history
u-monitor-platform    OS adapters (Windows / Linux / …)
u-monitor-simulation  educational memory-hierarchy models
u-monitor-ffi         stable C ABI over platform + simulation
u-monitor-core        shared contracts and result types
```

## Data provenance

Every successful value is wrapped in `Measurement` with an explicit
`DataSource`:

| Source | Meaning |
| --- | --- |
| `MEASURED` | Reported by the OS or hardware API |
| `CALCULATED` | Derived from other values (for example used = total − available) |
| `SIMULATED` | Produced only by `u-monitor-simulation` / FFI hierarchy APIs |

Unavailable data stays unavailable. Zeros are never used as stand-ins for
missing MEASURED fields.

## Platform boundary

`NativePlatformAdapter` discovers what the current binary can read.
`StubPlatformAdapter` returns `NOT_IMPLEMENTED` for every feature and must
never invent values. Live CPU/RAM sampling uses `NativeTelemetryAdapter`
owned by `TelemetryWorker`, not the request/response discovery path.

## FFI boundary

`u-monitor-ffi` exposes a C ABI (`u_monitor`). Caller-owned POD structs need
no free. Opaque `UmHierarchy*` handles use create/destroy pairs. Entry points
catch panics and map them to `UM_ERR_INTERNAL`.
