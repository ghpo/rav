use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use mysql::prelude::Queryable;
use serde::Deserialize;

use crate::config::AppConfig;

/// Private, per-instance configuration for the optional external SQL folder.
///
/// This is loaded at runtime from a JSON file **outside the repository**
/// (by default `{data_dir}/external-folder.json`, overridable with the
/// `EXTERNAL_FOLDER_CONFIG` env var). It holds the database connection, the
/// SQL queries, the domain-to-system mapping and the folder display name, so
/// none of those details live in the source tree.
#[derive(Debug, Clone, Deserialize)]
pub struct ExternalFolderConfig {
    /// Display name of the virtual folder.
    #[serde(default = "default_folder_name")]
    pub folder_name: String,
    /// Optional icon key hint for the client.
    #[serde(default)]
    pub icon: Option<String>,
    pub connection: ExternalConnection,
    /// Map of e-mail domain (lowercase) to an opaque system id.
    #[serde(default)]
    pub domains: HashMap<String, i64>,
    pub queries: ExternalQueries,
}

fn default_folder_name() -> String {
    "External".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExternalConnection {
    pub host: String,
    #[serde(default = "default_mysql_port")]
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub password: String,
    pub database: String,
    /// Use TLS for the connection (default true). Certificate verification is
    /// relaxed because these are operator-supplied servers.
    #[serde(default = "default_true")]
    pub tls: bool,
}

fn default_mysql_port() -> u16 {
    3306
}

fn default_true() -> bool {
    true
}

/// SQL templates with a fixed positional parameter/column contract:
///
/// - `verify_account`: param `(email)` → row `(account_id, password_hash, system)`
/// - `count`:          param `(system)` → row `(count)`
/// - `list`:           params `(system, limit, offset)` → rows
///                     `(id, subject, sender, date, snippet)`
/// - `detail`:         params `(id, system)` → row
///                     `(id, subject, sender, date, body_html, body_text)`
#[derive(Debug, Clone, Deserialize)]
pub struct ExternalQueries {
    pub verify_account: String,
    pub count: String,
    pub list: String,
    pub detail: String,
}

/// A single row rendered as a message-like item.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalItem {
    pub id: u32,
    pub subject: String,
    pub sender: String,
    pub date: String,
    pub snippet: String,
    pub body_html: Option<String>,
    pub body_text: Option<String>,
}

/// A verified external account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAccount {
    pub id: u64,
    pub system: i64,
    pub email: String,
}

#[derive(Debug, Clone)]
pub enum ExternalError {
    NotConfigured,
    Unreachable(String),
    Query(String),
    InvalidCredentials,
    SystemMismatch,
}

impl ExternalError {
    pub fn user_message(&self) -> String {
        match self {
            ExternalError::NotConfigured => {
                "Este recurso não está configurado no servidor.".to_string()
            }
            ExternalError::Unreachable(_) => {
                "Não foi possível conectar ao serviço externo. Tente novamente.".to_string()
            }
            ExternalError::Query(_) => {
                "Erro ao consultar os dados. Tente novamente.".to_string()
            }
            ExternalError::InvalidCredentials => "Usuário ou senha inválidos.".to_string(),
            ExternalError::SystemMismatch => {
                "Esta conta pertence a outro sistema, diferente do sistema do seu e-mail."
                    .to_string()
            }
        }
    }
}

/// Read-only access to the external SQL data source.
#[async_trait]
pub trait ExternalSqlClient: Send + Sync {
    fn is_configured(&self) -> bool;
    /// Configured folder display name (empty when not configured).
    fn folder_name(&self) -> &str;
    fn icon(&self) -> Option<&str>;
    /// Map a Rav account e-mail to the configured system id.
    fn system_for_email(&self, email: &str) -> Option<i64>;
    async fn verify_account(
        &self,
        email: &str,
        password: &str,
    ) -> Result<VerifiedAccount, ExternalError>;
    async fn count(&self, system: i64) -> Result<u64, ExternalError>;
    async fn list(
        &self,
        system: i64,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<ExternalItem>, ExternalError>;
    async fn detail(&self, system: i64, id: u32) -> Result<Option<ExternalItem>, ExternalError>;
}

/// Resolve the config file path from `AppConfig`, defaulting to the data dir.
fn config_path(config: &AppConfig) -> String {
    config
        .external_folder_config
        .clone()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "{}/external-folder.json",
                config.data_dir.trim_end_matches('/')
            )
        })
}

