use crate::config;

/// Mask a token for display: show first 4 + last 4 chars, or `****` if ≤8 chars.
fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    }
}

/// Prompt the user on stderr and read one line from stdin.
/// Returns `None` if the input is empty or reading fails.
fn prompt_stdin(prompt: &str) -> Option<String> {
    eprint!("{prompt}");
    let mut buf = String::new();
    if std::io::stdin().read_line(&mut buf).is_ok() {
        let trimmed = buf.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    } else {
        None
    }
}

/// Print stored URL and token (masked) to stdout.
fn print_credentials(url: &Option<String>, token: &Option<String>) {
    if let Some(ref u) = url {
        println!("  URL:   {u}");
    }
    if let Some(ref t) = token {
        println!("  Token: {}", mask_token(t));
    }
}

/// Serialize a JSON value with pretty-print and write to stdout.
fn print_json_value(value: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

/// Print the result of a successful login in human-readable or JSON format.
fn print_login_result(stored: &config::StoredConfig, json: bool) {
    if json {
        let obj = serde_json::json!({
            "status": "saved",
            "url": stored.url,
            "token": stored.token.as_deref().map(mask_token),
        });
        print_json_value(&obj);
    } else {
        println!("Credentials saved.");
        print_credentials(&stored.url, &stored.token);
    }
}

/// Merge url/token into stored config and validate token.
/// Returns an error message if validation fails.
fn apply_credentials(
    stored: &mut config::StoredConfig,
    url: Option<String>,
    token: Option<String>,
) -> Result<(), &'static str> {
    if let Some(u) = url {
        stored.url = Some(u);
    }
    if let Some(t) = token {
        if t.is_empty() {
            return Err("Token must not be empty.");
        }
        stored.token = Some(t);
    }
    Ok(())
}

pub async fn login(url: Option<String>, token: Option<String>, json: bool) -> i32 {
    // Prompt via stdin if flags are omitted
    let url = url.or_else(|| prompt_stdin("SonarQube URL (leave empty to keep current): "));
    let token = token.or_else(|| prompt_stdin("SonarQube token: "));

    if url.is_none() && token.is_none() {
        eprintln!("Nothing to save — both URL and token are empty.");
        return 1;
    }

    // Merge with existing config to preserve fields not being set
    let mut stored = config::load();
    if let Err(msg) = apply_credentials(&mut stored, url, token) {
        eprintln!("{msg}");
        return 1;
    }

    if let Err(e) = config::save(&stored) {
        eprintln!("Failed to save config: {e}");
        return 1;
    }

    print_login_result(&stored, json);
    0
}

pub async fn status(json: bool) -> i32 {
    let stored = config::load();

    if stored.url.is_none() && stored.token.is_none() {
        if json {
            let obj = serde_json::json!({"status": "not_configured"});
            print_json_value(&obj);
        } else {
            println!("No credentials configured. Run `sonar-cli auth login` to set up.");
        }
        return 0;
    }

    if json {
        let obj = serde_json::json!({
            "status": "configured",
            "url": stored.url,
            "token": stored.token.as_deref().map(mask_token),
        });
        print_json_value(&obj);
    } else {
        println!("Stored credentials:");
        print_credentials(&stored.url, &stored.token);
        if let Some(p) = config::config_path() {
            println!("  File:  {}", p.display());
        }
    }

    0
}

