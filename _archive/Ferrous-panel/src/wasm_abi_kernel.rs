//! ============================================================================
//! FerrousPanel #![no_std] Rust WebAssembly ABI Kernel (`wasm32-unknown-unknown`)
//! ============================================================================
//! Compiled to native `.wasm` bytecode and executed directly inside the Node.js
//! V8 WebAssembly VM (`src/rust_core_manager.ts`) for deterministic, zero-GC
//! cryptographic FNV-1a 64-bit hashing, adaptive load-shedding computation,
//! token-bucket rate limiters, and ZRAM compression ratio estimation.
//! ============================================================================

#![no_std]

/// Computes a deterministic 64-bit FNV-1a hash over a memory buffer in linear WASM memory.
#[no_mangle]
pub extern "C" fn ferrous_fnv1a_hash64(ptr: *const u8, len: usize) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;

    let mut hash = FNV_OFFSET_BASIS;
    let mut i = 0usize;
    while i < len {
        let byte = unsafe { *ptr.add(i) } as u64;
        hash ^= byte;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

/// Computes an integer system pressure score (0..=100) from CPU usage (0..=100)
/// and RAM usage (0..=100) to drive adaptive HTTP 503 load shedding.
#[no_mangle]
pub extern "C" fn ferrous_compute_pressure_score(cpu_pct: u32, ram_pct: u32) -> u32 {
    let weighted = (cpu_pct.saturating_mul(60)).saturating_add(ram_pct.saturating_mul(40)) / 100;
    if weighted > 100 {
        100
    } else {
        weighted
    }
}

/// Estimates ZRAM compressed memory savings in MB using zstd ~2.9x compression ratio.
#[no_mangle]
pub extern "C" fn ferrous_estimate_zram_saved_mb(pool_mb: u32, utilization_pct: u32) -> u32 {
    let active_mb = (pool_mb.saturating_mul(utilization_pct)) / 100;
    (active_mb.saturating_mul(19)) / 10
}

/// Evaluates token-bucket rate limiting for FerrousX & Pingora Layer-7 WAF.
/// Returns 1 if allowed, 0 if rate-limited (HTTP 429).
#[no_mangle]
pub extern "C" fn ferrous_token_bucket_allow(
    current_rps: u32,
    limit_rps: u32,
    burst_allowance: u32,
) -> u32 {
    if current_rps <= limit_rps.saturating_add(burst_allowance) {
        1
    } else {
        0
    }
}
