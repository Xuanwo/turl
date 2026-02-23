use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::error::{Result, TurlError};
use crate::model::{
    ProviderKind, ResolvedThread, SubagentDetailView, SubagentExcerptMessage,
    SubagentLifecycleEvent, SubagentListItem, SubagentListView, SubagentQuery, SubagentRelation,
    SubagentThreadRef, SubagentView,
};
use crate::provider::amp::AmpProvider;
use crate::provider::claude::ClaudeProvider;
use crate::provider::codex::CodexProvider;
use crate::provider::gemini::GeminiProvider;
use crate::provider::opencode::OpencodeProvider;
use crate::provider::{Provider, ProviderRoots};
use crate::render;
use crate::uri::ThreadUri;

const STATUS_PENDING_INIT: &str = "pendingInit";
const STATUS_RUNNING: &str = "running";
const STATUS_COMPLETED: &str = "completed";
const STATUS_ERRORED: &str = "errored";
const STATUS_SHUTDOWN: &str = "shutdown";
const STATUS_NOT_FOUND: &str = "notFound";

#[derive(Debug, Default, Clone)]
struct AgentTimeline {
    events: Vec<SubagentLifecycleEvent>,
    states: Vec<String>,
    has_spawn: bool,
    has_activity: bool,
    last_update: Option<String>,
}

#[derive(Debug, Clone)]
struct ClaudeAgentRecord {
    agent_id: String,
    path: PathBuf,
    status: String,
    last_update: Option<String>,
    relation: SubagentRelation,
    excerpt: Vec<SubagentExcerptMessage>,
    warnings: Vec<String>,
}

pub fn resolve_thread(uri: &ThreadUri, roots: &ProviderRoots) -> Result<ResolvedThread> {
    match uri.provider {
        ProviderKind::Amp => AmpProvider::new(&roots.amp_root).resolve(&uri.session_id),
        ProviderKind::Codex => CodexProvider::new(&roots.codex_root).resolve(&uri.session_id),
        ProviderKind::Claude => ClaudeProvider::new(&roots.claude_root).resolve(&uri.session_id),
        ProviderKind::Gemini => GeminiProvider::new(&roots.gemini_root).resolve(&uri.session_id),
        ProviderKind::Opencode => {
            OpencodeProvider::new(&roots.opencode_root).resolve(&uri.session_id)
        }
    }
}

pub fn read_thread_raw(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| TurlError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    if bytes.is_empty() {
        return Err(TurlError::EmptyThreadFile {
            path: path.to_path_buf(),
        });
    }

    String::from_utf8(bytes).map_err(|_| TurlError::NonUtf8ThreadFile {
        path: path.to_path_buf(),
    })
}

pub fn render_thread_markdown(uri: &ThreadUri, resolved: &ResolvedThread) -> Result<String> {
    let raw = read_thread_raw(&resolved.path)?;
    render::render_markdown(uri, &resolved.path, &raw)
}

pub fn resolve_subagent_view(
    uri: &ThreadUri,
    roots: &ProviderRoots,
    list: bool,
) -> Result<SubagentView> {
    if list && uri.agent_id.is_some() {
        return Err(TurlError::InvalidMode(
            "--list cannot be used with <provider>://<main_thread_id>/<agent_id>".to_string(),
        ));
    }

    if !list && uri.agent_id.is_none() {
        return Err(TurlError::InvalidMode(
            "subagent drill-down requires <provider>://<main_thread_id>/<agent_id>".to_string(),
        ));
    }

    match uri.provider {
        ProviderKind::Codex => resolve_codex_subagent_view(uri, roots, list),
        ProviderKind::Claude => resolve_claude_subagent_view(uri, roots, list),
        _ => Err(TurlError::UnsupportedSubagentProvider(
            uri.provider.to_string(),
        )),
    }
}

pub fn subagent_view_to_raw_json(view: &SubagentView) -> Result<String> {
    serde_json::to_string_pretty(view).map_err(|err| TurlError::Serialization(err.to_string()))
}

pub fn render_subagent_view_markdown(view: &SubagentView) -> String {
    match view {
        SubagentView::List(list_view) => render_subagent_list_markdown(list_view),
        SubagentView::Detail(detail_view) => render_subagent_detail_markdown(detail_view),
    }
}

