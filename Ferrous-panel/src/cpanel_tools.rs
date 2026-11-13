use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobSpec {
    pub id: String,
    pub title: String,
    pub schedule: String,
    pub command: String,
    pub run_as_user: String,
    pub enabled: bool,
    pub last_run_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecordSpec {
    pub id: String,
    pub domain: String,
    pub record_type: String, // A, AAAA, CNAME, MX, TXT, CAA
    pub host: String,
    pub value: String,
    pub ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupArchiveSpec {
    pub id: String,
    pub target_app: String,
    pub includes_db: String,
    pub archive_path: String,
    pub size_mb: f64,
    pub sha256_prefix: String,
    pub created_at: String,
}
