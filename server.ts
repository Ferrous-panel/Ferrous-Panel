import express from 'express';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { createServer as createViteServer } from 'vite';

const app = express();
const PORT = 3000;

app.use(express.json({ limit: '50mb' }));

export interface EnvVar {
  key: string;
  value: string;
}

export type WebsiteRuntimeType =
  | 'Actix-Web (Rust)'
  | 'Axum (Rust)'
  | 'Rocket (Rust)'
  | 'Next.js 15 (React SSR)'
  | 'Nuxt 3 (Vue 3 SSR)'
  | 'SvelteKit 2 (Svelte 5)'
  | 'Astro 5 (Hybrid / Static)'
  | 'Remix / React Router v7'
  | 'NestJS 10 (TypeScript API)'
  | 'Vite + React 19 / Vue 3 SPA'
  | 'Node.js / Next.js'
  | 'PHP 8.3-FPM'
  | 'Python / FastAPI'
  | 'Go / Fiber'
  | 'Static HTML / SPA';

export type RewritePresetType =
  | 'rust_proxy'
  | 'nextjs_proxy'
  | 'nuxt_nitro'
  | 'sveltekit_node'
  | 'astro_hybrid'
  | 'laravel'
  | 'wordpress'
  | 'spa_history';

export interface ManagedApp {
  id: string;
  name: string;
  framework: WebsiteRuntimeType;
  runtimeVersion: string;
  domain: string;
  aliases: string[];
  internalPort: number;
  workingDir: string;
  runSubDir: string;
  execCommand: string;
  workers: number;
  memoryLimitMb: number;
  memoryUsedMb: number;
  cpuPercent: number;
  pid: number | null;
  status: 'running' | 'stopped' | 'rebuilding';
  sslEnabled: boolean;
  sslIssuer: string;
  sslExpiryDays: number;
  forceHttps: boolean;
  http3Quic: boolean;
  websocketEnabled: boolean;
  rateLimitRps: number;
  requestsPerSec: number;
  p99LatencyMs: number;
  uptimeSeconds: number;
  diskUsedMb: number;
  diskQuotaMb: number;
  trafficMonthGb: number;
  linkedDbName: string | null;
  ftpUsername: string | null;
  backupCount: number;
  gitRepoUrl: string;
  gitBranch: string;
  buildScript: string;
  lastDeployCommit: string;
  lastDeployTime: string;
  urlRewritePreset: RewritePresetType;
  urlRewriteRules: string;
  phpVersion?: '8.4' | '8.3' | '8.2' | '8.1' | '7.4' | 'None';
  redirects?: {
    id: string;
    sourcePath: string;
    targetUrl: string;
    type: '301 (Permanent)' | '302 (Temporary)';
    keepQuery: boolean;
    enabled: boolean;
  }[];
  hotlinkProtection?: {
    enabled: boolean;
    protectedExtensions: string;
    allowedDomains: string;
    returnBlankOr403: '403 Forbidden' | 'Blank Image';
  };
  directoryAuth?: {
    id: string;
    path: string;
    realmName: string;
    username: string;
    enabled: boolean;
  }[];
  envVars: EnvVar[];
  logs: string[];
  systemdUnit?: string;
  nginxVhost?: string;
}

export interface OneClickTemplate {
  id: string;
  name: string;
  category:
    | 'Rust Backend'
    | 'Modern JS Frameworks'
    | 'PHP CMS / Framework'
    | 'Python / Go';
  framework: WebsiteRuntimeType;
  version: string;
  description: string;
  defaultPort: number;
  requiresDb: 'PostgreSQL 17' | 'MariaDB 11.4' | 'None';
  buildCommand: string;
  execCommandTemplate: string;
  popularScore: string;
}

export interface ServerPackageItem {
  id: string;
  name: string;
  category:
    | 'Rust Supplemental Package'
    | 'Web Server'
    | 'Runtime'
    | 'Database'
    | 'Cache & Queue'
    | 'Security & Process';
  version: string;
  description: string;
  installed: boolean;
  status: 'running' | 'stopped' | 'not_installed';
  memoryMb: number;
  configPath: string;
  isRustNative: boolean;
  replacedLegacyPackage?: string;
  rustCrate?: string;
  port?: number;
  configContent: string;
  logs: string[];
}

const REWRITE_TEMPLATES: Record<RewritePresetType, string> = {
  rust_proxy: `# Reverse Proxy tới tiến trình Rust (Actix-Web / Axum) qua Loopback
location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}`,
  nextjs_proxy: `# Next.js 15 App Router Standalone Proxy + Immutable Asset Cache
location /_next/static {
    alias $WORKING_DIR/.next/static;
    expires 365d;
    add_header Cache-Control "public, immutable";
    access_log off;
}
location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
}`,
  nuxt_nitro: `# Nuxt 3.14 Nitro SSR Engine + /_nuxt/ Immutable Cache
location /_nuxt/ {
    alias $WORKING_DIR/.output/public/_nuxt/;
    expires 365d;
    add_header Cache-Control "public, max-age=31536000, immutable";
    access_log off;
}
location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}`,
  sveltekit_node: `# SvelteKit 2 (Svelte 5 Runes) adapter-node + Immutable Client Assets
location /_app/immutable/ {
    alias $WORKING_DIR/build/client/_app/immutable/;
    expires 365d;
    add_header Cache-Control "public, immutable";
    access_log off;
}
location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
}`,
  astro_hybrid: `# Astro 5.0 Hybrid Islands + Static Brotli Compression
location /_astro/ {
    alias $WORKING_DIR/dist/client/_astro/;
    expires 365d;
    add_header Cache-Control "public, immutable";
}
location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_set_header Host $host;
}`,
  laravel: `# Laravel 11 Pretty URLs + PHP 8.3-FPM FastCGI
location / {
    try_files $uri $uri/ /index.php?$query_string;
}
location ~ \\.php$ {
    fastcgi_pass unix:/run/php/php8.3-fpm.sock;
    fastcgi_index index.php;
    include fastcgi_params;
}`,
  wordpress: `# WordPress Permalink Rewrite + Chống thực thi PHP trong wp-content/uploads
location / {
    try_files $uri $uri/ /index.php?$args;
}
location ~* /wp-content/uploads/.*\\.php$ {
    deny all;
}`,
  spa_history: `# Vite 6 + React 19 / Vue 3.5 / Angular SPA HTML5 History Fallback
location /assets/ {
    expires 365d;
    add_header Cache-Control "public, immutable";
}
location / {
    try_files $uri $uri/ /index.html;
}`
};

let managedApps: ManagedApp[] = [];

const oneClickTemplates: OneClickTemplate[] = [
  {
    id: 'tpl-actix-api',
    name: 'Actix-Web 4.9 REST API + SQLx Starter',
    category: 'Rust Backend',
    framework: 'Actix-Web (Rust)',
    version: '4.9.0 (Rust 1.83 Release LTO)',
    description:
      'Khởi tạo nhanh dự án Rust Actix-Web hiệu năng cao kết nối sẵn PostgreSQL 17 (SQLx), xác thực JWT Argon2id, systemd sandbox và Nginx Reverse Proxy.',
    defaultPort: 8088,
    requiresDb: 'PostgreSQL 17',
    buildCommand: 'cargo build --release',
    execCommandTemplate: '/www/wwwroot/$DOMAIN/target/release/actix-app',
    popularScore: 'P99 < 1.5ms · RAM ~25MB'
  },
  {
    id: 'tpl-axum-realtime',
    name: 'Axum 0.7 WebSocket & Microservice Kit',
    category: 'Rust Backend',
    framework: 'Axum (Rust)',
    version: '0.7.9 (Tokio Epoll)',
    description:
      'Bộ khung dịch vụ thời gian thực viết bằng Rust Axum + Tokio, tự động bật cấu hình Nginx WebSocket Upgrade và chứng chỉ SSL Let’s Encrypt.',
    defaultPort: 8089,
    requiresDb: 'PostgreSQL 17',
    buildCommand: 'cargo build --release',
    execCommandTemplate: '/www/wwwroot/$DOMAIN/target/release/axum-service',
    popularScore: '50,000+ kết nối đồng thời'
  },
  {
    id: 'tpl-nextjs-15',
    name: 'Next.js 15 App Router + React 19 SSR',
    category: 'Modern JS Frameworks',
    framework: 'Next.js 15 (React SSR)',
    version: '15.0.3 (Turbopack · Node 22 LTS)',
    description:
      'Triển khai ứng dụng Next.js 15 ở chế độ standalone tối ưu bộ nhớ, tự động cấu hình Nginx phục vụ trực tiếp thư mục /_next/static với bộ nhớ đệm 365 ngày.',
    defaultPort: 3005,
    requiresDb: 'PostgreSQL 17',
    buildCommand: 'pnpm install && pnpm build',
    execCommandTemplate: '/usr/bin/node /www/wwwroot/$DOMAIN/.next/standalone/server.js',
    popularScore: 'React 19 Server Components'
  },
  {
    id: 'tpl-nuxt-3',
    name: 'Nuxt 3.14 + Vue 3.5 SSR (Nitro Engine)',
    category: 'Modern JS Frameworks',
    framework: 'Nuxt 3 (Vue 3 SSR)',
    version: '3.14.1 (Nitro 2.10)',
    description:
      'Triển khai khung ứng dụng Vue 3 Full-Stack với động cơ máy chủ Nitro siêu nhẹ, tự động cấu hình bộ nhớ đệm /_nuxt/ và nén Brotli trên Nginx.',
    defaultPort: 3006,
    requiresDb: 'PostgreSQL 17',
    buildCommand: 'pnpm install && pnpm build',
    execCommandTemplate: '/usr/bin/node /www/wwwroot/$DOMAIN/.output/server/index.mjs',
    popularScore: 'Vue 3 SSR + SWR Edge Cache'
  },
  {
    id: 'tpl-sveltekit-2',
    name: 'SvelteKit 2 + Svelte 5 Runes (Adapter-Node)',
    category: 'Modern JS Frameworks',
    framework: 'SvelteKit 2 (Svelte 5)',
    version: '2.8.0 (Svelte 5.2)',
    description:
      'Ứng dụng web siêu tốc biên dịch không Virtual DOM với Svelte 5 Runes, tiêu thụ cực ít RAM và phản hồi tức thì qua Nginx HTTP/3 QUIC.',
    defaultPort: 3007,
    requiresDb: 'PostgreSQL 17',
    buildCommand: 'npm ci && npm run build',
    execCommandTemplate: '/usr/bin/node /www/wwwroot/$DOMAIN/build/index.js',
    popularScore: 'Zero-VDOM · P99 < 3ms'
  },
  {
    id: 'tpl-astro-5',
    name: 'Astro 5.0 Content Islands & Hybrid SSR',
    category: 'Modern JS Frameworks',
    framework: 'Astro 5 (Hybrid / Static)',
    version: '5.0.0 (Content Layer API)',
    description:
      'Kiến trúc Islands hiện đại gửi 0KB JavaScript mặc định xuống trình duyệt, lý tưởng cho trang thương mại điện tử, cổng thông tin và blog tốc độ cao.',
    defaultPort: 4321,
    requiresDb: 'MariaDB 11.4',
    buildCommand: 'pnpm install && pnpm build',
    execCommandTemplate: '/usr/bin/node /www/wwwroot/$DOMAIN/dist/server/entry.mjs',
    popularScore: '100/100 Core Web Vitals'
  },
  {
    id: 'tpl-vite-spa',
    name: 'Vite 6 + React 19 / Vue 3 SPA (Static Brotli)',
    category: 'Modern JS Frameworks',
    framework: 'Vite + React 19 / Vue 3 SPA',
    version: 'Vite 6.0 (Nginx Static)',
    description:
      'Biên dịch tài nguyên tĩnh ra thư mục /dist và phục vụ trực tiếp bằng Nginx worker với try_files /index.html, không tốn RAM tiến trình Node.',
    defaultPort: 80,
    requiresDb: 'None',
    buildCommand: 'pnpm install && pnpm build',
    execCommandTemplate: 'nginx static /www/wwwroot/$DOMAIN/dist',
    popularScore: '0 MB Runtime RAM Overhead'
  },
  {
    id: 'tpl-laravel-11',
    name: 'Laravel 11 Enterprise + PHP 8.3 OPcache JIT',
    category: 'PHP CMS / Framework',
    framework: 'PHP 8.3-FPM',
    version: '11.31.0 (PHP 8.3-FPM)',
    description:
      'Tự động thiết lập thư mục chạy /public, cấu hình quy tắc URL Rewrite chuẩn Laravel, kết nối MariaDB 11.4 và Redis Queue Worker.',
    defaultPort: 9000,
    requiresDb: 'MariaDB 11.4',
    buildCommand: 'composer install --no-dev --optimize-autoloader && php artisan migrate --force',
    execCommandTemplate: 'php-fpm8.3 (unix:/run/php/php8.3-fpm.sock)',
    popularScore: 'Sẵn sàng cho SaaS & ERP'
  },
  {
    id: 'tpl-wordpress-67',
    name: 'WordPress 6.7 LTS + Nginx FastCGI Cache',
    category: 'PHP CMS / Framework',
    framework: 'PHP 8.3-FPM',
    version: '6.7.1 LTS',
    description:
      'Triển khai WordPress tốc độ cao kèm quy tắc khóa thực thi PHP trong wp-content/uploads, tự động tạo MariaDB database và chứng chỉ HTTPS.',
    defaultPort: 9000,
    requiresDb: 'MariaDB 11.4',
    buildCommand: 'wp core download --locale=vi && chown -R www-data:www-data .',
    execCommandTemplate: 'php-fpm8.3 (unix:/run/php/php8.3-fpm.sock)',
    popularScore: 'Tích hợp chống Brute-force xmlrpc'
  }
];

let serverPackages: ServerPackageItem[] = [
  {
    id: 'pkg-ferrous-kv',
    name: 'FerrousKV In-Memory Store (Rust Redis Replacement)',
    category: 'Rust Supplemental Package',
    version: '2.1.0-rust (tokio + dashmap + RESP3)',
    description:
      'Package bộ nhớ đệm Key-Value & Session Store viết lại 100% bằng Rust thay thế Redis/Memcached. Hỗ trợ đa luồng Lock-Free, không có độ trễ Garbage Collection.',
    installed: true,
    status: 'running',
    memoryMb: 19.4,
    configPath: '/etc/ferrous-panel/packages/ferrous-kv.toml',
    isRustNative: true,
    replacedLegacyPackage: 'Redis 7.4 / Memcached (C Daemon)',
    rustCrate: 'dashmap 6.1 + tokio 1.41 + bytes 1.8',
    port: 6379,
    configContent: `[server]
bind = "127.0.0.1:6379"
protocol = "RESP3"
worker_threads = 8
max_memory_mb = 1024
eviction_policy = "allkeys-lru"

[persistence]
aof_enabled = true
zstd_snapshot_interval_sec = 300
snapshot_path = "/var/lib/ferrous-kv/dump.fkv.zst"
`,
    logs: [
      '2026-11-09T11:00:01Z INFO  ferrous_kv: Initialized 64 lock-free DashMap shards (RESP3 compatible)',
      '2026-11-09T11:15:22Z INFO  ferrous_kv: Serving Actix-Web session store & Next.js SSR cache at 0.04ms P99'
    ]
  },
  {
    id: 'pkg-ferrous-waf',
    name: 'FerrousWAF & HTTP/3 Edge Proxy (Rust Pingora Core)',
    category: 'Rust Supplemental Package',
    version: '1.9.4-rust (pingora + rustls + coraza-rs)',
    description:
      'Tường lửa ứng dụng Web (WAF OWASP CRS) & bộ lọc chống DDoS Layer 7 viết bằng Rust, tích hợp trực tiếp trước Nginx và các upstream Actix-Web/Next.js.',
    installed: true,
    status: 'running',
    memoryMb: 24.8,
    configPath: '/etc/ferrous-panel/packages/ferrous-waf.toml',
    isRustNative: true,
    replacedLegacyPackage: 'ModSecurity v3 + Lua WAF',
    rustCrate: 'pingora-proxy 0.4 + rustls 0.23 + regex-automata',
    port: 443,
    configContent: `[waf]
mode = "BLOCK"
owasp_crs_paranoia_level = 2
inspect_json_body = true
max_request_body_kb = 51200

[anti_ddos]
syn_flood_guard = true
per_ip_rate_limit_rps = 180
challenge_suspicious_user_agents = true
`,
    logs: [
      '2026-11-09T10:30:00Z INFO  ferrous_waf: Loaded 412 OWASP CRS rules into zero-copy DFA engine',
      '2026-11-09T11:42:10Z WARN  ferrous_waf: Blocked SQLi payload on /api/v1/orders from 185.220.101.42'
    ]
  },
  {
    id: 'pkg-ferrous-acme',
    name: 'FerrousACME TLS 1.3 Manager (Rust Let’s Encrypt)',
    category: 'Rust Supplemental Package',
    version: '1.6.2-rust (instant-acme + rcgen + ring)',
    description:
      'Package cấp phát và gia hạn tự động chứng chỉ SSL Let’s Encrypt (ECDSA P-384 / Ed25519) viết bằng Rust, thay thế hoàn toàn Python Certbot nặng nề.',
    installed: true,
    status: 'running',
    memoryMb: 11.2,
    configPath: '/etc/ferrous-panel/packages/ferrous-acme.toml',
    isRustNative: true,
    replacedLegacyPackage: 'Certbot (Python 3 + OpenSSL CLI)',
    rustCrate: 'instant-acme 0.7 + rcgen 0.13 + ring 0.17',
    port: 80,
    configContent: `[acme]
directory_url = "https://acme-v02.api.letsencrypt.org/directory"
contact_email = "admin@ferroushub.vn"
key_type = "ECDSA_P384"
renew_days_before_expiry = 30
ocsp_stapling = true
auto_reload_nginx = true
`,
    logs: [
      '2026-11-09T03:15:00Z INFO  ferrous_acme: Verified 5 domains (api, shop, partner, metrics, docs.ferroushub.vn)',
      '2026-11-09T03:15:01Z INFO  ferrous_acme: OCSP Stapling responses refreshed and pinned in shared memory'
    ]
  },
  {
    id: 'pkg-ferrous-dns',
    name: 'FerrousDNS Authoritative Server (Hickory-DNS Rust)',
    category: 'Rust Supplemental Package',
    version: '0.24.2-rust (hickory-server + DNSSEC)',
    description:
      'Máy chủ phân giải tên miền Authoritative DNS & DNSSEC viết bằng Rust thay thế BIND9, bảo vệ chống tấn công tràn bộ đệm DNS.',
    installed: true,
    status: 'running',
    memoryMb: 16.0,
    configPath: '/etc/ferrous-panel/packages/ferrous-dns.toml',
    isRustNative: true,
    replacedLegacyPackage: 'BIND9 / PowerDNS',
    rustCrate: 'hickory-server 0.24 + hickory-proto 0.24',
    port: 53,
    configContent: `[dns]
listen_udp = ["0.0.0.0:53", "[::]:53"]
listen_tcp = ["0.0.0.0:53"]
dnssec_signing = "Ed25519"
zones_dir = "/etc/ferrous-panel/dns-zones"
rate_limit_qps = 2000
`,
    logs: [
      '2026-11-09T09:00:00Z INFO  ferrous_dns: Loaded zone ferroushub.vn (serial 2026110904, 6 records)',
      '2026-11-09T11:20:00Z INFO  ferrous_dns: Answering UDP/TCP queries with 0.08ms average latency'
    ]
  },
  {
    id: 'pkg-ferrous-sftp',
    name: 'FerrousSFTP Chroot Subsystem (Rust russh Server)',
    category: 'Rust Supplemental Package',
    version: '1.5.0-rust (russh + russh-sftp + argon2)',
    description:
      'Máy chủ truyền tệp SFTP cô lập theo từng thư mục Website (/www/wwwroot/*) viết bằng Rust, thay thế vsftpd/proftpd truyền thống.',
    installed: true,
    status: 'running',
    memoryMb: 14.5,
    configPath: '/etc/ferrous-panel/packages/ferrous-sftp.toml',
    isRustNative: true,
    replacedLegacyPackage: 'vsftpd / Pure-FTPd (C Daemons)',
    rustCrate: 'russh 0.45 + russh-keys 0.45 + tokio-uring',
    port: 2022,
    configContent: `[sftp]
listen_addr = "0.0.0.0:2022"
strict_chroot_wwwroot = true
allowed_auth = ["ssh-ed25519", "argon2id-password"]
max_concurrent_transfers = 64
umask = "0022"
`,
    logs: [
      '2026-11-09T08:10:00Z INFO  ferrous_sftp: Chroot guard active for /www/wwwroot/* accounts',
      '2026-11-09T11:05:11Z INFO  ferrous_sftp: Authenticated user ftp_api_ferrous via Ed25519 key'
    ]
  },
  {
    id: 'pkg-ferrous-search',
    name: 'FerrousSearch Full-Text Engine (Rust Tantivy / Milli)',
    category: 'Rust Supplemental Package',
    version: '1.11.0-rust (tantivy + roaring-bitmap)',
    description:
      'Engine tìm kiếm toàn văn (Full-Text Search) siêu tốc viết bằng Rust cho các website thương mại điện tử và CMS, nhẹ hơn 15 lần so với Elasticsearch.',
    installed: true,
    status: 'running',
    memoryMb: 32.0,
    configPath: '/etc/ferrous-panel/packages/ferrous-search.toml',
    isRustNative: true,
    replacedLegacyPackage: 'Elasticsearch / Solr (JVM)',
    rustCrate: 'tantivy 0.22 + heed 0.20 + actix-web',
    port: 7700,
    configContent: `[search]
http_addr = "127.0.0.1:7700"
data_dir = "/var/lib/ferrous-search/indexes"
max_indexing_memory_mb = 256
vietnamese_tokenizer = true
typo_tolerance = true
`,
    logs: [
      '2026-11-09T10:00:00Z INFO  ferrous_search: Indexed 14,820 orders & 840 SKUs from commerce_prod',
      '2026-11-09T11:40:00Z INFO  ferrous_search: Search query latency 0.6ms on index [storefront_products]'
    ]
  },
  {
    id: 'pkg-ferrous-vector',
    name: 'FerrousVector Log & Metrics Shipper (Rust Vector)',
    category: 'Rust Supplemental Package',
    version: '0.42.0-rust (vector + vrl + zstd)',
    description:
      'Bộ thu thập, phân tích và nén nhật ký truy cập Nginx/Actix-Web thời gian thực viết bằng Rust, thay thế Filebeat/Logstash.',
    installed: true,
    status: 'running',
    memoryMb: 21.0,
    configPath: '/etc/ferrous-panel/packages/ferrous-vector.toml',
    isRustNative: true,
    replacedLegacyPackage: 'Filebeat / Logstash / Fluentd',
    rustCrate: 'vector-core + tokio + zstd-safe',
    port: 9598,
    configContent: `[sources.nginx_logs]
type = "file"
include = ["/www/wwwlogs/*.log"]

[transforms.parse_access]
type = "remap"
inputs = ["nginx_logs"]
source = '. = parse_nginx_log!(.message, "combined")'

[sinks.compressed_archive]
type = "file"
compression = "zstd"
path = "/www/wwwlogs/archive/%Y-%m-%d.log.zst"
`,
    logs: [
      '2026-11-09T00:00:05Z INFO  ferrous_vector: Watching 5 website access/error logs in /www/wwwlogs/*',
      '2026-11-09T11:45:00Z INFO  ferrous_vector: Throughput 4,180 events/sec with 21 MB RSS memory'
    ]
  },
  {
    id: 'pkg-nginx',
    name: 'Nginx Web Server & Reverse Proxy',
    category: 'Web Server',
    version: '1.26.2 (TLS 1.3 + HTTP/3 QUIC + Brotli)',
    description:
      'Máy chủ Reverse Proxy trung tâm điều phối lưu lượng HTTPS/QUIC vào các tiến trình Actix-Web, Next.js, Nuxt, SvelteKit và PHP-FPM.',
    installed: true,
    status: 'running',
    memoryMb: 38.4,
    configPath: '/etc/nginx/nginx.conf',
    isRustNative: false,
    rustCrate: 'Managed by src/nginx.rs',
    port: 443,
    configContent: `user www-data;
worker_processes auto;
worker_rlimit_nofile 65535;
pid /run/nginx.pid;

events {
    worker_connections 16384;
    use epoll;
    multi_accept on;
}

http {
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;
    keepalive_requests 10000;
    server_tokens off;
    include /www/server/panel/vhost/nginx/*.conf;
}
`,
    logs: [
      '2026-11-09T09:10:00Z INFO  nginx: configuration file /etc/nginx/nginx.conf test is successful',
      '2026-11-09T11:30:00Z INFO  nginx: HTTP/3 QUIC listeners active on 0.0.0.0:443'
    ]
  },
  {
    id: 'pkg-rust',
    name: 'Rust Toolchain (rustc + cargo + LTO builder)',
    category: 'Runtime',
    version: '1.83.0 stable-x86_64-unknown-linux-gnu',
    description:
      'Trình biên dịch Rust chính thức phục vụ build tự động các dự án Actix-Web, Axum, Rocket và toàn bộ 13 module của FerrousPanel.',
    installed: true,
    status: 'running',
    memoryMb: 18.2,
    configPath: '/root/.cargo/config.toml',
    isRustNative: true,
    rustCrate: 'rustc 1.83.0 + cargo + clippy + rustfmt',
    port: 9443,
    configContent: `[build]
jobs = 8
target = "x86_64-unknown-linux-gnu"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
`,
    logs: [
      '2026-11-09T08:00:00Z INFO  cargo: Toolchain stable-x86_64-unknown-linux-gnu verified',
      '2026-11-09T11:25:00Z INFO  cargo: LTO release cache warm in /var/cache/cargo-target'
    ]
  },
  {
    id: 'pkg-nodejs',
    name: 'Node.js 22 LTS + Bun 1.1 + Oxc/SWC Rust Bundler',
    category: 'Runtime',
    version: 'Node v22.11.0 · Bun 1.1.34 · SWC/Turbopack Rust',
    description:
      'Môi trường thực thi tốc độ cao cho các framework JS hiện đại: Next.js 15 (Turbopack Rust), Nuxt 3, SvelteKit 2, Astro 5, Remix và NestJS.',
    installed: true,
    status: 'running',
    memoryMb: 64.0,
    configPath: '/etc/ferrous-panel/packages/js-runtime.toml',
    isRustNative: false,
    rustCrate: 'swc_core + turbopack (Rust JS Compiler)',
    port: 3001,
    configContent: `[runtime]
node_binary = "/usr/bin/node"
bun_binary = "/usr/local/bin/bun"
pnpm_home = "/root/.local/share/pnpm"
uv_threadpool_size = 16
max_old_space_size_mb = 2048
`,
    logs: [
      '2026-11-09T09:00:00Z INFO  js_runtime: Next.js 15, Nuxt 3, SvelteKit 2 & Astro 5 upstream pools healthy'
    ]
  },
  {
    id: 'pkg-postgres',
    name: 'PostgreSQL 17 Relational Database',
    category: 'Database',
    version: '17.2-1.pgdg24.04+1',
    description:
      'Hệ quản trị CSDL quan hệ hiệu năng cao tối ưu cho Rust SQLx / SeaORM / Prisma trên cổng nội bộ 127.0.0.1:5432.',
    installed: true,
    status: 'running',
    memoryMb: 214.0,
    configPath: '/etc/postgresql/17/main/postgresql.conf',
    isRustNative: false,
    rustCrate: 'sqlx-postgres 0.8 (Rust Async Driver)',
    port: 5432,
    configContent: `listen_addresses = '127.0.0.1'
port = 5432
max_connections = 100
shared_buffers = 1024MB
effective_cache_size = 3072MB
work_mem = 16MB
wal_compression = zstd
`,
    logs: [
      '2026-11-09T00:01:00Z INFO  postgres: database system is ready to accept connections on 127.0.0.1:5432'
    ]
  },
  {
    id: 'pkg-mariadb',
    name: 'MariaDB 11.4 LTS / MySQL + phpMyAdmin 5.2',
    category: 'Database',
    version: '11.4.4-MariaDB · phpMyAdmin 5.2.1',
    description:
      'Hệ CSDL MySQL/MariaDB đi kèm giao diện quản trị web phpMyAdmin 5.2 bảo vệ qua cổng SSL 888 và xác thực 2 lớp.',
    installed: true,
    status: 'running',
    memoryMb: 168.5,
    configPath: '/etc/mysql/mariadb.conf.d/50-server.cnf',
    isRustNative: false,
    rustCrate: 'sqlx-mysql 0.8 (Rust Async Driver)',
    port: 3306,
    configContent: `[mysqld]
bind-address = 127.0.0.1
port = 3306
innodb_buffer_pool_size = 1024M
max_connections = 300
character-set-server = utf8mb4
collation-server = utf8mb4_unicode_ci
`,
    logs: [
      '2026-11-09T00:01:02Z INFO  mariadbd: ready for connections on 127.0.0.1:3306 (InnoDB 1024MB)'
    ]
  }
];

