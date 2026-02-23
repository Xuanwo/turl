use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

const SESSION_ID: &str = "019c871c-b1f9-7f60-9c4f-87ed09f13592";
const AMP_SESSION_ID: &str = "T-019c0797-c402-7389-bd80-d785c98df295";
const GEMINI_SESSION_ID: &str = "29d207db-ca7e-40ba-87f7-e14c9de60613";

fn setup_codex_tree() -> tempfile::TempDir {
    let temp = tempdir().expect("tempdir");
    let thread_path = temp.path().join(format!(
        "sessions/2026/02/23/rollout-2026-02-23T04-48-50-{SESSION_ID}.jsonl"
    ));
    fs::create_dir_all(thread_path.parent().expect("parent")).expect("mkdir");
    fs::write(
        &thread_path,
        "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"hello\"}]}}\n{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"world\"}]}}\n",
    )
    .expect("write");

    temp
}

fn setup_amp_tree() -> tempfile::TempDir {
    let temp = tempdir().expect("tempdir");
    let thread_path = temp
        .path()
        .join(format!("amp/threads/{AMP_SESSION_ID}.json"));
    fs::create_dir_all(thread_path.parent().expect("parent")).expect("mkdir");
    fs::write(
        &thread_path,
        r#"{"id":"T-019c0797-c402-7389-bd80-d785c98df295","messages":[{"role":"user","content":[{"type":"text","text":"hello"}]},{"role":"assistant","content":[{"type":"thinking","thinking":"analyze"},{"type":"text","text":"world"}]}]}"#,
    )
    .expect("write");
    temp
}

fn setup_gemini_tree() -> tempfile::TempDir {
    let temp = tempdir().expect("tempdir");
    let thread_path = temp.path().join(
        ".gemini/tmp/0c0d7b04c22749f3687ea60b66949fd32bcea2551d4349bf72346a9ccc9a9ba4/chats/session-2026-01-08T11-55-29-29d207db.json",
    );
    fs::create_dir_all(thread_path.parent().expect("parent")).expect("mkdir");
    fs::write(
        &thread_path,
        format!(
            r#"{{
  "sessionId": "{GEMINI_SESSION_ID}",
  "projectHash": "0c0d7b04c22749f3687ea60b66949fd32bcea2551d4349bf72346a9ccc9a9ba4",
  "startTime": "2026-01-08T11:55:12.379Z",
  "lastUpdated": "2026-01-08T12:31:14.881Z",
  "messages": [
    {{ "type": "info", "content": "ignored" }},
    {{ "type": "user", "content": "hello" }},
    {{ "type": "gemini", "content": "world" }}
  ]
}}"#
        ),
    )
    .expect("write");
    temp
}

fn codex_uri() -> String {
    format!("codex://{SESSION_ID}")
}

fn codex_deeplink_uri() -> String {
    format!("codex://threads/{SESSION_ID}")
}

fn amp_uri() -> String {
    format!("amp://{AMP_SESSION_ID}")
}

fn gemini_uri() -> String {
    format!("gemini://{GEMINI_SESSION_ID}")
}

#[test]
fn default_outputs_markdown() {
    let temp = setup_codex_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(codex_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Thread"))
        .stdout(predicate::str::contains("## 1. User"))
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn raw_outputs_json() {
    let temp = setup_codex_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(codex_uri())
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"response_item\""));
}

#[test]
fn codex_deeplink_outputs_markdown() {
    let temp = setup_codex_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(codex_deeplink_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Thread"))
        .stdout(predicate::str::contains("## 1. User"))
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn missing_thread_returns_non_zero() {
    let temp = tempdir().expect("tempdir");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path())
        .arg(codex_uri())
        .assert()
        .failure()
        .stderr(predicate::str::contains("thread not found"));
}

#[test]
fn amp_outputs_markdown() {
    let temp = setup_amp_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("XDG_DATA_HOME", temp.path())
        .env("CODEX_HOME", temp.path().join("missing-codex"))
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(amp_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Thread"))
        .stdout(predicate::str::contains("## 1. User"))
        .stdout(predicate::str::contains("hello"))
        .stdout(predicate::str::contains("analyze"))
        .stdout(predicate::str::contains("world"));
}

#[test]
fn amp_raw_outputs_json() {
    let temp = setup_amp_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("XDG_DATA_HOME", temp.path())
        .env("CODEX_HOME", temp.path().join("missing-codex"))
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(amp_uri())
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"messages\""));
}

#[test]
fn gemini_outputs_markdown() {
    let temp = setup_gemini_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("GEMINI_CLI_HOME", temp.path())
        .arg(gemini_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Thread"))
        .stdout(predicate::str::contains("## 1. User"))
        .stdout(predicate::str::contains("hello"))
        .stdout(predicate::str::contains("world"));
}

#[test]
fn gemini_raw_outputs_json() {
    let temp = setup_gemini_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("GEMINI_CLI_HOME", temp.path())
        .arg(gemini_uri())
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sessionId\""));
}
