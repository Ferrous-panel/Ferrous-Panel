-- ============================================================================
-- FerrousPanel Multi-Engine SQL Cluster Schema & Performance Tuning Suite
-- Engines: PostgreSQL 17, MariaDB 11.4 LTS, SQLite 3 (WAL Mode)
-- ============================================================================

-- 1.Managed Websites & Polyglot Runtime Registry
CREATE TABLE IF NOT EXISTS ferrous_managed_sites (
    site_id VARCHAR(64) PRIMARY KEY,
    site_name VARCHAR(128) NOT NULL,
    primary_domain VARCHAR(255) NOT NULL UNIQUE,
    web_server_engine VARCHAR(64) NOT NULL DEFAULT 'FerrousX (Rust Web Server)',
    runtime_framework VARCHAR(128) NOT NULL,
    internal_port INTEGER NOT NULL CHECK (internal_port BETWEEN 1 AND 65535),
    working_dir VARCHAR(512) NOT NULL,
    run_sub_dir VARCHAR(128) NOT NULL DEFAULT '/',
    workers INTEGER NOT NULL DEFAULT 4,
    memory_limit_mb INTEGER NOT NULL DEFAULT 512,
    rate_limit_rps INTEGER NOT NULL DEFAULT 120,
    ssl_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    http3_quic_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    status VARCHAR(32) NOT NULL DEFAULT 'running',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_ferrous_sites_engine
    ON ferrous_managed_sites (web_server_engine, status);

-- 2. Rust Binary Execution Audit Trail (Immutable Syscall Ledger)
CREATE TABLE IF NOT EXISTS ferrous_rust_audit_ledger (
    request_id VARCHAR(64) PRIMARY KEY,
    subsystem VARCHAR(32) NOT NULL,
    operation VARCHAR(128) NOT NULL,
    transport_mode VARCHAR(64) NOT NULL,
    binary_path VARCHAR(512) NOT NULL,
    exit_code INTEGER NOT NULL DEFAULT 0,
    execution_time_us BIGINT NOT NULL,
    syscall_summary TEXT NOT NULL,
    recorded_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_ferrous_audit_subsystem
    ON ferrous_rust_audit_ledger (subsystem, recorded_at DESC);

-- 3. Anti-Hacking eBPF/XDP & Pingora Layer-7 WAF Event Log
CREATE TABLE IF NOT EXISTS ferrous_waf_security_events (
    event_id VARCHAR(64) PRIMARY KEY,
    rule_code VARCHAR(64) NOT NULL,
    client_ip VARCHAR(64) NOT NULL,
    country_iso VARCHAR(8) NOT NULL DEFAULT 'UN',
    target_uri VARCHAR(512) NOT NULL,
    mitigation_action VARCHAR(32) NOT NULL DEFAULT 'XDP_DROP',
    detected_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 4. Anti-Crash Cgroups v2 & OOM Immunity Policies
CREATE TABLE IF NOT EXISTS ferrous_cgroup_oom_policies (
    service_unit VARCHAR(128) PRIMARY KEY,
    oom_score_adj INTEGER NOT NULL CHECK (oom_score_adj BETWEEN -1000 AND 1000),
    memory_high_pct INTEGER NOT NULL DEFAULT 85,
    memory_max_pct INTEGER NOT NULL DEFAULT 92,
    auto_heal_enabled BOOLEAN NOT NULL DEFAULT TRUE
);
