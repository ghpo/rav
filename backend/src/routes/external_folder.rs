use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use super::messages::types::{
    AddressEntry, ListMessagesResponse, MessageDetailResponse, MessageSummary, ThreadMessage,
};
use crate::auth::session::SessionState;
use crate::db;
use crate::error::AppError;
use crate::external_sql::{ExternalError, ExternalItem, SharedExternalClient};
use crate::folder_cipher::FolderCipher;

/// Attribute marking a virtual folder backed by the external SQL config.
pub(crate) const EXTERNAL_ATTR: &str = "\\External";

// ---------------------------------------------------------------------------
// Settings endpoints
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ExternalFolderStatus {
    /// The server has a valid private config.
    pub configured: bool,
    /// The feature can be used for this account (configured + mapped domain).
    pub available: bool,
    pub folder_name: Option<String>,
    pub enabled: bool,
    pub connected: bool,
    pub system: Option<i64>,
    pub account_email: Option<String>,
    pub connected_at: Option<String>,
    pub count: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub usuario: String,
    pub senha: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExternalFolderRequest {
    pub enabled: bool,
}

fn external_error_to_app(err: ExternalError) -> AppError {
    match &err {
        ExternalError::NotConfigured => AppError::ServiceUnavailable(err.user_message()),
        ExternalError::Unreachable(detail) => {
            tracing::warn!(error = %detail, "external folder: data source unreachable");
            AppError::ServiceUnavailable(err.user_message())
        }
        ExternalError::Query(detail) => {
            tracing::warn!(error = %detail, "external folder: query failed");
            AppError::ServiceUnavailable(err.user_message())
        }
        ExternalError::InvalidCredentials | ExternalError::SystemMismatch => {
            AppError::BadRequest(err.user_message())
        }
    }
}

async fn read_settings(
    db_pool_manager: &Arc<db::pool::DbPoolManager>,
    user_hash: &str,
) -> Result<db::external_folder::ExternalFolderSettings, AppError> {
    db::pool::with_user_db(db_pool_manager, user_hash, |conn| {
        db::external_folder::get_settings(conn)
    })
    .await
    .map_err(AppError::InternalError)
}

async fn build_status(
    client: &SharedExternalClient,
    db_pool_manager: &Arc<db::pool::DbPoolManager>,
    user_hash: &str,
    email: &str,
) -> Result<ExternalFolderStatus, AppError> {
    let settings = read_settings(db_pool_manager, user_hash).await?;
    let configured = client.is_configured();
    let domain_system = client.system_for_email(email);
    let available = configured && domain_system.is_some();

    let count = match (settings.enabled, settings.system, settings.account_id, available) {
        (true, Some(system), Some(_), true) => client.count(system).await.unwrap_or(0),
        _ => 0,
    };

    Ok(ExternalFolderStatus {
        configured,
        available,
        folder_name: if configured {
            Some(client.folder_name().to_string())
        } else {
            None
        },
        enabled: settings.enabled,
        connected: settings.account_id.is_some(),
        system: settings.system.or(domain_system),
        account_email: settings.account_email,
        connected_at: settings.connected_at,
        count,
    })
}

/// `GET /api/settings/external-folder`
pub async fn get_external_folder_settings(
    Extension(session): Extension<SessionState>,
    services: super::AppServices,
) -> Result<Response, AppError> {
    let super::AppServices {
        external_folder,
        db_pool_manager,
        ..
    } = services;
    let status =
        build_status(&external_folder, &db_pool_manager, &session.user_hash, &session.email)
            .await?;
    Ok(Json(status).into_response())
}

/// `POST /api/settings/external-folder/connect`
pub async fn connect_external_folder(
    Extension(session): Extension<SessionState>,
    services: super::AppServices,
    Json(body): Json<ConnectRequest>,
) -> Result<Response, AppError> {
    let super::AppServices {
        external_folder,
        db_pool_manager,
        mfa_crypto,
        ..
    } = services;

    if !external_folder.is_configured() {
        return Err(AppError::ServiceUnavailable(
            "Este recurso não está configurado no servidor.".to_string(),
        ));
    }

    let system = external_folder.system_for_email(&session.email).ok_or_else(|| {
        AppError::BadRequest(
            "O domínio do seu e-mail não está habilitado para este recurso.".to_string(),
        )
    })?;

    let usuario = body.usuario.trim().to_string();
    if usuario.is_empty() || body.senha.is_empty() {
        return Err(AppError::BadRequest("Informe usuário e senha.".to_string()));
    }

    let verified = external_folder
        .verify_account(&usuario, &body.senha)
        .await
        .map_err(external_error_to_app)?;

    if verified.system != system {
        return Err(external_error_to_app(ExternalError::SystemMismatch));
    }

    let (enc_password, enc_nonce) = mfa_crypto
        .encrypt(body.senha.as_bytes())
        .map_err(AppError::InternalError)?;

    let user_hash = session.user_hash.clone();
    let email = verified.email.clone();
    let verified_system = verified.system;
    let account_id = verified.id as i64;
    db::pool::with_user_db(&db_pool_manager, &user_hash, move |conn| {
        db::external_folder::save_connection(
            conn,
            account_id,
            &email,
            verified_system,
            &enc_password,
            &enc_nonce,
        )
    })
    .await
    .map_err(AppError::InternalError)?;

    let status =
        build_status(&external_folder, &db_pool_manager, &user_hash, &session.email).await?;
    Ok(Json(status).into_response())
}

/// `PUT /api/settings/external-folder`
pub async fn update_external_folder_settings(
    Extension(session): Extension<SessionState>,
    services: super::AppServices,
    Json(body): Json<UpdateExternalFolderRequest>,
) -> Result<Response, AppError> {
    let super::AppServices {
        external_folder,
        db_pool_manager,
        ..
    } = services;

    let current = read_settings(&db_pool_manager, &session.user_hash).await?;
    if body.enabled && current.account_id.is_none() {
        return Err(AppError::BadRequest(
            "Conecte uma conta antes de ativar.".to_string(),
        ));
    }

    let user_hash = session.user_hash.clone();
    db::pool::with_user_db(&db_pool_manager, &user_hash, move |conn| {
        db::external_folder::set_enabled(conn, body.enabled)
    })
    .await
    .map_err(AppError::InternalError)?;

    let status =
        build_status(&external_folder, &db_pool_manager, &session.user_hash, &session.email)
            .await?;
    Ok(Json(status).into_response())
}

// ---------------------------------------------------------------------------
// Virtual-folder synthesis (used by messages handlers)
// ---------------------------------------------------------------------------

fn item_to_summary(item: &ExternalItem, folder_name: &str, cipher: &FolderCipher) -> MessageSummary {
    MessageSummary {
        uid: item.id,
        folder_id: cipher.encrypt(folder_name),
        folder_name: folder_name.to_string(),
        subject: item.subject.clone(),
        from_address: String::new(),
        from_name: item.sender.clone(),
        to_addresses: "[]".to_string(),
        date: item.date.clone(),
        flags: String::new(),
        size: 0,
        has_attachments: false,
        snippet: item.snippet.clone(),
        reaction: None,
        tags: vec![],
        thread_count: 1,
        unread_count: 1,
    }
}

fn item_to_detail(
    item: &ExternalItem,
    folder_name: &str,
    cipher: &FolderCipher,
) -> MessageDetailResponse {
    let html = item.body_html.clone().or_else(|| {
        item.body_text.as_ref().map(|text| {
            format!(
                "<pre style=\"white-space: pre-wrap; word-break: break-word; font-family: inherit;\">{}</pre>",
                escape_html(text)
            )
        })
    });

    MessageDetailResponse {
        uid: item.id,
        folder_id: cipher.encrypt(folder_name),
        folder_name: folder_name.to_string(),
        subject: item.subject.clone(),
        from_address: String::new(),
        from_name: item.sender.clone(),
        to_addresses: Vec::<AddressEntry>::new(),
        cc_addresses: Vec::<AddressEntry>::new(),
        date: item.date.clone(),
        flags: vec![],
        has_attachments: false,
        html,
        text: item.body_text.clone().or_else(|| Some(item.snippet.clone())),
        raw_headers: String::new(),
        attachments: vec![],
        thread: Vec::<ThreadMessage>::new(),
        email_theme: None,
        pgp_status: None,
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Build a `ListMessagesResponse` for the external virtual folder.
pub(crate) async fn external_list_response(
    client: &SharedExternalClient,
    db_pool_manager: &Arc<db::pool::DbPoolManager>,
    user_hash: &str,
    page: u32,
    per_page: u32,
    cipher: &FolderCipher,
) -> Result<ListMessagesResponse, AppError> {
    let settings = read_settings(db_pool_manager, user_hash).await?;
    let system = settings
        .system
        .filter(|_| settings.enabled && settings.account_id.is_some())
        .ok_or_else(|| AppError::NotFound("A pasta não está ativa.".to_string()))?;

    let items = client
        .list(system, page, per_page)
        .await
        .map_err(external_error_to_app)?;
    let total = client.count(system).await.unwrap_or(items.len() as u64);
    let folder_name = client.folder_name();

    Ok(ListMessagesResponse {
        messages: items
            .iter()
            .map(|i| item_to_summary(i, folder_name, cipher))
            .collect(),
        total_count: total as u32,
        page,
        per_page,
        syncing: false,
    })
}

/// Build a `MessageDetailResponse` for a single external item.
pub(crate) async fn external_detail_response(
    client: &SharedExternalClient,
    db_pool_manager: &Arc<db::pool::DbPoolManager>,
    user_hash: &str,
    uid: u32,
    cipher: &FolderCipher,
) -> Result<MessageDetailResponse, AppError> {
    let settings = read_settings(db_pool_manager, user_hash).await?;
    let system = settings
        .system
        .filter(|_| settings.enabled && settings.account_id.is_some())
        .ok_or_else(|| AppError::NotFound("A pasta não está ativa.".to_string()))?;

    let item = client
        .detail(system, uid)
        .await
        .map_err(external_error_to_app)?
        .ok_or_else(|| AppError::NotFound(format!("Item {uid} não encontrado")))?;

    Ok(item_to_detail(&item, client.folder_name(), cipher))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_sql::{ExternalError, ExternalItem, ExternalSqlClient, VerifiedAccount};
    use async_trait::async_trait;

    #[derive(Default)]
    pub struct MockExternal {
        pub accounts: Vec<(String, String, i64)>,
        pub items: Vec<ExternalItem>,
    }

    #[async_trait]
    impl ExternalSqlClient for MockExternal {
        fn is_configured(&self) -> bool {
            true
        }
        fn folder_name(&self) -> &str {
            "Custom"
        }
        fn icon(&self) -> Option<&str> {
            None
        }
        fn system_for_email(&self, _email: &str) -> Option<i64> {
            Some(1)
        }
        async fn verify_account(
            &self,
            email: &str,
            password: &str,
        ) -> Result<VerifiedAccount, ExternalError> {
            for (e, p, system) in &self.accounts {
                if e == email && p == password {
                    return Ok(VerifiedAccount {
                        id: 1,
                        system: *system,
                        email: email.to_string(),
                    });
                }
            }
            Err(ExternalError::InvalidCredentials)
        }
        async fn count(&self, _system: i64) -> Result<u64, ExternalError> {
            Ok(self.items.len() as u64)
        }
        async fn list(
            &self,
            _system: i64,
            _page: u32,
            _per_page: u32,
        ) -> Result<Vec<ExternalItem>, ExternalError> {
            Ok(self.items.clone())
        }
        async fn detail(
            &self,
            _system: i64,
            id: u32,
        ) -> Result<Option<ExternalItem>, ExternalError> {
            Ok(self.items.iter().find(|i| i.id == id).cloned())
        }
    }

    fn sample_item() -> ExternalItem {
        ExternalItem {
            id: 10,
            subject: "Item 10".to_string(),
            sender: "Someone".to_string(),
            date: "2026-01-01T00:00:00".to_string(),
            snippet: "summary".to_string(),
            body_html: None,
            body_text: Some("body".to_string()),
        }
    }

    #[test]
    fn summary_maps_fields() {
        let cipher = FolderCipher::new(&[7u8; 32]);
        let summary = item_to_summary(&sample_item(), "Custom", &cipher);
        assert_eq!(summary.uid, 10);
        assert_eq!(summary.subject, "Item 10");
        assert_eq!(summary.from_name, "Someone");
        assert_eq!(summary.folder_name, "Custom");
    }

    #[test]
    fn detail_escapes_text_fallback() {
        let cipher = FolderCipher::new(&[7u8; 32]);
        let mut item = sample_item();
        item.body_text = Some("<script>x</script>".to_string());
        let detail = item_to_detail(&item, "Custom", &cipher);
        let html = detail.html.unwrap();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