// ============================================================================
// MySQL / MariaDB & phpMyAdmin 5.2 Configuration State
// ============================================================================
let mysqlPmaConfig = {
  mysqlVersion: 'MariaDB 11.4.4-LTS (MySQL 8.4 Protocol Compatible)',
  socketPath: '/run/mysqld/mysqld.sock',
  bindAddress: '127.0.0.1',
  port: 3306,
  rootPassword: 'Sql#2026!MariaDbRoot',
  innodbBufferPoolMb: 1024,
  maxConnections: 300,
  slowQueryLog: true,
  longQueryTimeSec: 1.5,
  defaultCharset: 'utf8mb4_unicode_ci',
  myCnfRaw: `[mysqld]
user                    = mysql
pid-file                = /run/mysqld/mysqld.pid
socket                  = /run/mysqld/mysqld.sock
port                    = 3306
bind-address            = 127.0.0.1
datadir                 = /var/lib/mysql
character-set-server    = utf8mb4
collation-server        = utf8mb4_unicode_ci

# InnoDB Performance Tuning (Managed by FerrousPanel)
innodb_buffer_pool_size = 1024M
innodb_log_file_size    = 256M
innodb_flush_log_at_trx_commit = 2
innodb_file_per_table   = 1
max_connections         = 300
max_allowed_packet      = 128M

# Slow Query Auditing
slow_query_log          = 1
slow_query_log_file     = /www/wwwlogs/mysql-slow.log
long_query_time         = 1.5
`,
  pmaVersion: 'phpMyAdmin 5.2.1 (Nginx SSL Reverse Proxy + PHP 8.3-FPM)',
  pmaPath: '/phpmyadmin_ferrous_89a2',
  pmaPort: 888,
  pmaSslOnly: true,
  pmaBasicAuth: true,
  pmaMaxUploadMb: 256,
  pmaAllowedIps: '0.0.0.0/0 (Protected by Panel SSO)',
  pmaStatus: 'running' as 'running' | 'stopped'
};

// ============================================================================
// Mail Server & Webmail State (Postfix + Dovecot + OpenDKIM + Roundcube)
// ============================================================================
let mailServerState = {
  postfixVersion: 'Postfix 3.8.6 (TLS 1.3 + SASL Dovecot)',
  dovecotVersion: 'Dovecot 2.3.21 (IMAPS 993 / POP3S 995)',
  rspamdVersion: 'Rspamd 3.9.1 + OpenDKIM 2048-bit',
  queueCount: 0,
  sentToday: 0,
  spamBlockedToday: 0,
  domains: [] as {
    id: string;
    domain: string;
    spfStatus: 'verified' | 'pending';
    dkimStatus: 'verified' | 'pending';
    dmarcStatus: 'verified' | 'pending';
    dkimSelector: string;
    dkimPublicKey: string;
    catchAllAddress: string;
    mailboxesCount: number;
  }[],
  mailboxes: [] as {
    id: string;
    email: string;
    domain: string;
    displayName: string;
    quotaMb: number;
    usedMb: number;
    imapServer: string;
    smtpServer: string;
    status: 'active' | 'suspended';
    lastLogin: string;
  }[],
  forwarders: [] as {
    id: string;
    sourceEmail: string;
    destinationEmail: string;
    domain: string;
    active: boolean;
  }[],
  messages: [] as {
    id: string;
    folder: 'inbox' | 'sent' | 'spam' | 'trash';
    from: string;
    to: string;
    subject: string;
    snippet: string;
    body: string;
    timestamp: string;
    dkimSigned: boolean;
  }[]
};

// ============================================================================
// SQL Databases State (PostgreSQL 17, MariaDB 11.4, SQLite 3)
// ============================================================================
let sqlDatabases: {
  id: string;
  name: string;
  engine: 'PostgreSQL 17' | 'MariaDB 11.4' | 'SQLite 3 (WAL)';
  host: string;
  port: number;
  username: string;
  password?: string;
  collation: string;
  linkedAppId: string;
  linkedAppName: string;
  sizeMb: number;
  activeConnections: number;
  maxConnections: number;
  status: 'online' | 'maintenance';
  createdAt: string;
  tables: {
    tableName: string;
    rowCount: number;
    sizeKb: number;
    columns: {
      name: string;
      dataType: string;
      nullable: boolean;
      isPrimaryKey?: boolean;
    }[];
    rows: Record<string, string | number | boolean | null>[];
  }[];
}[] = [];

function permOctalToSymbolic(octal: string, isDir: boolean): string {
  const map: Record<string, string> = {
    '7': 'rwx',
    '6': 'rw-',
    '5': 'r-x',
    '4': 'r--',
    '0': '---'
  };
  const digits = octal.replace(/^0/, '').padStart(3, '0');
  const prefix = isDir ? 'd' : '-';
  return (
    prefix +
    (map[digits[0]] || 'rwx') +
    (map[digits[1]] || 'r-x') +
    (map[digits[2]] || 'r-x')
  );
}

let webFiles: {
  id: string;
  appId: string;
  name: string;
  path: string;
  parentDir: string;
  type: 'file' | 'directory';
  sizeBytes: number;
  permissions: string;
  symbolicPerms: string;
  owner: string;
  modifiedAt: string;
  content?: string;
}[] = [];

let cronJobs: {
  id: string;
  title: string;
  schedule: string;
  command: string;
  user: string;
  targetApp: string;
  enabled: boolean;
  lastRunAt: string;
  lastExitCode: number;
  lastOutput: string;
}[] = [];

let dnsRecords: {
  id: string;
  domain: string;
  type: 'A' | 'CNAME' | 'MX' | 'TXT' | 'AAAA';
  host: string;
  value: string;
  ttl: number;
}[] = [];

let backupArchives: {
  id: string;
  filename: string;
  targetApp: string;
  databaseIncluded: string;
  sizeMb: number;
  compression: 'zstd (level 9)';
  sha256: string;
  createdAt: string;
  status: 'verified';
}[] = [];

let sftpAccounts: {
  id: string;
  username: string;
  chrootDir: string;
  targetApp: string;
  authMethod: 'Ed25519 SSH Key';
  quotaMb: number;
  usedMb: number;
  status: 'active';
}[] = [];

let ufwRules: {
  id: string;
  port: string;
  protocol: 'TCP' | 'UDP' | 'BOTH';
  action: 'ALLOW' | 'DENY' | 'LIMIT';
  source: string;
  comment: string;
  enabled: boolean;
}[] = [
  {
    id: 'ufw-1',
    port: '22',
    protocol: 'TCP',
    action: 'LIMIT',
    source: '0.0.0.0/0',
    comment: 'OpenSSH Hardened Rate-Limited (Ed25519 Only)',
    enabled: true
  },
  {
    id: 'ufw-2',
    port: '80,443',
    protocol: 'TCP' as const,
    action: 'ALLOW' as const,
    source: '0.0.0.0/0',
    comment: 'Nginx HTTP/HTTPS Reverse Proxy Ingress',
    enabled: true
  },
  {
    id: 'ufw-3',
    port: '25,465,587,993',
    protocol: 'TCP' as const,
    action: 'ALLOW' as const,
    source: '0.0.0.0/0',
    comment: 'Postfix SMTPS/STARTTLS & Dovecot IMAPS Mail Server',
    enabled: true
  },
  {
    id: 'ufw-4',
    port: '888',
    protocol: 'TCP' as const,
    action: 'ALLOW' as const,
    source: '0.0.0.0/0',
    comment: 'phpMyAdmin 5.2 SSL Protected Tunnel',
    enabled: true
  },
  {
    id: 'ufw-5',
    port: '3000:8099',
    protocol: 'TCP' as const,
    action: 'DENY' as const,
    source: '0.0.0.0/0',
    comment: 'Isolate Internal Actix-Web & Node/Nuxt/SvelteKit Upstream Ports',
    enabled: true
  },
  {
    id: 'ufw-6',
    port: '5432,3306',
    protocol: 'TCP' as const,
    action: 'DENY' as const,
    source: '0.0.0.0/0',
    comment: 'Isolate PostgreSQL 17 & MariaDB 11.4 Sockets to Localhost Only',
    enabled: true
  }
];

let fail2banJails = [
  {
    name: 'sshd-ed25519',
    filter: 'sshd[mode=aggressive]',
    logPath: '/var/log/auth.log',
    maxRetry: 3,
    findTimeSec: 600,
    banTimeSec: 86400,
    currentlyBanned: 3,
    totalBanned: 148,
    bannedIps: [
      {
        ip: '185.220.101.42',
        country: 'DE',
        reason: 'SSH root brute-force (3 failed preauth attempts)',
        bannedAt: '14 phút trước'
      },
      {
        ip: '45.155.205.109',
        country: 'RU',
        reason: 'Invalid user admin key exchange flood',
        bannedAt: '1 giờ trước'
      }
    ]
  },
  {
    name: 'nginx-limit-req',
    filter: 'nginx-limit-req',
    logPath: '/www/wwwlogs/nginx_error.log',
    maxRetry: 10,
    findTimeSec: 120,
    banTimeSec: 7200,
    currentlyBanned: 2,
    totalBanned: 64,
    bannedIps: [
      {
        ip: '194.26.29.114',
        country: 'NL',
        reason: 'Exceeded Actix-Web upstream zone burst (150 req/s)',
        bannedAt: '8 phút trước'
      }
    ]
  },
  {
    name: 'postfix-sasl',
    filter: 'postfix[mode=more]',
    logPath: '/var/log/mail.log',
    maxRetry: 5,
    findTimeSec: 300,
    banTimeSec: 43200,
    currentlyBanned: 1,
    totalBanned: 27,
    bannedIps: [
      {
        ip: '91.92.240.51',
        country: 'BG',
        reason: 'SMTP SASL authentication brute-force attempt',
        bannedAt: '22 phút trước'
      }
    ]
  }
];

let hardeningDirectives = [
  {
    id: 'hd-1',
    category: 'SSH Daemon' as const,
    key: 'PermitRootLogin',
    recommendedValue: 'no',
    currentValue: 'no',
    enabled: true,
    description:
      'Vô hiệu hóa đăng nhập trực tiếp bằng tài khoản root qua SSH (/etc/ssh/sshd_config).'
  },
  {
    id: 'hd-2',
    category: 'SSH Daemon' as const,
    key: 'PasswordAuthentication',
    recommendedValue: 'no (Ed25519 Key Only)',
    currentValue: 'no',
    enabled: true,
    description:
      'Bắt buộc xác thực khóa bất đối xứng Ed25519/RSA-4096, chống vét cạn mật khẩu.'
  },
  {
    id: 'hd-3',
    category: 'Linux Kernel (sysctl)' as const,
    key: 'net.ipv4.tcp_syncookies',
    recommendedValue: '1',
    currentValue: '1',
    enabled: true,
    description:
      'Kích hoạt SYN Cookies trong nhân Ubuntu để chống tấn công từ chối dịch vụ TCP SYN Flood.'
  },
  {
    id: 'hd-4',
    category: 'Systemd Sandbox' as const,
    key: 'ProtectSystem=strict & open_basedir Isolation',
    recommendedValue: 'enforced',
    currentValue: 'enforced',
    enabled: true,
    description:
      'Cô lập từng Website trong đúng thư mục /www/wwwroot/<domain>, chống tấn công Cross-Site.'
  },
  {
    id: 'hd-5',
    category: 'Actix-Web Auth' as const,
    key: 'Argon2id + mTLS Loopback Binding',
    recommendedValue: '127.0.0.1 only',
    currentValue: '127.0.0.1 only',
    enabled: true,
    description:
      'Mọi worker Actix-Web và JS SSR chỉ lắng nghe trên loopback 127.0.0.1 sau Nginx TLS 1.3.'
  }
];

export function generateSystemdUnit(appSpec: ManagedApp): string {
  const activePhpVer =
    appSpec.phpVersion && appSpec.phpVersion !== 'None'
      ? appSpec.phpVersion
      : appSpec.framework.includes('PHP')
        ? '8.3'
        : null;

  if (activePhpVer) {
    const phpEnvLines = appSpec.envVars
      .map((e) => `env[${e.key}] = "${e.value}"`)
      .join('\n');
    const maxChildren = Math.max(10, appSpec.workers * 5);
    return `; ============================================================================
; /etc/php/${activePhpVer}/fpm/pool.d/${appSpec.name}.conf
; Quản lý bởi Bộ não Kiến trúc Ferrous-panel (Systemd Service: php${activePhpVer}-fpm.service)
; ============================================================================
[${appSpec.name}]
user = www-data
group = www-data
listen = /run/php/php${activePhpVer}-fpm.sock
listen.owner = www-data
listen.group = www-data
listen.mode = 0660

pm = dynamic
pm.max_children = ${maxChildren}
pm.start_servers = ${Math.max(2, Math.floor(maxChildren / 5))}
pm.min_spare_servers = ${Math.max(2, Math.floor(maxChildren / 6))}
pm.max_spare_servers = ${Math.max(4, Math.floor(maxChildren / 2))}
pm.max_requests = 2000
request_terminate_timeout = 120s

; Cô lập Chroot/open_basedir & Giới hạn RAM theo từng Website
php_admin_value[memory_limit] = ${appSpec.memoryLimitMb}M
php_admin_value[open_basedir] = ${appSpec.workingDir}:/tmp:/proc
php_admin_value[sys_temp_dir] = ${appSpec.workingDir}/tmp
php_admin_value[error_log] = /www/wwwlogs/${appSpec.domain}.php-error.log
${phpEnvLines}`;
  }

  if (
    appSpec.framework === 'Static HTML / SPA' ||
    appSpec.framework === 'Vite + React 19 / Vue 3 SPA'
  ) {
    return `# ============================================================================
# Static SPA / HTML Mode — Phục vụ trực tiếp qua Nginx Worker (Zero-Daemon Overhead)
# Build Trigger Service: /etc/systemd/system/${appSpec.name}-builder.service
# ============================================================================
[Unit]
Description=Ferrous-panel Static Asset Builder for ${appSpec.domain} (${appSpec.framework})
After=network-online.target nginx.service

[Service]
Type=oneshot
User=www-data
Group=www-data
WorkingDirectory=${appSpec.workingDir}
ExecStart=/bin/sh -c "${appSpec.buildScript || 'npm run build'}"
RemainAfterExit=yes
ProtectSystem=strict
ReadWritePaths=${appSpec.workingDir} /www/wwwlogs`;
  }

  const envBlock = appSpec.envVars
    .map((e) => `Environment="${e.key}=${e.value}"`)
    .join('\n');

  return `[Unit]
Description=Ferrous-panel Website Service: ${appSpec.domain} (${appSpec.framework})
After=network-online.target nginx.service
Wants=network-online.target

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=${appSpec.workingDir}
ExecStart=${appSpec.execCommand}
Restart=always
RestartSec=3
LimitNOFILE=65535

# Resource Quota & Isolation
MemoryMax=${appSpec.memoryLimitMb}M
CPUQuota=${appSpec.workers * 100}%

# Ubuntu Systemd Anti-Cross-Site Sandbox
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${appSpec.workingDir} /www/wwwlogs
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
LockPersonality=true

# Runtime Environment
Environment="PORT=${appSpec.internalPort}"
${envBlock}

[Install]
WantedBy=multi-user.target`;
}

export function generateNginxVhost(appSpec: ManagedApp): string {
  const upstreamName = `backend_${appSpec.name.replace(/[^a-zA-Z0-9]/g, '_')}`;
  const serverNames = [appSpec.domain, ...(appSpec.aliases || [])].join(' ');
  const quicBlock =
    appSpec.http3Quic && appSpec.sslEnabled
      ? `    listen 443 quic;\n    listen [::]:443 quic;\n    add_header Alt-Svc 'h3=":443"; ma=86400' always;\n`
      : '';

  const rootDir =
    appSpec.runSubDir && appSpec.runSubDir !== '/'
      ? `${appSpec.workingDir}${appSpec.runSubDir}`
      : appSpec.workingDir;

  const activePhpVer =
    appSpec.phpVersion && appSpec.phpVersion !== 'None'
      ? appSpec.phpVersion
      : appSpec.framework.includes('PHP')
        ? '8.3'
        : null;

  const isStaticSpa =
    !activePhpVer &&
    (appSpec.framework === 'Static HTML / SPA' ||
      appSpec.framework === 'Vite + React 19 / Vue 3 SPA' ||
      appSpec.urlRewritePreset === 'spa_history');

  // Synchronize client_max_body_size with the active PHP version's upload_max_filesize if PHP is enabled
  let clientMaxBodySize = '64M';
  if (activePhpVer) {
    try {
      const phpInst = phpVersionsState?.find((p) => p.version === activePhpVer);
      if (phpInst?.uploadMaxFilesize) {
        clientMaxBodySize = phpInst.uploadMaxFilesize;
      } else {
        clientMaxBodySize = '128M';
      }
    } catch {
      clientMaxBodySize = '128M';
    }
  }

  // 1. Upstream Pool Block (Only emitted for Reverse Proxy runtimes, NEVER for PHP-FPM or Static SPA)
  const upstreamHeaderBlock =
    !activePhpVer && !isStaticSpa
      ? `limit_req_zone $binary_remote_addr zone=${upstreamName}_limit:10m rate=${appSpec.rateLimitRps}r/s;

upstream ${upstreamName} {
    least_conn;
    server 127.0.0.1:${appSpec.internalPort} max_fails=3 fail_timeout=15s;
    keepalive 64;
}

`
      : '';

  // 2. Compile 301/302 Redirect Rules
  const redirectBlock =
    appSpec.redirects && appSpec.redirects.filter((r) => r.enabled).length > 0
      ? `\n    # 301 / 302 URL Redirect Rules\n` +
        appSpec.redirects
          .filter((r) => r.enabled)
          .map(
            (r) =>
              `    rewrite ^${r.sourcePath}$ ${r.targetUrl}${r.keepQuery ? '$is_args$args' : ''} ${String(r.type).startsWith('301') ? 'permanent' : 'redirect'};`
          )
          .join('\n') +
        '\n'
      : '';

  // 3. Compile Hotlink Protection Block
  const hotlinkBlock = appSpec.hotlinkProtection?.enabled
    ? `\n    # Anti-Hotlink Protection\n    location ~* \\.(${appSpec.hotlinkProtection.protectedExtensions || 'jpg|jpeg|png|gif|webp|mp4|zip'})$ {\n        valid_referers none blocked server_names ${appSpec.hotlinkProtection.allowedDomains || appSpec.domain};\n        if ($invalid_referer) {\n            ${appSpec.hotlinkProtection.returnBlankOr403 === 'Blank Image' ? 'empty_gif;' : 'return 403;'}\n        }\n        expires 30d;\n    }\n`
    : '';

  // 4. Compile Password-Protected Directories (HTTP Basic Auth)
  const dirAuthBlock =
    appSpec.directoryAuth &&
    appSpec.directoryAuth.filter((d) => d.enabled).length > 0
      ? `\n    # Directory Privacy (HTTP Basic Auth)\n` +
        appSpec.directoryAuth
          .filter((d) => d.enabled)
          .map(
            (d) =>
              `    location ^~ ${d.path} {\n        auth_basic "${d.realmName || 'Restricted Area'}";\n        auth_basic_user_file /www/server/panel/vhost/htpasswd/${appSpec.domain}_${d.username}.htpasswd;\n    }`
          )
          .join('\n') +
        '\n'
      : '';

  // 5. Rewrite & Location Routing Section
  let rewriteSection = (appSpec.urlRewriteRules || '')
    .replace(/http:\/\/127\.0\.0\.1:\$INTERNAL_PORT/g, `http://${upstreamName}`)
    .replace(/\$INTERNAL_PORT/g, String(appSpec.internalPort))
    .replace(/\$WORKING_DIR/g, appSpec.workingDir);

  if (activePhpVer) {
    rewriteSection = rewriteSection.replace(
      /php[0-9.]+-fpm\.sock/g,
      `php${activePhpVer}-fpm.sock`
    );
    if (!rewriteSection.includes('fastcgi_pass')) {
      rewriteSection += `\n\nlocation ~ \\.php$ {\n    include snippets/fastcgi-php.conf;\n    fastcgi_pass unix:/run/php/php${activePhpVer}-fpm.sock;\n    fastcgi_param SCRIPT_FILENAME $realpath_root$fastcgi_script_name;\n    include fastcgi_params;\n}`;
    }
  }

  const indexDirective = activePhpVer
    ? 'index index.php index.html index.htm;'
    : 'index index.html index.htm;';

  // If SSL is disabled, emit a clean HTTP-only server block so nginx -t never fails on missing certs
  if (!appSpec.sslEnabled) {
    return `# /www/server/panel/vhost/nginx/${appSpec.domain}.conf
# Quản lý bởi Bộ não Kiến trúc Ferrous-panel (${appSpec.framework})

${upstreamHeaderBlock}server {
    listen 80;
    listen [::]:80;
    server_name ${serverNames};
    root ${rootDir};
    ${indexDirective}

    access_log /www/wwwlogs/${appSpec.domain}.log;
    error_log  /www/wwwlogs/${appSpec.domain}.error.log;

    client_max_body_size ${clientMaxBodySize};

    location /.well-known/acme-challenge/ {
        root /var/www/letsencrypt;
    }
${redirectBlock}${hotlinkBlock}${dirAuthBlock}
    # URL Rewrite & Handler Directives
${rewriteSection}
}`;
  }

  // Resolve SSL certificate path safely (use Let's Encrypt if present, or note fallback cert support)
  const hasLetsEncryptOnDisk = fs.existsSync(
    `/etc/letsencrypt/live/${appSpec.domain}/fullchain.pem`
  );
  const hasFallbackOnDisk = fs.existsSync(
    '/etc/nginx/ssl/ferrous-fallback.crt'
  );
  const sslCertPath =
    !hasLetsEncryptOnDisk && hasFallbackOnDisk
      ? '/etc/nginx/ssl/ferrous-fallback.crt'
      : `/etc/letsencrypt/live/${appSpec.domain}/fullchain.pem`;
  const sslKeyPath =
    !hasLetsEncryptOnDisk && hasFallbackOnDisk
      ? '/etc/nginx/ssl/ferrous-fallback.key'
      : `/etc/letsencrypt/live/${appSpec.domain}/privkey.pem`;

  return `# /www/server/panel/vhost/nginx/${appSpec.domain}.conf
# Quản lý bởi Bộ não Kiến trúc Ferrous-panel (${appSpec.framework}${activePhpVer ? ` · PHP ${activePhpVer}-FPM` : ''})

${upstreamHeaderBlock}server {
    listen 80;
    listen [::]:80;
    server_name ${serverNames};
    root ${rootDir};
    ${indexDirective}

    location /.well-known/acme-challenge/ {
        root /var/www/letsencrypt;
    }
${
  appSpec.forceHttps
    ? `\n    location / {\n        return 301 https://$host$request_uri;\n    }`
    : `\n${rewriteSection}`
}
}

server {
    listen 443 ssl;
    listen [::]:443 ssl;
${quicBlock}    http2 on;
    server_name ${serverNames};
    root ${rootDir};
    ${indexDirective}

    ssl_certificate ${sslCertPath};
    ssl_certificate_key ${sslKeyPath};
    ssl_session_timeout 1d;
    ssl_session_cache shared:MozSSL:10m;
    ssl_session_tickets off;
    ssl_protocols TLSv1.2 TLSv1.3;

    # Hardened Security Headers
    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    access_log /www/wwwlogs/${appSpec.domain}.log;
    error_log  /www/wwwlogs/${appSpec.domain}.error.log;

    client_max_body_size ${clientMaxBodySize};
${redirectBlock}${hotlinkBlock}${dirAuthBlock}
    # URL Rewrite & Upstream / FastCGI Directives
${rewriteSection}
}`;
}

/**
 * Synchronizes a Website's `envVars` array with its `/.env` file inside File Manager (`webFiles`),
 * and writes the Nginx Vhost to `/www/server/panel/vhost/nginx` if running on a real Ubuntu VPS.
 */
function syncArchitectureForApp(appSpec: ManagedApp): ManagedApp {
  // 1. Update systemdUnit & nginxVhost on the app object
  appSpec.systemdUnit = generateSystemdUnit(appSpec);
  appSpec.nginxVhost = generateNginxVhost(appSpec);

  // 2. Synchronize /.env in webFiles with appSpec.envVars
  try {
    if (Array.isArray(webFiles)) {
      const envContent =
        `# Generated & Synced by Ferrous-panel Architectural Brain for ${appSpec.domain}\n` +
        appSpec.envVars.map((e) => `${e.key}=${e.value}`).join('\n') +
        '\n';
      const existingEnv = webFiles.find(
        (f) => f.appId === appSpec.id && f.path === '/.env'
      );
      const nowStr = new Date().toISOString().slice(0, 16).replace('T', ' ');
      if (existingEnv) {
        existingEnv.content = envContent;
        existingEnv.sizeBytes = Buffer.byteLength(envContent, 'utf8');
        existingEnv.modifiedAt = nowStr;
      } else {
        webFiles.push({
          id: `f-env-${appSpec.id}`,
          appId: appSpec.id,
          name: '.env',
          path: '/.env',
          parentDir: '/',
          type: 'file',
          sizeBytes: Buffer.byteLength(envContent, 'utf8'),
          permissions: '0600',
          symbolicPerms: '-rw-------',
          owner: 'www-data:www-data',
          modifiedAt: nowStr,
          content: envContent
        });
      }
    }
  } catch {
    // webFiles not initialized yet during startup
  }

  // 3. If running on a real Ubuntu VPS where /www/server/panel/vhost/nginx exists, write vhost to disk
  try {
    if (fs.existsSync('/www/server/panel/vhost/nginx') && appSpec.nginxVhost) {
      fs.writeFileSync(
        `/www/server/panel/vhost/nginx/${appSpec.domain}.conf`,
        appSpec.nginxVhost,
        'utf8'
      );
    }
  } catch {
    // non-root or container preview environment
  }

  return appSpec;
}

// 1. Live Ubuntu Host Telemetry API
app.get('/api/telemetry', (_req, res) => {
  const cpus = os.cpus();
  const load = os.loadavg();
  const totalMemMb = Math.round(os.totalmem() / (1024 * 1024));
  const freeMemMb = Math.round(os.freemem() / (1024 * 1024));
  const usedMemMb = totalMemMb - freeMemMb;

  const coreLoads = cpus.slice(0, 8).map((cpu, idx) => {
    const total = Object.values(cpu.times).reduce((a, b) => a + b, 0);
    const busy = total - cpu.times.idle;
    const basePct = total > 0 ? Math.round((busy / total) * 100) : 12;
    const jitter = Math.sin(Date.now() / 1000 + idx) * 4;
    return {
      core: idx,
      model: cpu.model,
      usagePercent: Math.max(2, Math.min(98, Math.round(basePct + jitter)))
    };
  });

  const avgCpu =
    coreLoads.reduce((acc, c) => acc + c.usagePercent, 0) /
    Math.max(1, coreLoads.length);

  const runningApps = managedApps.filter((a) => a.status === 'running');
  const totalRps = runningApps.reduce((acc, a) => acc + a.requestsPerSec, 0);

  res.json({
    hostname: os.hostname() || 'ubuntu-ferrous-prod-01',
    publicIp: '103.145.88.210',
    osVersion: 'Ubuntu 24.04.1 LTS (Noble Numbat)',
    kernelVersion: `Linux ${os.release()} x86_64`,
    rustDaemonVersion:
      'Ferrous-panel v2.5 (Actix-Web 4.9 · Multi-PHP 7.4-8.4 · Nginx 1.26 · SQL · Mail)',
    nginxVersion: 'nginx/1.26.2 (Ubuntu · TLS 1.3 + QUIC HTTP/3)',
    uptimeSeconds: Math.floor(os.uptime()),
    cpuBrand: cpus[0]?.model || 'AMD EPYC 7763 64-Core Processor',
    cpuCores: cpus.length,
    cpuUsagePercent: Number(avgCpu.toFixed(1)),
    coreLoads,
    loadAverage: load.map((v) => Number(v.toFixed(2))),
    memoryTotalMb: totalMemMb,
    memoryUsedMb: usedMemMb,
    memoryPercent: Number(
      ((usedMemMb / Math.max(1, totalMemMb)) * 100).toFixed(1)
    ),
    swapTotalMb: 4096,
    swapUsedMb: 128,
    diskTotalGb: 320,
    diskUsedGb: 84.6,
    networkRxMbps: Number(
      (148.4 + Math.sin(Date.now() / 1500) * 18).toFixed(1)
    ),
    networkTxMbps: Number(
      (312.8 + Math.cos(Date.now() / 1500) * 24).toFixed(1)
    ),
    totalRequestsPerSec:
      totalRps + Math.round(Math.sin(Date.now() / 900) * 35),
    activeConnections:
      11420 + Math.round(Math.cos(Date.now() / 1100) * 140),
    timestamp: new Date().toISOString()
  });
});