fn resolve_codex_subagent_view(
    uri: &ThreadUri,
    roots: &ProviderRoots,
    list: bool,
) -> Result<SubagentView> {
    let main_uri = main_thread_uri(uri);
    let resolved_main = resolve_thread(&main_uri, roots)?;
    let main_raw = read_thread_raw(&resolved_main.path)?;

    let mut warnings = resolved_main.metadata.warnings.clone();
    let mut timelines = BTreeMap::<String, AgentTimeline>::new();
    warnings.extend(parse_codex_parent_lifecycle(&main_raw, &mut timelines));

    if list {
        return Ok(SubagentView::List(build_codex_list_view(
            uri, roots, &timelines, warnings,
        )));
    }

    let agent_id = uri
        .agent_id
        .clone()
        .ok_or_else(|| TurlError::InvalidMode("missing agent id".to_string()))?;

    Ok(SubagentView::Detail(build_codex_detail_view(
        uri, roots, &agent_id, &timelines, warnings,
    )))
}

fn build_codex_list_view(
    uri: &ThreadUri,
    roots: &ProviderRoots,
    timelines: &BTreeMap<String, AgentTimeline>,
    warnings: Vec<String>,
) -> SubagentListView {
    let mut agents = Vec::new();

    for (agent_id, timeline) in timelines {
        let mut relation = SubagentRelation::default();
        if timeline.has_spawn {
            relation.validated = true;
            relation
                .evidence
                .push("parent rollout contains spawn_agent output".to_string());
        }

        let mut child_ref = None;
        let mut last_update = timeline.last_update.clone();
        if let Some((thread_ref, relation_evidence, thread_last_update)) =
            resolve_codex_child_thread(agent_id, &uri.session_id, roots)
        {
            if !relation_evidence.is_empty() {
                relation.validated = true;
                relation.evidence.extend(relation_evidence);
            }
            if last_update.is_none() {
                last_update = thread_last_update;
            }
            child_ref = Some(thread_ref);
        }

        let (status, status_source) = infer_status_from_timeline(timeline, child_ref.is_some());

        agents.push(SubagentListItem {
            agent_id: agent_id.clone(),
            status,
            status_source,
            last_update,
            relation,
            child_thread: child_ref,
        });
    }

    SubagentListView {
        query: make_query(uri, None, true),
        agents,
        warnings,
    }
}

fn build_codex_detail_view(
    uri: &ThreadUri,
    roots: &ProviderRoots,
    agent_id: &str,
    timelines: &BTreeMap<String, AgentTimeline>,
    mut warnings: Vec<String>,
) -> SubagentDetailView {
    let timeline = timelines.get(agent_id).cloned().unwrap_or_default();
    let mut relation = SubagentRelation::default();
    if timeline.has_spawn {
        relation.validated = true;
        relation
            .evidence
            .push("parent rollout contains spawn_agent output".to_string());
    }

    let mut child_thread = None;
    let mut excerpt = Vec::new();
    let mut child_status = None;

    if let Some((resolved_child, relation_evidence, thread_ref)) =
        resolve_codex_child_resolved(agent_id, &uri.session_id, roots)
    {
        if !relation_evidence.is_empty() {
            relation.validated = true;
            relation.evidence.extend(relation_evidence);
        }

        match read_thread_raw(&resolved_child.path) {
            Ok(child_raw) => {
                if let Some(inferred) = infer_codex_child_status(&child_raw, &resolved_child.path) {
                    child_status = Some(inferred);
                }

                if let Ok(messages) =
                    render::extract_messages(ProviderKind::Codex, &resolved_child.path, &child_raw)
                {
                    excerpt = messages
                        .into_iter()
                        .rev()
                        .take(3)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .map(|message| SubagentExcerptMessage {
                            role: message.role,
                            text: message.text,
                        })
                        .collect();
                }
            }
            Err(err) => warnings.push(format!(
                "failed reading child thread for agent_id={agent_id}: {err}"
            )),
        }

        child_thread = Some(thread_ref);
    }

    let (status, status_source) =
        infer_status_for_detail(&timeline, child_status, child_thread.is_some());

    SubagentDetailView {
        query: make_query(uri, Some(agent_id.to_string()), false),
        relation,
        lifecycle: timeline.events,
        status,
        status_source,
        child_thread,
        excerpt,
        warnings,
    }
}