/// Load and parse the private config. Returns `None` (feature disabled) when
/// the file is absent or invalid.
pub fn load_config(config: &AppConfig) -> Option<ExternalFolderConfig> {
    let path = config_path(config);
    let path = Path::new(&path);
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<ExternalFolderConfig>(&contents) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                tracing::error!(error = %e, "external folder config is invalid JSON");
                None
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "failed to read external folder config");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Real client
// ---------------------------------------------------------------------------

pub struct RealExternalSqlClient {
    config: ExternalFolderConfig,
    pool: mysql::Pool,
}

impl RealExternalSqlClient {
    fn new(config: ExternalFolderConfig) -> Result<Self, String> {
        let mut builder = mysql::OptsBuilder::new()
            .ip_or_hostname(Some(config.connection.host.clone()))
            .tcp_port(config.connection.port)
            .user(Some(config.connection.user.clone()))
            .pass(Some(config.connection.password.clone()))
            .db_name(Some(config.connection.database.clone()))
            // Force a charset that matches the configured data, so the server
            // does not transcode rows into invalid byte sequences.
            .init(vec!["SET NAMES utf8mb4".to_string()]);

        if config.connection.tls {
            let ssl = mysql::SslOpts::default()
                .with_danger_accept_invalid_certs(true)
                .with_danger_skip_domain_validation(true);
            builder = builder.ssl_opts(ssl);
        }

        let pool = mysql::Pool::new(builder).map_err(|e| e.to_string())?;
        Ok(Self { config, pool })
    }
}

/// Build a lossy UTF-8 string from raw bytes.
///
/// The external database may use a charset that does not round-trip cleanly
/// (e.g. latin1 columns with accented text). Reading as bytes and decoding
/// lossily avoids a hard failure on a single undecodable row.
fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Borrow a pooled connection pinned to READ ONLY for this session.
fn read_only_conn(pool: &mysql::Pool) -> Result<mysql::PooledConn, ExternalError> {
    let mut conn = pool
        .get_conn()
        .map_err(|e| ExternalError::Unreachable(e.to_string()))?;
    conn.query_drop("SET SESSION TRANSACTION READ ONLY")
        .map_err(|e| ExternalError::Query(e.to_string()))?;
    Ok(conn)
}

#[async_trait]
impl ExternalSqlClient for RealExternalSqlClient {
    fn is_configured(&self) -> bool {
        true
    }

    fn folder_name(&self) -> &str {
        &self.config.folder_name
    }

    fn icon(&self) -> Option<&str> {
        self.config.icon.as_deref()
    }

    fn system_for_email(&self, email: &str) -> Option<i64> {
        let domain = email.rsplit('@').next()?.trim().to_ascii_lowercase();
        self.config.domains.get(&domain).copied()
    }

    async fn verify_account(
        &self,
        email: &str,
        password: &str,
    ) -> Result<VerifiedAccount, ExternalError> {
        let pool = self.pool.clone();
        let query = self.config.queries.verify_account.clone();
        let email = email.to_string();
        let password = password.to_string();

        tokio::task::spawn_blocking(move || {
            let mut conn = read_only_conn(&pool)?;
            let row: Option<(u64, Option<Vec<u8>>, Option<i64>)> = conn
                .exec_first(&query, (&email,))
                .map_err(|e| ExternalError::Query(e.to_string()))?;

            let Some((id, hash, system)) = row else {
                return Err(ExternalError::InvalidCredentials);
            };
            let hash = hash.map(text).ok_or(ExternalError::InvalidCredentials)?;
            let ok = bcrypt::verify(&password, &hash).unwrap_or(false);
            if !ok {
                return Err(ExternalError::InvalidCredentials);
            }
            let system = system.ok_or(ExternalError::InvalidCredentials)?;
            Ok(VerifiedAccount {
                id,
                system,
                email,
            })
        })
        .await
        .map_err(|e| ExternalError::Query(e.to_string()))?
    }

