use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

const SESSION_ID: &str = "019c871c-b1f9-7f60-9c4f-87ed09f13592";
const SUBAGENT_ID: &str = "019c87fb-38b9-7843-92b1-832f02598495";
const REAL_FIXTURE_MAIN_ID: &str = "55fe4488-c6bd-46fa-9390-dab3b8860b95";
const REAL_FIXTURE_AGENT_ID: &str = "29bf19c3-b83e-401d-8f38-5660b7f67152";
const AMP_SESSION_ID: &str = "T-019c0797-c402-7389-bd80-d785c98df295";
const GEMINI_SESSION_ID: &str = "29d207db-ca7e-40ba-87f7-e14c9de60613";
const GEMINI_REAL_SESSION_ID: &str = "da2ab190-85f8-4d5c-bcce-8292921a33bf";
const CLAUDE_SESSION_ID: &str = "2823d1df-720a-4c31-ac55-ae8ba726721f";
const CLAUDE_AGENT_ID: &str = "acompact-69d537";
const CLAUDE_REAL_MAIN_ID: &str = "b90fc33d-33cb-4027-8558-119e2b56c74e";
const CLAUDE_REAL_AGENT_ID: &str = "a4f21c7";
const OPENCODE_REAL_SESSION_ID: &str = "ses_7v2md9kx3c1p";

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

