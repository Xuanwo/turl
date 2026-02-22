use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

const SESSION_ID: &str = "019c871c-b1f9-7f60-9c4f-87ed09f13592";

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

fn codex_uri() -> String {
    format!("codex://{SESSION_ID}")
}

fn codex_deeplink_uri() -> String {
    format!("codex://threads/{SESSION_ID}")
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