pub async fn logout(json: bool) -> i32 {
    match config::remove() {
        Ok(()) => {
            if json {
                let obj = serde_json::json!({"status": "removed"});
                print_json_value(&obj);
            } else {
                println!("Credentials removed.");
            }
            0
        }
        Err(e) => {
            eprintln!("Failed to remove credentials: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_mask_token_short() {
        assert_eq!(mask_token("abc"), "****");
        assert_eq!(mask_token("12345678"), "****");
    }

    #[test]
    fn test_mask_token_long() {
        assert_eq!(mask_token("squ_abcdefgh1234"), "squ_...1234");
        assert_eq!(mask_token("123456789"), "1234...6789");
    }

    #[test]
    fn test_mask_token_exactly_nine_chars() {
        assert_eq!(mask_token("123456789"), "1234...6789");
    }

    // ── apply_credentials ───────────────────────────────────────────────────

    #[test]
    fn test_apply_credentials_url_only() {
        let mut stored = config::StoredConfig::default();
        let result = apply_credentials(&mut stored, Some("https://sonar.example.com".to_string()), None);
        assert!(result.is_ok());
        assert_eq!(stored.url.as_deref(), Some("https://sonar.example.com"));
        assert!(stored.token.is_none());
    }

    #[test]
    fn test_apply_credentials_token_only() {
        let mut stored = config::StoredConfig::default();
        let result = apply_credentials(&mut stored, None, Some("squ_abc123xyz".to_string()));
        assert!(result.is_ok());
        assert!(stored.url.is_none());
        assert_eq!(stored.token.as_deref(), Some("squ_abc123xyz"));
    }

    #[test]
    fn test_apply_credentials_both() {
        let mut stored = config::StoredConfig::default();
        let result = apply_credentials(
            &mut stored,
            Some("https://sonar.example.com".to_string()),
            Some("squ_abc123".to_string()),
        );
        assert!(result.is_ok());
        assert_eq!(stored.url.as_deref(), Some("https://sonar.example.com"));
        assert_eq!(stored.token.as_deref(), Some("squ_abc123"));
    }

    #[test]
    fn test_apply_credentials_neither_preserves_existing() {
        let mut stored = config::StoredConfig {
            url: Some("existing_url".to_string()),
            token: Some("existing_token".to_string()),
        };
        let result = apply_credentials(&mut stored, None, None);
        assert!(result.is_ok());
        assert_eq!(stored.url.as_deref(), Some("existing_url"));
        assert_eq!(stored.token.as_deref(), Some("existing_token"));
    }

    #[test]
    fn test_apply_credentials_empty_token_returns_error() {
        let mut stored = config::StoredConfig::default();
        let result = apply_credentials(&mut stored, None, Some(String::new()));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Token must not be empty.");
    }

    #[test]
    fn test_apply_credentials_overwrites_url() {
        let mut stored = config::StoredConfig {
            url: Some("old_url".to_string()),
            token: Some("old_token".to_string()),
        };
        let result = apply_credentials(&mut stored, Some("new_url".to_string()), None);
        assert!(result.is_ok());
        assert_eq!(stored.url.as_deref(), Some("new_url"));
        assert_eq!(stored.token.as_deref(), Some("old_token"));
    }

    // ── print_credentials ───────────────────────────────────────────────────

    #[test]
    fn test_print_credentials_both_set() {
        print_credentials(
            &Some("https://sonar.example.com".to_string()),
            &Some("squ_abcdefgh1234".to_string()),
        );
    }

    #[test]
    fn test_print_credentials_url_only() {
        print_credentials(&Some("https://sonar.example.com".to_string()), &None);
    }

    #[test]
    fn test_print_credentials_token_only() {
        print_credentials(&None, &Some("squ_abcdefgh1234".to_string()));
    }

    #[test]
    fn test_print_credentials_neither() {
        print_credentials(&None, &None);
    }

    // ── print_json_value ────────────────────────────────────────────────────

    #[test]
    fn test_print_json_value_object() {
        let value = serde_json::json!({"status": "ok", "count": 42});
        print_json_value(&value);
    }

    // ── print_login_result ──────────────────────────────────────────────────

    #[test]
    fn test_print_login_result_human_with_token() {
        let stored = config::StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: Some("squ_abcdefgh1234".to_string()),
        };
        print_login_result(&stored, false);
    }

    #[test]
    fn test_print_login_result_json_with_token() {
        let stored = config::StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: Some("squ_abcdefgh1234".to_string()),
        };
        print_login_result(&stored, true);
    }

    #[test]
    fn test_print_login_result_human_no_token() {
        let stored = config::StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: None,
        };
        print_login_result(&stored, false);
    }

    #[test]
    fn test_print_login_result_json_no_token() {
        let stored = config::StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: None,
        };
        print_login_result(&stored, true);
    }

    #[test]
    fn test_print_login_result_json_no_url_no_token() {
        let stored = config::StoredConfig { url: None, token: None };
        print_login_result(&stored, true);
    }

    // ── login ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_login_empty_token_returns_error() {
        // url provided, empty token → apply_credentials returns Err → returns 1
        // Does NOT write to config (early return before config::save)
        let result = login(
            Some("https://sonar.example.com".to_string()),
            Some(String::new()),
            false,
        )
        .await;
        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn test_login_empty_token_json_returns_error() {
        let result = login(
            Some("https://sonar.example.com".to_string()),
            Some(String::new()),
            true,
        )
        .await;
        assert_eq!(result, 1);
    }

    /// Saves test credentials, asserts login succeeds, then restores the previous
    /// config state so the test is idempotent on developer machines.
    #[tokio::test]
    #[serial]
    async fn test_login_success_human() {
        let backup = config::load();
        let result = login(
            Some("https://test.sonar.example.com".to_string()),
            Some("squ_test_token_abcdefgh1234".to_string()),
            false,
        )
        .await;
        // Restore prior state
        if backup.url.is_none() && backup.token.is_none() {
            let _ = config::remove();
        } else {
            let _ = config::save(&backup);
        }
        assert_eq!(result, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_login_success_json() {
        let backup = config::load();
        let result = login(
            Some("https://test.sonar.example.com".to_string()),
            Some("squ_test_token_abcdefgh1234".to_string()),
            true,
        )
        .await;
        if backup.url.is_none() && backup.token.is_none() {
            let _ = config::remove();
        } else {
            let _ = config::save(&backup);
        }
        assert_eq!(result, 0);
    }

    // ── status ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_status_human_returns_success() {
        // config::load() is read-only — safe in all environments
        let result = status(false).await;
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_status_json_returns_success() {
        let result = status(true).await;
        assert_eq!(result, 0);
    }

    /// Exercise the "configured" branch of status by saving credentials first.
    #[tokio::test]
    #[serial]
    async fn test_status_human_with_credentials() {
        let backup = config::load();
        let _ = config::save(&config::StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: Some("squ_abcdefgh1234".to_string()),
        });
        let result = status(false).await;
        // Restore
        if backup.url.is_none() && backup.token.is_none() {
            let _ = config::remove();
        } else {
            let _ = config::save(&backup);
        }
        assert_eq!(result, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_status_json_with_credentials() {
        let backup = config::load();
        let _ = config::save(&config::StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: Some("squ_abcdefgh1234".to_string()),
        });
        let result = status(true).await;
        if backup.url.is_none() && backup.token.is_none() {
            let _ = config::remove();
        } else {
            let _ = config::save(&backup);
        }
        assert_eq!(result, 0);
    }

    /// Exercise status when no credentials are configured.
    #[tokio::test]
    #[serial]
    async fn test_status_human_no_credentials() {
        let backup = config::load();
        let _ = config::remove();
        let result = status(false).await;
        // Restore
        if backup.url.is_some() || backup.token.is_some() {
            let _ = config::save(&backup);
        }
        assert_eq!(result, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_status_json_no_credentials() {
        let backup = config::load();
        let _ = config::remove();
        let result = status(true).await;
        if backup.url.is_some() || backup.token.is_some() {
            let _ = config::save(&backup);
        }
        assert_eq!(result, 0);
    }

    // ── logout ──────────────────────────────────────────────────────────────

    /// Saves credentials, then logs out — exercises the file-removal path.
    /// Uses #[serial] to avoid races with other config-touching tests.
    #[tokio::test]
    #[serial]
    async fn test_logout_human_removes_credentials() {
        let backup = config::load();
        let _ = config::save(&config::StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: Some("squ_abcdefgh1234".to_string()),
        });
        let result = logout(false).await;
        // Restore if there were real credentials before the test
        if backup.url.is_some() || backup.token.is_some() {
            let _ = config::save(&backup);
        }
        assert_eq!(result, 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_logout_json_removes_credentials() {
        let backup = config::load();
        let _ = config::save(&config::StoredConfig {
            url: Some("https://sonar.example.com".to_string()),
            token: Some("squ_abcdefgh1234".to_string()),
        });
        let result = logout(true).await;
        if backup.url.is_some() || backup.token.is_some() {
            let _ = config::save(&backup);
        }
        assert_eq!(result, 0);
    }
}