fn setup_codex_tree_with_sqlite_missing_threads() -> tempfile::TempDir {
    let temp = setup_codex_tree();
    fs::write(temp.path().join("state.sqlite"), "").expect("write sqlite");
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

fn setup_codex_subagent_tree() -> tempfile::TempDir {
    let temp = tempdir().expect("tempdir");
    let main_thread_path = temp.path().join(format!(
        "sessions/2026/02/23/rollout-2026-02-23T04-48-50-{SESSION_ID}.jsonl"
    ));
    fs::create_dir_all(main_thread_path.parent().expect("parent")).expect("mkdir");
    fs::write(
        &main_thread_path,
        format!(
            "{{\"timestamp\":\"2026-02-23T00:00:00Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"spawn_agent\",\"arguments\":\"{{}}\",\"call_id\":\"call_spawn\"}}}}\n{{\"timestamp\":\"2026-02-23T00:00:01Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call_output\",\"call_id\":\"call_spawn\",\"output\":\"{{\\\"agent_id\\\":\\\"{SUBAGENT_ID}\\\"}}\"}}}}\n{{\"timestamp\":\"2026-02-23T00:00:02Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"wait\",\"arguments\":\"{{\\\"ids\\\":[\\\"{SUBAGENT_ID}\\\"],\\\"timeout_ms\\\":120000}}\",\"call_id\":\"call_wait\"}}}}\n{{\"timestamp\":\"2026-02-23T00:00:03Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call_output\",\"call_id\":\"call_wait\",\"output\":\"{{\\\"status\\\":{{\\\"running\\\":\\\"in progress\\\"}},\\\"timed_out\\\":false}}\"}}}}\n{{\"timestamp\":\"2026-02-23T00:00:04Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"close_agent\",\"arguments\":\"{{\\\"id\\\":\\\"{SUBAGENT_ID}\\\"}}\",\"call_id\":\"call_close\"}}}}\n{{\"timestamp\":\"2026-02-23T00:00:05Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call_output\",\"call_id\":\"call_close\",\"output\":\"{{\\\"status\\\":{{\\\"completed\\\":\\\"done\\\"}}}}\"}}}}\n"
        ),
    )
    .expect("write main");

    let child_thread_path = temp.path().join(format!(
        "sessions/2026/02/23/rollout-2026-02-23T04-49-10-{SUBAGENT_ID}.jsonl"
    ));
    fs::create_dir_all(child_thread_path.parent().expect("parent")).expect("mkdir");
    fs::write(
        &child_thread_path,
        format!(
            "{{\"timestamp\":\"2026-02-23T00:00:10Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{SUBAGENT_ID}\",\"source\":{{\"subagent\":{{\"thread_spawn\":{{\"parent_thread_id\":\"{SESSION_ID}\",\"depth\":1}}}}}}}}}}\n{{\"timestamp\":\"2026-02-23T00:00:11Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":[{{\"type\":\"input_text\",\"text\":\"hello child\"}}]}}}}\n{{\"timestamp\":\"2026-02-23T00:00:12Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"done child\"}}]}}}}\n"
        ),
    )
    .expect("write child");

    temp
}

fn setup_codex_subagent_tree_with_sqlite_missing_threads() -> tempfile::TempDir {
    let temp = setup_codex_subagent_tree();
    fs::write(temp.path().join("state.sqlite"), "").expect("write sqlite");
    temp
}

fn setup_claude_subagent_tree() -> tempfile::TempDir {
    let temp = tempdir().expect("tempdir");
    let project = temp.path().join("projects/project-subagent");
    fs::create_dir_all(&project).expect("mkdir");

    let main_thread = project.join(format!("{CLAUDE_SESSION_ID}.jsonl"));
    fs::write(
        &main_thread,
        format!(
            "{{\"timestamp\":\"2026-02-23T00:00:00Z\",\"type\":\"user\",\"sessionId\":\"{CLAUDE_SESSION_ID}\",\"message\":{{\"role\":\"user\",\"content\":\"root thread\"}}}}\n"
        ),
    )
    .expect("write main");

    let subagents_dir = project.join(CLAUDE_SESSION_ID).join("subagents");
    fs::create_dir_all(&subagents_dir).expect("mkdir");
    let agent_thread = subagents_dir.join(format!("agent-{CLAUDE_AGENT_ID}.jsonl"));
    fs::write(
        &agent_thread,
        format!(
            "{{\"timestamp\":\"2026-02-23T00:00:10Z\",\"type\":\"user\",\"sessionId\":\"{CLAUDE_SESSION_ID}\",\"isSidechain\":true,\"agentId\":\"{CLAUDE_AGENT_ID}\",\"message\":{{\"role\":\"user\",\"content\":\"agent task\"}}}}\n{{\"timestamp\":\"2026-02-23T00:00:11Z\",\"type\":\"assistant\",\"sessionId\":\"{CLAUDE_SESSION_ID}\",\"isSidechain\":true,\"agentId\":\"{CLAUDE_AGENT_ID}\",\"message\":{{\"role\":\"assistant\",\"content\":\"agent done\"}}}}\n"
        ),
    )
    .expect("write agent");

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

fn codex_real_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex_real_sanitized")
}

fn claude_real_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude_real_sanitized")
}

fn gemini_real_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gemini_real_sanitized")
}

fn opencode_real_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/opencode_real_sanitized")
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

fn codex_subagent_uri() -> String {
    format!("codex://{SESSION_ID}/{SUBAGENT_ID}")
}

fn claude_uri() -> String {
    format!("claude://{CLAUDE_SESSION_ID}")
}

fn claude_subagent_uri() -> String {
    format!("claude://{CLAUDE_SESSION_ID}/{CLAUDE_AGENT_ID}")
}

fn gemini_uri() -> String {
    format!("gemini://{GEMINI_SESSION_ID}")
}

fn gemini_real_uri() -> String {
    format!("gemini://{GEMINI_REAL_SESSION_ID}")
}

fn claude_real_uri() -> String {
    format!("claude://{CLAUDE_REAL_MAIN_ID}")
}

fn claude_real_subagent_uri() -> String {
    format!("claude://{CLAUDE_REAL_MAIN_ID}/{CLAUDE_REAL_AGENT_ID}")
}

fn opencode_real_uri() -> String {
    format!("opencode://{OPENCODE_REAL_SESSION_ID}")
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
fn codex_list_raw_outputs_aggregate_json() {
    let temp = setup_codex_subagent_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(codex_uri())
        .arg("--list")
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"kind\": \"list\""))
        .stdout(predicate::str::contains(SUBAGENT_ID))
        .stdout(predicate::str::contains("\"warnings\"").not());
}

#[test]
fn codex_subagent_outputs_markdown_view() {
    let temp = setup_codex_subagent_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(codex_subagent_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Subagent Thread"))
        .stdout(predicate::str::contains("## Lifecycle (Parent Thread)"))
        .stdout(predicate::str::contains("## Thread Excerpt (Child Thread)"));
}

#[test]
fn codex_outputs_no_warning_text_for_markdown() {
    let temp = setup_codex_tree_with_sqlite_missing_threads();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(codex_uri())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning:").not());
}

#[test]
fn codex_subagent_outputs_no_warning_text_for_markdown() {
    let temp = setup_codex_subagent_tree_with_sqlite_missing_threads();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(codex_uri())
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("## Warnings").not())
        .stderr(predicate::str::contains("warning:").not());
}

#[test]
fn codex_real_fixture_subagent_list_outputs_markdown() {
    let fixture_root = codex_real_fixture_root();
    assert!(fixture_root.exists(), "fixture root must exist");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", fixture_root)
        .env("CLAUDE_CONFIG_DIR", "/tmp/missing-claude")
        .arg(format!("codex://{REAL_FIXTURE_MAIN_ID}"))
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Subagent Status"))
        .stdout(predicate::str::contains(REAL_FIXTURE_AGENT_ID));
}

#[test]
fn codex_real_fixture_subagent_detail_outputs_markdown() {
    let fixture_root = codex_real_fixture_root();
    assert!(fixture_root.exists(), "fixture root must exist");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", fixture_root)
        .env("CLAUDE_CONFIG_DIR", "/tmp/missing-claude")
        .arg(format!(
            "codex://{REAL_FIXTURE_MAIN_ID}/{REAL_FIXTURE_AGENT_ID}"
        ))
        .assert()
        .success()
        .stdout(predicate::str::contains("# Subagent Thread"))
        .stdout(predicate::str::contains("## Lifecycle (Parent Thread)"));
}

#[test]
fn list_mode_rejects_subagent_uri() {
    let temp = setup_codex_subagent_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CODEX_HOME", temp.path())
        .env("CLAUDE_CONFIG_DIR", temp.path().join("missing-claude"))
        .arg(codex_subagent_uri())
        .arg("--list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid mode"));
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

#[test]
fn claude_list_raw_outputs_aggregate_json() {
    let temp = setup_claude_subagent_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CLAUDE_CONFIG_DIR", temp.path())
        .env("CODEX_HOME", temp.path().join("missing-codex"))
        .arg(claude_uri())
        .arg("--list")
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"kind\": \"list\""))
        .stdout(predicate::str::contains(CLAUDE_AGENT_ID))
        .stdout(predicate::str::contains("\"warnings\"").not());
}

