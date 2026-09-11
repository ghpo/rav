use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use crate::auth::session::SessionState;
use crate::db;
use crate::db::custom_nav_link::UpdateCustomNavLink;
use crate::error::AppError;

fn map_update_error(message: String) -> AppError {
    if message.starts_with("Invalid custom link") {
        AppError::BadRequest(message)
    } else {
        AppError::InternalError(message)
    }
}

/// `GET /api/settings/custom-link`
pub async fn get_custom_nav_link(
    Extension(session): Extension<SessionState>,
    Extension(db_pool_manager): Extension<Arc<db::pool::DbPoolManager>>,
) -> Result<Response, AppError> {
    let link = db::pool::with_user_db(&db_pool_manager, &session.user_hash, |conn| {
        db::custom_nav_link::get_custom_nav_link(conn)
    })
    .await
    .map_err(AppError::InternalError)?;

    Ok(Json(link).into_response())
}

/// `PUT /api/settings/custom-link`
pub async fn update_custom_nav_link(
    Extension(session): Extension<SessionState>,
    Extension(db_pool_manager): Extension<Arc<db::pool::DbPoolManager>>,
    Json(data): Json<UpdateCustomNavLink>,
) -> Result<Response, AppError> {
    let link = db::pool::with_user_db(&db_pool_manager, &session.user_hash, move |conn| {
        db::custom_nav_link::update_custom_nav_link(conn, &data)
    })
    .await
    .map_err(map_update_error)?;

    Ok(Json(link).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_link_maps_to_bad_request() {
        match map_update_error("Invalid custom link url: must start with http:// or https://".into())
        {
            AppError::BadRequest(msg) => assert!(msg.contains("Invalid custom link")),
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn other_errors_map_to_internal() {
        assert!(matches!(
            map_update_error("Failed to update custom nav link: boom".into()),
            AppError::InternalError(_)
        ));
    }
}
