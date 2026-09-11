use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Shortcut shown below Settings in the navigation rail until the user sets one.
const DEFAULT_URL: &str = "https://producao.ghpo.com.br";
const DEFAULT_ICON: &str = "printer";

/// The user's configurable navigation-rail shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CustomNavLink {
    /// Target URL. An empty string means no shortcut button is shown.
    pub url: String,
    /// Icon key; resolved to a component on the client.
    pub icon: String,
}

/// Partial update for [`CustomNavLink`]. Omitted fields keep their current value.
#[derive(Debug, Deserialize)]
pub struct UpdateCustomNavLink {
    pub url: Option<String>,
    pub icon: Option<String>,
}

/// Retrieve the singleton custom link, falling back to the built-in default.
pub fn get_custom_nav_link(conn: &Connection) -> Result<CustomNavLink, String> {
    let result = conn.query_row(
        "SELECT url, icon FROM custom_nav_link WHERE id = 1",
        [],
        |row| {
            Ok(CustomNavLink {
                url: row
                    .get::<_, Option<String>>(0)?
                    .unwrap_or_else(|| DEFAULT_URL.to_string()),
                icon: row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| DEFAULT_ICON.to_string()),
            })
        },
    );

    match result {
        Ok(link) => Ok(link),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(CustomNavLink {
            url: DEFAULT_URL.to_string(),
            icon: DEFAULT_ICON.to_string(),
        }),
        Err(e) => Err(format!("Failed to get custom nav link: {e}")),
    }
}

/// Update the singleton custom link. Only provided fields are changed.
pub fn update_custom_nav_link(
    conn: &Connection,
    data: &UpdateCustomNavLink,
) -> Result<CustomNavLink, String> {
    let current = get_custom_nav_link(conn)?;

    let url = data.url.clone().unwrap_or(current.url);
    let icon = data.icon.clone().unwrap_or(current.icon);

    // Bound the stored values; the URL is opened by the browser and the icon
    // key is matched against a client-side whitelist.
    if url.len() > 2048 {
        return Err("Invalid custom link url: too long".to_string());
    }
    if icon.len() > 64 {
        return Err("Invalid custom link icon: too long".to_string());
    }
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Invalid custom link url: must start with http:// or https://".to_string());
    }

    conn.execute(
        "INSERT INTO custom_nav_link (id, url, icon, updated_at)
         VALUES (1, ?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
         ON CONFLICT(id) DO UPDATE SET
             url = excluded.url,
             icon = excluded.icon,
             updated_at = excluded.updated_at",
        rusqlite::params![url, icon],
    )
    .map_err(|e| format!("Failed to update custom nav link: {e}"))?;

    Ok(CustomNavLink { url, icon })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[test]
    fn test_get_default_returns_builtin_link() {
        let conn = open_test_db();
        let link = get_custom_nav_link(&conn).unwrap();
        assert_eq!(link.url, DEFAULT_URL);
        assert_eq!(link.icon, DEFAULT_ICON);
    }

    #[test]
    fn test_update_url_and_icon_round_trip() {
        let conn = open_test_db();
        let link = update_custom_nav_link(
            &conn,
            &UpdateCustomNavLink {
                url: Some("https://example.com".to_string()),
                icon: Some("globe".to_string()),
            },
        )
        .unwrap();

        assert_eq!(link.url, "https://example.com");
        assert_eq!(link.icon, "globe");
        assert_eq!(get_custom_nav_link(&conn).unwrap(), link);
    }

    #[test]
    fn test_update_omitted_field_keeps_existing() {
        let conn = open_test_db();
        update_custom_nav_link(
            &conn,
            &UpdateCustomNavLink {
                url: Some("https://example.com".to_string()),
                icon: Some("globe".to_string()),
            },
        )
        .unwrap();

        let link = update_custom_nav_link(
            &conn,
            &UpdateCustomNavLink {
                url: None,
                icon: Some("calendar".to_string()),
            },
        )
        .unwrap();

        assert_eq!(link.url, "https://example.com");
        assert_eq!(link.icon, "calendar");
    }

    #[test]
    fn test_empty_url_hides_button() {
        let conn = open_test_db();
        let link = update_custom_nav_link(
            &conn,
            &UpdateCustomNavLink {
                url: Some(String::new()),
                icon: None,
            },
        )
        .unwrap();
        assert_eq!(link.url, "");
    }

    #[test]
    fn test_invalid_scheme_rejected() {
        let conn = open_test_db();
        assert!(
            update_custom_nav_link(
                &conn,
                &UpdateCustomNavLink {
                    url: Some("javascript:alert(1)".to_string()),
                    icon: None,
                },
            )
            .is_err()
        );
    }
}