    async fn count(&self, system: i64) -> Result<u64, ExternalError> {
        let pool = self.pool.clone();
        let query = self.config.queries.count.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = read_only_conn(&pool)?;
            let count: Option<u64> = conn
                .exec_first(&query, (system,))
                .map_err(|e| ExternalError::Query(e.to_string()))?;
            Ok(count.unwrap_or(0))
        })
        .await
        .map_err(|e| ExternalError::Query(e.to_string()))?
    }

    async fn list(
        &self,
        system: i64,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<ExternalItem>, ExternalError> {
        let pool = self.pool.clone();
        let query = self.config.queries.list.clone();
        let offset = page.saturating_mul(per_page);
        tokio::task::spawn_blocking(move || {
            let mut conn = read_only_conn(&pool)?;
            let rows: Vec<(u32, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> = conn
                .exec(&query, (system, per_page, offset))
                .map_err(|e| ExternalError::Query(e.to_string()))?;
            Ok(rows
                .into_iter()
                .map(|(id, subject, sender, date, snippet)| ExternalItem {
                    id,
                    subject: text(subject),
                    sender: text(sender),
                    date: text(date),
                    snippet: text(snippet),
                    body_html: None,
                    body_text: None,
                })
                .collect())
        })
        .await
        .map_err(|e| ExternalError::Query(e.to_string()))?
    }

    async fn detail(
        &self,
        system: i64,
        id: u32,
    ) -> Result<Option<ExternalItem>, ExternalError> {
        let pool = self.pool.clone();
        let query = self.config.queries.detail.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = read_only_conn(&pool)?;
            let row: Option<(u32, Vec<u8>, Vec<u8>, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>)> =
                conn.exec_first(&query, (id, system))
                    .map_err(|e| ExternalError::Query(e.to_string()))?;
            Ok(
                row.map(|(id, subject, sender, date, body_html, body_text)| ExternalItem {
                    id,
                    subject: text(subject),
                    sender: text(sender),
                    date: text(date),
                    snippet: String::new(),
                    body_html: body_html.map(text),
                    body_text: body_text.map(text),
                }),
            )
        })
        .await
        .map_err(|e| ExternalError::Query(e.to_string()))?
    }
}

/// Client used when no valid private config is present.
pub struct UnconfiguredExternalClient;

#[async_trait]
impl ExternalSqlClient for UnconfiguredExternalClient {
    fn is_configured(&self) -> bool {
        false
    }
    fn folder_name(&self) -> &str {
        ""
    }
    fn icon(&self) -> Option<&str> {
        None
    }
    fn system_for_email(&self, _email: &str) -> Option<i64> {
        None
    }
    async fn verify_account(
        &self,
        _email: &str,
        _password: &str,
    ) -> Result<VerifiedAccount, ExternalError> {
        Err(ExternalError::NotConfigured)
    }
    async fn count(&self, _system: i64) -> Result<u64, ExternalError> {
        Err(ExternalError::NotConfigured)
    }
    async fn list(
        &self,
        _system: i64,
        _page: u32,
        _per_page: u32,
    ) -> Result<Vec<ExternalItem>, ExternalError> {
        Err(ExternalError::NotConfigured)
    }
    async fn detail(
        &self,
        _system: i64,
        _id: u32,
    ) -> Result<Option<ExternalItem>, ExternalError> {
        Err(ExternalError::NotConfigured)
    }
}

pub type SharedExternalClient = Arc<dyn ExternalSqlClient>;

/// Build the shared client: real when the private config is present, else disabled.
pub fn build_external_client(config: &AppConfig) -> SharedExternalClient {
    let Some(cfg) = load_config(config) else {
        tracing::info!("external SQL folder is disabled (no private config)");
        return Arc::new(UnconfiguredExternalClient);
    };
    match RealExternalSqlClient::new(cfg) {
        Ok(client) => Arc::new(client),
        Err(e) => {
            tracing::error!(error = %e, "failed to create external SQL pool");
            Arc::new(UnconfiguredExternalClient)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_deserializes() {
        let json = r#"{
            "folder_name": "Minha Pasta",
            "icon": "tag",
            "connection": { "host": "h", "user": "u", "password": "p", "database": "d" },
            "domains": { "example.com": 7 },
            "queries": { "verify_account": "a", "count": "b", "list": "c", "detail": "d" }
        }"#;
        let cfg: ExternalFolderConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.folder_name, "Minha Pasta");
        assert_eq!(cfg.connection.port, 3306);
        assert_eq!(cfg.domains.get("example.com"), Some(&7));
    }
}