fn resolve_codex_child_thread(
    agent_id: &str,
    main_thread_id: &str,
    roots: &ProviderRoots,
) -> Option<(SubagentThreadRef, Vec<String>, Option<String>)> {
    let resolved = CodexProvider::new(&roots.codex_root)
        .resolve(agent_id)
        .ok()?;
    let raw = read_thread_raw(&resolved.path).ok()?;

    let mut evidence = Vec::new();
    if extract_codex_parent_thread_id(&raw)
        .as_deref()
        .is_some_and(|parent| parent == main_thread_id)
    {
        evidence.push("child session_meta points to main thread".to_string());
    }

    let last_update = extract_last_timestamp(&raw);
    let thread_ref = SubagentThreadRef {
        thread_id: agent_id.to_string(),
        path: Some(resolved.path.display().to_string()),
        last_updated_at: last_update.clone(),
    };

    Some((thread_ref, evidence, last_update))
}

fn resolve_codex_child_resolved(
    agent_id: &str,
    main_thread_id: &str,
    roots: &ProviderRoots,
) -> Option<(ResolvedThread, Vec<String>, SubagentThreadRef)> {
    let resolved = CodexProvider::new(&roots.codex_root)
        .resolve(agent_id)
        .ok()?;
    let raw = read_thread_raw(&resolved.path).ok()?;

    let mut evidence = Vec::new();
    if extract_codex_parent_thread_id(&raw)
        .as_deref()
        .is_some_and(|parent| parent == main_thread_id)
    {
        evidence.push("child session_meta points to main thread".to_string());
    }

    let thread_ref = SubagentThreadRef {
        thread_id: agent_id.to_string(),
        path: Some(resolved.path.display().to_string()),
        last_updated_at: extract_last_timestamp(&raw),
    };

    Some((resolved, evidence, thread_ref))
}

fn infer_codex_child_status(raw: &str, path: &Path) -> Option<String> {
    let mut has_assistant_message = false;
    let mut has_error = false;

    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if value.get("type").and_then(Value::as_str) == Some("event_msg") {
            let payload_type = value
                .get("payload")
                .and_then(|payload| payload.get("type"))
                .and_then(Value::as_str);
            if payload_type == Some("turn_aborted") {
                has_error = true;
            }
        }

        if render::extract_messages(ProviderKind::Codex, path, line)
            .ok()
            .is_some_and(|messages| {
                messages
                    .iter()
                    .any(|message| matches!(message.role, crate::model::MessageRole::Assistant))
            })
        {
            has_assistant_message = true;
        }
    }

    if has_error {
        Some(STATUS_ERRORED.to_string())
    } else if has_assistant_message {
        Some(STATUS_COMPLETED.to_string())
    } else {
        None
    }
}

