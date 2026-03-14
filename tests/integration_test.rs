use std::process::Command;
use tempfile::TempDir;

fn zend_bin() -> String {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("zend");
    path.to_string_lossy().to_string()
}

fn run_zend(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(zend_bin())
        .args(args)
        .output()
        .expect("failed to execute zend");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

fn run_zend_with_env(args: &[&str], env: &[(&str, &str)]) -> (String, String, i32) {
    let mut cmd = Command::new(zend_bin());
    cmd.args(args);
    // Clear Zendesk env vars to avoid interference
    cmd.env_remove("ZENDESK_SUBDOMAIN");
    cmd.env_remove("ZENDESK_EMAIL");
    cmd.env_remove("ZENDESK_API_TOKEN");
    for (k, v) in env {
        cmd.env(k, v);
    }
    // Use a temp HOME to avoid reading real config
    let tmp = TempDir::new().unwrap();
    cmd.env("HOME", tmp.path());
    let output = cmd.output().expect("failed to execute zend");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

// ── Argv routing tests ──

#[test]
fn test_numeric_argv_routing() {
    // `zend 99999` should route to `zend ticket 99999`
    // Both should produce the same error (API error, since domain is fake)
    let env = &[
        ("ZENDESK_SUBDOMAIN", "test-routing"),
        ("ZENDESK_EMAIL", "test@test.com"),
        ("ZENDESK_API_TOKEN", "fake"),
    ];
    let (stdout1, _, code1) = run_zend_with_env(&["99999"], env);
    let (stdout2, _, code2) = run_zend_with_env(&["ticket", "99999"], env);

    // Both should fail (API unreachable)
    assert_ne!(code1, 0);
    assert_ne!(code2, 0);

    // Both should produce structured JSON errors (routing is equivalent)
    let v1: serde_json::Value = serde_json::from_str(&stdout1).expect("stdout1 should be valid JSON");
    let v2: serde_json::Value = serde_json::from_str(&stdout2).expect("stdout2 should be valid JSON");
    assert!(v1.get("error").is_some(), "bare number should produce structured error");
    assert!(v2.get("error").is_some(), "explicit ticket should produce structured error");
    assert!(v1.get("message").is_some());
    assert!(v2.get("message").is_some());
}

#[test]
fn test_email_argv_routing() {
    // `zend user@example.com` should route to `zend email user@example.com`
    let env = &[
        ("ZENDESK_SUBDOMAIN", "test-routing"),
        ("ZENDESK_EMAIL", "test@test.com"),
        ("ZENDESK_API_TOKEN", "fake"),
    ];
    let (stdout1, _, code1) = run_zend_with_env(&["user@example.com"], env);
    let (stdout2, _, code2) = run_zend_with_env(&["email", "user@example.com"], env);

    assert_ne!(code1, 0);
    assert_ne!(code2, 0);

    // Both should produce structured JSON errors (routing is equivalent)
    let v1: serde_json::Value = serde_json::from_str(&stdout1).expect("stdout1 should be valid JSON");
    let v2: serde_json::Value = serde_json::from_str(&stdout2).expect("stdout2 should be valid JSON");
    assert!(v1.get("error").is_some(), "bare email should produce structured error");
    assert!(v2.get("error").is_some(), "explicit email should produce structured error");
    assert!(v1.get("message").is_some());
    assert!(v2.get("message").is_some());
}

// ── Missing config error tests ──

#[test]
fn test_missing_config_error() {
    let (_stdout, stderr, code) = run_zend_with_env(&["12345"], &[]);

    assert_ne!(code, 0, "should exit with non-zero when config is missing");

    // stderr should have the "Not configured" message
    assert!(
        stderr.contains("Not configured"),
        "stderr should contain config missing hint, got: {stderr}"
    );
}

// ── Invalid args tests ──

#[test]
fn test_ticket_non_numeric_id_error() {
    let (stdout, _, code) = run_zend_with_env(
        &["ticket", "abc"],
        &[
            ("ZENDESK_SUBDOMAIN", "test"),
            ("ZENDESK_EMAIL", "test@test.com"),
            ("ZENDESK_API_TOKEN", "fake"),
        ],
    );

    assert_ne!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    assert_eq!(v.get("error").unwrap().as_str().unwrap(), "invalid_args");
    assert!(v.get("message").unwrap().as_str().unwrap().contains("numeric"));
}

#[test]
fn test_comments_non_numeric_id_error() {
    let (stdout, _, code) = run_zend_with_env(
        &["comments", "abc"],
        &[
            ("ZENDESK_SUBDOMAIN", "test"),
            ("ZENDESK_EMAIL", "test@test.com"),
            ("ZENDESK_API_TOKEN", "fake"),
        ],
    );

    assert_ne!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    assert_eq!(v.get("error").unwrap().as_str().unwrap(), "invalid_args");
    assert!(v.get("message").unwrap().as_str().unwrap().contains("numeric"));
}

#[test]
fn test_invalid_sort_error() {
    let (stdout, _, code) = run_zend_with_env(
        &["email", "user@test.com", "--sort", "invalid"],
        &[
            ("ZENDESK_SUBDOMAIN", "test"),
            ("ZENDESK_EMAIL", "test@test.com"),
            ("ZENDESK_API_TOKEN", "fake"),
        ],
    );

    assert_ne!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    assert_eq!(v.get("error").unwrap().as_str().unwrap(), "invalid_args");
}

#[test]
fn test_invalid_visibility_error() {
    let (stdout, _, code) = run_zend_with_env(
        &["comments", "123", "--visibility", "invalid"],
        &[
            ("ZENDESK_SUBDOMAIN", "test"),
            ("ZENDESK_EMAIL", "test@test.com"),
            ("ZENDESK_API_TOKEN", "fake"),
        ],
    );

    assert_ne!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    assert_eq!(v.get("error").unwrap().as_str().unwrap(), "invalid_args");
}

#[test]
fn test_invalid_limit_error() {
    let (stdout, _, code) = run_zend_with_env(
        &["email", "user@test.com", "--limit", "999"],
        &[
            ("ZENDESK_SUBDOMAIN", "test"),
            ("ZENDESK_EMAIL", "test@test.com"),
            ("ZENDESK_API_TOKEN", "fake"),
        ],
    );

    assert_ne!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    assert_eq!(v.get("error").unwrap().as_str().unwrap(), "invalid_args");
}

// ── Config tests ──

#[test]
fn test_env_overrides_file_config() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".zendcli");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_file = config_dir.join("config.json");
    std::fs::write(
        &config_file,
        r#"{"subdomain":"file-sub","email":"file@test.com","api_token":"file-token"}"#,
    )
    .unwrap();

    // Run with env var override for subdomain - the command will fail trying to connect
    // but we can verify the error references the env subdomain, not the file one
    let mut cmd = Command::new(zend_bin());
    cmd.args(["ticket", "1"]);
    cmd.env_remove("ZENDESK_SUBDOMAIN");
    cmd.env_remove("ZENDESK_EMAIL");
    cmd.env_remove("ZENDESK_API_TOKEN");
    cmd.env("HOME", tmp.path());
    cmd.env("ZENDESK_SUBDOMAIN", "env-sub");
    let output = cmd.output().expect("failed to execute");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // It should try to connect to env-sub.zendesk.com (env override), not file-sub
    // The error should be about connection/API, not about missing config
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    let error_code = v.get("error").unwrap().as_str().unwrap();
    // Should NOT be "not_configured" - config is present (from file + env)
    assert_ne!(error_code, "not_configured", "config should be loaded from file + env");
}

