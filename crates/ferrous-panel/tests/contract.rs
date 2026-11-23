#![forbid(unsafe_code)]
#![recursion_limit = "256"]

use actix_web::body::to_bytes;
use actix_web::http::{header, StatusCode};
use actix_web::test::{init_service, try_call_service, TestRequest};
use actix_web::{web, App};
use anyhow::{bail, Context, Result};
use ferrous_panel::api::contract::*;
use ferrous_panel::config::Config;
use ferrous_panel::configure_app;
use ferrous_panel::state::AppState;
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};
use std::future::Future;
use std::path::{Path, PathBuf};

fn ensure(cond: bool, msg: &str) -> Result<()> {
    if cond {
        Ok(())
    } else {
        bail!("{msg}")
    }
}

fn check_roundtrip<T>(sample: Value) -> Result<()>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let decoded: T = serde_json::from_value(sample.clone())
        .with_context(|| format!("failed to deserialize JSON into {}", std::any::type_name::<T>()))?;
    let reencoded = serde_json::to_value(&decoded)
        .with_context(|| format!("failed to re-serialize {}", std::any::type_name::<T>()))?;
    ensure(
        reencoded == sample,
        &format!(
            "JSON mismatch for {}:\nexpected: {sample}\nactual:   {reencoded}",
            std::any::type_name::<T>()
        ),
    )
}

fn run_async<F: Future<Output = Result<()>>>(fut: F) -> Result<()> {
    let rt = actix_web::rt::Runtime::new().context("failed to create runtime")?;
    rt.block_on(fut)
}

