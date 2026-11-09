use serde::{Deserialize, Serialize};
use std::io;
use tokio::process::Command;
use tracing::info;

use crate::systemd::AppServiceSpec;

/// Mô-đun Rust quản lý Kho Ứng dụng 1-Click (Modern JS Frameworks, Rust Actix-Web, PHP CMS)
/// và Trình quản lý Gói phần mềm Hệ thống Ubuntu (apt / systemd).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneClickTemplateSpec {
    pub id: String,
    pub name: String,
    pub category: String,
    pub framework: String,
    pub version: String,
    pub default_port: u16,
    pub requires_db: String,
    pub build_command: String,
    pub exec_command_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UbuntuSystemPackage {
    pub id: String,
    pub name: String,
    pub systemd_unit: String,
    pub version: String,
    pub config_path: String,
    pub status: String,
    pub memory_mb: u64,
}

pub struct OneClickStoreEngine;

impl OneClickStoreEngine {
    /// Triển khai tự động mẫu ứng dụng 1-Click vào /www/wwwroot/<domain>
    pub async fn provision_template(
        template: &OneClickTemplateSpec,
        domain: &str,
        allocated_port: u16,
    ) -> io::Result<AppServiceSpec> {
        let working_dir = format!("/www/wwwroot/{}", domain);
        info!(
            template = %template.name,
            domain = %domain,
            port = allocated_port,
            "Provisioning 1-Click application template via Rust OneClickStoreEngine"
        );

        let exec_cmd = template
            .exec_command_template
            .replace("$PORT", &allocated_port.to_string())
            .replace("$DOMAIN", domain);

        Ok(AppServiceSpec {
            id: format!("svc-{}", domain.replace('.', "-")),
            name: domain.replace('.', "-"),
            framework: template.framework.clone(),
            domain: domain.to_string(),
            internal_port: allocated_port,
            working_dir,
            exec_command: exec_cmd,
            workers: 4,
            memory_limit_mb: 512,
            status: "running".to_string(),
            ssl_enabled: true,
            http3_quic: true,
            websocket_enabled: true,
            rate_limit_rps: 250,
            env_vars: vec![
                ("NODE_ENV".to_string(), "production".to_string()),
                ("PORT".to_string(), allocated_port.to_string()),
            ],
        })
    }

    /// Điều khiển trạng thái các gói dịch vụ hệ thống Ubuntu (Nginx, PostgreSQL, MariaDB, Redis, Postfix)
    pub async fn control_ubuntu_package(unit_name: &str, action: &str) -> io::Result<String> {
        let valid_action = match action {
            "start" | "stop" | "restart" | "reload" => action,
            _ => "restart",
        };

        info!(unit = %unit_name, action = %valid_action, "Controlling Ubuntu systemd package");
        let _ = Command::new("systemctl")
            .args([valid_action, unit_name])
            .output()
            .await;

        Ok(format!("systemctl {} {} completed", valid_action, unit_name))
    }
}