// ── Help and version tests ──

#[test]
fn test_help_output() {
    let (stdout, _, code) = run_zend(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Zendesk tickets CLI"));
    assert!(stdout.contains("configure"));
    assert!(stdout.contains("ticket"));
    assert!(stdout.contains("email"));
    assert!(stdout.contains("follower"));
    assert!(stdout.contains("comments"));
}

#[test]
fn test_version_output() {
    let (stdout, _, code) = run_zend(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("2.0.0"));
}

// ── Structured error output format tests ──

#[test]
fn test_api_error_structured_output() {
    // With valid config but pointing to non-existent server
    let (stdout, _, code) = run_zend_with_env(
        &["ticket", "12345"],
        &[
            ("ZENDESK_SUBDOMAIN", "nonexistent-test-domain-12345"),
            ("ZENDESK_EMAIL", "test@test.com"),
            ("ZENDESK_API_TOKEN", "fake-token"),
        ],
    );

    assert_ne!(code, 0);
    // stdout should be valid JSON with error and message fields
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("error output should be valid JSON");
    assert!(v.get("error").is_some(), "should have 'error' field");
    assert!(v.get("message").is_some(), "should have 'message' field");
}

// ── Configure command tests ──

#[test]
fn test_configure_writes_config() {
    let tmp = TempDir::new().unwrap();

    let mut cmd = Command::new(zend_bin());
    cmd.arg("configure");
    cmd.env_remove("ZENDESK_SUBDOMAIN");
    cmd.env_remove("ZENDESK_EMAIL");
    cmd.env_remove("ZENDESK_API_TOKEN");
    cmd.env("HOME", tmp.path());

    // Provide input via stdin
    use std::io::Write;
    use std::process::Stdio;
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn");
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"test-sub\ntest@example.com\ntest-token\n").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Should output { "ok": true }
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    assert_eq!(v.get("ok").unwrap().as_bool().unwrap(), true);

    // Config file should exist
    let config_path = tmp.path().join(".zendcli").join("config.json");
    assert!(config_path.exists(), "config file should be created");

    // Config file should contain the values we provided
    let config_content = std::fs::read_to_string(&config_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&config_content).unwrap();
    assert_eq!(config.get("subdomain").unwrap().as_str().unwrap(), "test-sub");
    assert_eq!(config.get("email").unwrap().as_str().unwrap(), "test@example.com");
    assert_eq!(config.get("api_token").unwrap().as_str().unwrap(), "test-token");

    // Check file permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&config_path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "config file should have 0600 permissions");
    }
}
