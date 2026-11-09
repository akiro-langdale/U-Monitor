/**
 * U-Monitor C ABI
 *
 * Ownership:
 * - Caller-provided output structs need no free.
 * - Opaque UmHierarchy pointers from um_hierarchy_create must be released with
 *   um_hierarchy_destroy. Passing NULL to destroy is safe.
 * - String fields written by the library are UTF-8 and NUL-terminated.
 *
 * Provenance:
 * - MEASURED / CALCULATED come from platform adapters.
 * - Simulation entry points always report UM_SOURCE_SIMULATED.
 */

#ifndef U_MONITOR_H
#define U_MONITOR_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stddef.h>

#if defined(_WIN32) && defined(UM_MONITOR_DLL)
#  ifdef UM_MONITOR_BUILD
#    define UM_API __declspec(dllexport)
#  else
#    define UM_API __declspec(dllimport)
#  endif
#else
#  define UM_API
#endif

typedef enum UmStatus {
    UM_OK = 0,
    UM_ERR_NULL_ARGUMENT = 1,
    UM_ERR_BUFFER_TOO_SMALL = 2,
    UM_ERR_UNAVAILABLE = 3,
    UM_ERR_NOT_IMPLEMENTED = 4,
    UM_ERR_PERMISSION_REQUIRED = 5,
    UM_ERR_PLATFORM = 6,
    UM_ERR_VALIDATION = 7,
    UM_ERR_INTERNAL = 8
} UmStatus;

typedef enum UmDataSource {
    UM_SOURCE_MEASURED = 1,
    UM_SOURCE_CALCULATED = 2,
    UM_SOURCE_SIMULATED = 3
} UmDataSource;

typedef enum UmAvailability {
    UM_AVAIL_SUPPORTED = 1,
    UM_AVAIL_PERMISSION_REQUIRED = 2,
    UM_AVAIL_UNAVAILABLE = 3,
    UM_AVAIL_NOT_IMPLEMENTED = 4,
    UM_AVAIL_ERROR = 5
} UmAvailability;

typedef enum UmFeature {
    UM_FEATURE_SYSTEM_IDENTITY = 1,
    UM_FEATURE_CPU_DISCOVERY = 2,
    UM_FEATURE_CPU_METRICS = 3,
    UM_FEATURE_RAM_METRICS = 4,
    UM_FEATURE_VIRTUAL_MEMORY_MAP = 5,
    UM_FEATURE_MEMORY_BYTES = 6,
    UM_FEATURE_STORAGE_DISCOVERY = 7,
    UM_FEATURE_DISK_STORAGE = 8,
    UM_FEATURE_HARDWARE_REGISTERS = 9,
    UM_FEATURE_CURRENT_THREAD_REGISTERS = 10,
    UM_FEATURE_CACHE_TOPOLOGY = 11
} UmFeature;

typedef struct UmFieldU64 {
    UmStatus status;
    UmDataSource source;
    uint64_t value;
    int64_t timestamp_unix_ms;
} UmFieldU64;

typedef struct UmSystemIdentity {
    UmStatus status;
    UmDataSource source;
    int64_t timestamp_unix_ms;
    char operating_system[64];
    char process_architecture[64];
} UmSystemIdentity;

typedef struct UmRamInfo {
    UmStatus status;
    UmFieldU64 total_bytes;
    UmFieldU64 available_bytes;
    UmFieldU64 used_bytes;
} UmRamInfo;

typedef enum UmCacheKind {
    UM_CACHE_DATA = 1,
    UM_CACHE_INSTRUCTION = 2,
    UM_CACHE_UNIFIED = 3,
    UM_CACHE_TRACE = 4,
    UM_CACHE_UNKNOWN = 5
} UmCacheKind;

typedef struct UmFieldU32 {
    UmStatus status;
    UmDataSource source;
    uint32_t value;
    int64_t timestamp_unix_ms;
} UmFieldU32;

typedef struct UmCacheNode {
    uint32_t level;
    UmCacheKind kind;
    UmFieldU64 size_bytes;
    UmFieldU32 line_size_bytes;
    UmFieldU32 associativity;
    uint8_t fully_associative;
} UmCacheNode;

typedef struct UmCacheTopologyHeader {
    UmStatus status;
    UmDataSource source;
    int64_t timestamp_unix_ms;
    size_t node_count;
} UmCacheTopologyHeader;

typedef struct UmMemorySampleInfo {
    UmStatus status;
    UmDataSource source;
    uint64_t address;
    size_t bytes_read;
    int64_t timestamp_unix_ms;
} UmMemorySampleInfo;

typedef struct UmHierarchy UmHierarchy;

typedef struct UmAccessResult {
    UmStatus status;
    UmDataSource source;
    uint64_t address;
    uint64_t line_address;
    uint32_t resolved_level; /* 1=L1, 2=L2, 3=RAM */
    uint8_t hit;
    uint64_t latency_ns;
    uint32_t step_count;
} UmAccessResult;

typedef struct UmHierarchyStats {
    UmStatus status;
    UmDataSource source;
    uint64_t accesses;
    uint64_t l1_hits;
    uint64_t l2_hits;
    uint64_t ram_fills;
    uint64_t evictions;
    uint64_t total_latency_ns;
} UmHierarchyStats;

UM_API const char *um_version(void);
UM_API UmStatus um_status_name(UmStatus status, char *buffer, size_t capacity);
UM_API UmStatus um_system_identity(UmSystemIdentity *out);
UM_API UmStatus um_check_availability(int feature, UmAvailability *out);
UM_API UmStatus um_ram_info(UmRamInfo *out);
UM_API UmStatus um_cache_topology(UmCacheTopologyHeader *header, UmCacheNode *nodes,
                                    size_t capacity);
UM_API UmStatus um_memory_sample(uint64_t address, size_t length, uint8_t *buffer, size_t capacity,
                                 UmMemorySampleInfo *out);

UM_API UmHierarchy *um_hierarchy_create(void);
UM_API void um_hierarchy_destroy(UmHierarchy *handle);
UM_API UmStatus um_hierarchy_reset(UmHierarchy *handle);
UM_API UmStatus um_hierarchy_access(UmHierarchy *handle, uint64_t address, UmAccessResult *out);
UM_API UmStatus um_hierarchy_stats(const UmHierarchy *handle, UmHierarchyStats *out);

#ifdef __cplusplus
}
#endif

#endif /* U_MONITOR_H */