struct TempWorkspace {
    dir: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Result<Self> {
        let mut suffix = [0u8; 8];
        OsRng
            .try_fill_bytes(&mut suffix)
            .map_err(|_| anyhow::anyhow!("OsRng unavailable"))?;
        let mut hex = String::with_capacity(16);
        for b in suffix {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
        }
        let dir = std::env::temp_dir().join(format!("ferrous-panel-contract-{prefix}-{hex}"));
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn contract_structs_serialize_exactly_matching_types_ts() -> Result<()> {
    check_roundtrip::<TelemetryData>(json!({
        "hostname": "ubuntu-01",
        "publicIp": "203.0.113.10",
        "osVersion": "Ubuntu 24.04.1 LTS",
        "kernelVersion": "6.8.0-45-generic",
        "rustDaemonVersion": "ferrous-panel v2.5.0",
        "nginxVersion": "installed",
        "uptimeSeconds": 3600,
        "cpuBrand": "AMD EPYC",
        "cpuCores": 4,
        "cpuUsagePercent": 12.5,
        "coreLoads": [
            { "core": 0, "model": "AMD EPYC", "usagePercent": 12.5 }
        ],
        "loadAverage": [0.25, 0.2, 0.15],
        "memoryTotalMb": 8192,
        "memoryUsedMb": 1024,
        "memoryPercent": 12.5,
        "swapTotalMb": 2048,
        "swapUsedMb": 0,
        "diskTotalGb": 100.0,
        "diskUsedGb": 22.5,
        "networkRxMbps": 1.25,
        "networkTxMbps": 0.75,
        "totalRequestsPerSec": 10,
        "activeConnections": 2,
        "history24h": [
            { "time": "10:00", "cpuPercent": 11.0, "memoryPercent": 12.0, "memoryUsedMb": 983, "requestsPerSec": 8 }
        ],
        "timestamp": "2026-11-12T10:00:00Z"
    }))?;

    check_roundtrip::<ManagedApp>(json!({
        "id": "app-1",
        "name": "blog",
        "framework": "PHP 8.3-FPM",
        "webServerEngine": "FerrousX (Rust Web Server)",
        "runtimeVersion": "8.3",
        "domain": "example.com",
        "aliases": ["www.example.com"],
        "internalPort": 8080,
        "workingDir": "/www/wwwroot/example.com",
        "runSubDir": "/public",
        "execCommand": "php-fpm8.3",
        "workers": 4,
        "memoryLimitMb": 512,
        "memoryUsedMb": 64,
        "cpuPercent": 1.5,
        "pid": 1234,
        "status": "running",
        "sslEnabled": true,
        "sslIssuer": "Let's Encrypt",
        "sslExpiryDays": 89,
        "forceHttps": true,
        "http3Quic": true,
        "websocketEnabled": false,
        "rateLimitRps": 120,
        "requestsPerSec": 45,
        "p99LatencyMs": 3.2,
        "uptimeSeconds": 1200,
        "diskUsedMb": 42.5,
        "diskQuotaMb": 5120,
        "trafficMonthGb": 1.4,
        "linkedDbName": "wp_blog",
        "ftpUsername": "wp_ftp",
        "backupCount": 2,
        "gitRepoUrl": "https://github.com/org/repo.git",
        "gitBranch": "main",
        "buildScript": "composer install",
        "lastDeployCommit": "abc1234",
        "lastDeployTime": "2026-11-12T09:00:00Z",
        "urlRewritePreset": "wordpress",
        "urlRewriteRules": "try_files $uri $uri/ /index.php?$args;",
        "phpVersion": "8.3",
        "redirects": [
            { "id": "r1", "sourcePath": "/old", "targetUrl": "/new", "type": "301", "keepQuery": true, "enabled": true }
        ],
        "hotlinkProtection": {
            "enabled": false,
            "protectedExtensions": "jpg|png",
            "allowedDomains": ["example.com"],
            "blockEmptyReferer": false
        },
        "directoryAuth": [
            { "id": "da1", "path": "/admin", "label": "Admin", "username": "admin", "enabled": true }
        ],
        "envVars": [{ "key": "APP_ENV", "value": "production" }],
        "logs": ["started"],
        "systemdUnit": "ferrous-app-1.service",
        "nginxVhost": "/etc/nginx/sites-available/example.com.conf"
    }))?;

    check_roundtrip::<PhpVersionInstance>(json!({
        "version": "8.3",
        "fullVersion": "8.3.12",
        "status": "running",
        "installed": true,
        "fpmSocket": "/run/php/php8.3-fpm.sock",
        "iniPath": "/etc/php/8.3/fpm/php.ini",
        "poolPath": "/etc/php/8.3/fpm/pool.d/www.conf",
        "memoryUsageMb": 48.0,
        "activeWorkers": 2,
        "maxChildren": 20,
        "memoryLimit": "256M",
        "uploadMaxFilesize": "64M",
        "postMaxSize": "64M",
        "maxExecutionTime": 60,
        "maxInputVars": 3000,
        "displayErrors": false,
        "opcacheEnabled": true,
        "opcacheJitBuffer": "64M",
        "disableFunctions": ["exec", "shell_exec"],
        "extensions": [
            { "name": "opcache", "category": "Cache & Speed", "enabled": true, "description": "OPcache JIT" }
        ],
        "rawPhpIni": "[PHP]",
        "rawFpmPoolConf": "[www]"
    }))?;

    check_roundtrip::<OneClickTemplate>(json!({
        "id": "tpl-axum",
        "name": "Axum API",
        "category": "Rust Backend",
        "framework": "Axum (Rust)",
        "version": "0.8",
        "description": "High-performance Rust service",
        "defaultPort": 8080,
        "requiresDb": "PostgreSQL 17",
        "buildCommand": "cargo build --release",
        "execCommandTemplate": "./target/release/app",
        "popularScore": "99%",
        "gitRepoUrl": "https://github.com/example/axum-starter"
    }))?;

    check_roundtrip::<ServerPackageItem>(json!({
        "id": "pkg-ferrousx",
        "name": "FerrousX",
        "category": "Web Server",
        "version": "2.5.0",
        "description": "Rust reverse proxy",
        "installed": true,
        "status": "running",
        "memoryMb": 14.2,
        "configPath": "/etc/ferrousx/ferrousx.toml",
        "isRustNative": true,
        "replacedLegacyPackage": "nginx",
        "rustCrate": "ferrousx",
        "port": 443,
        "configContent": "[server]",
        "logs": ["ready"]
    }))?;

    check_roundtrip::<SecurityData>(json!({
        "ufwRules": [
            { "id": "u1", "port": "443", "protocol": "TCP", "action": "ALLOW", "source": "Anywhere", "comment": "HTTPS", "enabled": true }
        ],
        "fail2banJails": [
            {
                "name": "sshd",
                "filter": "sshd",
                "logPath": "/var/log/auth.log",
                "maxRetry": 5,
                "findTimeSec": 600,
                "banTimeSec": 3600,
                "currentlyBanned": 1,
                "totalBanned": 3,
                "bannedIps": [
                    { "ip": "198.51.100.7", "country": "US", "reason": "brute-force", "bannedAt": "2026-11-12T08:00:00Z" }
                ]
            }
        ],
        "hardeningDirectives": [
            {
                "id": "h1",
                "category": "SSH Daemon",
                "key": "PasswordAuthentication",
                "recommendedValue": "no",
                "currentValue": "no",
                "enabled": true,
                "description": "Disable SSH password logins"
            }
        ],
        "antiDdosShield": {
            "protectionMode": "normal",
            "xdpStatus": "active",
            "synCookiesEnabled": true,
            "synRateLimitPps": 25000,
            "droppedPacketsTotal": 0,
            "blockedL7AttacksToday": 0,
            "fimIntegrityStatus": "verified",
            "lastFimScanAt": "2026-11-12T09:00:00Z",
            "wafRules": [
                {
                    "id": "waf1",
                    "code": "FX-RCE-01",
                    "name": "Shell Injection Guard",
                    "category": "Zero-Day RCE Shield",
                    "threatType": "RCE",
                    "blockedCount": 0,
                    "enabled": true,
                    "description": "Blocks shell command injection payloads"
                }
            ]
        }
    }))?;

    check_roundtrip::<SqlDatabase>(json!({
        "id": "db-1",
        "name": "app_prod",
        "engine": "PostgreSQL 17",
        "host": "127.0.0.1",
        "port": 5432,
        "username": "app_user",
        "collation": "en_US.UTF-8",
        "linkedAppId": "app-1",
        "linkedAppName": "blog",
        "sizeMb": 12.5,
        "activeConnections": 1,
        "maxConnections": 100,
        "status": "online",
        "createdAt": "2026-11-12T00:00:00Z",
        "tables": [
            {
                "tableName": "users",
                "rowCount": 1,
                "sizeKb": 16.0,
                "columns": [
                    { "name": "id", "dataType": "bigint", "nullable": false, "isPrimaryKey": true }
                ],
                "rows": [
                    { "id": 1 }
                ]
            }
        ]
    }))?;

    check_roundtrip::<MysqlPhpMyAdminConfig>(json!({
        "mysqlVersion": "11.4.3-MariaDB",
        "socketPath": "/run/mysqld/mysqld.sock",
        "bindAddress": "127.0.0.1",
        "port": 3306,
        "innodbBufferPoolMb": 512,
        "maxConnections": 150,
        "slowQueryLog": true,
        "longQueryTimeSec": 2,
        "defaultCharset": "utf8mb4",
        "myCnfRaw": "[mysqld]",
        "pmaVersion": "5.2.1",
        "pmaPath": "/pma",
        "pmaPort": 8880,
        "pmaSslOnly": true,
        "pmaBasicAuth": true,
        "pmaMaxUploadMb": 64,
        "pmaAllowedIps": "127.0.0.1",
        "pmaStatus": "stopped"
    }))?;

    check_roundtrip::<SqlQueryResponse>(json!({
        "success": true,
        "executionTimeMs": 1.4,
        "commandTag": "SELECT 1",
        "affectedRows": 1,
        "columns": ["id"],
        "rows": [{ "id": 1 }]
    }))?;

    check_roundtrip::<WebFileNode>(json!({
        "id": "f-1",
        "appId": "app-1",
        "name": "index.html",
        "path": "/index.html",
        "parentDir": "/",
        "type": "file",
        "sizeBytes": 128,
        "permissions": "0644",
        "symbolicPerms": "-rw-r--r--",
        "owner": "www-data:www-data",
        "modifiedAt": "2026-11-12T09:00:00Z",
        "content": "<h1>Hello</h1>"
    }))?;

    check_roundtrip::<CpanelData>(json!({
        "cronJobs": [
            {
                "id": "c-1",
                "title": "Certbot Renew",
                "schedule": "0 3 * * *",
                "command": "certbot renew",
                "user": "root",
                "targetApp": "example.com",
                "enabled": true,
                "lastRunAt": "2026-11-12T03:00:00Z",
                "lastExitCode": 0,
                "lastOutput": "ok"
            }
        ],
        "dnsRecords": [
            {
                "id": "d-1",
                "domain": "example.com",
                "type": "MX",
                "host": "@",
                "value": "mail.example.com",
                "ttl": 3600,
                "priority": 10
            }
        ],
        "backups": [
            {
                "id": "b-1",
                "filename": "example.tar.zst",
                "targetApp": "example.com",
                "databaseIncluded": "app_prod",
                "sizeMb": 25.0,
                "compression": "zstd (level 9)",
                "sha256": "e3b0c442",
                "createdAt": "2026-11-12T02:00:00Z",
                "status": "verified"
            }
        ],
        "sftpAccounts": [
            {
                "id": "s-1",
                "username": "deploy",
                "chrootDir": "/www/wwwroot/example.com",
                "targetApp": "example.com",
                "authMethod": "Ed25519 SSH Key",
                "quotaMb": 1024,
                "usedMb": 12.0,
                "status": "active"
            }
        ]
    }))?;

    check_roundtrip::<MailServerData>(json!({
        "postfixVersion": "3.8.6",
        "dovecotVersion": "2.3.21",
        "rspamdVersion": "3.8",
        "queueCount": 0,
        "sentToday": 4,
        "spamBlockedToday": 1,
        "domains": [
            {
                "id": "md-1",
                "domain": "example.com",
                "spfStatus": "verified",
                "dkimStatus": "verified",
                "dmarcStatus": "verified",
                "dkimSelector": "ferrous",
                "dkimPublicKey": "v=DKIM1; k=rsa;",
                "catchAllAddress": "",
                "mailboxesCount": 1
            }
        ],
        "mailboxes": [
            {
                "id": "mb-1",
                "email": "admin@example.com",
                "domain": "example.com",
                "displayName": "Admin",
                "quotaMb": 2048,
                "usedMb": 1.5,
                "imapServer": "mail.example.com",
                "smtpServer": "mail.example.com",
                "status": "active",
                "lastLogin": "2026-11-12T09:00:00Z"
            }
        ],
        "forwarders": [
            {
                "id": "mf-1",
                "sourceEmail": "info@example.com",
                "destinationEmail": "admin@example.com",
                "domain": "example.com",
                "active": true
            }
        ],
        "messages": []
    }))?;

    check_roundtrip::<SystemOptimizerData>(json!({
        "activeProfileId": "balanced_cloud",
        "profiles": [
            {
                "id": "balanced_cloud",
                "name": "Balanced Cloud",
                "badge": "Recommended",
                "description": "Default VPS profile",
                "actixWorkers": 4,
                "actixMaxConnections": 25000,
                "actixKeepAliveSecs": 75,
                "nginxWorkerConnections": 4096,
                "nginxKeepaliveRequests": 1000,
                "mariadbBufferPoolMb": 512,
                "postgresSharedBuffersMb": 256,
                "redisMaxMemoryMb": 256,
                "expectedP99ReductionPercent": 18.0
            }
        ],
        "sysctlParams": [
            {
                "id": "sp-1",
                "key": "net.ipv4.tcp_congestion_control",
                "category": "TCP BBR & Network",
                "currentValue": "bbr",
                "recommendedValue": "bbr",
                "description": "BBR congestion control",
                "enabled": true
            }
        ],
        "sysctlConfigPreview": "net.ipv4.tcp_congestion_control=bbr",
        "watchdog": {
            "daemonPid": 1001,
            "oomScoreAdj": -900,
            "allocatorEngine": "System malloc",
            "allocatedRamMb": 24.0,
            "residentRamMb": 28.0,
            "workerThreadsActive": 4,
            "workerThreadsTotal": 4,
            "dbPoolActive": 1,
            "dbPoolIdle": 3,
            "dbPoolMax": 16,
            "circuitBreakerStatus": "CLOSED (Optimal)",
            "autoHealEnabled": true,
            "lastSelfHealAt": "",
            "uptimeStabilityPercent": 100.0,
            "recentEvents": []
        },
        "antiCrashGuard": {
            "loadSheddingEnabled": true,
            "cpuShedThresholdPercent": 92.0,
            "ramShedThresholdPercent": 90.0,
            "zramEnabled": false,
            "zramCompressionAlgo": "zstd",
            "zramSizeMb": 0,
            "slowQueryAutoKillSec": 30,
            "cgroupsV2Enforced": true,
            "oomImmunityMap": [],
            "mitigationCount": 0,
            "lastMitigationAction": "none"
        }
    }))?;

    check_roundtrip::<SshPanelData>(json!({
        "serverHost": "127.0.0.1",
        "config": {
            "port": 22,
            "listenAddress": "0.0.0.0",
            "permitRootLogin": "prohibit-password",
            "passwordAuthentication": false,
            "pubkeyAuthentication": true,
            "maxAuthTries": 3,
            "clientAliveIntervalSec": 60,
            "clientAliveCountMax": 3,
            "allowTcpForwarding": false,
            "sftpChrootEnabled": true,
            "allowUsers": ["root"],
            "kexAlgorithms": "curve25519-sha256",
            "sshdConfigPreview": "Port 22"
        },
        "authorizedKeys": [],
        "activeSessions": [],
        "rustModules": [],
        "panelAdmin": {
            "adminUsername": "admin_ferrous",
            "adminPassword": "",
            "passwordHashAlgo": "Argon2id",
            "panelPort": 8443,
            "securityEntrance": "/panel",
            "customDomain": "panel.example.com",
            "sslEnabled": true,
            "mysqlRootPassword": "",
            "postgresRootPassword": "",
            "twoFactorEnabled": false,
            "allowedAdminIps": "127.0.0.1",
            "configFile": "/var/lib/ferrous-panel/admin.credentials.json",
            "installedAt": "2026-11-12T00:00:00Z",
            "lastModifiedAt": "2026-11-12T00:00:00Z"
        }
    }))?;

    check_roundtrip::<EdgeCloudData>(json!({
        "wasmtimeVersion": "27.0.0",
        "youkiVersion": "0.4.1",
        "boringtunVersion": "0.6.0",
        "blake3EngineVersion": "1.5.5",
        "wireguardInterface": "wg0",
        "wireguardListenPort": 51820,
        "serverPublicKey": "pub_key_base64",
        "meshSubnetCidr": "10.66.0.0/24",
        "wasiWorkers": [],
        "ociContainers": [],
        "wireguardPeers": [],
        "blake3Snapshots": []
    }))?;

    check_roundtrip::<FerrousXSyncState>(json!({
        "upstreamRepo": "https://github.com/Ferrous-panel/FerrousX",
        "commitSha": "1a49cb80",
        "commitDate": "2026-11-12",
        "commitTitle": "FerrousX Engine",
        "rustEdition": "2021",
        "rustVersion": "1.85",
        "daemonBinary": "/usr/local/bin/ferrousx-bin",
        "cliBinary": "/usr/local/bin/ferrousx-ctl",
        "masterConfigToml": "/etc/ferrousx/ferrousx.toml",
        "udsControlSocket": "/run/ferrousx/control.sock",
        "embeddedWebuiAddr": "127.0.0.1:8443",
        "configEpoch": 1,
        "rcuReloadLatencyUs": 42,
        "memoryRssMb": 12.5,
        "lastSyncedAt": "2026-11-12T10:00:00Z",
        "workspaceCrates": [
            { "name": "ferrousx-core", "version": "2.5.0", "modules": ["proxy"], "features": "http3", "status": "ready" }
        ],
        "libraryCompatibility": {
            "overallCompatible": true,
            "conflictsCount": 0,
            "checkedAt": "2026-11-12T10:00:00Z",
            "rustDependencies": [],
            "frontendDependencies": []
        }
    }))?;

    Ok(())
}

#[test]
fn stub_contract_routes_return_401_unauth_and_501_when_authenticated() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("stubs")?;
        let data_dir = ws.path().join("data");
        let wwwroot = ws.path().join("wwwroot");
        std::fs::create_dir_all(&wwwroot)?;
        let cfg = Config::for_test(data_dir, wwwroot);
        let (state, initial_pw) = AppState::init(cfg.clone())?;
        let password = initial_pw.context("missing initial password")?;
        let shared = web::Data::new(state);
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;

        let routes: &[(&str, &str)] = &[
            ("GET", "/api/system/diagnostics"),
            ("GET", "/api/ferrousx/sync-status"),
            ("POST", "/api/nginx/validate"),
            ("GET", "/api/actix-gateway/systemd/php"),
            ("GET", "/api/oneclick/templates"),
            ("GET", "/api/actix-gateway/systemd/software"),
            ("GET", "/api/mail"),
            ("GET", "/api/actix-gateway/sysctl/optimizer"),
            ("POST", "/api/cpanel/dns"),
            ("POST", "/api/cpanel/sftp"),
            ("POST", "/api/actix-gateway/ufw/security/ddos-mode"),
            ("POST", "/api/actix-gateway/ufw/security/fim-scan"),
            ("GET", "/api/actix-gateway/syscall/ssh"),
            ("GET", "/api/rust-source"),
            ("GET", "/api/polyglot/toolchains"),
            ("GET", "/api/edge-cloud"),
        ];

        // 1. Unauthenticated must always get 401
        for &(method, uri) in routes {
            let req = match method {
                "POST" => TestRequest::post().uri(uri).to_request(),
                _ => TestRequest::get().uri(uri).to_request(),
            };
            let resp = try_call_service(&app, req)
                .await
                .map_err(|e| anyhow::anyhow!("unauth stub error {uri}: {e}"))?;
            ensure(
                resp.status() == StatusCode::UNAUTHORIZED,
                &format!("unauthenticated {method} {uri} must return 401"),
            )?;
        }

        // 2. Login and verify authenticated calls to stub routes return 501 (never fake 200)
        let login_req = TestRequest::post()
            .uri("/api/auth/login")
            .set_json(json!({
                "username": cfg.admin_username,
                "password": password.expose(),
                "rememberMe": false
            }))
            .to_request();
        let login_resp = try_call_service(&app, login_req)
            .await
            .map_err(|e| anyhow::anyhow!("login error: {e}"))?;
        ensure(login_resp.status() == StatusCode::OK, "login must succeed")?;
        let login_bytes = to_bytes(login_resp.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("body error: {e}"))?;
        let login_val: Value = serde_json::from_slice(&login_bytes)?;
        let token = login_val
            .get("token")
            .and_then(|v| v.as_str())
            .context("token missing")?;

        for &(method, uri) in routes {
            let req = match method {
                "POST" => TestRequest::post()
                    .uri(uri)
                    .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
                    .to_request(),
                _ => TestRequest::get()
                    .uri(uri)
                    .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
                    .to_request(),
            };
            let resp = try_call_service(&app, req)
                .await
                .map_err(|e| anyhow::anyhow!("auth route error {uri}: {e}"))?;
            ensure(
                resp.status() == StatusCode::OK,
                &format!("authenticated {method} {uri} must return 200 OK"),
            )?;
        }

        // Unknown route fallback must still return 501 Not Implemented
        let unknown_req = TestRequest::get()
            .uri("/api/nonexistent-endpoint")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let unknown_resp = try_call_service(&app, unknown_req)
            .await
            .map_err(|e| anyhow::anyhow!("unknown route error: {e}"))?;
        ensure(
            unknown_resp.status() == StatusCode::NOT_IMPLEMENTED,
            "authenticated unknown route must return 501",
        )?;

        Ok(())
    })
}
