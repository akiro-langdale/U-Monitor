"""ctypes bindings for the U-Monitor C ABI.

Load the shared library produced by `cargo build -p u-monitor-ffi`:
  Windows: target/debug/u_monitor.dll
  Linux:   target/debug/libu_monitor.so
  macOS:   target/debug/libu_monitor.dylib
"""

from __future__ import annotations

import ctypes
import sys
from ctypes import (
    POINTER,
    Structure,
    byref,
    c_char,
    c_char_p,
    c_int,
    c_int64,
    c_uint32,
    c_uint64,
    c_uint8,
    c_void_p,
)
from pathlib import Path


UM_OK = 0
UM_SOURCE_MEASURED = 1
UM_SOURCE_CALCULATED = 2
UM_SOURCE_SIMULATED = 3
UM_FEATURE_SYSTEM_IDENTITY = 1
UM_FEATURE_RAM_METRICS = 4


class UmFieldU64(Structure):
    _fields_ = [
        ("status", c_int),
        ("source", c_int),
        ("value", c_uint64),
        ("timestamp_unix_ms", c_int64),
    ]


class UmSystemIdentity(Structure):
    _fields_ = [
        ("status", c_int),
        ("source", c_int),
        ("timestamp_unix_ms", c_int64),
        ("operating_system", c_char * 64),
        ("process_architecture", c_char * 64),
    ]


class UmRamInfo(Structure):
    _fields_ = [
        ("status", c_int),
        ("total_bytes", UmFieldU64),
        ("available_bytes", UmFieldU64),
        ("used_bytes", UmFieldU64),
    ]


class UmAccessResult(Structure):
    _fields_ = [
        ("status", c_int),
        ("source", c_int),
        ("address", c_uint64),
        ("line_address", c_uint64),
        ("resolved_level", c_uint32),
        ("hit", c_uint8),
        ("latency_ns", c_uint64),
        ("step_count", c_uint32),
    ]


class UmHierarchyStats(Structure):
    _fields_ = [
        ("status", c_int),
        ("source", c_int),
        ("accesses", c_uint64),
        ("l1_hits", c_uint64),
        ("l2_hits", c_uint64),
        ("ram_fills", c_uint64),
        ("evictions", c_uint64),
        ("total_latency_ns", c_uint64),
    ]


class UmFieldU32(Structure):
    _fields_ = [
        ("status", c_int),
        ("source", c_int),
        ("value", c_uint32),
        ("timestamp_unix_ms", c_int64),
    ]


class UmCacheNode(Structure):
    _fields_ = [
        ("level", c_uint32),
        ("kind", c_int),
        ("size_bytes", UmFieldU64),
        ("line_size_bytes", UmFieldU32),
        ("associativity", UmFieldU32),
        ("fully_associative", c_uint8),
    ]


class UmCacheTopologyHeader(Structure):
    _fields_ = [
        ("status", c_int),
        ("source", c_int),
        ("timestamp_unix_ms", c_int64),
        ("node_count", c_uint64),
    ]


class UmMemorySampleInfo(Structure):
    _fields_ = [
        ("status", c_int),
        ("source", c_int),
        ("address", c_uint64),
        ("bytes_read", c_uint64),
        ("timestamp_unix_ms", c_int64),
    ]


def _library_name() -> str:
    if sys.platform.startswith("win"):
        return "u_monitor.dll"
    if sys.platform == "darwin":
        return "libu_monitor.dylib"
    return "libu_monitor.so"


