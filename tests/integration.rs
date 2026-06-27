use std::io::Write;
use std::process::{Command, Stdio};

/// Helper: run clio in a specific data dir.
fn clio_in_dir(data_dir: Option<&str>, args: &[&str]) -> (String, String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_clio"));
    cmd.args(args);

    if let Some(dir) = data_dir {
        cmd.env("CLIO_DATA_DIR", dir);
    }

    let output = cmd.output().expect("failed to run clio");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (stdout, stderr, code)
}

/// Helper: run clio with stdin piped.
fn clio_with_stdin(data_dir: Option<&str>, args: &[&str], stdin_data: &[u8]) -> (String, String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_clio"));
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = data_dir {
        cmd.env("CLIO_DATA_DIR", dir);
    }

    let mut child = cmd.spawn().expect("failed to spawn clio");

    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(stdin_data).expect("failed to write to stdin");
    }

    let output = child.wait_with_output().expect("failed to wait on clio");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (stdout, stderr, code)
}

// ── set / get ──

#[test]
fn test_set_and_get() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    let (_, _, code) = clio_in_dir(Some(&d), &["set", "test-key", "hello"]);
    assert_eq!(code, 0, "set failed");

    let (stdout, _, code) = clio_in_dir(Some(&d), &["get", "test-key"]);
    assert_eq!(code, 0, "get failed");
    assert!(stdout.contains("hello"), "got: {}", stdout);
}

#[test]
fn test_set_stdin() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    let (_, _, code) = clio_with_stdin(
        Some(&d),
        &["set", "stdin-key"],
        b"from stdin\nwith newlines",
    );
    assert_eq!(code, 0, "set stdin failed");

    let (stdout, _, code) = clio_in_dir(Some(&d), &["get", "stdin-key"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("from stdin"));
    assert!(stdout.contains("with newlines"));
}

#[test]
fn test_get_nonexistent() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    let (_, stderr, code) = clio_in_dir(Some(&d), &["get", "no-such-key"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("not found"), "stderr: {}", stderr);
}

// ── delete ──

#[test]
fn test_delete() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "del-key", "bye"]);
    let (_, _, code) = clio_in_dir(Some(&d), &["delete", "del-key"]);
    assert_eq!(code, 0, "delete failed");

    let (_, stderr, code) = clio_in_dir(Some(&d), &["get", "del-key"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("not found"));
}

#[test]
fn test_delete_alias() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "rm-key", "x"]);
    let (_, _, code) = clio_in_dir(Some(&d), &["rm", "rm-key"]);
    assert_eq!(code, 0);
}

// ── list ──

#[test]
fn test_list() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "list-a", "alpha"]);
    clio_in_dir(Some(&d), &["set", "list-b", "bravo"]);

    let (stdout, _, code) = clio_in_dir(Some(&d), &["list"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list-a"));
    assert!(stdout.contains("alpha"));
    assert!(stdout.contains("list-b"));
    assert!(stdout.contains("bravo"));
}

#[test]
fn test_list_keys_only() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "keys-a", "v1"]);
    clio_in_dir(Some(&d), &["set", "keys-b", "v2"]);

    let (stdout, _, code) = clio_in_dir(Some(&d), &["list", "--keys-only"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("keys-a"));
    assert!(stdout.contains("keys-b"));
    assert!(!stdout.contains("v1"));
}

#[test]
fn test_list_reverse() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "ra", "1"]);
    clio_in_dir(Some(&d), &["set", "rb", "2"]);
    clio_in_dir(Some(&d), &["set", "rc", "3"]);

    let (stdout, _, code) = clio_in_dir(Some(&d), &["list", "--reverse"]);
    assert_eq!(code, 0);

    let rc_pos = stdout.find("rc").unwrap();
    let ra_pos = stdout.find("ra").unwrap();
    assert!(rc_pos < ra_pos, "reverse order: rc should appear before ra");
}

#[test]
fn test_list_values_only() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "val-x", "xv"]);
    clio_in_dir(Some(&d), &["set", "val-y", "yv"]);

    let (stdout, _, code) = clio_in_dir(Some(&d), &["list", "--values-only"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("xv"));
    assert!(stdout.contains("yv"));
    assert!(!stdout.contains("val-x"));
}