fn parse_codex_parent_lifecycle(
    raw: &str,
    timelines: &mut BTreeMap<String, AgentTimeline>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut calls: HashMap<String, (String, Value, Option<String>)> = HashMap::new();

    for (line_idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value = match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => value,
            Err(err) => {
                warnings.push(format!(
                    "failed to parse parent rollout line {}: {err}",
                    line_idx + 1
                ));
                continue;
            }
        };

        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }

        let Some(payload) = value.get("payload") else {
            continue;
        };
        let Some(payload_type) = payload.get("type").and_then(Value::as_str) else {
            continue;
        };

        if payload_type == "function_call" {
            let call_id = payload
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if call_id.is_empty() {
                continue;
            }

            let name = payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if name.is_empty() {
                continue;
            }

            let args = payload
                .get("arguments")
                .and_then(Value::as_str)
                .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
                .unwrap_or_else(|| Value::Object(Default::default()));

            let timestamp = value
                .get("timestamp")
                .and_then(Value::as_str)
                .map(ToString::to_string);

            calls.insert(call_id, (name, args, timestamp));
            continue;
        }

        if payload_type != "function_call_output" {
            continue;
        }

        let Some(call_id) = payload.get("call_id").and_then(Value::as_str) else {
            continue;
        };

        let Some((name, args, timestamp)) = calls.remove(call_id) else {
            continue;
        };

        let output_raw = payload
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let output_value =
            serde_json::from_str::<Value>(&output_raw).unwrap_or(Value::String(output_raw));

        match name.as_str() {
            "spawn_agent" => {
                let Some(agent_id) = output_value
                    .get("agent_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                else {
                    warnings.push(
                        "spawn_agent output did not include agent_id; skipping subagent mapping"
                            .to_string(),
                    );
                    continue;
                };

                let timeline = timelines.entry(agent_id).or_default();
                timeline.has_spawn = true;
                timeline.has_activity = true;
                timeline.last_update = timestamp.clone();
                timeline.events.push(SubagentLifecycleEvent {
                    timestamp,
                    event: "spawn_agent".to_string(),
                    detail: "subagent spawned".to_string(),
                });
            }
            "wait" => {
                let ids = args
                    .get("ids")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();

                let timed_out = output_value
                    .get("timed_out")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);

                for agent_id in ids {
                    let timeline = timelines.entry(agent_id).or_default();
                    timeline.has_activity = true;
                    timeline.last_update = timestamp.clone();

                    let mut detail = if timed_out {
                        "wait timed out".to_string()
                    } else {
                        "wait returned".to_string()
                    };

                    if let Some(state) = infer_state_from_status_payload(&output_value) {
                        timeline.states.push(state.clone());
                        detail = format!("wait state={state}");
                    } else if timed_out {
                        timeline.states.push(STATUS_RUNNING.to_string());
                    }

                    timeline.events.push(SubagentLifecycleEvent {
                        timestamp: timestamp.clone(),
                        event: "wait".to_string(),
                        detail,
                    });
                }
            }
            "send_input" | "resume_agent" | "close_agent" => {
                let Some(agent_id) = args
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                else {
                    continue;
                };

                let timeline = timelines.entry(agent_id).or_default();
                timeline.has_activity = true;
                timeline.last_update = timestamp.clone();

                if name == "close_agent" {
                    if let Some(state) = infer_state_from_status_payload(&output_value) {
                        timeline.states.push(state.clone());
                    } else {
                        timeline.states.push(STATUS_SHUTDOWN.to_string());
                    }
                }

                timeline.events.push(SubagentLifecycleEvent {
                    timestamp,
                    event: name,
                    detail: "agent lifecycle event".to_string(),
                });
            }
            _ => {}
        }
    }

    warnings
}

fn infer_state_from_status_payload(payload: &Value) -> Option<String> {
    let status = payload.get("status")?;

    if let Some(object) = status.as_object() {
        for key in object.keys() {
            if [
                STATUS_PENDING_INIT,
                STATUS_RUNNING,
                STATUS_COMPLETED,
                STATUS_ERRORED,
                STATUS_SHUTDOWN,
                STATUS_NOT_FOUND,
            ]
            .contains(&key.as_str())
            {
                return Some(key.clone());
            }
        }

        if object.contains_key("completed") {
            return Some(STATUS_COMPLETED.to_string());
        }
    }

    None
}

fn infer_status_from_timeline(timeline: &AgentTimeline, child_exists: bool) -> (String, String) {
    if timeline.states.iter().any(|state| state == STATUS_ERRORED) {
        return (STATUS_ERRORED.to_string(), "parent_rollout".to_string());
    }
    if timeline.states.iter().any(|state| state == STATUS_SHUTDOWN) {
        return (STATUS_SHUTDOWN.to_string(), "parent_rollout".to_string());
    }
    if timeline
        .states
        .iter()
        .any(|state| state == STATUS_COMPLETED)
    {
        return (STATUS_COMPLETED.to_string(), "parent_rollout".to_string());
    }
    if timeline.states.iter().any(|state| state == STATUS_RUNNING) || timeline.has_activity {
        return (STATUS_RUNNING.to_string(), "parent_rollout".to_string());
    }
    if timeline.has_spawn {
        return (
            STATUS_PENDING_INIT.to_string(),
            "parent_rollout".to_string(),
        );
    }
    if child_exists {
        return (STATUS_RUNNING.to_string(), "child_rollout".to_string());
    }

    (STATUS_NOT_FOUND.to_string(), "inferred".to_string())
}

fn infer_status_for_detail(
    timeline: &AgentTimeline,
    child_status: Option<String>,
    child_exists: bool,
) -> (String, String) {
    let (status, source) = infer_status_from_timeline(timeline, child_exists);
    if status == STATUS_NOT_FOUND
        && let Some(child_status) = child_status
    {
        return (child_status, "child_rollout".to_string());
    }

    (status, source)
}