#[test]
fn claude_subagent_outputs_markdown_view() {
    let temp = setup_claude_subagent_tree();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CLAUDE_CONFIG_DIR", temp.path())
        .env("CODEX_HOME", temp.path().join("missing-codex"))
        .arg(claude_subagent_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Subagent Thread"))
        .stdout(predicate::str::contains("## Agent Status Summary"));
}

#[test]
fn claude_real_fixture_subagent_list_outputs_markdown() {
    let fixture_root = claude_real_fixture_root();
    assert!(fixture_root.exists(), "fixture root must exist");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CLAUDE_CONFIG_DIR", fixture_root)
        .env("CODEX_HOME", "/tmp/missing-codex")
        .arg(claude_real_uri())
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Subagent Status"))
        .stdout(predicate::str::contains(CLAUDE_REAL_AGENT_ID));
}

#[test]
fn claude_real_fixture_subagent_detail_outputs_markdown() {
    let fixture_root = claude_real_fixture_root();
    assert!(fixture_root.exists(), "fixture root must exist");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("CLAUDE_CONFIG_DIR", fixture_root)
        .env("CODEX_HOME", "/tmp/missing-codex")
        .arg(claude_real_subagent_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Subagent Thread"))
        .stdout(predicate::str::contains("## Thread Excerpt (Child Thread)"));
}

#[test]
fn gemini_real_fixture_outputs_markdown() {
    let fixture_root = gemini_real_fixture_root();
    assert!(fixture_root.exists(), "fixture root must exist");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("GEMINI_CLI_HOME", fixture_root)
        .arg(gemini_real_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Thread"))
        .stdout(predicate::str::contains("## 1. User"));
}

#[test]
fn gemini_real_fixture_raw_outputs_json() {
    let fixture_root = gemini_real_fixture_root();
    assert!(fixture_root.exists(), "fixture root must exist");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("GEMINI_CLI_HOME", fixture_root)
        .arg(gemini_real_uri())
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"sessionId\": \"da2ab190-85f8-4d5c-bcce-8292921a33bf\"",
        ));
}

#[test]
fn opencode_real_fixture_outputs_markdown() {
    let fixture_root = opencode_real_fixture_root();
    assert!(fixture_root.exists(), "fixture root must exist");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("XDG_DATA_HOME", fixture_root)
        .arg(opencode_real_uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("# Thread"))
        .stdout(predicate::str::contains("## 1. User"));
}

#[test]
fn opencode_real_fixture_raw_outputs_json() {
    let fixture_root = opencode_real_fixture_root();
    assert!(fixture_root.exists(), "fixture root must exist");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("turl"));
    cmd.env("XDG_DATA_HOME", fixture_root)
        .arg(opencode_real_uri())
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"type\":\"session\""))
        .stdout(predicate::str::contains(OPENCODE_REAL_SESSION_ID));
}