def load_library(path: Path | None = None) -> ctypes.CDLL:
    if path is None:
        root = Path(__file__).resolve().parents[2]
        name = _library_name()
        candidates = [
            root / "target" / "debug" / name,
            root / "target" / "release" / name,
        ]
        cargo_target = Path(__import__("os").environ.get("CARGO_TARGET_DIR", ""))
        if cargo_target.name:
            candidates.extend(
                [
                    cargo_target / "debug" / name,
                    cargo_target / "release" / name,
                ]
            )
        for candidate in candidates:
            if candidate.exists():
                path = candidate
                break
        else:
            raise FileNotFoundError(
                f"shared library not found; build with `cargo build -p u-monitor-ffi` "
                f"(looked for {name})"
            )
    lib = ctypes.CDLL(str(path))
    lib.um_version.restype = c_char_p
    lib.um_system_identity.argtypes = [POINTER(UmSystemIdentity)]
    lib.um_system_identity.restype = c_int
    lib.um_check_availability.argtypes = [c_int, POINTER(c_int)]
    lib.um_check_availability.restype = c_int
    lib.um_ram_info.argtypes = [POINTER(UmRamInfo)]
    lib.um_ram_info.restype = c_int
    lib.um_hierarchy_create.restype = c_void_p
    lib.um_hierarchy_destroy.argtypes = [c_void_p]
    lib.um_hierarchy_access.argtypes = [c_void_p, c_uint64, POINTER(UmAccessResult)]
    lib.um_hierarchy_access.restype = c_int
    lib.um_hierarchy_stats.argtypes = [c_void_p, POINTER(UmHierarchyStats)]
    lib.um_hierarchy_stats.restype = c_int
    lib.um_cache_topology.argtypes = [
        POINTER(UmCacheTopologyHeader),
        POINTER(UmCacheNode),
        c_uint64,
    ]
    lib.um_cache_topology.restype = c_int
    lib.um_memory_sample.argtypes = [
        c_uint64,
        c_uint64,
        POINTER(c_uint8),
        c_uint64,
        POINTER(UmMemorySampleInfo),
    ]
    lib.um_memory_sample.restype = c_int
    return lib


def source_name(code: int) -> str:
    return {
        UM_SOURCE_MEASURED: "MEASURED",
        UM_SOURCE_CALCULATED: "CALCULATED",
        UM_SOURCE_SIMULATED: "SIMULATED",
    }.get(code, f"UNKNOWN({code})")


def main() -> None:
    lib = load_library()
    print("u-monitor", lib.um_version().decode("utf-8"))

    identity = UmSystemIdentity()
    status = lib.um_system_identity(byref(identity))
    if status == UM_OK:
        print(
            "identity:",
            identity.operating_system.decode("utf-8"),
            "/",
            identity.process_architecture.decode("utf-8"),
            f"({source_name(identity.source)})",
        )

    header = UmCacheTopologyHeader()
    if lib.um_cache_topology(byref(header), None, 0) == UM_OK and header.status == UM_OK:
        print("cache nodes:", header.node_count, f"({source_name(header.source)})")
        if header.node_count:
            nodes = (UmCacheNode * header.node_count)()
            if lib.um_cache_topology(byref(header), nodes, header.node_count) == UM_OK:
                first = nodes[0]
                print(
                    "cache L",
                    first.level,
                    "kind=",
                    first.kind,
                    f"({source_name(header.source)})",
                )

    ram = UmRamInfo()
    if lib.um_ram_info(byref(ram)) == UM_OK and ram.total_bytes.status == UM_OK:
        print(
            "RAM total=",
            ram.total_bytes.value,
            f"({source_name(ram.total_bytes.source)})",
        )
        if ram.used_bytes.status == UM_OK:
            print(
                "RAM used=",
                ram.used_bytes.value,
                f"({source_name(ram.used_bytes.source)})",
            )

    handle = lib.um_hierarchy_create()
    if not handle:
        raise RuntimeError("um_hierarchy_create failed")
    try:
        access = UmAccessResult()
        lib.um_hierarchy_access(handle, 0x1000, byref(access))
        print(
            "sim first:",
            f"hit={access.hit}",
            f"level={access.resolved_level}",
            f"latency={access.latency_ns}",
            f"({source_name(access.source)})",
        )
        lib.um_hierarchy_access(handle, 0x1000, byref(access))
        print(
            "sim second:",
            f"hit={access.hit}",
            f"level={access.resolved_level}",
            f"latency={access.latency_ns}",
            f"({source_name(access.source)})",
        )
        stats = UmHierarchyStats()
        lib.um_hierarchy_stats(handle, byref(stats))
        print(
            "sim stats:",
            f"accesses={stats.accesses}",
            f"l1_hits={stats.l1_hits}",
            f"({source_name(stats.source)})",
        )
    finally:
        lib.um_hierarchy_destroy(handle)


if __name__ == "__main__":
    main()
