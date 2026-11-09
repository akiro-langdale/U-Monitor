/* Build (after `cargo build -p u-monitor-ffi`):
 *
 * Windows (MSVC):
 *   cl /I..\..\crates\u-monitor-ffi\include main.c /Fe:u_monitor_demo.exe ^
 *      /link /LIBPATH:..\..\target\debug u_monitor.dll.lib
 *
 * Linux/macOS:
 *   cc -I../../crates/u-monitor-ffi/include main.c \
 *      -L../../target/debug -lu_monitor -Wl,-rpath,$PWD/../../target/debug \
 *      -o u_monitor_demo
 */

#include <stdio.h>
#include <stdint.h>

#include "u_monitor.h"

static const char *source_name(UmDataSource source) {
    switch (source) {
        case UM_SOURCE_MEASURED: return "MEASURED";
        case UM_SOURCE_CALCULATED: return "CALCULATED";
        case UM_SOURCE_SIMULATED: return "SIMULATED";
        default: return "?";
    }
}

int main(void) {
    char status_name[32];
    UmSystemIdentity identity;
    UmRamInfo ram;
    UmAvailability availability;
    UmHierarchy *hierarchy;
    UmAccessResult access;
    UmHierarchyStats stats;

    printf("u-monitor %s\n", um_version());

    if (um_system_identity(&identity) == UM_OK) {
        printf("identity: %s / %s (%s)\n",
               identity.operating_system,
               identity.process_architecture,
               source_name(identity.source));
    }

    if (um_check_availability(UM_FEATURE_RAM_METRICS, &availability) == UM_OK) {
        um_status_name((UmStatus)availability, status_name, sizeof status_name);
        printf("RAM feature availability code=%d\n", (int)availability);
    }

    if (um_ram_info(&ram) == UM_OK && ram.total_bytes.status == UM_OK) {
        printf("RAM total=%llu (%s)\n",
               (unsigned long long)ram.total_bytes.value,
               source_name(ram.total_bytes.source));
        if (ram.used_bytes.status == UM_OK) {
            printf("RAM used=%llu (%s)\n",
                   (unsigned long long)ram.used_bytes.value,
                   source_name(ram.used_bytes.source));
        }
    }

    hierarchy = um_hierarchy_create();
    if (!hierarchy) {
        fprintf(stderr, "hierarchy create failed\n");
        return 1;
    }
    um_hierarchy_access(hierarchy, 0x1000, &access);
    printf("sim first access hit=%u level=%u latency=%llu (%s)\n",
           access.hit,
           access.resolved_level,
           (unsigned long long)access.latency_ns,
           source_name(access.source));
    um_hierarchy_access(hierarchy, 0x1000, &access);
    printf("sim second access hit=%u level=%u latency=%llu (%s)\n",
           access.hit,
           access.resolved_level,
           (unsigned long long)access.latency_ns,
           source_name(access.source));
    um_hierarchy_stats(hierarchy, &stats);
    printf("sim stats accesses=%llu l1_hits=%llu (%s)\n",
           (unsigned long long)stats.accesses,
           (unsigned long long)stats.l1_hits,
           source_name(stats.source));
    um_hierarchy_destroy(hierarchy);
    return 0;
}