fn extract_codex_parent_thread_id(raw: &str) -> Option<String> {
    let first = raw.lines().find(|line| !line.trim().is_empty())?;
    let value = serde_json::from_str::<Value>(first).ok()?;

    value
        .get("payload")
        .and_then(|payload| payload.get("source"))
        .and_then(|source| source.get("subagent"))
        .and_then(|subagent| subagent.get("thread_spawn"))
        .and_then(|thread_spawn| thread_spawn.get("parent_thread_id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn resolve_claude_subagent_view(
    uri: &ThreadUri,
    roots: &ProviderRoots,
    list: bool,
) -> Result<SubagentView> {
    let main_uri = main_thread_uri(uri);
    let resolved_main = resolve_thread(&main_uri, roots)?;

    let mut warnings = resolved_main.metadata.warnings.clone();
    let records = discover_claude_agents(&resolved_main, &uri.session_id, &mut warnings);

    if list {
        return Ok(SubagentView::List(SubagentListView {
            query: make_query(uri, None, true),
            agents: records
                .iter()
                .map(|record| SubagentListItem {
                    agent_id: record.agent_id.clone(),
                    status: record.status.clone(),
                    status_source: "inferred".to_string(),
                    last_update: record.last_update.clone(),
                    relation: record.relation.clone(),
                    child_thread: Some(SubagentThreadRef {
                        thread_id: record.agent_id.clone(),
                        path: Some(record.path.display().to_string()),
                        last_updated_at: record.last_update.clone(),
                    }),
                })
                .collect(),
            warnings,
        }));
    }

    let requested_agent = uri
        .agent_id
        .clone()
        .ok_or_else(|| TurlError::InvalidMode("missing agent id".to_string()))?;

    let normalized_requested = normalize_agent_id(&requested_agent);

    if let Some(record) = records
        .into_iter()
        .find(|record| normalize_agent_id(&record.agent_id) == normalized_requested)
    {
        let lifecycle = vec![SubagentLifecycleEvent {
            timestamp: record.last_update.clone(),
            event: "discovered_agent_file".to_string(),
            detail: "agent transcript discovered and analyzed".to_string(),
        }];

        warnings.extend(record.warnings.clone());

        return Ok(SubagentView::Detail(SubagentDetailView {
            query: make_query(uri, Some(requested_agent), false),
            relation: record.relation.clone(),
            lifecycle,
            status: record.status.clone(),
            status_source: "inferred".to_string(),
            child_thread: Some(SubagentThreadRef {
                thread_id: record.agent_id.clone(),
                path: Some(record.path.display().to_string()),
                last_updated_at: record.last_update.clone(),
            }),
            excerpt: record.excerpt,
            warnings,
        }));
    }

    warnings.push(format!(
        "agent not found for main_session_id={} agent_id={requested_agent}",
        uri.session_id
    ));

    Ok(SubagentView::Detail(SubagentDetailView {
        query: make_query(uri, Some(requested_agent), false),
        relation: SubagentRelation::default(),
        lifecycle: Vec::new(),
        status: STATUS_NOT_FOUND.to_string(),
        status_source: "inferred".to_string(),
        child_thread: None,
        excerpt: Vec::new(),
        warnings,
    }))
}

fn discover_claude_agents(
    resolved_main: &ResolvedThread,
    main_session_id: &str,
    warnings: &mut Vec<String>,
) -> Vec<ClaudeAgentRecord> {
    let Some(project_dir) = resolved_main.path.parent() else {
        warnings.push(format!(
            "cannot determine project directory from resolved main thread path: {}",
            resolved_main.path.display()
        ));
        return Vec::new();
    };

    let mut candidate_files = BTreeSet::new();

    let nested_subagent_dir = project_dir.join(main_session_id).join("subagents");
    if nested_subagent_dir.exists()
        && let Ok(entries) = fs::read_dir(&nested_subagent_dir)
    {
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if is_claude_agent_filename(&path) {
                candidate_files.insert(path);
            }
        }
    }

    if let Ok(entries) = fs::read_dir(project_dir) {
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if is_claude_agent_filename(&path) {
                candidate_files.insert(path);
            }
        }
    }

    let mut latest_by_agent = BTreeMap::<String, ClaudeAgentRecord>::new();

    for path in candidate_files {
        let Some(record) = analyze_claude_agent_file(&path, main_session_id, warnings) else {
            continue;
        };

        match latest_by_agent.get(&record.agent_id) {
            Some(existing) => {
                let new_stamp = file_modified_epoch(&record.path).unwrap_or(0);
                let old_stamp = file_modified_epoch(&existing.path).unwrap_or(0);
                if new_stamp > old_stamp {
                    latest_by_agent.insert(record.agent_id.clone(), record);
                }
            }
            None => {
                latest_by_agent.insert(record.agent_id.clone(), record);
            }
        }
    }

    latest_by_agent.into_values().collect()
}

