use rusqlite::Connection;
use serde::Serialize;

/// Per-user settings for the external SQL folder (connected account + system).
/// The password is stored encrypted and never serialized to clients.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExternalFolderSettings {
    pub enabled: bool,
    pub account_id: Option<i64>,
    pub account_email: Option<String>,
    pub system: Option<i64>,
    pub base_url: Option<String>,
    pub connected_at: Option<String>,
}

fn row_to_settings(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExternalFolderSettings> {
    let enabled: i32 = row.get(0)?;
    Ok(ExternalFolderSettings {
        enabled: enabled != 0,
        account_id: row.get(1)?,
        account_email: row.get(2)?,
        base_url: row.get(3)?,
        system: row.get(4)?,
        connected_at: row.get(5)?,
    })
}

pub fn get_settings(conn: &Connection) -> Result<ExternalFolderSettings, String> {
    let result = conn.query_row(
        "SELECT enabled, account_id, account_email, base_url, system, connected_at
         FROM external_folder_settings WHERE id = 1",
        [],
        row_to_settings,
    );

    match result {
        Ok(settings) => Ok(settings),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(ExternalFolderSettings {
            enabled: false,
            account_id: None,
            account_email: None,
            system: None,
            base_url: None,
            connected_at: None,
        }),
        Err(e) => Err(format!("Failed to get external folder settings: {e}")),
    }
}

/// Persist a verified connection and enable the folder.
pub fn save_connection(
    conn: &Connection,
    account_id: i64,
    account_email: &str,
    system: i64,
    base_url: Option<&str>,
    enc_password: &[u8],
    enc_nonce: &[u8],
) -> Result<ExternalFolderSettings, String> {
    conn.execute(
        "INSERT INTO external_folder_settings
            (id, enabled, account_id, account_email, system, base_url,
             enc_password, enc_nonce, connected_at, updated_at)
         VALUES
            (1, 1, ?1, ?2, ?3, ?4, ?5, ?6,
             strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
             strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
         ON CONFLICT(id) DO UPDATE SET
            enabled = 1,
            account_id = excluded.account_id,
            account_email = excluded.account_email,
            system = excluded.system,
            base_url = excluded.base_url,
            enc_password = excluded.enc_password,
            enc_nonce = excluded.enc_nonce,
            connected_at = excluded.connected_at,
            updated_at = excluded.updated_at",
        rusqlite::params![account_id, account_email, system, base_url, enc_password, enc_nonce],
    )
    .map_err(|e| format!("Failed to save external folder settings: {e}"))?;

    get_settings(conn)
}

/// Enable/disable the folder without changing the stored connection.
pub fn set_enabled(conn: &Connection, enabled: bool) -> Result<ExternalFolderSettings, String> {
    conn.execute(
        "INSERT OR IGNORE INTO external_folder_settings (id) VALUES (1)",
        [],
    )
    .map_err(|e| format!("Failed to ensure external folder settings row: {e}"))?;
    conn.execute(
        "UPDATE external_folder_settings
         SET enabled = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE id = 1",
        rusqlite::params![enabled as i32],
    )
    .map_err(|e| format!("Failed to update external folder settings: {e}"))?;

    get_settings(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[test]
    fn defaults_to_disabled() {
        let conn = open_test_db();
        let settings = get_settings(&conn).unwrap();
        assert!(!settings.enabled);
        assert_eq!(settings.system, None);
    }

    #[test]
    fn save_connection_round_trip() {
        let conn = open_test_db();
        let settings =
            save_connection(&conn, 2300, "contato@example.com", 1, Some("https://img/"), b"cipher", b"nonce").unwrap();
        assert!(settings.enabled);
        assert_eq!(settings.account_id, Some(2300));
        assert_eq!(settings.system, Some(1));
        assert_eq!(settings.base_url.as_deref(), Some("https://img/"));
        assert!(settings.connected_at.is_some());
    }

    #[test]
    fn set_enabled_toggles() {
        let conn = open_test_db();
        save_connection(&conn, 1, "a@b.com", 1, None, b"c", b"n").unwrap();
        let off = set_enabled(&conn, false).unwrap();
        assert!(!off.enabled);
        let on = set_enabled(&conn, true).unwrap();
        assert!(on.enabled);
    }
}