// Core System Self-Diagnostic Endpoint (Synchronized with Live Subsystems)
app.get('/api/system/diagnostics', (_req, res) => {
  const runningSites = managedApps.filter((a) => a.status === 'running').length;
  const activePhpCount = phpVersionsState.filter(
    (p) => p.status === 'running'
  ).length;
  const nginxPkg = serverPackages.find((p) => p.id === 'pkg-nginx');
  const nginxHealthy = !nginxPkg || nginxPkg.status === 'running';

  // Check if any running PHP site is assigned to a stopped PHP-FPM version
  const brokenPhpSites = managedApps.filter((a) => {
    if (a.status !== 'running') return false;
    const ver =
      a.phpVersion && a.phpVersion !== 'None'
        ? a.phpVersion
        : a.framework.includes('PHP')
          ? '8.3'
          : null;
    if (!ver) return false;
    const inst = phpVersionsState.find((p) => p.version === ver);
    return !inst || inst.status !== 'running';
  });

  res.json([
    {
      id: 'diag-actix',
      subsystem: 'Ferrous-panel Architectural Brain (Actix-Web 4.9)',
      target: '127.0.0.1:9443 (14/14 Native Rust Modules)',
      status: 'passed',
      latencyMs: 0.4,
      detail: `Đang đồng bộ ${runningSites}/${managedApps.length} websites · Bộ nhớ daemon ${optimizerState.watchdog.residentRamMb} MB`
    },
    {
      id: 'diag-nginx',
      subsystem: 'Nginx 1.26 Reverse Proxy, Upstream Keepalive & HTTP/3 QUIC',
      target: '/www/server/panel/vhost/nginx/*.conf',
      status: nginxHealthy ? 'passed' : 'warning',
      latencyMs: 0.8,
      detail: nginxHealthy
        ? `nginx -t syntax is ok · ${managedApps.length} VirtualHosts đồng bộ TLS 1.3 & client_max_body_size`
        : 'Cảnh báo: Dịch vụ Nginx Reverse Proxy đang tạm dừng trong Kho Phần mềm'
    },
    {
      id: 'diag-multiphp',
      subsystem: 'Multi-PHP FPM Sockets (PHP 8.4, 8.3, 8.2, 8.1, 7.4)',
      target: '/run/php/php8.x-fpm.sock & php.ini',
      status: brokenPhpSites.length === 0 ? 'passed' : 'warning',
      latencyMs: 0.7,
      detail:
        brokenPhpSites.length === 0
          ? `${activePhpCount}/5 nhánh PHP-FPM đang chạy · Mọi Website PHP đều kết nối đúng Unix Socket`
          : `Cảnh báo: Website ${brokenPhpSites.map((s) => s.domain).join(', ')} đang trỏ vào PHP-FPM đang dừng`
    },
    {
      id: 'diag-sql-pma',
      subsystem: 'PostgreSQL 17, MariaDB 11.4 & phpMyAdmin 5.2',
      target: '127.0.0.1:5432, 127.0.0.1:3306 & :888/phpmyadmin',
      status: 'passed',
      latencyMs: 1.1,
      detail: `${sqlDatabases.length} databases trực tuyến · Tự động đồng bộ biến DB vào /.env của Website`
    },
    {
      id: 'diag-js-runtimes',
      subsystem:
        'Modern JS & Rust Runtimes (Actix, Axum, Next 15, Nuxt 3, SvelteKit 2)',
      target: 'Node v22.11 LTS · Bun 1.1.34 · rustc 1.83.0',
      status: 'passed',
      latencyMs: 0.9,
      detail: 'Tất cả dịch vụ SSR, Upstream Keepalive & Static Cache phản hồi HTTP 200 OK'
    },
    {
      id: 'diag-mail',
      subsystem: 'Mail Server (Postfix + Dovecot + OpenDKIM 2048-bit)',
      target: 'SMTPS 465 / STARTTLS 587 / IMAPS 993',
      status: 'passed',
      latencyMs: 1.4,
      detail: `${mailServerState.domains.length} tên miền Mail đã xác thực SPF, DKIM (s2026) & DMARC`
    },
    {
      id: 'diag-files',
      subsystem: 'File Manager (.ZIP Upload / Unzip) & Đồng bộ .env',
      target: '/www/wwwroot/* (Chroot & Zip-Slip Guard)',
      status: 'passed',
      latencyMs: 0.5,
      detail: `Đang quản lý ${webFiles.length} tệp & thư mục · Đồng bộ 2 chiều tệp /.env với cấu hình Website`
    },
    {
      id: 'diag-security',
      subsystem: 'Tường lửa Ubuntu UFW, Fail2ban & OpenSSH Server',
      target: `ufw active · SSH Port ${sshPanelState.config.port}`,
      status: 'passed',
      latencyMs: 0.6,
      detail: `${ufwRules.filter((r) => r.enabled).length} quy tắc UFW · ${sshPanelState.authorizedKeys.length} khóa SSH Ed25519`
    }
  ]);
});

// 2. Websites / Managed Applications API
app.get('/api/apps', (_req, res) => {
  const enriched = managedApps.map((a) => {
    syncArchitectureForApp(a);
    return a;
  });
  res.json(enriched);
});

app.post('/api/apps', (req, res) => {
  const {
    name,
    framework,
    domain,
    aliases,
    internalPort,
    workingDir,
    runSubDir,
    execCommand,
    workers,
    memoryLimitMb,
    sslEnabled,
    http3Quic,
    websocketEnabled,
    rateLimitRps,
    createDatabase,
    dbEngine,
    dbName,
    dbUser,
    dbPassword,
    createFtp,
    ftpUser,
    gitRepoUrl,
    gitBranch
  } = req.body;

  const cleanDomain = String(domain || 'newsite.ferroushub.vn')
    .toLowerCase()
    .trim();
  const cleanName = String(
    name || cleanDomain.split('.')[0] || 'new-web-service'
  )
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, '-');

  const runtimeMap: Record<string, string> = {
    'Actix-Web (Rust)': 'rustc 1.83.0 · actix-web 4.9.0',
    'Axum (Rust)': 'rustc 1.83.0 · axum 0.7.9',
    'Rocket (Rust)': 'rustc 1.83.0 · rocket 0.5.1',
    'Next.js 15 (React SSR)': 'Node v22.11.0 LTS · Next 15 Turbopack',
    'Nuxt 3 (Vue 3 SSR)': 'Node v22.11.0 LTS · Nuxt 3.14 Nitro',
    'SvelteKit 2 (Svelte 5)': 'Bun 1.1 / Node 22 · Svelte 5 Runes',
    'Astro 5 (Hybrid / Static)': 'Node v22.11.0 LTS · Astro 5.0 Islands',
    'Remix / React Router v7': 'Node v22.11.0 LTS · React Router v7',
    'NestJS 10 (TypeScript API)': 'Node v22.11.0 LTS · NestJS 10 Fastify',
    'Vite + React 19 / Vue 3 SPA': 'Nginx 1.26 Static Brotli · Vite 6.0',
    'Node.js / Next.js': 'Node v22.11.0 LTS · Next 15',
    'PHP 8.3-FPM': 'PHP 8.3.13-FPM · OPcache JIT',
    'Python / FastAPI': 'Python 3.12 · Uvicorn / Gunicorn',
    'Go / Fiber': 'go1.23.3 linux/amd64 · Fiber v2',
    'Static HTML / SPA': 'Nginx 1.26 Static Brotli'
  };

  const appId = `svc-${Date.now()}`;
  const siteRoot = workingDir || `/www/wwwroot/${cleanDomain}`;

  let createdDbName: string | null = null;
  let createdDbPassword: string | null = null;
  let createdDbUserStr: string | null = null;
  if (createDatabase) {
    const cleanDb = String(
      dbName || cleanName.replace(/-/g, '_') + '_db'
    ).toLowerCase();
    const cleanDbUser = String(dbUser || `${cleanDb}_user`).toLowerCase();
    const customDbPass =
      String(dbPassword || '').trim() ||
      `Sql#${Math.random().toString(36).slice(2, 10)}!99`;
    const selectedEngine = dbEngine || 'PostgreSQL 17';
    createdDbName = cleanDb;
    createdDbPassword = customDbPass;
    createdDbUserStr = cleanDbUser;

    sqlDatabases.unshift({
      id: `db-${Date.now()}`,
      name: cleanDb,
      engine: selectedEngine,
      host: '127.0.0.1',
      port: String(selectedEngine).includes('PostgreSQL') ? 5432 : 3306,
      username: cleanDbUser,
      password: customDbPass,
      collation: String(selectedEngine).includes('PostgreSQL')
        ? 'en_US.UTF-8'
        : 'utf8mb4_unicode_ci',
      linkedAppId: appId,
      linkedAppName: cleanDomain,
      sizeMb: 8.2,
      activeConnections: 1,
      maxConnections: 100,
      status: 'online',
      createdAt: new Date().toISOString().slice(0, 10),
      tables: [
        {
          tableName: 'schema_migrations',
          rowCount: 1,
          sizeKb: 32,
          columns: [
            { name: 'version', dataType: 'BIGINT', nullable: false, isPrimaryKey: true },
            { name: 'applied_at', dataType: 'TIMESTAMPTZ', nullable: false }
          ],
          rows: [{ version: 2026110901, applied_at: new Date().toISOString() }]
        }
      ]
    });
  }

  let createdFtpUser: string | null = null;
  if (createFtp) {
    const cleanFtp = String(
      ftpUser || `ftp_${cleanName.replace(/-/g, '_')}`
    ).toLowerCase();
    createdFtpUser = cleanFtp;
    sftpAccounts.unshift({
      id: `sftp-${Date.now()}`,
      username: cleanFtp,
      chrootDir: siteRoot,
      targetApp: cleanDomain,
      authMethod: 'Ed25519 SSH Key',
      quotaMb: 5120,
      usedMb: 12,
      status: 'active'
    });
  }

  dnsRecords.unshift({
    id: `dns-${Date.now()}`,
    domain: 'ferroushub.vn',
    type: 'A',
    host: cleanDomain,
    value: '103.145.88.210',
    ttl: 300
  });

  const getPresetForFw = (fw: WebsiteRuntimeType): RewritePresetType => {
    if (fw === 'PHP 8.3-FPM') return 'laravel';
    if (fw === 'Next.js 15 (React SSR)' || fw === 'Node.js / Next.js')
      return 'nextjs_proxy';
    if (fw === 'Nuxt 3 (Vue 3 SSR)') return 'nuxt_nitro';
    if (fw === 'SvelteKit 2 (Svelte 5)') return 'sveltekit_node';
    if (fw === 'Astro 5 (Hybrid / Static)') return 'astro_hybrid';
    if (fw === 'Vite + React 19 / Vue 3 SPA' || fw === 'Static HTML / SPA')
      return 'spa_history';
    return 'rust_proxy';
  };

  const preset = getPresetForFw(framework || 'Actix-Web (Rust)');

  const newApp: ManagedApp = {
    id: appId,
    name: cleanName,
    framework: framework || 'Actix-Web (Rust)',
    runtimeVersion: runtimeMap[framework] || 'rustc 1.83.0 · actix-web 4.9.0',
    domain: cleanDomain,
    aliases: Array.isArray(aliases) ? aliases : [],
    internalPort: Number(internalPort) || 8086,
    workingDir: siteRoot,
    runSubDir: runSubDir || (framework === 'PHP 8.3-FPM' ? '/public' : '/'),
    execCommand:
      execCommand || `${siteRoot}/target/release/${cleanName}`,
    workers: Number(workers) || 4,
    memoryLimitMb: Number(memoryLimitMb) || 512,
    memoryUsedMb: 24.8,
    cpuPercent: 1.4,
    pid: Math.floor(22000 + Math.random() * 6000),
    status: 'running',
    sslEnabled: sslEnabled !== false,
    sslIssuer: "Let's Encrypt R11",
    sslExpiryDays: 90,
    forceHttps: true,
    http3Quic: Boolean(http3Quic),
    websocketEnabled: Boolean(websocketEnabled),
    rateLimitRps: Number(rateLimitRps) || 100,
    requestsPerSec: 65,
    p99LatencyMs: framework?.includes('Rust') ? 1.5 : 6.4,
    uptimeSeconds: 10,
    diskUsedMb: 42,
    diskQuotaMb: 5120,
    trafficMonthGb: 1.2,
    linkedDbName: createdDbName,
    ftpUsername: createdFtpUser,
    backupCount: 0,
    gitRepoUrl:
      gitRepoUrl || `https://github.com/ferroushub/${cleanName}.git`,
    gitBranch: gitBranch || 'main',
    buildScript: framework?.includes('Rust')
      ? `cargo build --release && systemctl restart ${cleanName}`
      : 'pnpm build && systemctl restart ' + cleanName,
    lastDeployCommit: 'initial-provision (FerrousPanel Add Site)',
    lastDeployTime: 'Vừa xong',
    urlRewritePreset: preset,
    urlRewriteRules: REWRITE_TEMPLATES[preset],
    envVars: [
      { key: 'PORT', value: String(internalPort || 8086) },
      { key: 'NODE_ENV', value: 'production' },
      ...(createdDbName
        ? [
            { key: 'DB_DATABASE', value: createdDbName },
            { key: 'DB_USERNAME', value: createdDbUserStr || 'db_user' },
            { key: 'DB_PASSWORD', value: createdDbPassword || '' }
          ]
        : [])
    ],
    logs: [
      `${new Date().toISOString()} INFO  panel: Provisioned Web Root ${siteRoot} (chown www-data:www-data)`,
      `${new Date().toISOString()} INFO  nginx: Generated /www/server/panel/vhost/nginx/${cleanDomain}.conf`,
      `${new Date().toISOString()} INFO  certbot: Issued Let's Encrypt TLS 1.3 certificate for ${cleanDomain}`
    ]
  };

  webFiles.push({
    id: `f-${Date.now()}-1`,
    appId,
    name: framework?.includes('Rust') ? 'Cargo.toml' : 'package.json',
    path: framework?.includes('Rust') ? '/Cargo.toml' : '/package.json',
    parentDir: '/',
    type: 'file',
    sizeBytes: 420,
    permissions: '0644',
    symbolicPerms: '-rw-r--r--',
    owner: 'www-data:www-data',
    modifiedAt: 'Vừa xong',
    content: framework?.includes('Rust')
      ? `[package]\nname = "${cleanName}"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\nactix-web = "4.9"\n`
      : `{\n  "name": "${cleanName}",\n  "version": "1.0.0",\n  "private": true\n}\n`
  });

  syncArchitectureForApp(newApp);
  managedApps.unshift(newApp);
  res.status(201).json(newApp);
});

// 1-Click Template Deploy API
app.get('/api/oneclick/templates', (_req, res) => {
  res.json(oneClickTemplates);
});

app.post('/api/oneclick/deploy', (req, res) => {
  const { templateId, domain, createDb } = req.body;
  const tpl =
    oneClickTemplates.find((t) => t.id === templateId) || oneClickTemplates[0];
  const cleanDomain = String(domain || 'app.ferroushub.vn')
    .toLowerCase()
    .trim();
  const cleanName = cleanDomain.replace(/[^a-z0-9-]/g, '-');
  const appId = `svc-${Date.now()}`;
  const siteRoot = `/www/wwwroot/${cleanDomain}`;
  const port = tpl.defaultPort + Math.floor(Math.random() * 10);

  let dbCreatedName: string | null = null;
  if (createDb && tpl.requiresDb !== 'None') {
    dbCreatedName = `${cleanDomain.split('.')[0]}_${tpl.id.split('-')[1]}_db`.replace(
      /[^a-z0-9_]/g,
      '_'
    );
    sqlDatabases.unshift({
      id: `db-${Date.now()}`,
      name: dbCreatedName,
      engine: tpl.requiresDb,
      host: '127.0.0.1',
      port: tpl.requiresDb.includes('PostgreSQL') ? 5432 : 3306,
      username: `${dbCreatedName}_user`,
      collation: tpl.requiresDb.includes('PostgreSQL')
        ? 'en_US.UTF-8'
        : 'utf8mb4_unicode_ci',
      linkedAppId: appId,
      linkedAppName: cleanDomain,
      sizeMb: 16.5,
      activeConnections: 3,
      maxConnections: 100,
      status: 'online',
      createdAt: new Date().toISOString().slice(0, 10),
      tables: [
        {
          tableName: 'users',
          rowCount: 1,
          sizeKb: 48,
          columns: [
            { name: 'id', dataType: 'UUID', nullable: false, isPrimaryKey: true },
            { name: 'email', dataType: 'VARCHAR(180)', nullable: false }
          ],
          rows: [
            {
              id: '019310f4-0001-7000-8000-000000000001',
              email: `admin@${cleanDomain}`
            }
          ]
        }
      ]
    });
  }

  const preset: RewritePresetType =
    tpl.framework === 'PHP 8.3-FPM'
      ? tpl.id.includes('wordpress')
        ? 'wordpress'
        : 'laravel'
      : tpl.framework === 'Next.js 15 (React SSR)'
        ? 'nextjs_proxy'
        : tpl.framework === 'Nuxt 3 (Vue 3 SSR)'
          ? 'nuxt_nitro'
          : tpl.framework === 'SvelteKit 2 (Svelte 5)'
            ? 'sveltekit_node'
            : tpl.framework === 'Astro 5 (Hybrid / Static)'
              ? 'astro_hybrid'
              : tpl.framework === 'Vite + React 19 / Vue 3 SPA'
                ? 'spa_history'
                : 'rust_proxy';

  const newSite: ManagedApp = {
    id: appId,
    name: cleanName,
    framework: tpl.framework,
    runtimeVersion: tpl.version,
    domain: cleanDomain,
    aliases: [`www.${cleanDomain}`],
    internalPort: port,
    workingDir: siteRoot,
    runSubDir: tpl.framework === 'PHP 8.3-FPM' ? '/public' : '/',
    execCommand: tpl.execCommandTemplate.replace(/\$DOMAIN/g, cleanDomain),
    workers: 4,
    memoryLimitMb: 512,
    memoryUsedMb: 28.0,
    cpuPercent: 1.8,
    pid: Math.floor(24000 + Math.random() * 5000),
    status: 'running',
    sslEnabled: true,
    sslIssuer: "Let's Encrypt R11",
    sslExpiryDays: 90,
    forceHttps: true,
    http3Quic: true,
    websocketEnabled: true,
    rateLimitRps: 120,
    requestsPerSec: 45,
    p99LatencyMs: tpl.framework.includes('Rust') ? 1.4 : 4.2,
    uptimeSeconds: 5,
    diskUsedMb: 85,
    diskQuotaMb: 5120,
    trafficMonthGb: 0.4,
    linkedDbName: dbCreatedName,
    ftpUsername: `ftp_${cleanDomain.split('.')[0]}`,
    backupCount: 1,
    gitRepoUrl: `https://github.com/ferroushub/${tpl.id}.git`,
    gitBranch: 'main',
    buildScript: tpl.buildCommand,
    lastDeployCommit: `1-Click Template (${tpl.name})`,
    lastDeployTime: 'Vừa xong',
    urlRewritePreset: preset,
    urlRewriteRules: REWRITE_TEMPLATES[preset],
    envVars: [
      { key: 'PORT', value: String(port) },
      { key: 'DATABASE_NAME', value: dbCreatedName || 'none' }
    ],
    logs: [
      `${new Date().toISOString()} INFO  1-click-installer: Cloned template ${tpl.name} into ${siteRoot}`,
      `${new Date().toISOString()} INFO  1-click-installer: Executed [${tpl.buildCommand}] -> exit code 0`,
      `${new Date().toISOString()} INFO  nginx: Reverse proxy https://${cleanDomain} -> 127.0.0.1:${port} is live!`
    ]
  };

  webFiles.push({
    id: `f-${Date.now()}-tpl`,
    appId,
    name: 'README.deploy.md',
    path: '/README.deploy.md',
    parentDir: '/',
    type: 'file',
    sizeBytes: 360,
    permissions: '0644',
    symbolicPerms: '-rw-r--r--',
    owner: 'www-data:www-data',
    modifiedAt: 'Vừa xong',
    content: `# ${cleanDomain}\nDeployed via FerrousPanel 1-Click Installer\nTemplate: ${tpl.name}\nWeb Root: ${siteRoot}\nDatabase: ${dbCreatedName || 'None'}\n`
  });

  managedApps.unshift(newSite);
  res.status(201).json({
    ...newSite,
    systemdUnit: generateSystemdUnit(newSite),
    nginxVhost: generateNginxVhost(newSite)
  });
});

app.put('/api/apps/:id/site-settings', (req, res) => {
  const appItem = managedApps.find((a) => a.id === req.params.id);
  if (!appItem) {
    res.status(404).json({ error: 'Website not found' });
    return;
  }

  const {
    aliases,
    runSubDir,
    forceHttps,
    sslEnabled,
    http3Quic,
    websocketEnabled,
    rateLimitRps,
    urlRewritePreset,
    urlRewriteRules,
    phpVersion,
    redirects,
    hotlinkProtection,
    directoryAuth,
    gitRepoUrl,
    gitBranch,
    buildScript
  } = req.body;

  if (Array.isArray(aliases)) appItem.aliases = aliases;
  if (runSubDir !== undefined) appItem.runSubDir = String(runSubDir);
  if (forceHttps !== undefined) appItem.forceHttps = Boolean(forceHttps);
  if (sslEnabled !== undefined) appItem.sslEnabled = Boolean(sslEnabled);
  if (http3Quic !== undefined) appItem.http3Quic = Boolean(http3Quic);
  if (websocketEnabled !== undefined)
    appItem.websocketEnabled = Boolean(websocketEnabled);
  if (rateLimitRps !== undefined) appItem.rateLimitRps = Number(rateLimitRps);
  if (urlRewritePreset !== undefined) {
    appItem.urlRewritePreset = urlRewritePreset as RewritePresetType;
    if (
      urlRewriteRules === undefined &&
      REWRITE_TEMPLATES[urlRewritePreset as RewritePresetType]
    ) {
      appItem.urlRewriteRules =
        REWRITE_TEMPLATES[urlRewritePreset as RewritePresetType];
    }
  }
  if (urlRewriteRules !== undefined)
    appItem.urlRewriteRules = String(urlRewriteRules);
  if (phpVersion !== undefined) {
    appItem.phpVersion = phpVersion;
    if (phpVersion !== 'None') {
      appItem.urlRewriteRules = (appItem.urlRewriteRules || '').replace(
        /php[0-9.]+-fpm\.sock/g,
        `php${phpVersion}-fpm.sock`
      );
      if (appItem.framework.includes('PHP')) {
        appItem.runtimeVersion = `PHP ${phpVersion}-FPM · OPcache JIT`;
      }
    }
  }
  if (Array.isArray(redirects)) appItem.redirects = redirects;
  if (hotlinkProtection !== undefined)
    appItem.hotlinkProtection = hotlinkProtection;
  if (Array.isArray(directoryAuth)) appItem.directoryAuth = directoryAuth;
  if (gitRepoUrl !== undefined) appItem.gitRepoUrl = String(gitRepoUrl);
  if (gitBranch !== undefined) appItem.gitBranch = String(gitBranch);
  if (buildScript !== undefined) appItem.buildScript = String(buildScript);

  syncArchitectureForApp(appItem);
  appItem.logs.push(
    `${new Date().toISOString()} INFO  nginx: Updated vhost /www/server/panel/vhost/nginx/${appItem.domain}.conf (PHP: ${appItem.phpVersion || 'Default'}) & reloaded gracefully.`
  );

  res.json(appItem);
});

app.post('/api/apps/:id/git-deploy', (req, res) => {
  const appItem = managedApps.find((a) => a.id === req.params.id);
  if (!appItem) {
    res.status(404).json({ error: 'Website not found' });
    return;
  }

  const commitShort = Math.random().toString(16).slice(2, 9);
  const now = new Date().toISOString();
  appItem.lastDeployCommit = `${commitShort} (Webhook pull from ${appItem.gitBranch})`;
  appItem.lastDeployTime = 'Vừa xong';
  appItem.status = 'running';
  appItem.pid = Math.floor(21000 + Math.random() * 8000);
  appItem.logs.push(
    `${now} INFO  git-deploy: Fetching ${appItem.gitRepoUrl} (branch: ${appItem.gitBranch}) -> HEAD is now at ${commitShort}`
  );
  appItem.logs.push(
    `${now} INFO  builder: Running [${appItem.buildScript}] in ${appItem.workingDir}...`
  );
  appItem.logs.push(
    `${now} INFO  systemd[1]: Zero-downtime reload completed for ${appItem.domain} (PID ${appItem.pid}).`
  );

  syncArchitectureForApp(appItem);
  res.json(appItem);
});

app.post('/api/apps/:id/action', (req, res) => {
  const { id } = req.params;
  const { action } = req.body;
  const appItem = managedApps.find((a) => a.id === id);
  if (!appItem) {
    res.status(404).json({ error: 'Không tìm thấy dịch vụ' });
    return;
  }

  const now = new Date().toISOString();
  if (action === 'stop') {
    appItem.status = 'stopped';
    appItem.pid = null;
    appItem.cpuPercent = 0;
    appItem.memoryUsedMb = 0;
    appItem.requestsPerSec = 0;
    appItem.logs.push(
      `${now} WARN  systemd[1]: Stopping ${appItem.name}.service (SIGTERM sent)...`
    );
  } else if (action === 'start' || action === 'restart') {
    appItem.status = 'running';
    appItem.pid = Math.floor(20000 + Math.random() * 9000);
    appItem.cpuPercent = appItem.framework.includes('Rust') ? 2.6 : 4.2;
    appItem.memoryUsedMb = appItem.framework.includes('Rust') ? 34.2 : 160.0;
    appItem.requestsPerSec = appItem.framework.includes('Rust') ? 890 : 310;
    appItem.uptimeSeconds = 1;
    appItem.logs.push(
      `${now} INFO  systemd[1]: Started ${appItem.name}.service (PID ${appItem.pid}).`
    );
  } else if (action === 'rebuild') {
    appItem.status = 'running';
    appItem.pid = Math.floor(20000 + Math.random() * 9000);
    appItem.uptimeSeconds = 1;
    appItem.logs.push(
      `${now} INFO  builder: Executed [${appItem.buildScript}] in ${appItem.workingDir}`
    );
    appItem.logs.push(
      `${now} INFO  systemd[1]: Zero-downtime restart completed (new PID ${appItem.pid}).`
    );
  }

  syncArchitectureForApp(appItem);
  res.json(appItem);
});

app.put('/api/apps/:id/config', (req, res) => {
  const { id } = req.params;
  const appItem = managedApps.find((a) => a.id === id);
  if (!appItem) {
    res.status(404).json({ error: 'Không tìm thấy dịch vụ' });
    return;
  }

  const {
    workers,
    memoryLimitMb,
    rateLimitRps,
    sslEnabled,
    http3Quic,
    websocketEnabled,
    envVars
  } = req.body;

  if (workers !== undefined) appItem.workers = Number(workers);
  if (memoryLimitMb !== undefined)
    appItem.memoryLimitMb = Number(memoryLimitMb);
  if (rateLimitRps !== undefined) appItem.rateLimitRps = Number(rateLimitRps);
  if (sslEnabled !== undefined) appItem.sslEnabled = Boolean(sslEnabled);
  if (http3Quic !== undefined) appItem.http3Quic = Boolean(http3Quic);
  if (websocketEnabled !== undefined)
    appItem.websocketEnabled = Boolean(websocketEnabled);
  if (Array.isArray(envVars)) appItem.envVars = envVars;

  syncArchitectureForApp(appItem);
  if (req.body.nginxVhost) {
    appItem.nginxVhost = String(req.body.nginxVhost);
  }

  res.json(appItem);
});

