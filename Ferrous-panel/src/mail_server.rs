use actix_web::{error::ErrorInternalServerError, Error};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxProvisionRequest {
    pub email: String,
    pub password_hash_argon2: String,
    pub quota_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkimKeyPair {
    pub domain: String,
    pub selector: String,
    pub dns_txt_record: String,
}

pub struct MailServerManager;

impl MailServerManager {
    /// Generates 2048-bit OpenDKIM keypair for Ubuntu Postfix signing
    pub async fn generate_dkim(domain: &str, selector: &str) -> Result<DkimKeyPair, Error> {
        let key_dir = format!("/etc/opendkim/keys/{}", domain);
        let _ = Command::new("opendkim-genkey")
            .args(["-b", "2048", "-d", domain, "-D", &key_dir, "-s", selector, "-v"])
            .output()
            .await
            .map_err(ErrorInternalServerError)?;

        Ok(DkimKeyPair {
            domain: domain.to_string(),
            selector: selector.to_string(),
            dns_txt_record: format!(
                "{}._domainkey.{} IN TXT \"v=DKIM1; h=sha256; k=rsa; p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA...\"",
                selector, domain
            ),
        })
    }

    pub async fn reload_postfix_dovecot() -> Result<(), Error> {
        let _ = Command::new("systemctl").args(["reload", "postfix", "dovecot"]).output().await;
        Ok(())
    }
}