#[test]
fn test_list_has_indices() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "a", "1"]);
    clio_in_dir(Some(&d), &["set", "b", "2"]);

    let (stdout, _, code) = clio_in_dir(Some(&d), &["list"]);
    assert_eq!(code, 0);
    // Each line should start with "0 " or "1 " (index + space)
    assert!(stdout.contains("0 "), "expected index 0 in: {}", stdout);
    assert!(stdout.contains("1 "), "expected index 1 in: {}", stdout);
    // Make sure keys and values still appear
    assert!(stdout.contains("a"));
    assert!(stdout.contains("b"));
    assert!(stdout.contains("1"));
    assert!(stdout.contains("2"));
}

#[test]
fn test_list_aligned_columns() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    // Keys of different lengths — values should start at the same column
    clio_in_dir(Some(&d), &["set", "k", "short-val"]);
    clio_in_dir(Some(&d), &["set", "long-key-name", "long-val"]);

    let (stdout, _, code) = clio_in_dir(Some(&d), &["list", "--delimiter", "|"]);
    assert_eq!(code, 0);

    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 lines, got: {:?}", lines);

    // The delimiter | should appear at the same column in both lines
    let col0 = lines[0].find('|').unwrap();
    let col1 = lines[1].find('|').unwrap();
    assert_eq!(col0, col1, "values not aligned: line0 col={col0}, line1 col={col1}\nlines: {:?}", lines);
}

#[test]
fn test_index_get() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "first", "alpha"]);
    clio_in_dir(Some(&d), &["set", "second", "bravo"]);
    clio_in_dir(Some(&d), &["set", "third", "charlie"]);

    let (stdout, _, code) = clio_in_dir(Some(&d), &["-i", "1"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("bravo"), "expected 'bravo', got: {}", stdout);
}

#[test]
fn test_index_out_of_range() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "only", "value"]);

    let (_, stderr, code) = clio_in_dir(Some(&d), &["-i", "99"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("out of range"), "stderr: {}", stderr);
}

// ── list-dbs ──

#[test]
fn test_list_dbs() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "k1@work", "v1"]);
    clio_in_dir(Some(&d), &["set", "k2@personal", "v2"]);

    let (stdout, _, code) = clio_in_dir(Some(&d), &["list-dbs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("@work"));
    assert!(stdout.contains("@personal"));
}

#[test]
fn test_ls_db_alias() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "k@lsdb", "v"]);
    let (stdout, _, code) = clio_in_dir(Some(&d), &["ls-db"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("@lsdb"));
}

// ── key parsing ──

#[test]
fn test_default_db() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "plain-key", "default-db-value"]);
    let (stdout, _, code) = clio_in_dir(Some(&d), &["get", "plain-key"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("default-db-value"));
}

#[test]
fn test_case_insensitive_keys() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "MiXeDcAsE", "mixed-value"]);
    let (stdout, _, code) = clio_in_dir(Some(&d), &["get", "mixedcase"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("mixed-value"));
}

// ── binary handling ──

/// Helper: run clio and capture raw stdout bytes.
fn clio_raw_stdout(data_dir: &str, args: &[&str]) -> (Vec<u8>, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_clio"))
        .args(args)
        .env("CLIO_DATA_DIR", data_dir)
        .output()
        .expect("failed to run clio");

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (output.stdout, stderr, code)
}

#[test]
fn test_binary_value_show_binary() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    let binary = vec![0x00, 0x01, 0x02, 0xFF, 0xFE];
    clio_with_stdin(Some(&d), &["set", "binary"], &binary);

    // With --show-binary, raw bytes are printed.
    let (stdout, _, code) = clio_raw_stdout(&d, &["get", "binary", "--show-binary"]);
    assert_eq!(code, 0);
    assert_eq!(stdout, binary, "raw bytes should match");
}

// ── db commands ──

#[test]
fn test_delete_db_multiple() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_string_lossy();

    clio_in_dir(Some(&d), &["set", "x@db1", "1"]);
    clio_in_dir(Some(&d), &["set", "y@db1", "2"]);
    clio_in_dir(Some(&d), &["set", "z@db2", "3"]);

    // Delete db1 with confirmation
    let (_, _, code) = clio_with_stdin(Some(&d), &["delete-db", "db1"], b"y\n");
    assert_eq!(code, 0);

    // db2 should still exist
    let (stdout, _, code) = clio_in_dir(Some(&d), &["list", "@db2"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("z"));
}