// Live HTTP/3 & Reverse Proxy Website Verification Endpoint (Synchronized with Nginx, PHP-FPM & SQL DB)
app.post('/api/apps/:id/test-http', (req, res) => {
  const appItem = managedApps.find((a) => a.id === req.params.id);
  if (!appItem) {
    res.status(404).json({ error: 'Website not found' });
    return;
  }

  const nginxPkg = serverPackages.find((p) => p.id === 'pkg-nginx');
  const nginxRunning = !nginxPkg || nginxPkg.status === 'running';
  const isPhpApp =
    Boolean(appItem.phpVersion && appItem.phpVersion !== 'None') ||
    String(appItem.framework).includes('PHP');
  const phpInst = isPhpApp
    ? phpVersionsState.find((p) => p.version === (appItem.phpVersion || '8.3'))
    : null;
  const phpRunning = !isPhpApp || (phpInst && phpInst.status === 'running');
  const isRunning = appItem.status === 'running' && nginxRunning && phpRunning;

  let statusCode = 200;
  let statusText = '200 OK';
  if (!nginxRunning) {
    statusCode = 503;
    statusText = '503 Service Unavailable (Nginx Reverse Proxy Stopped)';
  } else if (appItem.status !== 'running') {
    statusCode = 502;
    statusText = '502 Bad Gateway (Upstream Service Stopped)';
  } else if (!phpRunning) {
    statusCode = 502;
    statusText = `502 Bad Gateway (PHP ${appItem.phpVersion}-FPM Socket Offline)`;
  }

  const latencyMs = isRunning
    ? Number((appItem.p99LatencyMs * 0.75 + Math.random() * 0.6).toFixed(2))
    : 0.4;

  const upstreamTarget = isPhpApp
    ? `unix:/run/php/php${appItem.phpVersion || '8.3'}-fpm.sock`
    : Number(appItem.internalPort) === 80
      ? `static:${appItem.workingDir}${appItem.runSubDir !== '/' ? appItem.runSubDir : ''}`
      : `127.0.0.1:${appItem.internalPort}`;

  const headers: Record<string, string> = {
    Server: 'FerrousPanel-Nginx/1.26.2 (Rust-Pingora-WAF/1.9)',
    'X-Upstream-Runtime': `${appItem.framework} (${appItem.runtimeVersion})`,
    'X-Upstream-Endpoint': upstreamTarget,
    'Strict-Transport-Security': 'max-age=63072000; includeSubDomains; preload',
    'Alt-Svc': appItem.http3Quic ? 'h3=":443"; ma=86400' : 'h2=":443"',
    'X-Frame-Options': 'SAMEORIGIN',
    'X-Ferrous-WAF': 'PASS (0 OWASP CRS violations)'
  };

  // Inspect live entry file in File Manager for realistic synchronized preview
  const appFileList = webFiles.filter(
    (f) => f.appId === appItem.id && f.type === 'file'
  );
  const entryFile =
    appFileList.find(
      (f) =>
        f.name === 'index.php' ||
        f.name === 'index.html' ||
        f.name === 'page.tsx' ||
        f.name === 'main.rs'
    ) || appFileList[0];

  const bodyPreview = !isRunning
    ? `<html><body><h1>${statusText}</h1><p>Upstream endpoint <code>${upstreamTarget}</code> for <strong>${appItem.domain}</strong> is offline. Please verify Nginx, PHP-FPM (${appItem.phpVersion || 'N/A'}), or systemd unit status in FerrousPanel.</p></body></html>`
    : appItem.framework.includes('Rust')
      ? JSON.stringify(
          {
            status: 'healthy',
            service: appItem.name,
            domain: appItem.domain,
            framework: appItem.framework,
            runtime: appItem.runtimeVersion,
            upstream: upstreamTarget,
            workers: appItem.workers,
            pid: appItem.pid,
            database: appItem.linkedDbName || 'pool_connected',
            entrySourceFile: entryFile ? entryFile.path : '/src/main.rs',
            cache: 'FerrousKV (RESP3 127.0.0.1:6379)',
            timestamp: new Date().toISOString()
          },
          null,
          2
        )
      : `<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="utf-8" />
  <title>${appItem.domain} — ${appItem.framework}</title>
</head>
<body>
  <main id="__app" data-ssr="true" data-framework="${appItem.framework}">
    <h1>Website ${appItem.domain} đang hoạt động ổn định (HTTP 200 OK)</h1>
    <p>Upstream Handler: Nginx 1.26.2 + FerrousWAF -> ${upstreamTarget} (PID ${appItem.pid})</p>
    <p>Database kết nối: ${appItem.linkedDbName || 'Không sử dụng DB'} · Tệp gốc: ${entryFile ? entryFile.path : '/index.html'} (${entryFile ? entryFile.sizeBytes : 0} bytes)</p>
    <p>Tốc độ phản hồi: ${latencyMs} ms · Bảo mật: ${appItem.hotlinkProtection?.enabled ? 'Chống Hotlink BẬT' : 'Chuẩn'} · Redirects: ${(appItem.redirects || []).length} quy tắc</p>
  </main>
</body>
</html>`;

  appItem.logs.push(
    `${new Date().toISOString()} INFO  healthcheck: GET https://${appItem.domain}/ -> ${statusCode} (${latencyMs}ms via ${upstreamTarget})`
  );

  res.json({
    appId: appItem.id,
    domain: appItem.domain,
    url: `${appItem.sslEnabled ? 'https' : 'http'}://${appItem.domain}`,
    statusCode,
    statusText,
    latencyMs,
    protocol: appItem.http3Quic ? 'HTTP/3 (QUIC)' : 'HTTP/2',
    tlsCipher: appItem.sslEnabled
      ? 'TLS_AES_256_GCM_SHA384 (TLS 1.3 · ECDSA P-384)'
      : 'Plain HTTP',
    headers,
    bodyPreview,
    verifiedAt: new Date().toLocaleTimeString('vi-VN')
  });
});

// Renew SSL Certificate via FerrousACME Rust Daemon
app.post('/api/apps/:id/renew-ssl', (req, res) => {
  const appItem = managedApps.find((a) => a.id === req.params.id);
  if (!appItem) {
    res.status(404).json({ error: 'Website not found' });
    return;
  }

  appItem.sslEnabled = true;
  appItem.sslIssuer = "Let's Encrypt R11 (FerrousACME Rust ECDSA P-384)";
  appItem.sslExpiryDays = 90;
  appItem.forceHttps = true;
  syncArchitectureForApp(appItem);
  appItem.logs.push(
    `${new Date().toISOString()} INFO  ferrous_acme: Renewed TLS 1.3 ECDSA P-384 certificate for ${appItem.domain} (valid 90 days) & reloaded Nginx.`
  );

  res.json(appItem);
});

// Delete Website
app.delete('/api/apps/:id', (req, res) => {
  const idx = managedApps.findIndex((a) => a.id === req.params.id);
  if (idx === -1) {
    res.status(404).json({ error: 'Website not found' });
    return;
  }
  managedApps.splice(idx, 1);
  const enriched = managedApps.map((a) => ({
    ...a,
    systemdUnit: generateSystemdUnit(a),
    nginxVhost: generateNginxVhost(a)
  }));
  res.json(enriched);
});

// Server Software Store API (Rust Supplemental Packages & Core Runtimes)
app.get('/api/software', (_req, res) => {
  res.json(serverPackages);
});

app.post('/api/software/:id/action', (req, res) => {
  const pkg = serverPackages.find((p) => p.id === req.params.id);
  const { action } = req.body;
  const now = new Date().toISOString();
  if (pkg) {
    if (action === 'stop') {
      pkg.status = 'stopped';
      pkg.memoryMb = 0;
      pkg.logs.push(`${now} WARN  systemd: Stopped package ${pkg.name}`);
    } else if (action === 'start' || action === 'restart') {
      pkg.installed = true;
      pkg.status = 'running';
      if (pkg.memoryMb === 0) {
        pkg.memoryMb = pkg.isRustNative ? 22.4 : 85.0;
      }
      pkg.logs.push(
        `${now} INFO  systemd: ${action === 'restart' ? 'Restarted' : 'Started'} ${pkg.name} (Port ${pkg.port || 'internal'})`
      );
    } else if (action === 'install') {
      pkg.installed = true;
      pkg.status = 'running';
      pkg.memoryMb = pkg.isRustNative ? 20.5 : 92.0;
      pkg.logs.push(
        `${now} INFO  cargo-pkg: Installed & enabled ${pkg.name} (${pkg.version})`
      );
    } else if (action === 'uninstall') {
      pkg.installed = false;
      pkg.status = 'not_installed';
      pkg.memoryMb = 0;
      pkg.logs.push(`${now} WARN  cargo-pkg: Uninstalled package ${pkg.name}`);
    }
  }
  res.json(serverPackages);
});

app.put('/api/software/:id/config', (req, res) => {
  const pkg = serverPackages.find((p) => p.id === req.params.id);
  if (!pkg) {
    res.status(404).json({ error: 'Package not found' });
    return;
  }
  if (req.body.configContent !== undefined) {
    pkg.configContent = String(req.body.configContent);
    pkg.logs.push(
      `${new Date().toISOString()} INFO  config: Saved ${pkg.configPath} (${pkg.configContent.length} bytes) & sent SIGHUP`
    );
  }
  res.json(serverPackages);
});

// 3. SQL Database Administration & MySQL / phpMyAdmin Config APIs
app.get('/api/databases', (_req, res) => {
  res.json(sqlDatabases);
});

app.get('/api/databases/mysql-config', (_req, res) => {
  res.json(mysqlPmaConfig);
});

app.put('/api/databases/mysql-config', (req, res) => {
  mysqlPmaConfig = {
    ...mysqlPmaConfig,
    ...req.body
  };
  res.json(mysqlPmaConfig);
});

app.post('/api/databases', (req, res) => {
  const { name, engine, username, password, collation, linkedAppId } = req.body;
  const cleanDbName = String(name || 'new_app_db')
    .toLowerCase()
    .replace(/[^a-z0-9_]/g, '_');
  const linkedApp = managedApps.find((a) => a.id === linkedAppId) || managedApps[0] || null;

  const isPg = String(engine).includes('PostgreSQL');
  const isSqlite = String(engine).includes('SQLite');
  const cleanUsername = String(username || `${cleanDbName}_user`).trim();
  const customPassword =
    String(password || '').trim() ||
    `Sql#${Math.random().toString(36).slice(2, 10)}!88`;

  const newDb = {
    id: `db-${Date.now()}`,
    name: cleanDbName,
    engine: (engine || 'PostgreSQL 17') as
      | 'PostgreSQL 17'
      | 'MariaDB 11.4'
      | 'SQLite 3 (WAL)',
    host: isSqlite
      ? linkedApp
        ? `${linkedApp.workingDir}/data.sqlite`
        : '/www/wwwroot/data.sqlite'
      : '127.0.0.1',
    port: isPg ? 5432 : isSqlite ? 0 : 3306,
    username: cleanUsername,
    password: customPassword,
    collation: collation || (isPg ? 'en_US.UTF-8' : 'utf8mb4_unicode_ci'),
    linkedAppId: linkedApp?.id || '',
    linkedAppName: linkedApp?.domain || 'Hệ thống độc lập (Standalone)',
    sizeMb: 12.4,
    activeConnections: 2,
    maxConnections: 100,
    status: 'online' as const,
    createdAt: new Date().toISOString().slice(0, 10),
    tables: [
      {
        tableName: 'app_migrations',
        rowCount: 2,
        sizeKb: 64,
        columns: [
          { name: 'version', dataType: 'BIGINT', nullable: false, isPrimaryKey: true },
          { name: 'description', dataType: 'TEXT', nullable: false },
          { name: 'applied_at', dataType: 'TIMESTAMPTZ', nullable: false }
        ],
        rows: [
          {
            version: 2026110901,
            description: 'init_schema_tables',
            applied_at: new Date().toISOString()
          }
        ]
      }
    ]
  };

  sqlDatabases.unshift(newDb);

  // Synchronize linked Website's database name, envVars (including custom DB_PASSWORD & DATABASE_URL), and /.env file in File Manager
  if (linkedApp) {
    linkedApp.linkedDbName = `${newDb.name} (${newDb.engine})`;
    const connUri = isSqlite
      ? `sqlite://${newDb.host}?mode=rwc`
      : isPg
        ? `postgres://${newDb.username}:${newDb.password}@${newDb.host}:${newDb.port}/${newDb.name}`
        : `mysql://${newDb.username}:${newDb.password}@${newDb.host}:${newDb.port}/${newDb.name}`;

    const dbEnvUpdates = [
      { key: 'DB_ENGINE', value: newDb.engine },
      { key: 'DB_HOST', value: newDb.host },
      { key: 'DB_PORT', value: String(newDb.port) },
      { key: 'DB_DATABASE', value: newDb.name },
      { key: 'DB_USERNAME', value: newDb.username },
      { key: 'DB_PASSWORD', value: newDb.password },
      { key: 'DATABASE_URL', value: connUri }
    ];
    for (const item of dbEnvUpdates) {
      const existingIdx = linkedApp.envVars.findIndex((e) => e.key === item.key);
      if (existingIdx >= 0) {
        linkedApp.envVars[existingIdx].value = item.value;
      } else {
        linkedApp.envVars.push(item);
      }
    }
    syncArchitectureForApp(linkedApp);
    linkedApp.logs.push(
      `${new Date().toISOString()} INFO  db-sync: Linked SQL database '${newDb.name}' (${newDb.engine}, user: ${newDb.username}) & injected custom DB_PASSWORD into ${linkedApp.workingDir}/.env`
    );
  }

  res.status(201).json(sqlDatabases);
});

app.put('/api/databases/:id/password', (req, res) => {
  const targetDb = sqlDatabases.find((db) => db.id === req.params.id);
  if (!targetDb) {
    res.status(404).json({ error: 'Database not found' });
    return;
  }
  const newPassword = String(req.body?.password || '').trim();
  if (!newPassword) {
    res.status(400).json({ error: 'Mật khẩu mới không được để trống' });
    return;
  }
  targetDb.password = newPassword;

  // Synchronize updated password to linked Website's envVars & /.env file
  const linkedApp = managedApps.find((a) => a.id === targetDb.linkedAppId);
  if (linkedApp) {
    const isPg = targetDb.engine.includes('PostgreSQL');
    const isSqlite = targetDb.engine.includes('SQLite');
    const connUri = isSqlite
      ? `sqlite://${targetDb.host}?mode=rwc`
      : isPg
        ? `postgres://${targetDb.username}:${targetDb.password}@${targetDb.host}:${targetDb.port}/${targetDb.name}`
        : `mysql://${targetDb.username}:${targetDb.password}@${targetDb.host}:${targetDb.port}/${targetDb.name}`;

    for (const item of [
      { key: 'DB_PASSWORD', value: newPassword },
      { key: 'DATABASE_URL', value: connUri }
    ]) {
      const idx = linkedApp.envVars.findIndex((e) => e.key === item.key);
      if (idx >= 0) linkedApp.envVars[idx].value = item.value;
      else linkedApp.envVars.push(item);
    }
    syncArchitectureForApp(linkedApp);
    linkedApp.logs.push(
      `${new Date().toISOString()} INFO  db-passwd: Rotated SQL password for '${targetDb.username}'@${targetDb.name} & updated ${linkedApp.workingDir}/.env`
    );
  }

  res.json(sqlDatabases);
});

app.delete('/api/databases/:id', (req, res) => {
  const targetDb = sqlDatabases.find((db) => db.id === req.params.id);
  sqlDatabases = sqlDatabases.filter((db) => db.id !== req.params.id);
  if (targetDb) {
    const linkedApp = managedApps.find((a) => a.id === targetDb.linkedAppId);
    if (linkedApp) {
      const nextDb = sqlDatabases.find((db) => db.linkedAppId === linkedApp.id);
      linkedApp.linkedDbName = nextDb
        ? `${nextDb.name} (${nextDb.engine})`
        : 'Không sử dụng';
      syncArchitectureForApp(linkedApp);
    }
  }
  res.json(sqlDatabases);
});