fn analyze_claude_agent_file(
    path: &Path,
    main_session_id: &str,
    warnings: &mut Vec<String>,
) -> Option<ClaudeAgentRecord> {
    let raw = match read_thread_raw(path) {
        Ok(raw) => raw,
        Err(err) => {
            warnings.push(format!(
                "failed to read Claude agent transcript {}: {err}",
                path.display()
            ));
            return None;
        }
    };

    let mut agent_id = None::<String>;
    let mut is_sidechain = false;
    let mut session_matches = false;
    let mut has_error = false;
    let mut has_assistant = false;
    let mut has_user = false;
    let mut last_update = None::<String>;

    for (line_idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let value = match serde_json::from_str::<Value>(line) {
            Ok(value) => value,
            Err(err) => {
                warnings.push(format!(
                    "failed to parse Claude agent transcript line {} in {}: {err}",
                    line_idx + 1,
                    path.display()
                ));
                continue;
            }
        };

        if line_idx == 0 {
            agent_id = value
                .get("agentId")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            is_sidechain = value
                .get("isSidechain")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            session_matches = value
                .get("sessionId")
                .and_then(Value::as_str)
                .is_some_and(|session_id| session_id == main_session_id);
        }

        if let Some(timestamp) = value
            .get("timestamp")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            last_update = Some(timestamp);
        }

        if value
            .get("isApiErrorMessage")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || !value.get("error").is_none_or(Value::is_null)
        {
            has_error = true;
        }

        if let Some(kind) = value.get("type").and_then(Value::as_str) {
            if kind == "assistant" {
                has_assistant = true;
            }
            if kind == "user" {
                has_user = true;
            }
        }
    }

    if !is_sidechain || !session_matches {
        return None;
    }

    let Some(agent_id) = agent_id else {
        warnings.push(format!(
            "missing agentId in Claude sidechain transcript: {}",
            path.display()
        ));
        return None;
    };

    let status = if has_error {
        STATUS_ERRORED.to_string()
    } else if has_assistant {
        STATUS_COMPLETED.to_string()
    } else if has_user {
        STATUS_RUNNING.to_string()
    } else {
        STATUS_PENDING_INIT.to_string()
    };

    let excerpt = render::extract_messages(ProviderKind::Claude, path, &raw)
        .map(|messages| {
            messages
                .into_iter()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|message| SubagentExcerptMessage {
                    role: message.role,
                    text: message.text,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut relation = SubagentRelation {
        validated: true,
        ..SubagentRelation::default()
    };
    relation
        .evidence
        .push("agent transcript is sidechain and sessionId matches main thread".to_string());

    Some(ClaudeAgentRecord {
        agent_id,
        path: path.to_path_buf(),
        status,
        last_update: last_update.or_else(|| modified_timestamp_string(path)),
        relation,
        excerpt,
        warnings: Vec::new(),
    })
}

fn is_claude_agent_filename(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "jsonl")
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("agent-"))
}

fn file_modified_epoch(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

fn modified_timestamp_string(path: &Path) -> Option<String> {
    file_modified_epoch(path).map(|stamp| stamp.to_string())
}

fn normalize_agent_id(agent_id: &str) -> String {
    agent_id
        .strip_prefix("agent-")
        .unwrap_or(agent_id)
        .to_string()
}

fn extract_last_timestamp(raw: &str) -> Option<String> {
    for line in raw.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(timestamp) = value
            .get("timestamp")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            return Some(timestamp);
        }
    }

    None
}

fn main_thread_uri(uri: &ThreadUri) -> ThreadUri {
    ThreadUri {
        provider: uri.provider,
        session_id: uri.session_id.clone(),
        agent_id: None,
    }
}

fn make_query(uri: &ThreadUri, agent_id: Option<String>, list: bool) -> SubagentQuery {
    SubagentQuery {
        provider: uri.provider.to_string(),
        main_thread_id: uri.session_id.clone(),
        agent_id,
        list,
    }
}

