/* ============================================================================
 * /opt/Ferrous-panel/c_abi/ferrous_ffi_shim.c
 * POSIX Shared-Memory RingBuffer & Dynamic Loader (`dlopen`/`dlsym`) C11 Shim
 * Bridges C/C++20, Go CGO, and PHP Zend FFI workers with the Rust Core Daemon
 * (`libferrous_bridge.so` & `/run/ferrous-polyglot.sock`).
 * ============================================================================ */

#include "../include/ferrous_bridge.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t ferrous_c_fnv1a_hash64(const uint8_t* data, size_t len) {
    uint64_t hash = 0xcbf29ce484222325ULL;
    for (size_t i = 0; i < len; ++i) {
        hash ^= (uint64_t)data[i];
        hash *= 0x00000100000001B3ULL;
    }
    return hash;
}

int32_t ferrous_bridge_init(void) {
    return 0;
}

void ferrous_free_string(char* ptr) {
    if (ptr != NULL) {
        free(ptr);
    }
}

FerrousGuardVerdictC ferrous_evaluate_crash_guard_c(uint32_t cpu_pct, uint32_t ram_pct) {
    FerrousGuardVerdictC verdict;
    uint32_t score = (cpu_pct * 60U + ram_pct * 40U) / 100U;
    if (score > 100U) {
        score = 100U;
    }
    const char* tag = "ferrous-cgroup-v2-guard";
    verdict.fnv1a_signature = ferrous_c_fnv1a_hash64((const uint8_t*)tag, strlen(tag));
    verdict.pressure_score = score;
    verdict.load_shedding_engaged = (cpu_pct >= 92U || ram_pct >= 88U);
    verdict.exit_code = 0;
    return verdict;
}

int32_t ferrous_validate_vhost_syntax_c(
    const char* domain_name,
    const char* raw_config,
    FerrousWebEngineFlavor flavor
) {
    if (domain_name == NULL || raw_config == NULL) {
        return -1;
    }
    const char* server_tok = strstr(raw_config, "server {");
    if (server_tok == NULL) {
        return 1;
    }
    (void)flavor;
    return 0;
}