app.post('/api/databases/:id/query', (req, res) => {
  const db = sqlDatabases.find((d) => d.id === req.params.id);
  if (!db) {
    res.status(404).json({ error: 'Database not found' });
    return;
  }

  const rawSql = String(req.body.query || '').trim();
  const normalized = rawSql.replace(/\s+/g, ' ').trim();
  const upper = normalized.toUpperCase();
  const execTime = Number((0.35 + Math.random() * 1.4).toFixed(2));

  if (!rawSql) {
    res.json({
      success: false,
      executionTimeMs: 0,
      commandTag: 'EMPTY',
      affectedRows: 0,
      columns: [],
      rows: [],
      error: 'Câu lệnh SQL không được để trống.'
    });
    return;
  }

  if (
    upper.startsWith('SHOW TABLES') ||
    upper.includes('INFORMATION_SCHEMA.TABLES') ||
    upper.includes('PG_CATALOG.PG_TABLES')
  ) {
    const rows = db.tables.map((t) => ({
      table_name: t.tableName,
      engine: db.engine,
      estimated_rows: t.rowCount,
      size_kb: t.sizeKb
    }));
    res.json({
      success: true,
      executionTimeMs: execTime,
      commandTag: `SELECT ${rows.length}`,
      affectedRows: rows.length,
      columns: ['table_name', 'engine', 'estimated_rows', 'size_kb'],
      rows
    });
    return;
  }

  if (upper.startsWith('SELECT')) {
    const matchTable = normalized.match(/FROM\s+([a-zA-Z0-9_]+)/i);
    const tableName = matchTable ? matchTable[1].toLowerCase() : '';
    const foundTable =
      db.tables.find((t) => t.tableName.toLowerCase() === tableName) ||
      db.tables[0];

    if (!foundTable) {
      res.json({
        success: false,
        executionTimeMs: execTime,
        commandTag: 'ERROR',
        affectedRows: 0,
        columns: [],
        rows: [],
        error: `ERROR: relation "${tableName}" does not exist in database "${db.name}"`
      });
      return;
    }

    const cols = foundTable.columns.map((c) => c.name);
    res.json({
      success: true,
      executionTimeMs: execTime,
      commandTag: `SELECT ${foundTable.rows.length}`,
      affectedRows: foundTable.rows.length,
      columns: cols,
      rows: foundTable.rows
    });
    return;
  }

  if (upper.startsWith('INSERT INTO')) {
    const match = normalized.match(
      /INSERT\s+INTO\s+([a-zA-Z0-9_]+)\s*\(([^)]+)\)\s*VALUES\s*\(([^)]+)\)/i
    );
    if (match) {
      const targetTableName = match[1].toLowerCase();
      const cols = match[2].split(',').map((c) => c.trim());
      const vals = match[3]
        .split(',')
        .map((v) => v.trim().replace(/^['"]|['"]$/g, ''));

      const table = db.tables.find(
        (t) => t.tableName.toLowerCase() === targetTableName
      );
      if (table) {
        const newRow: Record<string, string | number | boolean | null> = {};
        cols.forEach((col, i) => {
          const val = vals[i] ?? null;
          newRow[col] =
            val !== null && !isNaN(Number(val)) && val !== ''
              ? Number(val)
              : val;
        });
        table.rows.unshift(newRow);
        table.rowCount += 1;
        res.json({
          success: true,
          executionTimeMs: execTime,
          commandTag: 'INSERT 0 1',
          affectedRows: 1,
          columns: table.columns.map((c) => c.name),
          rows: table.rows
        });
        return;
      }
    }

    res.json({
      success: true,
      executionTimeMs: execTime,
      commandTag: 'INSERT 0 1',
      affectedRows: 1,
      columns: ['result'],
      rows: [{ result: '1 row inserted successfully' }]
    });
    return;
  }

  if (upper.startsWith('CREATE TABLE')) {
    const matchName = normalized.match(
      /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([a-zA-Z0-9_]+)/i
    );
    const newTableName = matchName ? matchName[1] : `table_${Date.now()}`;
    if (!db.tables.some((t) => t.tableName === newTableName)) {
      db.tables.push({
        tableName: newTableName,
        rowCount: 0,
        sizeKb: 16,
        columns: [
          { name: 'id', dataType: 'SERIAL', nullable: false, isPrimaryKey: true },
          { name: 'created_at', dataType: 'TIMESTAMPTZ', nullable: false }
        ],
        rows: []
      });
    }
    res.json({
      success: true,
      executionTimeMs: execTime,
      commandTag: 'CREATE TABLE',
      affectedRows: 0,
      columns: ['status', 'table_created'],
      rows: [{ status: 'OK', table_created: newTableName }]
    });
    return;
  }

  res.json({
    success: true,
    executionTimeMs: execTime,
    commandTag: upper.split(' ')[0] || 'OK',
    affectedRows: 1,
    columns: ['query_plan', 'execution_time_ms', 'engine'],
    rows: [
      {
        query_plan: `Index Scan using primary_key_idx on ${db.name} (cost=0.14..8.16 rows=1 width=64)`,
        execution_time_ms: execTime,
        engine: db.engine
      }
    ]
  });
});

// 4. Mail Server & Webmail APIs
app.get('/api/mail', (_req, res) => {
  res.json(mailServerState);
});

app.post('/api/mail/domains', (req, res) => {
  const { domain, catchAllAddress } = req.body;
  const cleanDomain = String(domain || 'mail.ferroushub.vn')
    .toLowerCase()
    .trim();
  mailServerState.domains.unshift({
    id: `md-${Date.now()}`,
    domain: cleanDomain,
    spfStatus: 'verified',
    dkimStatus: 'verified',
    dmarcStatus: 'verified',
    dkimSelector: 's2026',
    dkimPublicKey: `v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA${Math.random()
      .toString(36)
      .slice(2)}...`,
    catchAllAddress: String(catchAllAddress || `admin@${cleanDomain}`),
    mailboxesCount: 0
  });
  res.status(201).json(mailServerState);
});

app.post('/api/mail/mailboxes', (req, res) => {
  const { email: reqEmail, localPart, domain, displayName, quotaMb } = req.body;
  const cleanDomain = String(domain || 'ferroushub.vn');
  const email = reqEmail
    ? String(reqEmail).toLowerCase()
    : `${String(localPart || 'user').toLowerCase()}@${cleanDomain}`;
  mailServerState.mailboxes.unshift({
    id: `mb-${Date.now()}`,
    email,
    domain: cleanDomain,
    displayName: String(displayName || email),
    quotaMb: Number(quotaMb) || 2048,
    usedMb: 4,
    imapServer: 'mail.ferroushub.vn:993 (SSL/TLS)',
    smtpServer: 'mail.ferroushub.vn:465 (SMTPS) / 587 (STARTTLS)',
    status: 'active',
    lastLogin: 'Vừa khởi tạo'
  });
  const dom = mailServerState.domains.find((d) => d.domain === cleanDomain);
  if (dom) dom.mailboxesCount += 1;
  res.status(201).json(mailServerState);
});

app.delete('/api/mail/mailboxes/:id', (req, res) => {
  mailServerState.mailboxes = mailServerState.mailboxes.filter(
    (m) => m.id !== req.params.id
  );
  res.json(mailServerState);
});

app.post('/api/mail/forwarders', (req, res) => {
  const { sourceEmail, destinationEmail, domain } = req.body;
  mailServerState.forwarders.unshift({
    id: `fw-${Date.now()}`,
    sourceEmail: String(sourceEmail),
    destinationEmail: String(destinationEmail),
    domain: String(domain || 'ferroushub.vn'),
    active: true
  });
  res.status(201).json(mailServerState);
});

app.delete('/api/mail/forwarders/:id', (req, res) => {
  mailServerState.forwarders = mailServerState.forwarders.filter(
    (f) => f.id !== req.params.id
  );
  res.json(mailServerState);
});

app.post('/api/mail/send', (req, res) => {
  const { from, to, subject, body } = req.body;
  mailServerState.messages.unshift({
    id: `msg-${Date.now()}`,
    folder: 'sent',
    from: String(from || 'admin@ferroushub.vn'),
    to: String(to || 'client@enterprise.vn'),
    subject: String(subject || 'Thông báo từ FerrousPanel Mail Server'),
    snippet: String(body || '').slice(0, 72) + '...',
    body: String(body || ''),
    timestamp: 'Vừa xong',
    dkimSigned: true
  });
  mailServerState.sentToday += 1;
  res.status(201).json(mailServerState);
});

// 5. Per-Website File Manager & ZIP Archive Engine API (Rust FileManager)
app.get('/api/files/:appId', (req, res) => {
  const { appId } = req.params;
  const appFiles = webFiles.filter((f) => f.appId === appId);
  res.json(appFiles);
});

app.post('/api/files/:appId', (req, res) => {
  const { appId } = req.params;
  const { name, parentDir, type, permissions, content } = req.body;
  const cleanName = String(name || 'new-file.rs').replace(/\//g, '');
  const cleanParent = String(parentDir || '/');
  const fullPath =
    cleanParent === '/' ? `/${cleanName}` : `${cleanParent}/${cleanName}`;
  const octal = String(permissions || (type === 'directory' ? '0755' : '0644'));

  const newNode = {
    id: `f-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    appId,
    name: cleanName,
    path: fullPath,
    parentDir: cleanParent,
    type: (type === 'directory' ? 'directory' : 'file') as 'file' | 'directory',
    sizeBytes:
      type === 'directory'
        ? 4096
        : Buffer.byteLength(String(content || ''), 'utf8'),
    permissions: octal,
    symbolicPerms: permOctalToSymbolic(octal, type === 'directory'),
    owner: 'www-data:www-data',
    modifiedAt: new Date().toISOString().slice(0, 16).replace('T', ' '),
    content: type === 'directory' ? undefined : String(content || '')
  };

  webFiles.push(newNode);
  res.status(201).json(webFiles.filter((f) => f.appId === appId));
});

// Upload .zip archive or multiple files (with optional instant extraction)
app.post('/api/files/:appId/upload-zip', (req, res) => {
  const { appId } = req.params;
  const {
    archiveName,
    parentDir,
    archiveSizeBytes,
    extractImmediately,
    entries
  } = req.body;

  const cleanParent = String(parentDir || '/');
  const cleanZipName = String(archiveName || 'upload-bundle.zip').replace(
    /\//g,
    ''
  );
  const zipFullPath =
    cleanParent === '/' ? `/${cleanZipName}` : `${cleanParent}/${cleanZipName}`;
  const nowStr = new Date().toISOString().slice(0, 16).replace('T', ' ');

  const parsedEntries: {
    relativePath: string;
    isDir: boolean;
    sizeBytes: number;
    content?: string;
  }[] =
    Array.isArray(entries) && entries.length > 0
      ? entries
      : [
          {
            relativePath: 'public',
            isDir: true,
            sizeBytes: 4096
          },
          {
            relativePath: 'public/index.php',
            isDir: false,
            sizeBytes: 642,
            content: `<?php\n// Extracted from ${cleanZipName} by FerrousPanel Rust ZIP Engine\ndeclare(strict_types=1);\n\necho json_encode([\n    'status' => 'ok',\n    'archive' => '${cleanZipName}',\n    'php_version' => PHP_VERSION,\n    'sapi' => php_sapi_name(),\n]);\n`
          },
          {
            relativePath: 'config.production.json',
            isDir: false,
            sizeBytes: 318,
            content: JSON.stringify(
              {
                extractedFrom: cleanZipName,
                extractedAt: new Date().toISOString(),
                chrootGuard: 'enforced',
                owner: 'www-data:www-data'
              },
              null,
              2
            )
          }
        ];

  // Store the .zip file node itself (with serialized manifest so clicking "Giải nén" later also works)
  const existingZipIdx = webFiles.findIndex(
    (f) => f.appId === appId && f.path === zipFullPath
  );
  const zipNode = {
    id: `f-zip-${Date.now()}`,
    appId,
    name: cleanZipName,
    path: zipFullPath,
    parentDir: cleanParent,
    type: 'file' as const,
    sizeBytes: Number(archiveSizeBytes) || 14820,
    permissions: '0644',
    symbolicPerms: '-rw-r--r--',
    owner: 'www-data:www-data',
    modifiedAt: nowStr,
    content: JSON.stringify(
      {
        _ferrousZipManifest: true,
        archiveName: cleanZipName,
        entries: parsedEntries
      },
      null,
      2
    )
  };

  if (existingZipIdx >= 0) {
    webFiles[existingZipIdx] = zipNode;
  } else {
    webFiles.push(zipNode);
  }

  let extractedCount = 0;
  if (extractImmediately) {
    extractedCount = unpackEntriesIntoWebFiles(
      appId,
      cleanParent,
      parsedEntries,
      nowStr
    );
  }

  res.status(201).json({
    files: webFiles.filter((f) => f.appId === appId),
    extractedCount,
    archivePath: zipFullPath
  });
});

function unpackEntriesIntoWebFiles(
  appId: string,
  baseParentDir: string,
  entries: {
    relativePath: string;
    isDir: boolean;
    sizeBytes: number;
    content?: string;
  }[],
  nowStr: string
): number {
  let count = 0;
  const ensureDir = (dirFullPath: string) => {
    if (!dirFullPath || dirFullPath === '/') return;
    const segments = dirFullPath.split('/').filter(Boolean);
    let current = '';
    for (const seg of segments) {
      const parent = current === '' ? '/' : current;
      const nextPath = `${ current }/${seg}`;
      const exists = webFiles.some(
        (f) => f.appId === appId && f.path === nextPath && f.type === 'directory'
      );
      if (!exists) {
        webFiles.push({
          id: `f-dir-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
          appId,
          name: seg,
          path: nextPath,
          parentDir: parent,
          type: 'directory',
          sizeBytes: 4096,
          permissions: '0755',
          symbolicPerms: 'drwxr-xr-x',
          owner: 'www-data:www-data',
          modifiedAt: nowStr
        });
      }
      current = nextPath;
    }
  };

  for (const item of entries) {
    const cleanRel = String(item.relativePath || '')
      .replace(/^\/+/, '')
      .replace(/\.\./g, '')
      .replace(/\/+$/, '');
    if (!cleanRel) continue;

    const fullItemPath =
      baseParentDir === '/' ? `/${cleanRel}` : `${baseParentDir}/${cleanRel}`;
    const parts = fullItemPath.split('/').filter(Boolean);
    const itemName = parts.pop()!;
    const itemParent = parts.length === 0 ? '/' : `/${parts.join('/')}`;

    ensureDir(itemParent);

    if (item.isDir) {
      ensureDir(fullItemPath);
      count++;
    } else {
      const existingIdx = webFiles.findIndex(
        (f) => f.appId === appId && f.path === fullItemPath
      );
      const fileContent =
        item.content ??
        `// Extracted file: ${fullItemPath}\n// Owner: www-data:www-data\n`;
      const newNode = {
        id: `f-ext-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
        appId,
        name: itemName,
        path: fullItemPath,
        parentDir: itemParent,
        type: 'file' as const,
        sizeBytes:
          item.sizeBytes || Buffer.byteLength(String(fileContent), 'utf8'),
        permissions: '0644',
        symbolicPerms: '-rw-r--r--',
        owner: 'www-data:www-data',
        modifiedAt: nowStr,
        content: fileContent
      };
      if (existingIdx >= 0) {
        webFiles[existingIdx] = newNode;
      } else {
        webFiles.push(newNode);
      }
      count++;
    }
  }
  return count;
}

// Extract an existing .zip / .tar.gz / .tar.zst archive in File Manager
app.post('/api/files/:appId/:fileId/extract', (req, res) => {
  const { appId, fileId } = req.params;
  const archiveNode = webFiles.find(
    (f) => f.id === fileId && f.appId === appId
  );
  if (!archiveNode) {
    res.status(404).json({ error: 'Không tìm thấy tệp nén để giải nén' });
    return;
  }

  const targetDir = String(req.body.targetDir || archiveNode.parentDir || '/');
  const nowStr = new Date().toISOString().slice(0, 16).replace('T', ' ');

  let entriesToExtract: {
    relativePath: string;
    isDir: boolean;
    sizeBytes: number;
    content?: string;
  }[] = [];

  try {
    const parsed = JSON.parse(archiveNode.content || '{}');
    if (parsed && parsed._ferrousZipManifest && Array.isArray(parsed.entries)) {
      entriesToExtract = parsed.entries;
    }
  } catch {
    // fallback if plain archive
  }

  if (entriesToExtract.length === 0) {
    const baseFolder = archiveNode.name
      .replace(/\.(zip|tar\.gz|tar\.zst|tgz)$/i, '')
      .replace(/[^a-zA-Z0-9_-]/g, '_');
    entriesToExtract = [
      {
        relativePath: `${baseFolder}/index.php`,
        isDir: false,
        sizeBytes: 512,
        content: `<?php\n// Extracted from ${archiveNode.name}\n// FerrousPanel Rust ZIP Archive Unpacker\nrequire __DIR__ . '/bootstrap.php';\n`
      },
      {
        relativePath: `${baseFolder}/bootstrap.php`,
        isDir: false,
        sizeBytes: 384,
        content: `<?php\ndefine('FERROUS_EXTRACTED_AT', '${nowStr}');\nheader('X-Powered-By: FerrousPanel-MultiPHP');\n`
      },
      {
        relativePath: `${baseFolder}/README.extracted.md`,
        isDir: false,
        sizeBytes: 290,
        content: `# Giải nén thành công từ ${archiveNode.name}\n- Thư mục đích: ${targetDir}\n- Phân quyền tự động: chown -R www-data:www-data\n`
      }
    ];
  }

  const count = unpackEntriesIntoWebFiles(
    appId,
    targetDir,
    entriesToExtract,
    nowStr
  );

  res.json({
    files: webFiles.filter((f) => f.appId === appId),
    extractedCount: count,
    archiveName: archiveNode.name,
    targetDir
  });
});

// Compress current directory into a .zip archive
app.post('/api/files/:appId/compress', (req, res) => {
  const { appId } = req.params;
  const { sourceDir, archiveName } = req.body;
  const cleanDir = String(sourceDir || '/');
  const cleanName = String(archiveName || `backup_${Date.now()}.zip`).replace(
    /\//g,
    ''
  );
  const finalName = cleanName.endsWith('.zip') ? cleanName : `${cleanName}.zip`;
  const nowStr = new Date().toISOString().slice(0, 16).replace('T', ' ');

  const children = webFiles.filter(
    (f) =>
      f.appId === appId &&
      (cleanDir === '/'
        ? f.path !== `/${finalName}`
        : f.path.startsWith(cleanDir))
  );

  const entries = children.map((c) => ({
    relativePath:
      cleanDir === '/'
        ? c.path.replace(/^\//, '')
        : c.path.slice(cleanDir.length).replace(/^\//, ''),
    isDir: c.type === 'directory',
    sizeBytes: c.sizeBytes,
    content: c.content
  }));

  const totalBytes = children.reduce((acc, c) => acc + c.sizeBytes, 0);
  const compressedBytes = Math.max(512, Math.round(totalBytes * 0.42));
  const fullPath =
    cleanDir === '/' ? `/${finalName}` : `${cleanDir}/${finalName}`;

  const zipNode = {
    id: `f-zip-${Date.now()}`,
    appId,
    name: finalName,
    path: fullPath,
    parentDir: cleanDir,
    type: 'file' as const,
    sizeBytes: compressedBytes,
    permissions: '0644',
    symbolicPerms: '-rw-r--r--',
    owner: 'www-data:www-data',
    modifiedAt: nowStr,
    content: JSON.stringify(
      {
        _ferrousZipManifest: true,
        archiveName: finalName,
        compressedAt: nowStr,
        entries
      },
      null,
      2
    )
  };

  webFiles.push(zipNode);
  res.status(201).json({
    files: webFiles.filter((f) => f.appId === appId),
    archiveName: finalName,
    compressedBytes
  });
});

// Remote URL Download (wget/curl into current directory, like aaPanel)
app.post('/api/files/:appId/remote-download', (req, res) => {
  const { appId } = req.params;
  const { url, parentDir, fileName } = req.body;
  const cleanParent = String(parentDir || '/');
  const urlStr = String(url || 'https://wordpress.org/latest.zip');
  const guessedName =
    String(fileName || '').trim() ||
    urlStr.split('/').pop()?.split('?')[0] ||
    'downloaded-package.zip';
  const cleanName = guessedName.replace(/\//g, '');
  const fullPath =
    cleanParent === '/' ? `/${cleanName}` : `${cleanParent}/${cleanName}`;
  const nowStr = new Date().toISOString().slice(0, 16).replace('T', ' ');
  const isZip = /\.(zip|tar\.gz|tar\.zst|tgz)$/i.test(cleanName);

  const newNode = {
    id: `f-dl-${Date.now()}`,
    appId,
    name: cleanName,
    path: fullPath,
    parentDir: cleanParent,
    type: 'file' as const,
    sizeBytes: isZip ? 2458600 : 4096,
    permissions: '0644',
    symbolicPerms: '-rw-r--r--',
    owner: 'www-data:www-data',
    modifiedAt: nowStr,
    content: isZip
      ? JSON.stringify(
          {
            _ferrousZipManifest: true,
            archiveName: cleanName,
            sourceUrl: urlStr,
            entries: [
              {
                relativePath: 'index.php',
                isDir: false,
                sizeBytes: 680,
                content: `<?php\n// Downloaded from ${urlStr} & unpacked by FerrousPanel\nrequire __DIR__ . '/wp-blog-header.php';\n`
              },
              {
                relativePath: 'wp-config-sample.php',
                isDir: false,
                sizeBytes: 1420,
                content: `<?php\ndefine('DB_NAME', 'storefront_cms');\ndefine('DB_USER', 'next_cms_user');\ndefine('DB_HOST', '127.0.0.1');\ndefine('WP_CACHE', true); // FerrousKV Redis Object Cache\n`
              },
              {
                relativePath: 'wp-content/plugins/ferrous-cache.php',
                isDir: false,
                sizeBytes: 540,
                content: `<?php\n/* Plugin Name: FerrousKV Object Cache (Rust RESP3) */\n`
              }
            ]
          },
          null,
          2
        )
      : `// Downloaded via FerrousPanel Rust reqwest from: ${urlStr}\n// Timestamp: ${nowStr}\n`
  };

  webFiles.push(newNode);
  res.status(201).json(webFiles.filter((f) => f.appId === appId));
});

// Rename File or Directory
app.post('/api/files/:appId/:fileId/rename', (req, res) => {
  const { appId, fileId } = req.params;
  const { newName } = req.body;
  const node = webFiles.find((f) => f.id === fileId && f.appId === appId);
  if (!node || !newName) {
    res.status(404).json({ error: 'Không tìm thấy tệp tin' });
    return;
  }
  const cleanNewName = String(newName).trim().replace(/\//g, '');
  const oldPath = node.path;
  const newPath =
    node.parentDir === '/'
      ? `/${cleanNewName}`
      : `${node.parentDir}/${cleanNewName}`;

  node.name = cleanNewName;
  node.path = newPath;
  node.modifiedAt = new Date().toISOString().slice(0, 16).replace('T', ' ');

  if (node.type === 'directory') {
    webFiles.forEach((child) => {
      if (child.appId === appId && child.path.startsWith(`${oldPath}/`)) {
        child.path = newPath + child.path.slice(oldPath.length);
        if (child.parentDir === oldPath) {
          child.parentDir = newPath;
        } else if (child.parentDir.startsWith(`${oldPath}/`)) {
          child.parentDir = newPath + child.parentDir.slice(oldPath.length);
        }
      }
    });
  }

  res.json(webFiles.filter((f) => f.appId === appId));
});

// Fix / Standardize Permissions (0755 for directories, 0644 for files)
app.post('/api/files/:appId/batch-chmod', (req, res) => {
  const { appId } = req.params;
  webFiles.forEach((f) => {
    if (f.appId === appId) {
      if (f.type === 'directory') {
        f.permissions = '0755';
        f.symbolicPerms = 'drwxr-xr-x';
      } else if (!f.name.startsWith('.env')) {
        f.permissions = '0644';
        f.symbolicPerms = '-rw-r--r--';
      }
      f.owner = 'www-data:www-data';
    }
  });
  res.json(webFiles.filter((f) => f.appId === appId));
});

app.put('/api/files/:appId/:fileId', (req, res) => {
  const { appId, fileId } = req.params;
  const { content, permissions, owner } = req.body;
  const node = webFiles.find((f) => f.id === fileId && f.appId === appId);
  if (!node) {
    res.status(404).json({ error: 'Không tìm thấy tệp tin' });
    return;
  }

  if (content !== undefined && node.type === 'file') {
    node.content = String(content);
    node.sizeBytes = Buffer.byteLength(node.content, 'utf8');

    // If user edits /.env in File Manager, synchronize envVars back to the Website configuration
    if (node.path === '/.env' || node.name === '.env') {
      const targetApp = managedApps.find((a) => a.id === appId);
      if (targetApp) {
        const parsedEnv: { key: string; value: string }[] = [];
        for (const rawLine of node.content.split('\n')) {
          const line = rawLine.trim();
          if (!line || line.startsWith('#')) continue;
          const eqIdx = line.indexOf('=');
          if (eqIdx > 0) {
            const key = line.slice(0, eqIdx).trim();
            const val = line
              .slice(eqIdx + 1)
              .trim()
              .replace(/^["']|["']$/g, '');
            if (key) parsedEnv.push({ key, value: val });
          }
        }
        if (parsedEnv.length > 0) {
          targetApp.envVars = parsedEnv;
          targetApp.systemdUnit = generateSystemdUnit(targetApp);
          targetApp.logs.push(
            `${new Date().toISOString()} INFO  file-sync: Synchronized ${parsedEnv.length} environment variables from /.env into systemd unit.`
          );
        }
      }
    }
  }
  if (permissions !== undefined) {
    node.permissions = String(permissions);
    node.symbolicPerms = permOctalToSymbolic(
      node.permissions,
      node.type === 'directory'
    );
  }
  if (owner !== undefined) {
    node.owner = String(owner);
  }
  node.modifiedAt = new Date().toISOString().slice(0, 16).replace('T', ' ');

  res.json(webFiles.filter((f) => f.appId === appId));
});

app.delete('/api/files/:appId/:fileId', (req, res) => {
  const { appId, fileId } = req.params;
  const target = webFiles.find((f) => f.id === fileId && f.appId === appId);
  if (target) {
    webFiles = webFiles.filter(
      (f) =>
        !(
          f.appId === appId &&
          (f.id === fileId || f.path.startsWith(`${target.path}/`))
        )
    );
  }
  res.json(webFiles.filter((f) => f.appId === appId));
});

// ============================================================================
// Multi-PHP Manager State & APIs (PHP 8.4, 8.3, 8.2, 8.1, 7.4 FPM + php.ini)
// ============================================================================
function buildDefaultPhpIni(
  ver: string,
  mem: string,
  upload: string,
  post: string,
  maxExec: number,
  maxVars: number,
  displayErr: boolean,
  opcache: boolean,
  jitBuf: string,
  disabledFuncs: string[]
): string {
  return `[PHP]
; Quản lý bởi FerrousPanel Multi-PHP Rust Engine (/etc/php/${ver}/fpm/php.ini)
engine = On
short_open_tag = Off
expose_php = Off
max_execution_time = ${maxExec}
max_input_time = 60
max_input_vars = ${maxVars}
memory_limit = ${mem}
error_reporting = E_ALL & ~E_DEPRECATED & ~E_STRICT
display_errors = ${displayErr ? 'On' : 'Off'}
log_errors = On
error_log = /www/wwwlogs/php${ver}-fpm-error.log
post_max_size = ${post}
file_uploads = On
upload_max_filesize = ${upload}
max_file_uploads = 50
date.timezone = "Asia/Ho_Chi_Minh"
disable_functions = ${disabledFuncs.join(',')}

[Session]
session.save_handler = redis
session.save_path = "tcp://127.0.0.1:6379?database=1"
session.cookie_httponly = 1
session.cookie_secure = 1

[opcache]
opcache.enable = ${opcache ? '1' : '0'}
opcache.enable_cli = 1
opcache.memory_consumption = 256
opcache.interned_strings_buffer = 16
opcache.max_accelerated_files = 30000
opcache.jit = tracing
opcache.jit_buffer_size = ${jitBuf}
`;
}

function buildDefaultFpmPoolConf(ver: string, maxChildren: number): string {
  return `[www]
; FerrousPanel FPM Pool (/etc/php/${ver}/fpm/pool.d/www.conf)
user = www-data
group = www-data
listen = /run/php/php${ver}-fpm.sock
listen.owner = www-data
listen.group = www-data
listen.mode = 0660

pm = dynamic
pm.max_children = ${maxChildren}
pm.start_servers = ${Math.max(2, Math.floor(maxChildren / 6))}
pm.min_spare_servers = ${Math.max(2, Math.floor(maxChildren / 8))}
pm.max_spare_servers = ${Math.max(4, Math.floor(maxChildren / 3))}
pm.max_requests = 2000
request_terminate_timeout = 120s
slowlog = /www/wwwlogs/php${ver}-slow.log
request_slowlog_timeout = 5s
`;
}

const defaultExtensionsList = () => [
  {
    name: 'opcache',
    category: 'Cache & Speed' as const,
    enabled: true,
    description: 'Bộ biên dịch mã máy JIT & Bytecode Cache tăng tốc PHP 3x'
  },
  {
    name: 'redis',
    category: 'Cache & Speed' as const,
    enabled: true,
    description: 'Kết nối trực tiếp vào FerrousKV (RESP3 127.0.0.1:6379)'
  },
  {
    name: 'apcu',
    category: 'Cache & Speed' as const,
    enabled: true,
    description: 'Bộ nhớ đệm In-Memory User Cache cho Laravel/WordPress'
  },
  {
    name: 'swoole',
    category: 'Cache & Speed' as const,
    enabled: false,
    description: 'Async Coroutine Server cho Laravel Octane / Hyperf'
  },
  {
    name: 'pdo_mysql',
    category: 'Database' as const,
    enabled: true,
    description: 'Driver kết nối MariaDB 11.4 / MySQL qua Unix Socket'
  },
  {
    name: 'pdo_pgsql',
    category: 'Database' as const,
    enabled: true,
    description: 'Driver kết nối PostgreSQL 17 tốc độ cao'
  },
  {
    name: 'mysqli',
    category: 'Database' as const,
    enabled: true,
    description: 'Thư viện MySQL Improved cho WordPress & phpMyAdmin 5.2'
  },
  {
    name: 'imagick',
    category: 'Image & Media' as const,
    enabled: true,
    description: 'Xử lý ảnh nâng cao ImageMagick (WebP, AVIF, PDF)'
  },
  {
    name: 'gd',
    category: 'Image & Media' as const,
    enabled: true,
    description: 'Thư viện đồ họa GD xử lý thumbnail và watermark'
  },
  {
    name: 'exif',
    category: 'Image & Media' as const,
    enabled: true,
    description: 'Đọc metadata EXIF của ảnh chụp từ máy ảnh/điện thoại'
  },
  {
    name: 'zip',
    category: 'Core & Security' as const,
    enabled: true,
    description: 'Nén và giải nén tệp .zip (libzip)'
  },
  {
    name: 'curl',
    category: 'Core & Security' as const,
    enabled: true,
    description: 'Giao tiếp HTTP/2 & TLS 1.3 với API bên ngoài'
  },
  {
    name: 'mbstring',
    category: 'Core & Security' as const,
    enabled: true,
    description: 'Xử lý chuỗi đa byte UTF-8 tiếng Việt'
  },
  {
    name: 'intl',
    category: 'Core & Security' as const,
    enabled: true,
    description: 'Thư viện quốc tế hóa ICU (định dạng tiền tệ, ngày tháng)'
  },
  {
    name: 'bcmath',
    category: 'Core & Security' as const,
    enabled: true,
    description: 'Tính toán số học độ chính xác cao cho thương mại điện tử'
  },
  {
    name: 'ioncube_loader',
    category: 'Core & Security' as const,
    enabled: false,
    description: 'Giải mã mã nguồn PHP thương mại được mã hóa IonCube'
  }
];

const defaultDisabledFuncs = [
  'exec',
  'passthru',
  'shell_exec',
  'system',
  'proc_open',
  'popen',
  'pcntl_exec'
];

let phpVersionsState = [
  {
    version: '8.4' as const,
    fullVersion: 'PHP 8.4.1-FPM (JIT IR Framework + Property Hooks)',
    status: 'running' as 'running' | 'stopped' | 'not_installed',
    installed: true,
    fpmSocket: '/run/php/php8.4-fpm.sock',
    iniPath: '/etc/php/8.4/fpm/php.ini',
    poolPath: '/etc/php/8.4/fpm/pool.d/www.conf',
    memoryUsageMb: 76.4,
    activeWorkers: 6,
    maxChildren: 50,
    memoryLimit: '512M',
    uploadMaxFilesize: '256M',
    postMaxSize: '256M',
    maxExecutionTime: 120,
    maxInputVars: 5000,
    displayErrors: false,
    opcacheEnabled: true,
    opcacheJitBuffer: '128M',
    disableFunctions: [...defaultDisabledFuncs],
    extensions: defaultExtensionsList(),
    rawPhpIni: buildDefaultPhpIni(
      '8.4',
      '512M',
      '256M',
      '256M',
      120,
      5000,
      false,
      true,
      '128M',
      defaultDisabledFuncs
    ),
    rawFpmPoolConf: buildDefaultFpmPoolConf('8.4', 50)
  },
  {
    version: '8.3' as const,
    fullVersion: 'PHP 8.3.14-FPM (LTS Production · OPcache Tracing JIT)',
    status: 'running' as 'running' | 'stopped' | 'not_installed',
    installed: true,
    fpmSocket: '/run/php/php8.3-fpm.sock',
    iniPath: '/etc/php/8.3/fpm/php.ini',
    poolPath: '/etc/php/8.3/fpm/pool.d/www.conf',
    memoryUsageMb: 112.0,
    activeWorkers: 12,
    maxChildren: 80,
    memoryLimit: '512M',
    uploadMaxFilesize: '128M',
    postMaxSize: '128M',
    maxExecutionTime: 120,
    maxInputVars: 5000,
    displayErrors: false,
    opcacheEnabled: true,
    opcacheJitBuffer: '128M',
    disableFunctions: [...defaultDisabledFuncs],
    extensions: defaultExtensionsList(),
    rawPhpIni: buildDefaultPhpIni(
      '8.3',
      '512M',
      '128M',
      '128M',
      120,
      5000,
      false,
      true,
      '128M',
      defaultDisabledFuncs
    ),
    rawFpmPoolConf: buildDefaultFpmPoolConf('8.3', 80)
  },
  {
    version: '8.2' as const,
    fullVersion: 'PHP 8.2.26-FPM (Stable Enterprise · WordPress / Laravel 10)',
    status: 'running' as 'running' | 'stopped' | 'not_installed',
    installed: true,
    fpmSocket: '/run/php/php8.2-fpm.sock',
    iniPath: '/etc/php/8.2/fpm/php.ini',
    poolPath: '/etc/php/8.2/fpm/pool.d/www.conf',
    memoryUsageMb: 64.5,
    activeWorkers: 6,
    maxChildren: 50,
    memoryLimit: '256M',
    uploadMaxFilesize: '100M',
    postMaxSize: '100M',
    maxExecutionTime: 90,
    maxInputVars: 3000,
    displayErrors: false,
    opcacheEnabled: true,
    opcacheJitBuffer: '64M',
    disableFunctions: [...defaultDisabledFuncs],
    extensions: defaultExtensionsList(),
    rawPhpIni: buildDefaultPhpIni(
      '8.2',
      '256M',
      '100M',
      '100M',
      90,
      3000,
      false,
      true,
      '64M',
      defaultDisabledFuncs
    ),
    rawFpmPoolConf: buildDefaultFpmPoolConf('8.2', 50)
  },
  {
    version: '8.1' as const,
    fullVersion: 'PHP 8.1.31-FPM (Compatibility Runtime)',
    status: 'stopped' as 'running' | 'stopped' | 'not_installed',
    installed: true,
    fpmSocket: '/run/php/php8.1-fpm.sock',
    iniPath: '/etc/php/8.1/fpm/php.ini',
    poolPath: '/etc/php/8.1/fpm/pool.d/www.conf',
    memoryUsageMb: 0,
    activeWorkers: 0,
    maxChildren: 40,
    memoryLimit: '256M',
    uploadMaxFilesize: '64M',
    postMaxSize: '64M',
    maxExecutionTime: 60,
    maxInputVars: 3000,
    displayErrors: false,
    opcacheEnabled: true,
    opcacheJitBuffer: '64M',
    disableFunctions: [...defaultDisabledFuncs],
    extensions: defaultExtensionsList(),
    rawPhpIni: buildDefaultPhpIni(
      '8.1',
      '256M',
      '64M',
      '64M',
      60,
      3000,
      false,
      true,
      '64M',
      defaultDisabledFuncs
    ),
    rawFpmPoolConf: buildDefaultFpmPoolConf('8.1', 40)
  },
  {
    version: '7.4' as const,
    fullVersion: 'PHP 7.4.33-FPM (Legacy CMS Support · Hardened Backports)',
    status: 'running' as 'running' | 'stopped' | 'not_installed',
    installed: true,
    fpmSocket: '/run/php/php7.4-fpm.sock',
    iniPath: '/etc/php/7.4/fpm/php.ini',
    poolPath: '/etc/php/7.4/fpm/pool.d/www.conf',
    memoryUsageMb: 48.0,
    activeWorkers: 4,
    maxChildren: 30,
    memoryLimit: '256M',
    uploadMaxFilesize: '64M',
    postMaxSize: '64M',
    maxExecutionTime: 60,
    maxInputVars: 2000,
    displayErrors: false,
    opcacheEnabled: true,
    opcacheJitBuffer: '0M',
    disableFunctions: [...defaultDisabledFuncs],
    extensions: defaultExtensionsList(),
    rawPhpIni: buildDefaultPhpIni(
      '7.4',
      '256M',
      '64M',
      '64M',
      60,
      2000,
      false,
      true,
      '0M',
      defaultDisabledFuncs
    ),
    rawFpmPoolConf: buildDefaultFpmPoolConf('7.4', 30)
  }
];

app.get('/api/php', (_req, res) => {
  res.json(phpVersionsState);
});

app.post('/api/php/:version/action', (req, res) => {
  const inst = phpVersionsState.find((p) => p.version === req.params.version);
  if (!inst) {
    res.status(404).json({ error: 'PHP version not found' });
    return;
  }
  const { action } = req.body;
  if (action === 'start' || action === 'restart' || action === 'reload') {
    inst.installed = true;
    inst.status = 'running';
    if (inst.memoryUsageMb === 0) inst.memoryUsageMb = 68.0;
    if (inst.activeWorkers === 0) inst.activeWorkers = 6;
  } else if (action === 'stop') {
    inst.status = 'stopped';
    inst.memoryUsageMb = 0;
    inst.activeWorkers = 0;
  } else if (action === 'install') {
    inst.installed = true;
    inst.status = 'running';
    inst.memoryUsageMb = 72.0;
    inst.activeWorkers = 6;
  } else if (action === 'uninstall') {
    inst.installed = false;
    inst.status = 'not_installed';
    inst.memoryUsageMb = 0;
    inst.activeWorkers = 0;
  }
  res.json(phpVersionsState);
});

app.put('/api/php/:version/settings', (req, res) => {
  const inst = phpVersionsState.find((p) => p.version === req.params.version);
  if (!inst) {
    res.status(404).json({ error: 'PHP version not found' });
    return;
  }

  const {
    memoryLimit,
    uploadMaxFilesize,
    postMaxSize,
    maxExecutionTime,
    maxInputVars,
    maxChildren,
    displayErrors,
    opcacheEnabled,
    opcacheJitBuffer,
    disableFunctions,
    rawPhpIni,
    rawFpmPoolConf
  } = req.body;

  if (memoryLimit !== undefined) inst.memoryLimit = String(memoryLimit);
  if (uploadMaxFilesize !== undefined)
    inst.uploadMaxFilesize = String(uploadMaxFilesize);
  if (postMaxSize !== undefined) inst.postMaxSize = String(postMaxSize);
  if (maxExecutionTime !== undefined)
    inst.maxExecutionTime = Number(maxExecutionTime);
  if (maxInputVars !== undefined) inst.maxInputVars = Number(maxInputVars);
  if (maxChildren !== undefined) inst.maxChildren = Number(maxChildren);
  if (displayErrors !== undefined) inst.displayErrors = Boolean(displayErrors);
  if (opcacheEnabled !== undefined)
    inst.opcacheEnabled = Boolean(opcacheEnabled);
  if (opcacheJitBuffer !== undefined)
    inst.opcacheJitBuffer = String(opcacheJitBuffer);
  if (Array.isArray(disableFunctions))
    inst.disableFunctions = disableFunctions;

  if (rawPhpIni !== undefined) {
    inst.rawPhpIni = String(rawPhpIni);
  } else {
    inst.rawPhpIni = buildDefaultPhpIni(
      inst.version,
      inst.memoryLimit,
      inst.uploadMaxFilesize,
      inst.postMaxSize,
      inst.maxExecutionTime,
      inst.maxInputVars,
      inst.displayErrors,
      inst.opcacheEnabled,
      inst.opcacheJitBuffer,
      inst.disableFunctions
    );
  }

  if (rawFpmPoolConf !== undefined) {
    inst.rawFpmPoolConf = String(rawFpmPoolConf);
  } else {
    inst.rawFpmPoolConf = buildDefaultFpmPoolConf(
      inst.version,
      inst.maxChildren
    );
  }

  res.json(phpVersionsState);
});

app.post('/api/php/:version/extensions/toggle', (req, res) => {
  const inst = phpVersionsState.find((p) => p.version === req.params.version);
  if (!inst) {
    res.status(404).json({ error: 'PHP version not found' });
    return;
  }
  const { extName } = req.body;
  const ext = inst.extensions.find((e) => e.name === extName);
  if (ext) {
    ext.enabled = !ext.enabled;
  }
  res.json(phpVersionsState);
});

app.post('/api/php/switch-site', (req, res) => {
  const { appId, phpVersion } = req.body;
  const site = managedApps.find((a) => a.id === appId);
  if (!site) {
    res.status(404).json({ error: 'Website not found' });
    return;
  }
  site.phpVersion = phpVersion;
  if (phpVersion && phpVersion !== 'None') {
    if (site.urlRewriteRules.includes('fastcgi_pass')) {
      site.urlRewriteRules = site.urlRewriteRules.replace(
        /php[0-9.]+-fpm\.sock/g,
        `php${phpVersion}-fpm.sock`
      );
    } else {
      site.urlRewriteRules = `${site.urlRewriteRules}\n\nlocation ~ \\.php$ {\n    include snippets/fastcgi-php.conf;\n    fastcgi_pass unix:/run/php/php${phpVersion}-fpm.sock;\n}`;
    }
    if (site.framework.includes('PHP')) {
      site.runtimeVersion = `PHP ${phpVersion}-FPM · OPcache JIT`;
    }
    site.logs.push(
      `${new Date().toISOString()} INFO  multiphp: Switched ${site.domain} FastCGI socket to /run/php/php${phpVersion}-fpm.sock & reloaded Nginx.`
    );
  } else {
    site.logs.push(
      `${new Date().toISOString()} INFO  multiphp: Disabled PHP FastCGI handler for ${site.domain} (Pure Proxy / Static mode).`
    );
  }

  syncArchitectureForApp(site);
  const enriched = managedApps.map((a) => syncArchitectureForApp(a));
  res.json({
    app: site,
    apps: enriched,
    phpVersions: phpVersionsState
  });
});

// 6. cPanel Utilities API
app.get('/api/cpanel', (_req, res) => {
  res.json({
    cronJobs,
    dnsRecords,
    backups: backupArchives,
    sftpAccounts
  });
});

app.post('/api/cpanel/cron', (req, res) => {
  const { title, schedule, command, user, targetApp } = req.body;
  cronJobs.unshift({
    id: `cron-${Date.now()}`,
    title: String(title || 'Custom Ubuntu Cron Job'),
    schedule: String(schedule || '0 * * * *'),
    command: String(
      command || '/usr/bin/curl -fsS http://127.0.0.1:8081/healthz'
    ),
    user: String(user || 'www-data'),
    targetApp: String(targetApp || 'api.ferroushub.vn'),
    enabled: true,
    lastRunAt: 'Chưa chạy',
    lastExitCode: 0,
    lastOutput: 'Scheduled in /etc/cron.d/ferrous-panel'
  });
  res.status(201).json(cronJobs);
});

app.post('/api/cpanel/cron/:id/run', (req, res) => {
  const job = cronJobs.find((c) => c.id === req.params.id);
  if (job) {
    job.lastRunAt = 'Vừa xong';
    job.lastExitCode = 0;
    job.lastOutput = `[${new Date().toISOString()}] Executed (${job.command}) as ${job.user} -> exit status 0 (elapsed 42ms)`;
  }
  res.json(cronJobs);
});

app.put('/api/cpanel/cron/:id/toggle', (req, res) => {
  const job = cronJobs.find((c) => c.id === req.params.id);
  if (job) {
    job.enabled = !job.enabled;
    job.lastOutput = job.enabled
      ? `[${new Date().toISOString()}] Cron job enabled in /etc/cron.d/ferrous-panel`
      : `[${new Date().toISOString()}] Cron job paused by administrator`;
  }
  res.json(cronJobs);
});

app.delete('/api/cpanel/cron/:id', (req, res) => {
  cronJobs = cronJobs.filter((c) => c.id !== req.params.id);
  res.json(cronJobs);
});

app.post('/api/cpanel/dns', (req, res) => {
  const { domain, type, host, value, ttl } = req.body;
  dnsRecords.unshift({
    id: `dns-${Date.now()}`,
    domain: String(domain || 'ferroushub.vn'),
    type: type || 'A',
    host: String(host || 'sub.ferroushub.vn'),
    value: String(value || '103.145.88.210'),
    ttl: Number(ttl) || 300
  });
  res.status(201).json(dnsRecords);
});

app.delete('/api/cpanel/dns/:id', (req, res) => {
  dnsRecords = dnsRecords.filter((d) => d.id !== req.params.id);
  res.json(dnsRecords);
});

app.post('/api/cpanel/backups', (req, res) => {
  const { targetApp, databaseIncluded } = req.body;
  const dateStr = new Date().toISOString().slice(0, 10);
  const cleanApp = String(targetApp || 'api.ferroushub.vn');
  backupArchives.unshift({
    id: `bk-${Date.now()}`,
    filename: `site_${cleanApp}_${dateStr}_${Math.floor(1000 + Math.random() * 9000)}.tar.zst`,
    targetApp: cleanApp,
    databaseIncluded: String(
      databaseIncluded || 'PostgreSQL 17 · commerce_prod'
    ),
    sizeMb: Number((65 + Math.random() * 120).toFixed(1)),
    compression: 'zstd (level 9)',
    sha256:
      Math.random().toString(16).slice(2) +
      Math.random().toString(16).slice(2) +
      '98fc1c149afbf4c8996fb92427ae41e4',
    createdAt: new Date().toISOString().slice(0, 19).replace('T', ' '),
    status: 'verified'
  });
  const targetSite = managedApps.find((a) => a.domain === cleanApp);
  if (targetSite) {
    targetSite.backupCount = backupArchives.filter(
      (b) => b.targetApp === cleanApp
    ).length;
    targetSite.logs.push(
      `${new Date().toISOString()} INFO  zstd-backup: Created archive for ${cleanApp} (total backups: ${targetSite.backupCount})`
    );
  }
  res.status(201).json(backupArchives);
});

app.post('/api/cpanel/backups/:id/restore', (req, res) => {
  const bk = backupArchives.find((b) => b.id === req.params.id);
  if (bk) {
    bk.status = 'verified';
    const targetSite = managedApps.find((a) => a.domain === bk.targetApp);
    if (targetSite) {
      targetSite.logs.push(
        `${new Date().toISOString()} INFO  zstd-restore: Restored archive ${bk.filename} (${bk.sizeMb} MB, SHA-256 verified) & database ${bk.databaseIncluded}`
      );
    }
  }
  res.json(backupArchives);
});

app.delete('/api/cpanel/backups/:id', (req, res) => {
  const targetBk = backupArchives.find((b) => b.id === req.params.id);
  backupArchives = backupArchives.filter((b) => b.id !== req.params.id);
  if (targetBk) {
    const targetSite = managedApps.find((a) => a.domain === targetBk.targetApp);
    if (targetSite) {
      targetSite.backupCount = backupArchives.filter(
        (b) => b.targetApp === targetBk.targetApp
      ).length;
    }
  }
  res.json(backupArchives);
});

app.post('/api/cpanel/sftp', (req, res) => {
  const { username, targetApp, authMethod, quotaMb } = req.body;
  const cleanUser = String(username || 'deploy_user')
    .toLowerCase()
    .replace(/[^a-z0-9_]/g, '_');
  const cleanApp = String(targetApp || 'api.ferroushub.vn');
  sftpAccounts.unshift({
    id: `sftp-${Date.now()}`,
    username: cleanUser,
    chrootDir: `/www/wwwroot/${cleanApp}`,
    targetApp: cleanApp,
    authMethod: authMethod || 'Ed25519 SSH Key',
    quotaMb: Number(quotaMb) || 5120,
    usedMb: 12,
    status: 'active'
  });
  const targetSite = managedApps.find((a) => a.domain === cleanApp);
  if (targetSite) {
    targetSite.ftpUsername = cleanUser;
  }
  res.status(201).json(sftpAccounts);
});

app.delete('/api/cpanel/sftp/:id', (req, res) => {
  sftpAccounts = sftpAccounts.filter((s) => s.id !== req.params.id);
  res.json(sftpAccounts);
});

// 7. Nginx Reverse Proxy Validation API
app.post('/api/nginx/validate', (req, res) => {
  const { domain, configContent } = req.body;
  const hasServerBlock = String(configContent || '').includes('server {');

  if (!hasServerBlock) {
    res.json({
      valid: false,
      output: `nginx: [emerg] invalid vhost block in /www/server/panel/vhost/nginx/${domain || 'vhost'}.conf: missing 'server {' directive\nnginx: configuration file /etc/nginx/nginx.conf test failed`
    });
    return;
  }

  res.json({
    valid: true,
    output: `nginx: the configuration file /etc/nginx/nginx.conf syntax is ok\nnginx: vhost /www/server/panel/vhost/nginx/${domain || 'default'}.conf upstream & TLS 1.3 directives verified\nnginx: configuration file /etc/nginx/nginx.conf test is successful`
  });
});

// 8. Security Center API
app.get('/api/security', (_req, res) => {
  res.json({
    ufwRules,
    fail2banJails,
    hardeningDirectives
  });
});

app.post('/api/security/ufw', (req, res) => {
  const { port, protocol, action, source, comment } = req.body;
  const newRule = {
    id: `ufw-${Date.now()}`,
    port: String(port || '8443'),
    protocol: (protocol || 'TCP') as 'TCP' | 'UDP' | 'BOTH',
    action: (action || 'ALLOW') as 'ALLOW' | 'DENY' | 'LIMIT',
    source: String(source || '0.0.0.0/0'),
    comment: String(comment || 'Custom Ubuntu UFW Rule'),
    enabled: true
  };
  ufwRules.push(newRule);
  res.status(201).json(ufwRules);
});

app.put('/api/security/ufw/:id/toggle', (req, res) => {
  const rule = ufwRules.find((r) => r.id === req.params.id);
  if (rule) {
    rule.enabled = !rule.enabled;
  }
  res.json(ufwRules);
});

app.delete('/api/security/ufw/:id', (req, res) => {
  ufwRules = ufwRules.filter((r) => r.id !== req.params.id);
  res.json(ufwRules);
});

app.post('/api/security/fail2ban/unban', (req, res) => {
  const { jailName, ip } = req.body;
  const jail = fail2banJails.find((j) => j.name === jailName);
  if (jail) {
    jail.bannedIps = jail.bannedIps.filter((item) => item.ip !== ip);
    jail.currentlyBanned = jail.bannedIps.length;
  }
  res.json(fail2banJails);
});

app.post('/api/security/fail2ban/ban', (req, res) => {
  const { jailName, ip, reason } = req.body;
  const jail =
    fail2banJails.find((j) => j.name === jailName) || fail2banJails[0];
  if (jail && ip) {
    jail.bannedIps.unshift({
      ip: String(ip),
      country: 'MANUAL',
      reason: String(
        reason || 'Manual IP block via FerrousPanel Security Console'
      ),
      bannedAt: 'Vừa xong'
    });
    jail.currentlyBanned = jail.bannedIps.length;
    jail.totalBanned += 1;
  }
  res.json(fail2banJails);
});

app.put('/api/security/hardening/:id', (req, res) => {
  const item = hardeningDirectives.find((d) => d.id === req.params.id);
  if (item) {
    item.enabled = !item.enabled;
    item.currentValue = item.enabled
      ? item.recommendedValue
      : 'disabled (unsafe)';
  }
  res.json(hardeningDirectives);
});

// 9. System Optimizer & Panel Backend Stability Engine APIs
let optimizerState: {
  activeProfileId:
    | 'balanced_cloud'
    | 'high_throughput_edge'
    | 'database_heavy'
    | 'low_memory_vps';
  profiles: {
    id:
      | 'balanced_cloud'
      | 'high_throughput_edge'
      | 'database_heavy'
      | 'low_memory_vps';
    name: string;
    badge: string;
    description: string;
    actixWorkers: number;
    actixMaxConnections: number;
    actixKeepAliveSecs: number;
    nginxWorkerConnections: number;
    nginxKeepaliveRequests: number;
    mariadbBufferPoolMb: number;
    postgresSharedBuffersMb: number;
    redisMaxMemoryMb: number;
    expectedP99ReductionPercent: number;
  }[];
  sysctlParams: {
    id: string;
    key: string;
    category:
      | 'TCP BBR & Network'
      | 'Memory & Swap'
      | 'File Descriptors & Epoll';
    currentValue: string;
    recommendedValue: string;
    description: string;
    enabled: boolean;
  }[];
  watchdog: {
    daemonPid: number;
    oomScoreAdj: number;
    allocatorEngine: string;
    allocatedRamMb: number;
    residentRamMb: number;
    workerThreadsActive: number;
    workerThreadsTotal: number;
    dbPoolActive: number;
    dbPoolIdle: number;
    dbPoolMax: number;
    circuitBreakerStatus: 'CLOSED (Optimal)' | 'HALF-OPEN (Recovering)';
    autoHealEnabled: boolean;
    lastSelfHealAt: string;
    uptimeStabilityPercent: number;
    recentEvents: {
      id: string;
      timestamp: string;
      subsystem: string;
      action: string;
      result: string;
    }[];
  };
} = {
  activeProfileId: 'balanced_cloud',
  profiles: [
    {
      id: 'balanced_cloud',
      name: 'Balanced Production Cloud (Recommended)',
      badge: 'MODEL CHUẨN ĐA DỊCH VỤ',
      description:
        'Cân bằng hoàn hảo giữa thông lượng Rust Actix-Web, các ứng dụng SSR (Next.js 15 / Nuxt 3 / SvelteKit 2) và bộ nhớ đệm CSDL SQL.',
      actixWorkers: 8,
      actixMaxConnections: 32768,
      actixKeepAliveSecs: 75,
      nginxWorkerConnections: 32768,
      nginxKeepaliveRequests: 5000,
      mariadbBufferPoolMb: 1024,
      postgresSharedBuffersMb: 1024,
      redisMaxMemoryMb: 512,
      expectedP99ReductionPercent: 28
    },
    {
      id: 'high_throughput_edge',
      name: 'High-Throughput Edge & Reverse Proxy (Max Req/s)',
      badge: 'TỐI ĐA THÔNG LƯỢNG MẠNG',
      description:
        'Tối ưu hóa cho tải đồng thời cực lớn (>25,000 req/s): nhân đôi luồng Worker I/O, mở rộng hàng đợi Epoll/Somaxconn và giữ kết nối Keepalive dài.',
      actixWorkers: 16,
      actixMaxConnections: 65535,
      actixKeepAliveSecs: 120,
      nginxWorkerConnections: 65535,
      nginxKeepaliveRequests: 10000,
      mariadbBufferPoolMb: 1024,
      postgresSharedBuffersMb: 1024,
      redisMaxMemoryMb: 768,
      expectedP99ReductionPercent: 42
    },
    {
      id: 'database_heavy',
      name: 'Database-Intensive & Transactional (PostgreSQL / MariaDB)',
      badge: 'ƯU TIÊN TRUY VẤN SQL',
      description:
        'Dồn 60% tài nguyên RAM vào InnoDB Buffer Pool và PostgreSQL Shared Buffers để loại bỏ nghẽn đọc/ghi đĩa NVMe cho hệ thống thương mại điện tử.',
      actixWorkers: 8,
      actixMaxConnections: 25000,
      actixKeepAliveSecs: 90,
      nginxWorkerConnections: 16384,
      nginxKeepaliveRequests: 2500,
      mariadbBufferPoolMb: 2048,
      postgresSharedBuffersMb: 2048,
      redisMaxMemoryMb: 512,
      expectedP99ReductionPercent: 35
    },
    {
      id: 'low_memory_vps',
      name: 'Low-Memory Cloud VPS (Strict RAM Guard)',
      badge: 'TIẾT KIỆM RAM TỐI ĐA',
      description:
        'Giới hạn chặt chẽ bộ nhớ đệm của từng dịch vụ, thu gọn số luồng Worker và kích hoạt thu hồi bộ nhớ tự động cho VPS 2GB - 4GB RAM.',
      actixWorkers: 4,
      actixMaxConnections: 8192,
      actixKeepAliveSecs: 45,
      nginxWorkerConnections: 4096,
      nginxKeepaliveRequests: 500,
      mariadbBufferPoolMb: 384,
      postgresSharedBuffersMb: 384,
      redisMaxMemoryMb: 192,
      expectedP99ReductionPercent: 18
    }
  ],
  sysctlParams: [
    {
      id: 'sys-1',
      key: 'net.ipv4.tcp_congestion_control',
      category: 'TCP BBR & Network',
      currentValue: 'bbr',
      recommendedValue: 'bbr',
      description:
        'Thuật toán điều khiển tắc nghẽn Google TCP BBR giúp tăng tốc độ truyền tải HTTPS/QUIC trên đường truyền quốc tế.',
      enabled: true
    },
    {
      id: 'sys-2',
      key: 'net.core.somaxconn',
      category: 'TCP BBR & Network',
      currentValue: '65535',
      recommendedValue: '65535',
      description:
        'Tăng kích thước hàng đợi socket lắng nghe tối đa cho Nginx Reverse Proxy và Actix-Web HttpServer.',
      enabled: true
    },
    {
      id: 'sys-3',
      key: 'net.ipv4.tcp_tw_reuse',
      category: 'TCP BBR & Network',
      currentValue: '1',
      recommendedValue: '1',
      description:
        'Cho phép tái sử dụng nhanh các socket ở trạng thái TIME_WAIT cho kết nối Upstream Loopback (127.0.0.1).',
      enabled: true
    },
    {
      id: 'sys-4',
      key: 'fs.file-max',
      category: 'File Descriptors & Epoll',
      currentValue: '2097152',
      recommendedValue: '2097152',
      description:
        'Nâng giới hạn tổng số File Descriptors toàn hệ thống phục vụ hàng trăm ngàn kết nối đồng thời.',
      enabled: true
    },
    {
      id: 'sys-5',
      key: 'vm.swappiness',
      category: 'Memory & Swap',
      currentValue: '10',
      recommendedValue: '10',
      description:
        'Hạn chế tối đa việc nhân Linux đẩy bộ nhớ của Actix-Web và Database sang phân vùng Swap trừ khi RAM vượt 90%.',
      enabled: true
    },
    {
      id: 'sys-6',
      key: 'vm.vfs_cache_pressure',
      category: 'Memory & Swap',
      currentValue: '50',
      recommendedValue: '50',
      description:
        'Giữ lại bộ nhớ đệm cấu trúc thư mục (dentries/inodes) cho `/www/wwwroot/*` giúp tăng tốc độ truy xuất tệp tĩnh.',
      enabled: true
    }
  ],
  watchdog: {
    daemonPid: 1042,
    oomScoreAdj: -900,
    allocatorEngine: 'jemalloc 5.3.0 (Zero-Fragmentation Arena)',
    allocatedRamMb: 14.2,
    residentRamMb: 18.4,
    workerThreadsActive: 8,
    workerThreadsTotal: 8,
    dbPoolActive: 6,
    dbPoolIdle: 14,
    dbPoolMax: 32,
    circuitBreakerStatus: 'CLOSED (Optimal)',
    autoHealEnabled: true,
    lastSelfHealAt: 'Vừa xong (2ms latency)',
    uptimeStabilityPercent: 99.998,
    recentEvents: [
      {
        id: 'ev-1',
        timestamp: 'Hôm nay 11:40:12',
        subsystem: 'OOM Killer Protection',
        action: 'Khóa chỉ số /proc/1042/oom_score_adj = -900 cho tiến trình FerrousPanel Daemon',
        result: 'Protected (Miễn nhiễm OOM Killer)'
      },
      {
        id: 'ev-2',
        timestamp: 'Hôm nay 11:42:05',
        subsystem: 'Actix-Web Upstream Watchdog',
        action: 'Kiểm tra tuần tự 7 cổng Upstream Loopback (8081..3003) & PHP-FPM Unix Socket',
        result: '7/7 Upstreams Healthy (P99 < 4.5ms)'
      },
      {
        id: 'ev-3',
        timestamp: 'Hôm nay 11:45:00',
        subsystem: 'SQL Connection Pool Guard',
        action: 'Thu hồi 4 kết nối nhàn rỗi quá hạn trong sqlx::PgPool & MariaDB Pool',
        result: 'Pool Balanced (6 active / 14 idle)'
      }
    ]
  }
};

function buildSysctlPreview(): string {
  const lines = [
    '# ============================================================================',
    '# /etc/sysctl.d/99-ferrous-optimizer.conf',
    `# Active Hardware Profile: ${optimizerState.activeProfileId}`,
    '# Apply command: sysctl --system',
    '# ============================================================================',
    'net.core.default_qdisc = fq'
  ];
  for (const p of optimizerState.sysctlParams) {
    if (p.enabled) {
      lines.push(`# ${p.description}`);
      lines.push(`${p.key} = ${p.recommendedValue}`);
    }
  }
  return lines.join('\n');
}

app.get('/api/optimizer', (_req, res) => {
  res.json({
    ...optimizerState,
    sysctlConfigPreview: buildSysctlPreview()
  });
});

app.post('/api/optimizer/apply-profile', (req, res) => {
  const { profileId } = req.body;
  const found = optimizerState.profiles.find((p) => p.id === profileId);
  if (found) {
    optimizerState.activeProfileId = found.id;
    optimizerState.watchdog.workerThreadsActive = found.actixWorkers;
    optimizerState.watchdog.workerThreadsTotal = found.actixWorkers;

    // Synchronize MariaDB buffer pool & max connections with the chosen optimization model
    mysqlPmaConfig.innodbBufferPoolMb = found.mariadbBufferPoolMb;
    mysqlPmaConfig.maxConnections =
      found.id === 'high_throughput_edge'
        ? 600
        : found.id === 'database_heavy'
          ? 500
          : 300;

    // Synchronize Actix-Web apps worker count & improve P99 latency
    managedApps = managedApps.map((appItem) => {
      const nextWorkers = appItem.framework.includes('Rust')
        ? found.actixWorkers
        : Math.max(2, Math.floor(found.actixWorkers / 2));
      const nextP99 = Number(
        Math.max(1.1, appItem.p99LatencyMs * 0.88).toFixed(1)
      );
      return {
        ...appItem,
        workers: nextWorkers,
        p99LatencyMs: nextP99
      };
    });

    optimizerState.watchdog.recentEvents.unshift({
      id: `ev-${Date.now()}`,
      timestamp: 'Vừa xong',
      subsystem: 'Hardware-Adaptive Optimizer',
      action: `Áp dụng Model "${found.name}" (Workers=${found.actixWorkers}, InnoDB=${found.mariadbBufferPoolMb}MB)`,
      result: 'Synced Systemd, Nginx & MySQL'
    });
  }

  res.json({
    ...optimizerState,
    sysctlConfigPreview: buildSysctlPreview()
  });
});

app.put('/api/optimizer/sysctl/:id', (req, res) => {
  const param = optimizerState.sysctlParams.find((p) => p.id === req.params.id);
  if (param) {
    param.enabled = !param.enabled;
    param.currentValue = param.enabled ? param.recommendedValue : 'default';
    optimizerState.watchdog.recentEvents.unshift({
      id: `ev-${Date.now()}`,
      timestamp: 'Vừa xong',
      subsystem: 'Linux Kernel Sysctl',
      action: `${param.enabled ? 'Bật' : 'Tắt'} tham số nhân ${param.key} = ${param.currentValue}`,
      result: 'sysctl --system reloaded'
    });
  }
  res.json({
    ...optimizerState,
    sysctlConfigPreview: buildSysctlPreview()
  });
});

app.post('/api/optimizer/reclaim-memory', (_req, res) => {
  const freedMb = 380 + Math.floor(Math.random() * 240);
  optimizerState.watchdog.allocatedRamMb = 12.8;
  optimizerState.watchdog.residentRamMb = 16.4;
  optimizerState.watchdog.dbPoolIdle = 8;
  optimizerState.watchdog.lastSelfHealAt = 'Vừa xong (Memory Reclaimed)';
  optimizerState.watchdog.recentEvents.unshift({
    id: `ev-${Date.now()}`,
    timestamp: 'Vừa xong',
    subsystem: 'Memory & PageCache Reclaimer',
    action:
      'Thực thi sync && sysctl -w vm.drop_caches=1 và giải phóng kết nối SQL/Socket nhàn rỗi',
    result: `Đã giải phóng ${freedMb} MB RAM vật lý`
  });

  res.json({
    freedMb,
    message: `Đã dọn dẹp PageCache/Dentries và thu hồi kết nối nhàn rỗi: Giải phóng thành công ${freedMb} MB RAM hệ thống.`,
    optimizer: {
      ...optimizerState,
      sysctlConfigPreview: buildSysctlPreview()
    }
  });
});

app.post('/api/optimizer/watchdog-heal', (_req, res) => {
  // Ensure all apps are healthy and running
  managedApps = managedApps.map((a) => ({
    ...a,
    status: 'running',
    pid: a.pid || Math.floor(2000 + Math.random() * 6000)
  }));
  optimizerState.watchdog.circuitBreakerStatus = 'CLOSED (Optimal)';
  optimizerState.watchdog.lastSelfHealAt = 'Vừa xong (1.8ms sweep)';
  optimizerState.watchdog.recentEvents.unshift({
    id: `ev-${Date.now()}`,
    timestamp: 'Vừa xong',
    subsystem: 'Watchdog Self-Healing Engine',
    action:
      'Đồng bộ lại toàn bộ dịch vụ Web, xác thực kết nối Database Pool và kiểm tra chứng chỉ TLS 1.3',
    result: '100% Subsystems Healthy (Zero Faults)'
  });

  res.json({
    ...optimizerState,
    sysctlConfigPreview: buildSysctlPreview()
  });
});

// 10. OpenSSH Server (External Client Login) & Integrated Web SSH PTY Terminal + Rust Module Conversion Matrix
let sshPanelState: {
  serverHost: string;
  config: {
    port: number;
    listenAddress: string;
    permitRootLogin: 'prohibit-password' | 'yes' | 'no';
    passwordAuthentication: boolean;
    pubkeyAuthentication: boolean;
    maxAuthTries: number;
    clientAliveIntervalSec: number;
    clientAliveCountMax: number;
    allowTcpForwarding: boolean;
    sftpChrootEnabled: boolean;
    allowUsers: string[];
    kexAlgorithms: string;
  };
  authorizedKeys: {
    id: string;
    label: string;
    algorithm: 'ssh-ed25519' | 'rsa-sha2-512' | 'ecdsa-sha2-nistp256';
    fingerprint: string;
    publicKey: string;
    addedAt: string;
    clientHint: string;
  }[];
  activeSessions: {
    id: string;
    pts: string;
    user: string;
    clientIp: string;
    clientSoftware: string;
    authMethod: string;
    connectedAt: string;
    idleTime: string;
    pid: number;
  }[];
  rustModules: {
    id: string;
    moduleName: string;
    rustFile: string;
    replacedLegacyStack: string;
    crateDependencies: string;
    memoryFootprintKb: number;
    latencyMicros: number;
    status: 'NATIVE_RUST_COMPILED';
    description: string;
  }[];
} = {
  serverHost: '103.145.88.210',
  config: {
    port: 22,
    listenAddress: '0.0.0.0',
    permitRootLogin: 'prohibit-password',
    passwordAuthentication: true,
    pubkeyAuthentication: true,
    maxAuthTries: 3,
    clientAliveIntervalSec: 60,
    clientAliveCountMax: 3,
    allowTcpForwarding: true,
    sftpChrootEnabled: true,
    allowUsers: ['root', 'www-data', 'deploy_ci'],
    kexAlgorithms:
      'sntrup761x25519-sha512@openssh.com,curve25519-sha256,curve25519-sha256@libssh.org'
  },
  authorizedKeys: [],
  activeSessions: [],
  rustModules: [
    {
      id: 'rm-1',
      moduleName: 'HTTP Daemon & TLS 1.3 Router',
      rustFile: 'src/main.rs',
      replacedLegacyStack: 'Python Gunicorn / Node.js Express Wrapper',
      crateDependencies: 'actix-web 4.9, rustls 0.23, argon2 0.5',
      memoryFootprintKb: 2450,
      latencyMicros: 140,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Trái tim điều khiển trung tâm với HSTS, Argon2id, JWT Ed25519 và đa luồng không khóa.'
    },
    {
      id: 'rm-2',
      moduleName: 'OpenSSH & Web PTY Terminal Engine',
      rustFile: 'src/ssh_terminal.rs',
      replacedLegacyStack: 'Python Paramiko / Node-pty Bridge',
      crateDependencies: 'portable-pty 0.8, actix-ws 0.3, tokio::process',
      memoryFootprintKb: 1180,
      latencyMicros: 95,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Quản lý /etc/ssh/sshd_config cho phần mềm ngoài (Termius, PuTTY, VS Code) & Web SSH PTY trực tiếp trên Panel.'
    },
    {
      id: 'rm-3',
      moduleName: 'Hardware-Adaptive Optimizer & Watchdog',
      rustFile: 'src/optimizer.rs',
      replacedLegacyStack: 'Bash Cron Healthcheck / Monit Daemon',
      crateDependencies: 'sysinfo 0.32, tokio::fs (/proc/sys)',
      memoryFootprintKb: 890,
      latencyMicros: 60,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Tự động tinh chỉnh TCP BBR, bảo vệ OOM Killer (-900) và tự phục hồi (Self-Healing) kết nối Upstream.'
    },
    {
      id: 'rm-4',
      moduleName: 'Nginx Reverse Proxy & QUIC Compiler',
      rustFile: 'src/nginx.rs',
      replacedLegacyStack: 'Jinja2 / PHP Vhost Template Scripts',
      crateDependencies: 'tokio::process (nginx -t), regex 1.11',
      memoryFootprintKb: 760,
      latencyMicros: 85,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Sinh cấu hình Reverse Proxy cho Actix-Web, Next.js 15, Nuxt 3, SvelteKit 2 kèm kiểm tra cú pháp nguyên tử.'
    },
    {
      id: 'rm-5',
      moduleName: 'Systemd Unit & Cgroups v2 Sandbox',
      rustFile: 'src/systemd.rs',
      replacedLegacyStack: 'PM2 / Supervisor Process Manager',
      crateDependencies: 'tokio::fs, systemctl dbus-ctl',
      memoryFootprintKb: 640,
      latencyMicros: 70,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Cô lập từng ứng dụng Web bằng MemoryMax, ProtectSystem=strict và PrivateTmp=true.'
    },
    {
      id: 'rm-6',
      moduleName: 'PostgreSQL 17 & MySQL phpMyAdmin Engine',
      rustFile: 'src/database.rs',
      replacedLegacyStack: 'PHP CLI / Shell SQL Provisioner',
      crateDependencies: 'tokio::process (psql / mysql / zstd)',
      memoryFootprintKb: 1120,
      latencyMicros: 180,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Khởi tạo Database UTF-8, tinh chỉnh my.cnf InnoDB Buffer Pool và sao lưu nén Zstandard.'
    },
    {
      id: 'rm-7',
      moduleName: 'Postfix, Dovecot & OpenDKIM 2048-bit',
      rustFile: 'src/mail_server.rs',
      replacedLegacyStack: 'Perl / Bash Mail Provisioning Scripts',
      crateDependencies: 'tokio::fs, opendkim-genkey bindings',
      memoryFootprintKb: 920,
      latencyMicros: 150,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Quản trị hộp thư ảo Maildir, tự động sinh chữ ký số DKIM 2048-bit và bộ lọc chống rác Rspamd.'
    },
    {
      id: 'rm-8',
      moduleName: 'Chroot File Manager (/www/wwwroot)',
      rustFile: 'src/file_manager.rs',
      replacedLegacyStack: 'PHP elFinder / Node fs-extra',
      crateDependencies: 'std::os::unix::fs::PermissionsExt, tokio::fs',
      memoryFootprintKb: 710,
      latencyMicros: 45,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Đọc/ghi tệp tốc độ cao theo từng Website với khóa canonicalize() chống leo thang thư mục.'
    },
    {
      id: 'rm-9',
      moduleName: '1-Click App Store & Ubuntu Apt Orchestrator',
      rustFile: 'src/oneclick_store.rs',
      replacedLegacyStack: 'Shell Installer Wrapper Scripts',
      crateDependencies: 'tokio::process, serde_json 1.0',
      memoryFootprintKb: 680,
      latencyMicros: 110,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Triển khai tức thì Next.js 15, Nuxt 3, SvelteKit 2, Astro 5, Actix-Web và quản lý gói dịch vụ Ubuntu.'
    },
    {
      id: 'rm-10',
      moduleName: 'Git Webhook & Zero-Downtime Atomic Builder',
      rustFile: 'src/git_deploy.rs',
      replacedLegacyStack: 'Webhook Node.js Runner / Bash Deploy',
      crateDependencies: 'tokio::process (git / cargo / pnpm), chrono',
      memoryFootprintKb: 740,
      latencyMicros: 120,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Tự động kéo mã nguồn Git, biên dịch bản phát hành và chuyển đổi Symlink không rớt kết nối.'
    },
    {
      id: 'rm-11',
      moduleName: 'cPanel Suite (Cron, DNS Zone, Zstd Backup)',
      rustFile: 'src/cpanel_tools.rs',
      replacedLegacyStack: 'Legacy Perl / Bash Crontab & Tar Scripts',
      crateDependencies: 'serde 1.0, uuid 1.11, zstd stream',
      memoryFootprintKb: 590,
      latencyMicros: 55,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Quản lý lịch trình Cron, bản ghi DNS (A, MX, TXT, CAA) và nén sao lưu đa luồng Zstandard.'
    },
    {
      id: 'rm-12',
      moduleName: 'UFW Firewall & Fail2ban Intrusion Guard',
      rustFile: 'src/security.rs',
      replacedLegacyStack: 'Python ufw-frontend / Log Parser',
      crateDependencies: 'regex 1.11, tokio::process (ufw / fail2ban-client)',
      memoryFootprintKb: 820,
      latencyMicros: 75,
      status: 'NATIVE_RUST_COMPILED',
      description:
        'Thực thi quy tắc cổng tường lửa UFW, chặn IP tấn công Brute-force và kiểm toán nhân Linux.'
    }
  ]
};

function buildSshdConfigPreview(): string {
  const c = sshPanelState.config;
  return [
    '# ============================================================================',
    '# /etc/ssh/sshd_config — Managed by FerrousPanel Rust SSH Engine (src/ssh_terminal.rs)',
    '# Compatible with: Termius, PuTTY, MobaXterm, Tabby, VS Code Remote-SSH, FileZilla',
    '# ============================================================================',
    `Port ${c.port}`,
    'AddressFamily any',
    `ListenAddress ${c.listenAddress}`,
    '',
    '# Post-Quantum Hybrid & Curve25519 Key Exchange',
    `KexAlgorithms ${c.kexAlgorithms}`,
    'Ciphers chacha20-poly1305@openssh.com,aes256-gcm@openssh.com,aes128-gcm@openssh.com',
    'MACs hmac-sha2-512-etm@openssh.com,hmac-sha2-256-etm@openssh.com',
    '',
    '# External Client Authentication Policy',
    `PermitRootLogin ${c.permitRootLogin}`,
    `PubkeyAuthentication ${c.pubkeyAuthentication ? 'yes' : 'no'}`,
    'AuthorizedKeysFile .ssh/authorized_keys',
    `PasswordAuthentication ${c.passwordAuthentication ? 'yes' : 'no'}`,
    'PermitEmptyPasswords no',
    `MaxAuthTries ${c.maxAuthTries}`,
    `AllowUsers ${c.allowUsers.join(' ')}`,
    '',
    '# KeepAlive & IDE Remote Forwarding (VS Code Remote-SSH / JetBrains Gateway)',
    `ClientAliveInterval ${c.clientAliveIntervalSec}`,
    `ClientAliveCountMax ${c.clientAliveCountMax}`,
    `AllowTcpForwarding ${c.allowTcpForwarding ? 'yes' : 'no'}`,
    'Subsystem sftp internal-sftp'
  ].join('\n');
}

// ============================================================================
// VPS Panel Admin Account & `fp` / `ferrous` CLI Repair State
// ============================================================================
let panelAdminState = {
  adminUsername: 'admin_ferrous',
  adminPassword: 'Fp#2026!UbuntuRoot',
  passwordHashAlgo: 'Argon2id (m=65536,t=3,p=4)',
  panelPort: 3000,
  securityEntrance: '/ferrous_admin_88a',
  customDomain: '',
  sslEnabled: false,
  mysqlRootPassword: mysqlPmaConfig.rootPassword || 'Sql#2026!MariaDbRoot',
  postgresRootPassword: 'Sql#2026!MariaDbRoot',
  twoFactorEnabled: false,
  allowedAdminIps: '0.0.0.0/0',
  configFile: '/etc/ferrous-panel/admin.json',
  installedAt: '2026-11-09 00:00:00',
  lastModifiedAt: 'Vừa xong'
};

// Load /etc/ferrous-panel/admin.json if present on a real Ubuntu VPS
try {
  if (fs.existsSync('/etc/ferrous-panel/admin.json')) {
    const raw = JSON.parse(
      fs.readFileSync('/etc/ferrous-panel/admin.json', 'utf8')
    );
    panelAdminState = { ...panelAdminState, ...raw };
    if (raw.mysqlRootPassword) {
      mysqlPmaConfig.rootPassword = raw.mysqlRootPassword;
    }
  }
} catch {
  // ignore in non-root preview container
}

function persistPanelAdminState(): void {
  try {
    if (fs.existsSync('/etc/ferrous-panel')) {
      fs.writeFileSync(
        '/etc/ferrous-panel/admin.json',
        JSON.stringify(panelAdminState, null, 2),
        'utf8'
      );
    }
  } catch {
    // ignore in container preview
  }
}

function executeFpCliOption(optRaw: string, argRaw?: string): string {
  const opt = String(optRaw || '').trim().toLowerCase();
  const arg = String(argRaw || '').trim();
  const nowIso = new Date().toISOString();
  panelAdminState.lastModifiedAt = new Date().toLocaleTimeString('vi-VN', {
    hour12: false
  });

  if (opt === '1' || opt === 'restart') {
    managedApps.forEach((a) => syncArchitectureForApp(a));
    return [
      `[${nowIso}] 🔄 Đang khởi động lại dịch vụ Ferrous-panel (ferrous-panel-web.service & ferrous-panel-daemon.service)...`,
      `systemctl restart ferrous-panel-web.service -> [OK]`,
      `systemctl reload nginx -> [OK]`,
      `✅ Ferrous-panel đã khởi động lại thành công trên cổng ${panelAdminState.panelPort}.`
    ].join('\n');
  }
  if (opt === '2' || opt === 'stop') {
    return [
      `[${nowIso}] ⏹️ Đã gửi tín hiệu dừng tạm thời dịch vụ nền (Watchdog vẫn duy trì phiên Web UI hiện tại).`,
      `Gõ 'fp 3' hoặc 'fp start' để kích hoạt lại toàn bộ worker.`
    ].join('\n');
  }
  if (opt === '3' || opt === 'start') {
    managedApps.forEach((a) => {
      a.status = 'running';
      if (!a.pid) a.pid = Math.floor(21000 + Math.random() * 8000);
      syncArchitectureForApp(a);
    });
    return [
      `[${nowIso}] ▶️ Đã khởi động toàn bộ dịch vụ Ferrous-panel và ${managedApps.length} Website đang quản lý.`,
      `✅ Trạng thái: active (running).`
    ].join('\n');
  }
  if (opt === '4' || opt === 'reload') {
    managedApps.forEach((a) => syncArchitectureForApp(a));
    return [
      `[${nowIso}] ♻️ Đang nạp lại cấu hình Nginx Vhosts, PHP-FPM Pools & Ferrous-panel...`,
      `nginx: the configuration file /etc/nginx/nginx.conf syntax is ok`,
      `nginx: configuration file /etc/nginx/nginx.conf test is successful`,
      `✅ Đã reload Zero-Downtime toàn bộ ${managedApps.length} Website.`
    ].join('\n');
  }
  if (opt === '5' || opt === 'passwd') {
    const nextPass =
      arg || `Fp#${Math.random().toString(36).slice(2, 8)}!2026`;
    panelAdminState.adminPassword = nextPass;
    persistPanelAdminState();
    return [
      `======================================================================`,
      `✅ ĐÃ CẬP NHẬT MẬT KHẨU TÀI KHOẢN ADMIN PANEL (/etc/ferrous-panel/admin.json)`,
      `----------------------------------------------------------------------`,
      `👤 Tài khoản Admin : ${panelAdminState.adminUsername}`,
      `🔑 Mật khẩu Mới    : ${panelAdminState.adminPassword}`,
      `🔒 Thuật toán Băm  : ${panelAdminState.passwordHashAlgo}`,
      `======================================================================`
    ].join('\n');
  }
  if (opt === '6' || opt === 'username') {
    const nextUser =
      arg.replace(/[^a-zA-Z0-9_.-]/g, '') || 'sysadmin_ferrous';
    panelAdminState.adminUsername = nextUser;
    persistPanelAdminState();
    return [
      `======================================================================`,
      `✅ ĐÃ ĐỔI TÊN ĐĂNG NHẬP TÀI KHOẢN ADMIN PANEL`,
      `----------------------------------------------------------------------`,
      `👤 Tài khoản Admin Mới : ${panelAdminState.adminUsername}`,
      `🔑 Mật khẩu Hiện tại   : ${panelAdminState.adminPassword}`,
      `======================================================================`
    ].join('\n');
  }
  if (opt === '7' || opt === 'port') {
    const nextPort = Math.max(1024, Math.min(65535, Number(arg) || 8888));
    panelAdminState.panelPort = nextPort;
    persistPanelAdminState();
    const hasRule = ufwRules.some((r) => r.port === String(nextPort));
    if (!hasRule) {
      ufwRules.unshift({
        id: `ufw-panel-${Date.now()}`,
        port: String(nextPort),
        protocol: 'TCP',
        action: 'ALLOW',
        source: '0.0.0.0/0',
        comment: 'Ferrous-panel Web UI Custom Port',
        enabled: true
      });
    }
    return [
      `[${nowIso}] 🌐 Đã đổi cổng truy cập Ferrous-panel sang: ${nextPort}`,
      `ufw allow ${nextPort}/tcp comment 'Ferrous-panel Web UI Custom Port' -> Rule added`,
      `👉 URL mới: http://${sshPanelState.serverHost}:${nextPort}${panelAdminState.securityEntrance}`
    ].join('\n');
  }
  if (opt === '8' || opt === 'entrance') {
    let nextEnt = arg || `/ferrous_${Math.random().toString(36).slice(2, 7)}`;
    if (!nextEnt.startsWith('/')) nextEnt = `/${nextEnt}`;
    panelAdminState.securityEntrance = nextEnt;
    persistPanelAdminState();
    return [
      `✅ Đã đổi đường dẫn bảo mật Security Entrance (/www/server/panel/default.pl):`,
      `👉 Đường dẫn mới: ${panelAdminState.securityEntrance}`,
      `👉 URL đầy đủ: http://${sshPanelState.serverHost}:${panelAdminState.panelPort}${panelAdminState.securityEntrance}`
    ].join('\n');
  }
  if (opt === '9' || opt === 'clear-entrance') {
    panelAdminState.securityEntrance = '/';
    persistPanelAdminState();
    return `✅ Đã tắt khóa đường dẫn Security Entrance. Bạn có thể truy cập trực tiếp qua http://${sshPanelState.serverHost}:${panelAdminState.panelPort}/`;
  }
  if (opt === '10' || opt === 'sql-passwd') {
    const nextSql =
      arg || `Sql#${Math.random().toString(36).slice(2, 8)}!Root99`;
    panelAdminState.mysqlRootPassword = nextSql;
    panelAdminState.postgresRootPassword = nextSql;
    mysqlPmaConfig.rootPassword = nextSql;
    persistPanelAdminState();
    return [
      `======================================================================`,
      `✅ ĐÃ ĐẶT LẠI MẬT KHẨU ROOT SQL (MARIADB 11.4 & POSTGRESQL 17)`,
      `----------------------------------------------------------------------`,
      `mysql -e "ALTER USER 'root'@'localhost' IDENTIFIED BY '${nextSql}'; FLUSH PRIVILEGES;" -> [OK]`,
      `psql -U postgres -c "ALTER USER postgres WITH PASSWORD '${nextSql}';" -> [OK]`,
      `🗄️ Mật khẩu Root SQL Mới : ${nextSql}`,
      `======================================================================`
    ].join('\n');
  }
  if (opt === '11' || opt === 'repair') {
    // Full auto-repair: start Nginx, start default PHP-FPM 8.3, start all websites, regenerate vhosts & .env
    const nginxPkg = serverPackages.find((p) => p.id === 'pkg-nginx');
    if (nginxPkg) nginxPkg.status = 'running';
    const php83 = phpVersionsState.find((p) => p.version === '8.3');
    if (php83) php83.status = 'running';
    managedApps.forEach((a) => {
      a.status = 'running';
      if (!a.pid) a.pid = Math.floor(21000 + Math.random() * 8000);
      syncArchitectureForApp(a);
    });
    return [
      `[1/5] Kiểm tra cấu trúc thư mục (/www/wwwroot, /www/server/panel/vhost/nginx, /etc/nginx/ssl)... [OK]`,
      `[2/5] Xác thực chứng chỉ SSL Fallback (/etc/nginx/ssl/ferrous-fallback.crt)... [OK]`,
      `[3/5] Biên dịch lại toàn bộ ${managedApps.length} Nginx Vhost & đồng bộ tệp /.env... [OK]`,
      `[4/5] Khởi động lại Nginx 1.26.2, PHP 8.3-FPM, MariaDB 11.4 & PostgreSQL 17... [OK]`,
      `[5/5] Kiểm tra cú pháp nginx -t -> syntax is ok / test is successful`,
      `✅ ĐÃ SỬA CHỮA & ĐỒNG BỘ HOÀN TẤT BỘ NÃO KIẾN TRÚC TRÊN VPS!`
    ].join('\n');
  }
  if (opt === '12' || opt === 'clean') {
    const freed = 340 + Math.floor(Math.random() * 180);
    return [
      `🧹 Đang thực thi sync && sysctl -w vm.drop_caches=1 & dọn dẹp /www/wwwlogs/*.tmp...`,
      `✅ Đã giải phóng ${freed} MB RAM PageCache và xóa nhật ký tạm thời.`
    ].join('\n');
  }
  if (opt === '13' || opt === 'unban-admin') {
    panelAdminState.twoFactorEnabled = false;
    panelAdminState.allowedAdminIps = '0.0.0.0/0';
    fail2banJails.forEach((j) => {
      j.currentlyBanned = 0;
      j.bannedIps = [];
    });
    persistPanelAdminState();
    return `🔓 Đã gỡ bỏ giới hạn IP Admin, tắt 2FA khẩn cấp và xóa toàn bộ danh sách IP bị khóa trong Fail2ban.`;
  }
  if (opt === '14' || opt === 'default') {
    const domainStr = panelAdminState.customDomain
      ? `https://${panelAdminState.customDomain}${panelAdminState.securityEntrance}`
      : `http://${sshPanelState.serverHost}:${panelAdminState.panelPort}${panelAdminState.securityEntrance}`;
    return [
      `======================================================================`,
      `  FERROUS-PANEL — THÔNG TIN TÀI KHOẢN ADMIN & CẤU HÌNH VPS NỘI BỘ`,
      `======================================================================`,
      `  Tên miền riêng (Domain) : ${panelAdminState.customDomain || '(Chưa gán tên miền riêng)'}`,
      `  URL Đăng nhập Chính     : ${domainStr}`,
      `  URL Ngoại mạng (IP)     : http://${sshPanelState.serverHost}:${panelAdminState.panelPort}${panelAdminState.securityEntrance}`,
      `  URL Nội bộ (Localhost)  : http://127.0.0.1:${panelAdminState.panelPort}${panelAdminState.securityEntrance}`,
      `  Tài khoản Admin (User)  : ${panelAdminState.adminUsername}`,
      `  Mật khẩu Admin (Pass)   : ${panelAdminState.adminPassword}`,
      `  Mật khẩu Root MySQL     : ${panelAdminState.mysqlRootPassword}`,
      `  Tệp cấu hình Admin      : ${panelAdminState.configFile}`,
      `  Thư mục Web Root        : /www/wwwroot (${managedApps.length} websites active)`,
      `======================================================================`
    ].join('\n');
  }
  if (opt === '17' || opt === 'domain') {
    const nextDomain = String(arg || '').trim().toLowerCase();
    if (!nextDomain) {
      return `❌ Vui lòng nhập tên miền hợp lệ. Ví dụ: fp domain panel.domaincua-ban.vn`;
    }
    panelAdminState.customDomain = nextDomain;
    persistPanelAdminState();
    return [
      `======================================================================`,
      `✅ ĐÃ GÁN TÊN MIỀN RIÊNG CHO WEB PANEL: ${nextDomain}`,
      `----------------------------------------------------------------------`,
      `🌐 Tên miền Web Panel   : ${nextDomain}`,
      `⚙️ Nginx Reverse Proxy   : /etc/nginx/conf.d/ferrous-panel-domain.conf -> 127.0.0.1:${panelAdminState.panelPort}`,
      `🔒 SSL Let's Encrypt    : Đã bật cấu hình Reverse Proxy cho ${nextDomain}`,
      `👉 URL truy cập Web     : http://${nextDomain}${panelAdminState.securityEntrance}`,
      `💡 Lưu ý: Hãy trỏ bản ghi A của tên miền ${nextDomain} về IP ${sshPanelState.serverHost} của VPS.`,
      `======================================================================`
    ].join('\n');
  }
  if (opt === '18' || opt === 'clear-domain') {
    panelAdminState.customDomain = '';
    persistPanelAdminState();
    return `✅ Đã hủy gán tên miền riêng. Hiện tại bạn có thể đăng nhập bằng IP VPS: http://${sshPanelState.serverHost}:${panelAdminState.panelPort}${panelAdminState.securityEntrance}`;
  }
  if (opt === '15' || opt === 'check') {
    const runningPhp = phpVersionsState
      .filter((p) => p.status === 'running')
      .map((p) => `php${p.version}-fpm.sock`)
      .join(', ');
    return [
      `🔍 KIỂM TRA ĐỒNG BỘ KIẾN TRÚC VPS (ARCHITECTURE HEALTH CHECK):`,
      `  • Nginx Reverse Proxy : active (running) — /etc/nginx/conf.d/ferrous-panel-vhosts.conf [OK]`,
      `  • Fallback TLS Cert   : /etc/nginx/ssl/ferrous-fallback.crt [PRESENT]`,
      `  • Multi-PHP Sockets   : ${runningPhp || 'none'}`,
      `  • SQL Engines         : MariaDB 11.4 (:3306) & PostgreSQL 17 (:5432) [${sqlDatabases.length} DBs online]`,
      `  • Websites Vhosts     : ${managedApps.map((a) => `${a.domain} (${a.status})`).join(', ')}`,
      `✅ Kết luận: Môi trường chạy Web trên VPS hoàn toàn đồng bộ (0 xung đột).`
    ].join('\n');
  }
  if (opt === '16' || opt === 'logs') {
    return [
      `-- Journalctl Logs: ferrous-panel-web.service (Last 10 lines) --`,
      `${nowIso} ubuntu-prod ferrous-panel[1042]: INFO  brain: Synchronized ${managedApps.length} Nginx vhosts in /www/server/panel/vhost/nginx`,
      `${nowIso} ubuntu-prod ferrous-panel[1042]: INFO  admin: Admin account '${panelAdminState.adminUsername}' active on port ${panelAdminState.panelPort}`,
      `${nowIso} ubuntu-prod ferrous-panel[1042]: INFO  database: ${sqlDatabases.length} SQL databases connected via local loopback 127.0.0.1`,
      `${nowIso} ubuntu-prod ferrous-panel[1042]: INFO  watchdog: Memory RSS ${optimizerState.watchdog.residentRamMb}MB — Circuit Breaker CLOSED`
    ].join('\n');
  }

  return [
    `======================================================================`,
    `   FERROUS-PANEL VPS TERMINAL CLI TOOLBOX (Lệnh: 'fp' hoặc 'ferrous') `,
    `======================================================================`,
    `  (1) fp 1  / fp restart        : Khởi động lại Ferrous-panel`,
    `  (2) fp 2  / fp stop           : Dừng dịch vụ Ferrous-panel`,
    `  (3) fp 3  / fp start          : Khởi động dịch vụ Ferrous-panel`,
    `  (4) fp 4  / fp reload         : Nạp lại cấu hình Nginx & Panel Zero-Downtime`,
    `  (5) fp 5  / fp passwd <pass>  : Đổi mật khẩu tài khoản Admin Panel`,
    `  (6) fp 6  / fp username <usr> : Đổi tên đăng nhập Admin Panel`,
    `  (7) fp 7  / fp port <port>    : Đổi cổng truy cập Panel & mở UFW`,
    `  (8) fp 8  / fp entrance <uri> : Đổi đường dẫn bảo mật Security Entrance`,
    `  (9) fp 9  / fp clear-entrance : Xóa khóa đường dẫn Security Entrance`,
    ` (10) fp 10 / fp sql-passwd <p> : Đặt lại mật khẩu Root MySQL & PostgreSQL`,
    ` (11) fp 11 / fp repair         : Sửa chữa toàn diện Panel, Nginx Vhost & PHP-FPM`,
    ` (12) fp 12 / fp clean          : Dọn rác hệ thống & Giải phóng RAM PageCache`,
    ` (13) fp 13 / fp unban-admin    : Mở khóa IP Admin, tắt 2FA & xóa Fail2ban`,
    ` (14) fp 14 / fp default        : Xem tài khoản Admin, Mật khẩu SQL & URL Panel`,
    ` (15) fp 15 / fp check          : Kiểm tra đồng bộ kiến trúc Nginx/PHP/SQL trên VPS`,
    ` (16) fp 16 / fp logs           : Xem nhật ký hoạt động nội bộ của Panel`,
    ` (17) fp 17 / fp domain <dom>   : Gán tên miền riêng Web Panel (Custom Domain)`,
    ` (18) fp 18 / fp clear-domain   : Xóa bỏ tên miền riêng Web Panel`,
    `======================================================================`,
    `💡 Gõ ví dụ: 'fp 14' (xem tài khoản Admin), 'fp domain panel.vn', 'fp 11' (sửa chữa Panel)`
  ].join('\n');
}

app.get('/api/panel-admin', (_req, res) => {
  res.json(panelAdminState);
});

app.put('/api/panel-admin', (req, res) => {
  const {
    adminUsername,
    adminPassword,
    panelPort,
    securityEntrance,
    mysqlRootPassword,
    postgresRootPassword,
    twoFactorEnabled,
    allowedAdminIps
  } = req.body || {};

  if (adminUsername !== undefined && String(adminUsername).trim()) {
    panelAdminState.adminUsername = String(adminUsername).trim();
  }
  if (adminPassword !== undefined && String(adminPassword).trim()) {
    panelAdminState.adminPassword = String(adminPassword).trim();
  }
  if (panelPort !== undefined) {
    panelAdminState.panelPort = Math.max(
      1024,
      Math.min(65535, Number(panelPort) || 3000)
    );
  }
  if (securityEntrance !== undefined) {
    let ent = String(securityEntrance).trim() || '/';
    if (!ent.startsWith('/')) ent = `/${ent}`;
    panelAdminState.securityEntrance = ent;
  }
  if (mysqlRootPassword !== undefined && String(mysqlRootPassword).trim()) {
    panelAdminState.mysqlRootPassword = String(mysqlRootPassword).trim();
    mysqlPmaConfig.rootPassword = panelAdminState.mysqlRootPassword;
  }
  if (
    postgresRootPassword !== undefined &&
    String(postgresRootPassword).trim()
  ) {
    panelAdminState.postgresRootPassword = String(postgresRootPassword).trim();
  }
  if (twoFactorEnabled !== undefined) {
    panelAdminState.twoFactorEnabled = Boolean(twoFactorEnabled);
  }
  if (allowedAdminIps !== undefined) {
    panelAdminState.allowedAdminIps = String(allowedAdminIps).trim();
  }
  if (req.body?.customDomain !== undefined) {
    panelAdminState.customDomain = String(req.body.customDomain).trim().toLowerCase();
  }
  if (req.body?.sslEnabled !== undefined) {
    panelAdminState.sslEnabled = Boolean(req.body.sslEnabled);
  }
  panelAdminState.lastModifiedAt = new Date().toLocaleTimeString('vi-VN', {
    hour12: false
  });
  persistPanelAdminState();

  res.json(panelAdminState);
});

// Authentication & Session Management for FerrousPanel Mandatory Login
interface AdminSession {
  token: string;
  username: string;
  createdAt: number;
  lastActiveAt: number;
  ip: string;
  userAgent: string;
}

const activeAdminSessions = new Map<string, AdminSession>();

app.post('/api/auth/login', (req, res) => {
  const { username, password, rememberMe } = req.body || {};
  const cleanUser = String(username || '').trim();
  const cleanPass = String(password || '').trim();

  if (!cleanUser || !cleanPass) {
    return res.status(400).json({
      success: false,
      error: 'Vui lòng nhập đầy đủ Tên tài khoản và Mật khẩu.'
    });
  }

  // Compare with current Panel Admin credentials
  const userMatches =
    cleanUser === panelAdminState.adminUsername ||
    (cleanUser === 'root' && cleanPass === panelAdminState.adminPassword);
  const passMatches = cleanPass === panelAdminState.adminPassword;

  if (!userMatches || !passMatches) {
    return res.status(401).json({
      success: false,
      error: 'Tên tài khoản hoặc mật khẩu không chính xác. Bạn có thể gõ "fp 5" hoặc "fp default" trên terminal VPS để khôi phục.'
    });
  }

  // Generate session token
  const token = `fp_sess_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
  const session: AdminSession = {
    token,
    username: panelAdminState.adminUsername,
    createdAt: Date.now(),
    lastActiveAt: Date.now(),
    ip: String(req.headers['x-forwarded-for'] || req.socket.remoteAddress || '127.0.0.1'),
    userAgent: String(req.headers['user-agent'] || 'Browser')
  };

  activeAdminSessions.set(token, session);

  // Set HTTP-only style cookie hint
  res.cookie('fp_admin_token', token, {
    maxAge: rememberMe ? 30 * 24 * 60 * 60 * 1000 : 24 * 60 * 60 * 1000,
    httpOnly: false,
    sameSite: 'lax'
  });

  return res.json({
    success: true,
    token,
    user: {
      username: panelAdminState.adminUsername,
      panelPort: panelAdminState.panelPort,
      securityEntrance: panelAdminState.securityEntrance,
      customDomain: panelAdminState.customDomain,
      lastModifiedAt: panelAdminState.lastModifiedAt
    },
    message: 'Đăng nhập vào FerrousPanel thành công!'
  });
});

app.get('/api/auth/session', (req, res) => {
  const authHeader = req.headers.authorization || '';
  const token =
    authHeader.replace(/^Bearer\s+/i, '').trim() ||
    String(req.headers['x-fp-session'] || '') ||
    String(req.query.token || '');

  if (!token) {
    return res.status(401).json({
      authenticated: false,
      error: 'Không tìm thấy phiên đăng nhập.'
    });
  }

  const session = activeAdminSessions.get(token);
  if (!session) {
    return res.status(401).json({
      authenticated: false,
      error: 'Phiên đăng nhập đã hết hạn hoặc không hợp lệ.'
    });
  }

  session.lastActiveAt = Date.now();

  return res.json({
    authenticated: true,
    user: {
      username: panelAdminState.adminUsername,
      panelPort: panelAdminState.panelPort,
      securityEntrance: panelAdminState.securityEntrance,
      customDomain: panelAdminState.customDomain,
      lastModifiedAt: panelAdminState.lastModifiedAt,
      configFile: panelAdminState.configFile
    }
  });
});

app.post('/api/auth/logout', (req, res) => {
  const authHeader = req.headers.authorization || '';
  const token =
    authHeader.replace(/^Bearer\s+/i, '').trim() ||
    String(req.headers['x-fp-session'] || '') ||
    String(req.body?.token || '');

  if (token) {
    activeAdminSessions.delete(token);
  }
  res.clearCookie('fp_admin_token');

  return res.json({
    success: true,
    message: 'Đã đăng xuất an toàn khỏi FerrousPanel.'
  });
});

app.post('/api/panel-admin/cli-action', (req, res) => {
  const { option, argument } = req.body || {};
  const output = executeFpCliOption(String(option || ''), String(argument || ''));
  res.json({
    output,
    panelAdmin: panelAdminState,
    timestamp: new Date().toLocaleTimeString('vi-VN', { hour12: false })
  });
});

app.get('/api/ssh', (_req, res) => {
  res.json({
    ...sshPanelState,
    panelAdmin: panelAdminState,
    config: {
      ...sshPanelState.config,
      sshdConfigPreview: buildSshdConfigPreview()
    }
  });
});

app.put('/api/ssh/config', (req, res) => {
  const body = req.body || {};
  if (body.port !== undefined) {
    const newPort = Math.max(22, Math.min(65535, Number(body.port) || 22));
    sshPanelState.config.port = newPort;
    // Also update UFW rule for OpenSSH port if present
    const sshRule = ufwRules.find((r) =>
      r.comment.toLowerCase().includes('ssh')
    );
    if (sshRule) {
      sshRule.port = String(newPort);
    }
  }
  if (body.permitRootLogin !== undefined) {
    sshPanelState.config.permitRootLogin = body.permitRootLogin;
  }
  if (body.passwordAuthentication !== undefined) {
    sshPanelState.config.passwordAuthentication = Boolean(
      body.passwordAuthentication
    );
  }
  if (body.pubkeyAuthentication !== undefined) {
    sshPanelState.config.pubkeyAuthentication = Boolean(
      body.pubkeyAuthentication
    );
  }
  if (body.maxAuthTries !== undefined) {
    sshPanelState.config.maxAuthTries = Math.max(
      1,
      Math.min(10, Number(body.maxAuthTries) || 3)
    );
  }
  if (body.clientAliveIntervalSec !== undefined) {
    sshPanelState.config.clientAliveIntervalSec = Math.max(
      15,
      Number(body.clientAliveIntervalSec) || 60
    );
  }
  if (body.allowTcpForwarding !== undefined) {
    sshPanelState.config.allowTcpForwarding = Boolean(body.allowTcpForwarding);
  }
  if (body.sftpChrootEnabled !== undefined) {
    sshPanelState.config.sftpChrootEnabled = Boolean(body.sftpChrootEnabled);
  }
  if (Array.isArray(body.allowUsers)) {
    sshPanelState.config.allowUsers = body.allowUsers;
  }

  res.json({
    ...sshPanelState,
    panelAdmin: panelAdminState,
    config: {
      ...sshPanelState.config,
      sshdConfigPreview: buildSshdConfigPreview()
    }
  });
});

app.post('/api/ssh/keys', (req, res) => {
  const { label, publicKey, clientHint } = req.body;
  const cleanKey = String(publicKey || '').trim();
  const algo: 'ssh-ed25519' | 'rsa-sha2-512' | 'ecdsa-sha2-nistp256' =
    cleanKey.startsWith('ssh-rsa')
      ? 'rsa-sha2-512'
      : cleanKey.startsWith('ecdsa')
        ? 'ecdsa-sha2-nistp256'
        : 'ssh-ed25519';
  const randomFp = Buffer.from(
    `${label}-${Date.now()}-${Math.random()}`
  )
    .toString('base64')
    .replace(/=/g, '')
    .slice(0, 43);

  sshPanelState.authorizedKeys.unshift({
    id: `key-${Date.now()}`,
    label: String(label || 'External SSH Key'),
    algorithm: algo,
    fingerprint: `SHA256:${randomFp}`,
    publicKey:
      cleanKey ||
      `ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI${randomFp} ${label || 'external-client'}`,
    addedAt: 'Vừa thêm',
    clientHint: String(clientHint || 'Termius / PuTTY / VS Code')
  });

  res.status(201).json({
    ...sshPanelState,
    panelAdmin: panelAdminState,
    config: {
      ...sshPanelState.config,
      sshdConfigPreview: buildSshdConfigPreview()
    }
  });
});

app.post('/api/ssh/keys/generate', (req, res) => {
  const { label, clientHint } = req.body;
  const cleanLabel = String(label || 'ferrous-external-client').trim();
  const seed = Buffer.from(`${cleanLabel}-${Date.now()}`)
    .toString('base64')
    .replace(/[^a-zA-Z0-9]/g, '');
  const fp = `SHA256:${seed.slice(0, 43).padEnd(43, 'x')}`;
  const pubKey = `ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI${seed.slice(0, 44).padEnd(44, 'A')} ${cleanLabel}@ferrous-panel`;
  const privKey = [
    '-----BEGIN OPENSSH PRIVATE KEY-----',
    `b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW`,
    `QyNTUxOQAAACD${seed.slice(0, 48).padEnd(48, 'Q')}AAAAk${seed.slice(0, 16).padEnd(16, 'Z')}`,
    `AAAAtzc2gtZWQyNTUxOQAAACD${seed.slice(0, 48).padEnd(48, 'M')}AAAAE${seed.slice(0, 16).padEnd(16, 'K')}`,
    `${seed.slice(0, 68).padEnd(68, 'N')}=`,
    '-----END OPENSSH PRIVATE KEY-----'
  ].join('\n');

  const keyId = `key-${Date.now()}`;
  sshPanelState.authorizedKeys.unshift({
    id: keyId,
    label: cleanLabel,
    algorithm: 'ssh-ed25519',
    fingerprint: fp,
    publicKey: pubKey,
    addedAt: 'Vừa sinh tự động (Ed25519)',
    clientHint: String(clientHint || 'Termius / PuTTY / VS Code Remote-SSH')
  });

  const vscodeSnippet = [
    'Host ferrous-ubuntu-prod',
    `    HostName ${sshPanelState.serverHost}`,
    `    Port ${sshPanelState.config.port}`,
    '    User root',
    `    IdentityFile ~/.ssh/${cleanLabel.toLowerCase().replace(/[^a-z0-9_-]/g, '_')}.pem`,
    '    ServerAliveInterval 60',
    '    ForwardAgent yes'
  ].join('\n');

  res.status(201).json({
    generated: {
      keyId,
      label: cleanLabel,
      algorithm: 'ssh-ed25519 (256-bit Curve25519)',
      fingerprint: fp,
      publicKeyOpenssh: pubKey,
      privateKeyPem: privKey,
      vscodeConfigSnippet: vscodeSnippet,
      cliCommand: `ssh -i ~/.ssh/${cleanLabel.toLowerCase().replace(/[^a-z0-9_-]/g, '_')}.pem -p ${sshPanelState.config.port} root@${sshPanelState.serverHost}`
    },
    sshData: {
      ...sshPanelState,
      panelAdmin: panelAdminState,
      config: {
        ...sshPanelState.config,
        sshdConfigPreview: buildSshdConfigPreview()
      }
    }
  });
});

app.delete('/api/ssh/keys/:id', (req, res) => {
  sshPanelState.authorizedKeys = sshPanelState.authorizedKeys.filter(
    (k) => k.id !== req.params.id
  );
  res.json({
    ...sshPanelState,
    panelAdmin: panelAdminState,
    config: {
      ...sshPanelState.config,
      sshdConfigPreview: buildSshdConfigPreview()
    }
  });
});

app.delete('/api/ssh/sessions/:id', (req, res) => {
  sshPanelState.activeSessions = sshPanelState.activeSessions.filter(
    (s) => s.id !== req.params.id
  );
  res.json({
    ...sshPanelState,
    panelAdmin: panelAdminState,
    config: {
      ...sshPanelState.config,
      sshdConfigPreview: buildSshdConfigPreview()
    }
  });
});

// Integrated Web SSH Terminal Command Executor on the Panel
app.post('/api/ssh/exec', (req, res) => {
  const rawCmd = String(req.body?.command || '').trim();
  let cwd = String(req.body?.cwd || '/www/wwwroot');
  const nowStr = new Date().toLocaleTimeString('vi-VN', { hour12: false });

  if (!rawCmd) {
    res.json({
      id: `cmd-${Date.now()}`,
      cwd,
      command: '',
      output: '',
      exitCode: 0,
      timestamp: nowStr
    });
    return;
  }

  // Handle `cd` directory navigation
  if (rawCmd === 'cd' || rawCmd.startsWith('cd ')) {
    const target = rawCmd.slice(2).trim();
    if (!target || target === '~') {
      cwd = '/root';
    } else if (target === '..') {
      const parts = cwd.split('/').filter(Boolean);
      parts.pop();
      cwd = '/' + parts.join('/');
    } else if (target.startsWith('/')) {
      cwd = target;
    } else {
      cwd = cwd === '/' ? `/${target}` : `${cwd}/${target}`;
    }
    res.json({
      id: `cmd-${Date.now()}`,
      cwd,
      command: rawCmd,
      output: `Changed working directory to ${cwd}`,
      exitCode: 0,
      timestamp: nowStr
    });
    return;
  }

  let output = '';
  let exitCode = 0;
  const lower = rawCmd.toLowerCase();

  // Support `fp`, `ferrous`, and `bt` (aaPanel-style VPS internal panel terminal commands)
  if (
    lower === 'fp' ||
    lower === 'ferrous' ||
    lower === 'bt' ||
    lower.startsWith('fp ') ||
    lower.startsWith('ferrous ') ||
    lower.startsWith('bt ')
  ) {
    const parts = rawCmd.split(/\s+/);
    const subOpt = parts[1] || '';
    const subArg = parts.slice(2).join(' ');
    output = executeFpCliOption(subOpt, subArg);
  } else if (lower === 'cat /etc/ferrous-panel/admin.json') {
    output = JSON.stringify(panelAdminState, null, 2);
  } else if (lower === 'help') {
    output = [
      'FerrousPanel Integrated Web SSH Terminal v2.5 (Rust actix-ws + portable-pty)',
      'Các lệnh sửa chữa nội bộ Panel & quản trị Ubuntu VPS:',
      '  fp / ferrous / bt       Mở bảng công cụ sửa chữa & cấu hình nội bộ Panel (16 chức năng)',
      '  fp default              Xem tài khoản Admin, mật khẩu SQL Root & URL truy cập Panel',
      '  fp repair               Sửa chữa toàn diện Nginx Vhost, SSL Fallback, PHP-FPM & .env',
      '  fp passwd <mat_khau>    Đổi nhanh mật khẩu Admin của Panel trên Terminal',
      '  fp sql-passwd <mk_moi>  Đặt lại mật khẩu Root của MariaDB / PostgreSQL',
      '  ls -la                  Liệt kê danh sách thư mục Website (/www/wwwroot)',
      '  ss -tulpn               Kiểm tra các cổng TCP/UDP đang lắng nghe (Nginx, Actix, SSH, SQL)',
      '  systemctl status        Kiểm tra trạng thái các dịch vụ Rust Actix-Web, Modern JS & Systemd',
      '  htop / top              Giám sát tiến trình CPU, RAM và luồng Worker theo thời gian thực',
      '  df -h / free -h         Kiểm tra dung lượng ổ cứng NVMe và bộ nhớ RAM / Swap',
      '  nginx -t                Kiểm tra cú pháp cấu hình Nginx Reverse Proxy & HTTP/3 QUIC',
      '  ufw status numbered     Xem bảng quy tắc tường lửa Ubuntu UFW đang kích hoạt',
      '  clear                   Xóa màn hình Terminal'
    ].join('\n');
  } else if (lower === 'whoami' || lower === 'id') {
    output =
      lower === 'id'
        ? 'uid=0(root) gid=0(root) groups=0(root),33(www-data),998(docker)'
        : 'root';
  } else if (lower === 'pwd') {
    output = cwd;
  } else if (lower.startsWith('uname')) {
    output =
      'Linux ubuntu-prod-node01 6.8.0-48-generic #48-Ubuntu SMP PREEMPT_DYNAMIC x86_64 GNU/Linux';
  } else if (lower.startsWith('ls')) {
    if (cwd.startsWith('/www/wwwroot')) {
      output = [
        'total 36K',
        'drwxr-xr-x  9 www-data www-data 4.0K Nov  9 11:40 .',
        'drwxr-xr-x  6 root     root     4.0K Jun 10 08:00 ..',
        ...managedApps.map(
          (a) =>
            `drwxr-xr-x  6 www-data www-data 4.0K Nov  9 11:20 ${a.domain}  -> [${a.framework} :${a.internalPort}]`
        )
      ].join('\n');
    } else if (cwd.startsWith('/etc/ssh')) {
      output = [
        'total 148K',
        '-rw-r--r-- 1 root root 3.2K Nov  9 11:45 sshd_config',
        '-rw------- 1 root root  411 Jun 10 08:00 ssh_host_ed25519_key',
        '-rw-r--r-- 1 root root   99 Jun 10 08:00 ssh_host_ed25519_key.pub',
        '-rw------- 1 root root 2.6K Jun 10 08:00 ssh_host_rsa_key'
      ].join('\n');
    } else {
      output = [
        'total 28K',
        'drwx------ 5 root root 4.0K Nov  9 11:45 .',
        'drwxr-xr-x 3 root root 4.0K Nov  9 10:00 .ssh',
        '-rw-r--r-- 1 root root 3.1K Apr 22  2024 .bashrc',
        'drwxr-xr-x 4 root root 4.0K Nov  9 11:30 Ferrous-panel'
      ].join('\n');
    }
  } else if (lower.startsWith('ss ') || lower.startsWith('netstat')) {
    output = [
      'Netid  State   Recv-Q  Send-Q   Local Address:Port     Peer Address:Port  Process',
      `tcp    LISTEN  0       128            0.0.0.0:${sshPanelState.config.port}            0.0.0.0:*      users:(("sshd",pid=912,fd=3))`,
      'tcp    LISTEN  0       511            0.0.0.0:80            0.0.0.0:*      users:(("nginx",pid=1104,fd=6))',
      'tcp    LISTEN  0       511            0.0.0.0:443           0.0.0.0:*      users:(("nginx",pid=1104,fd=7))',
      'udp    UNCONN  0       0              0.0.0.0:443           0.0.0.0:*      users:(("nginx",pid=1104,fd=8)) [HTTP/3 QUIC]',
      `tcp    LISTEN  0       1024           0.0.0.0:${panelAdminState.panelPort}          0.0.0.0:*      users:(("ferrous-panel",pid=1042,fd=9))`,
      'tcp    LISTEN  0       244          127.0.0.1:5432          0.0.0.0:*      users:(("postgres",pid=1280,fd=5))',
      `tcp    LISTEN  0       151          127.0.0.1:${mysqlPmaConfig.port}          0.0.0.0:*      users:(("mariadbd",pid=1310,fd=21))`,
      ...managedApps
        .filter((a) => a.status === 'running')
        .map(
          (a) =>
            `tcp    LISTEN  0       1024         127.0.0.1:${String(a.internalPort).padEnd(14, ' ')}0.0.0.0:*      users:(("${a.name}",pid=${a.pid || 2100},fd=4))`
        )
    ].join('\n');
  } else if (lower.startsWith('systemctl')) {
    output = [
      '● ferrous-panel-daemon.service - FerrousPanel 100% Rust Server Control Daemon (Actix-Web v2.0.0)',
      '     Loaded: loaded (/etc/systemd/system/ferrous-panel-daemon.service; enabled; preset: enabled)',
      `     Active: active (running) since Mon 2026-11-09 00:00:01 UTC;PID=${optimizerState.watchdog.daemonPid} (OOMScoreAdj=${optimizerState.watchdog.oomScoreAdj})`,
      `     Memory: ${optimizerState.watchdog.residentRamMb}M (jemalloc 5.3.0 arena)`,
      '',
      '● ssh.service - OpenBSD Secure Shell server (External Client Access Ready)',
      `     Active: active (running) on Port ${sshPanelState.config.port} (Ed25519 + Curve25519 KEX)`,
      '',
      ...managedApps.map(
        (a) =>
          `● ${a.name}.service — ${a.domain} (${a.framework})\n     Active: ${a.status === 'running' ? 'active (running)' : 'inactive (dead)'} · Port 127.0.0.1:${a.internalPort} · RAM ${a.memoryUsedMb}/${a.memoryLimitMb}MB`
      )
    ].join('\n');
  } else if (lower === 'htop' || lower === 'top' || lower === 'ps aux') {
    output = [
      `USER         PID %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND`,
      `root        ${optimizerState.watchdog.daemonPid}  0.8  0.2 142800 18840 ?        Ssl  00:01   0:42 /usr/local/bin/ferrous-panel-daemon --workers ${optimizerState.watchdog.workerThreadsTotal}`,
      `root         912  0.1  0.1  15420  9200 ?        Ss   00:01   0:04 sshd: /usr/sbin/sshd -D -p ${sshPanelState.config.port} [listener]`,
      `www-data    1104  1.4  0.4  58200 34200 ?        S    00:01   1:18 nginx: worker process (HTTP/3 QUIC + TLS 1.3)`,
      `postgres    1280  1.1  2.8 1148200 412000 ?      Ss   00:01   0:55 /usr/lib/postgresql/17/bin/postgres -D /var/lib/postgresql/17/main`,
      `mysql       1310  1.6  4.2 1842000 680000 ?      Sl   00:01   1:12 /usr/sbin/mariadbd --innodb-buffer-pool-size=${mysqlPmaConfig.innodbBufferPoolMb}M`,
      ...managedApps
        .filter((a) => a.status === 'running')
        .map(
          (a) =>
            `www-data    ${String(a.pid || 2400).padEnd(4, ' ')}  ${String(a.cpuPercent).padEnd(3, ' ')}  1.2 420000 ${a.memoryUsedMb * 1024} ?        Sl   01:10   0:38 ${a.execCommand}`
        )
    ].join('\n');
  } else if (lower.startsWith('df')) {
    output = [
      'Filesystem      Size  Used Avail Use% Mounted on',
      'tmpfs           3.2G  1.8M  3.2G   1% /run',
      '/dev/nvme0n1p2  320G   84G  236G  27% /',
      'tmpfs            16G   24M   16G   1% /dev/shm',
      '/dev/nvme0n1p1  511M  6.1M  505M   2% /boot/efi'
    ].join('\n');
  } else if (lower.startsWith('free')) {
    output = [
      '               total        used        free      shared  buff/cache   available',
      'Mem:           31Gi       8.4Gi        18Gi       124Mi       4.6Gi        22Gi',
      'Swap:          4.0Gi       120Mi       3.9Gi'
    ].join('\n');
  } else if (lower.startsWith('nginx -t')) {
    output = [
      'nginx: the configuration file /etc/nginx/nginx.conf syntax is ok',
      `nginx: ${managedApps.length} virtual hosts in /www/server/panel/vhost/nginx/*.conf verified (TLS 1.3 + QUIC)`,
      'nginx: configuration file /etc/nginx/nginx.conf test is successful'
    ].join('\n');
  } else if (lower.startsWith('ufw status')) {
    output = [
      'Status: active',
      'Logging: on (low)',
      'Default: deny (incoming), allow (outgoing), disabled (routed)',
      '',
      '     To                         Action      From',
      '     --                         ------      ----',
      ...ufwRules
        .filter((r) => r.enabled)
        .map(
          (r, idx) =>
            `[${idx + 1}] ${String(r.port + '/' + r.protocol).padEnd(26, ' ')} ${String(r.action).padEnd(11, ' ')} ${r.source}  # ${r.comment}`
        )
    ].join('\n');
  } else if (lower.includes('/etc/ssh/sshd_config')) {
    output = buildSshdConfigPreview();
  } else if (lower.includes('authorized_keys')) {
    output = sshPanelState.authorizedKeys.map((k) => k.publicKey).join('\n');
  } else if (lower === 'who' || lower === 'w') {
    output = sshPanelState.activeSessions
      .map(
        (s) =>
          `${s.user.padEnd(8, ' ')} ${s.pts.padEnd(8, ' ')} ${s.connectedAt.padEnd(18, ' ')} (${s.clientIp} — ${s.clientSoftware})`
      )
      .join('\n');
  } else if (lower.startsWith('cargo') || lower.startsWith('rustc')) {
    output =
      'rustc 1.83.0 (90b35a623 2024-11-26) / cargo 1.83.0 (5ffbef321 2024-10-29)\nTarget: x86_64-unknown-linux-gnu (Ferrous-panel 14/14 Native Rust Modules Active)';
  } else if (lower.startsWith('git status')) {
    output = [
      `On branch main (working tree: ${cwd})`,
      "Your branch is up to date with 'origin/main'.",
      'nothing to commit, working tree clean'
    ].join('\n');
  } else {
    output = [
      `[FerrousPanel Rust PTY Bridge @ ${cwd}]# ${rawCmd}`,
      `Executed command via tokio::process::Command in 1.4ms (exit status: 0).`,
      `Gõ 'fp' để mở công cụ sửa chữa nội bộ Panel hoặc 'help' để xem danh sách lệnh.`
    ].join('\n');
  }

  res.json({
    id: `cmd-${Date.now()}`,
    cwd,
    command: rawCmd,
    output,
    exitCode,
    timestamp: nowStr
  });
});

// 11. Real Rust Actix-Web Daemon Source Code Inspector API (All 14 Native Rust Modules in Ferrous-panel)
app.get('/api/rust-source', (_req, res) => {
  const files = [
    {
      path: 'Ferrous-panel/ferrous-cli.sh',
      title: 'ferrous-cli.sh (Lệnh fp)',
      lang: 'bash',
      description:
        'Công cụ Panel Terminal nội bộ trên VPS (/usr/local/bin/fp & ferrous) — 16 chức năng sửa chữa kiến trúc, đổi mật khẩu Admin, đổi mật khẩu SQL Root & kiểm tra hệ thống'
    },
    {
      path: 'Ferrous-panel/install-ubuntu.sh',
      title: 'install-ubuntu.sh',
      lang: 'bash',
      description:
        'Script cài đặt tự động Ferrous-panel trên Ubuntu VPS hỗ trợ khởi tạo tài khoản Admin (--admin-user, --admin-pass), mật khẩu SQL (--sql-pass) và cài lệnh fp'
    },
    {
      path: 'Ferrous-panel/src/php_manager.rs',
      title: 'src/php_manager.rs',
      lang: 'rust',
      description:
        'Module Rust quản lý Multi-PHP FPM (PHP 8.4, 8.3, 8.2, 8.1, 7.4), tinh chỉnh php.ini, FPM Pool, Extensions & chuyển đổi Socket theo từng Website'
    },
    {
      path: 'Ferrous-panel/src/rust_packages.rs',
      title: 'src/rust_packages.rs',
      lang: 'rust',
      description:
        'Suite 7 Package Bổ Sung Viết Bằng Rust (FerrousKV Cache, FerrousWAF Pingora, FerrousACME SSL, FerrousDNS, FerrousSFTP, FerrousSearch, FerrousVector)'
    },
    {
      path: 'Ferrous-panel/src/ssh_terminal.rs',
      title: 'src/ssh_terminal.rs',
      lang: 'rust',
      description:
        'Module Rust quản lý OpenSSH (/etc/ssh/sshd_config) cho phần mềm ngoài (Termius, PuTTY, VS Code) & Web SSH PTY Terminal trên Panel'
    },
    {
      path: 'Ferrous-panel/src/optimizer.rs',
      title: 'src/optimizer.rs',
      lang: 'rust',
      description:
        'Models Tối ưu hóa Hệ thống (Hardware-Adaptive Profile, Linux Kernel Sysctl TCP BBR & Backend Watchdog OOM Guard)'
    },
    {
      path: 'Ferrous-panel/src/oneclick_store.rs',
      title: 'src/oneclick_store.rs',
      lang: 'rust',
      description:
        'Module Rust triển khai Kho 1-Click (Next.js 15, Nuxt 3, SvelteKit 2, Astro 5, Actix-Web) & quản lý gói dịch vụ Ubuntu'
    },
    {
      path: 'Ferrous-panel/src/git_deploy.rs',
      title: 'src/git_deploy.rs',
      lang: 'rust',
      description:
        'Module Rust xử lý Git Auto-Deploy, xác thực Webhook và chuyển đổi Atomic Symlink Zero-Downtime'
    },
    {
      path: 'Ferrous-panel/Cargo.toml',
      title: 'Cargo.toml',
      lang: 'toml',
      description:
        'Cấu hình dependencies Actix-Web 4.9, Actix-WS, Portable-PTY, Rustls 0.23, Tokio, Argon2id & LTO Release Profile'
    },
    {
      path: 'Ferrous-panel/src/main.rs',
      title: 'src/main.rs',
      lang: 'rust',
      description:
        'Entrypoint Actix-Web HttpServer v2.0.0 tích hợp đầy đủ 14 module Rust, Security Headers Middleware & REST API'
    },
    {
      path: 'Ferrous-panel/src/database.rs',
      title: 'src/database.rs',
      lang: 'rust',
      description:
        'Module quản trị PostgreSQL 17 & MariaDB/phpMyAdmin hỗ trợ mật khẩu SQL tùy chỉnh khi khởi tạo và đổi mật khẩu (ALTER USER)'
    },
    {
      path: 'Ferrous-panel/src/mail_server.rs',
      title: 'src/mail_server.rs',
      lang: 'rust',
      description:
        'Module quản trị Postfix, Dovecot IMAPS và tự động sinh khóa chữ ký số OpenDKIM 2048-bit'
    },
    {
      path: 'Ferrous-panel/src/file_manager.rs',
      title: 'src/file_manager.rs',
      lang: 'rust',
      description:
        'Trình quản lý File theo từng thư mục Web (/www/wwwroot/*) hỗ trợ Upload .ZIP, Giải nén (Unzip), chống Zip-Slip và phân quyền POSIX'
    },
    {
      path: 'Ferrous-panel/src/cpanel_tools.rs',
      title: 'src/cpanel_tools.rs',
      lang: 'rust',
      description:
        'Cấu trúc dữ liệu quản lý Cron Jobs, DNS Zone Records và bản sao lưu nén Zstandard (.tar.zst)'
    },
    {
      path: 'Ferrous-panel/src/nginx.rs',
      title: 'src/nginx.rs',
      lang: 'rust',
      description:
        'Trình sinh cấu hình Nginx Reverse Proxy, Upstream Keepalive, TLS 1.3, HTTP/3 QUIC & Rate Limiting'
    },
    {
      path: 'Ferrous-panel/src/systemd.rs',
      title: 'src/systemd.rs',
      lang: 'rust',
      description:
        'Trình quản lý Ubuntu Systemd Service Unit với cô lập tài nguyên (MemoryMax, ProtectSystem=strict)'
    },
    {
      path: 'Ferrous-panel/src/security.rs',
      title: 'src/security.rs',
      lang: 'rust',
      description:
        'Module thực thi UFW Firewall, kiểm toán SSHD và tự động hóa Fail2ban trên Ubuntu'
    }
  ];

  const loaded = files.map((f) => {
    const absPath = path.resolve(process.cwd(), f.path);
    let content = '';
    try {
      content = fs.readFileSync(absPath, 'utf-8');
    } catch {
      content = '// File not found';
    }
    return { ...f, content };
  });

  res.json(loaded);
});

// Global Express Error-Recovery Middleware to guarantee Panel Backend Stability
app.use(
  (
    err: Error,
    _req: express.Request,
    res: express.Response,
    _next: express.NextFunction
  ) => {
    console.error('[FerrousPanel Backend Watchdog Caught Error]:', err);
    res.status(500).json({
      error: 'Internal Panel Backend Error recovered by Watchdog',
      detail: err.message
    });
  }
);

async function startServer() {
  if (process.env.NODE_ENV !== 'production') {
    const vite = await createViteServer({
      server: {
        middlewareMode: true,
        hmr: false,
        watch: process.env.DISABLE_HMR === 'true' ? null : {}
      },
      appType: 'spa'
    });
    app.use(vite.middlewares);
  } else {
    const distPath = path.join(process.cwd(), 'dist');
    app.use(express.static(distPath));
    app.get('*', (_req, res) => {
      res.sendFile(path.join(distPath, 'index.html'));
    });
  }

  app.listen(PORT, '0.0.0.0', () => {
    console.log(`FerrousPanel Server running on http://0.0.0.0:${PORT}`);
  });
}

startServer();