fn render_subagent_list_markdown(view: &SubagentListView) -> String {
    let mut output = String::new();
    output.push_str("# Subagent Status\n\n");
    output.push_str(&format!("- Provider: `{}`\n", view.query.provider));
    output.push_str(&format!("- Main Thread: `{}`\n", view.query.main_thread_id));
    output.push_str("- Mode: `list`\n\n");

    if view.agents.is_empty() {
        output.push_str("_No subagents found for this thread._\n");
        return output;
    }

    for (index, agent) in view.agents.iter().enumerate() {
        output.push_str(&format!("## {}. `{}`\n\n", index + 1, agent.agent_id));
        output.push_str(&format!(
            "- Status: `{}` (`{}`)\n",
            agent.status, agent.status_source
        ));
        output.push_str(&format!(
            "- Last Update: `{}`\n",
            agent.last_update.as_deref().unwrap_or("unknown")
        ));
        output.push_str(&format!(
            "- Relation: `{}`\n",
            if agent.relation.validated {
                "validated"
            } else {
                "inferred"
            }
        ));
        if let Some(thread) = &agent.child_thread
            && let Some(path) = &thread.path
        {
            output.push_str(&format!("- Thread Path: `{}`\n", path));
        }
        output.push('\n');
    }

    output
}

fn render_subagent_detail_markdown(view: &SubagentDetailView) -> String {
    let mut output = String::new();
    output.push_str("# Subagent Thread\n\n");
    output.push_str(&format!("- Provider: `{}`\n", view.query.provider));
    output.push_str(&format!("- Main Thread: `{}`\n", view.query.main_thread_id));
    if let Some(agent_id) = &view.query.agent_id {
        output.push_str(&format!("- Agent: `{}`\n", agent_id));
    }
    output.push_str(&format!(
        "- Status: `{}` (`{}`)\n\n",
        view.status, view.status_source
    ));

    output.push_str("## Agent Status Summary\n\n");
    output.push_str(&format!(
        "- Relation: `{}`\n",
        if view.relation.validated {
            "validated"
        } else {
            "inferred"
        }
    ));
    for evidence in &view.relation.evidence {
        output.push_str(&format!("- Evidence: {}\n", evidence));
    }
    if let Some(thread) = &view.child_thread {
        if let Some(path) = &thread.path {
            output.push_str(&format!("- Child Path: `{}`\n", path));
        }
        if let Some(last_updated_at) = &thread.last_updated_at {
            output.push_str(&format!("- Child Last Update: `{}`\n", last_updated_at));
        }
    }
    output.push('\n');

    output.push_str("## Lifecycle (Parent Thread)\n\n");
    if view.lifecycle.is_empty() {
        output.push_str("_No lifecycle events found in parent thread._\n\n");
    } else {
        for event in &view.lifecycle {
            output.push_str(&format!(
                "- `{}` `{}` {}\n",
                event.timestamp.as_deref().unwrap_or("unknown"),
                event.event,
                event.detail
            ));
        }
        output.push('\n');
    }

    output.push_str("## Thread Excerpt (Child Thread)\n\n");
    if view.excerpt.is_empty() {
        output.push_str("_No child thread messages found._\n\n");
    } else {
        for (index, message) in view.excerpt.iter().enumerate() {
            let title = match message.role {
                crate::model::MessageRole::User => "User",
                crate::model::MessageRole::Assistant => "Assistant",
            };
            output.push_str(&format!("### {}. {}\n\n", index + 1, title));
            output.push_str(message.text.trim());
            output.push_str("\n\n");
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::service::{extract_last_timestamp, read_thread_raw};

    #[test]
    fn empty_file_returns_error() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("thread.jsonl");
        fs::write(&path, "").expect("write");

        let err = read_thread_raw(&path).expect_err("must fail");
        assert!(format!("{err}").contains("thread file is empty"));
    }

    #[test]
    fn extract_last_timestamp_from_jsonl() {
        let raw =
            "{\"timestamp\":\"2026-02-23T00:00:01Z\"}\n{\"timestamp\":\"2026-02-23T00:00:02Z\"}\n";
        let timestamp = extract_last_timestamp(raw).expect("must extract timestamp");
        assert_eq!(timestamp, "2026-02-23T00:00:02Z");
    }
}
