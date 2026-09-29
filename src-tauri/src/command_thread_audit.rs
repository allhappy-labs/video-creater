//! Source audit of Tauri command threading in `main.rs`.
//!
//! Synchronous Tauri commands run on the main thread, which on Linux iterates the GLib main
//! context GTK and WebKitGTK share. A render holds the storage lease and the project's artifact
//! lease for its whole duration, and the project mutation lease for each of its writes, so a
//! synchronous command that waits on any of them, or reads or writes the project, freezes the
//! webview until that hold ends. This audit fails when a synchronous `#[tauri::command]` reaches
//! such a call, directly or through same-file helpers.

use std::collections::{BTreeMap, BTreeSet};

/// Calls that take a lease or read or write canonical project state.
const LEASE_OR_PROJECT_IO_TOKENS: &[&str] = &[
    // Leases.
    "acquire_split_project_mutation_lease",
    "acquire_split_project_artifact_lease",
    "acquire_storage_mutation_lease",
    "mutation_coordinator",
    // Project reads and writes.
    "load_split_project",
    "save_split_project",
    "apply_project_action_to_split_project",
    "apply_project_actions_to_split_project",
    "replace_split_project_if_revision",
    "update_project_settings_in_split_project",
    "migrate_single_file_project",
    "validate_split_project",
    // Imports and mattes.
    "import_media_files",
    "create_matte",
    // Agent sessions and history.
    "apply_agent_session_action",
    "load_agent_session_manifest",
    "load_app_server_conversation_history",
    // Speakers.
    "rename_speaker",
    "recolor_speaker",
    "assign_media_speaker",
    // Search and frame caches.
    "query_project_search_for_project_dir",
    "rebuild_project_search_index_for_project_dir",
    "cache_visual_frames",
    "caption_cached_visual_frames_with_fal",
    // Codex turns.
    "codex_project_for_turn",
];

/// Synchronous commands that may stay on the main thread, with the reason.
const SYNC_COMMAND_ALLOWLIST: &[(&str, &str)] = &[
    (
        "get_project_speaker_registry",
        "load_speaker_registry reads the registry file without a lease",
    ),
    (
        "load_render_pipeline_report_from_split_project_folder",
        "reads renders/<job>/pipeline-report.json without a lease",
    ),
    (
        "list_shader_background_templates",
        "reads bundled templates and takes no lease",
    ),
    ("get_storage_health", "inventory walks take no lease"),
    ("refresh_storage_inventory", "inventory walks take no lease"),
];

const HELPER_WALK_DEPTH: usize = 3;

#[derive(Debug, PartialEq, Eq)]
struct Finding {
    command: String,
    token: &'static str,
    via: Vec<String>,
}

struct SourceItem {
    name: String,
    is_command: bool,
    is_async: bool,
    body: String,
}

const ITEM_PREFIXES: &[&str] = &[
    "fn ",
    "async fn ",
    "pub fn ",
    "pub async fn ",
    "pub(crate) fn ",
    "#[",
    "#![",
    "struct ",
    "pub struct ",
    "pub(crate) struct ",
    "enum ",
    "pub enum ",
    "impl",
    "const ",
    "static ",
    "mod ",
    "use ",
    "pub use ",
    "type ",
    "trait ",
    "macro_rules!",
    "thread_local!",
];

fn function_header(line: &str) -> Option<(&str, bool)> {
    let (rest, is_async) = match line
        .strip_prefix("async fn ")
        .or_else(|| line.strip_prefix("pub async fn "))
    {
        Some(rest) => (rest, true),
        None => (
            line.strip_prefix("fn ")
                .or_else(|| line.strip_prefix("pub fn "))
                .or_else(|| line.strip_prefix("pub(crate) fn "))?,
            false,
        ),
    };
    let end = rest
        .find(|character: char| !(character.is_alphanumeric() || character == '_'))
        .unwrap_or(rest.len());
    Some((&rest[..end], is_async))
}

fn is_item_header(line: &str) -> bool {
    !line.starts_with("#[") && ITEM_PREFIXES.iter().any(|prefix| line.starts_with(prefix))
}

/// Splits non-test source into top-level items. Attribute lines join the item they precede.
fn parse_items(source: &str) -> Vec<SourceItem> {
    let source = source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap_or(source);
    let mut chunks: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut current_has_header = false;
    for line in source.lines() {
        let starts_item = ITEM_PREFIXES.iter().any(|prefix| line.starts_with(prefix));
        if starts_item && current_has_header {
            chunks.push(std::mem::take(&mut current));
            current_has_header = false;
        }
        if is_item_header(line) {
            current_has_header = true;
        }
        current.push(line);
    }
    chunks.push(current);

    chunks
        .into_iter()
        .filter_map(|chunk| {
            let (name, is_async) = chunk.iter().find_map(|line| function_header(line))?;
            Some(SourceItem {
                name: name.to_string(),
                is_command: chunk.iter().any(|line| line.trim() == "#[tauri::command]"),
                is_async,
                body: chunk.join("\n"),
            })
        })
        .collect()
}

/// Identifiers in `body` that are not method calls or paths into other modules.
fn free_identifiers(body: &str) -> BTreeSet<&str> {
    let bytes = body.as_bytes();
    let mut identifiers = BTreeSet::new();
    let mut index = 0;
    while index < bytes.len() {
        let character = bytes[index];
        if character.is_ascii_alphabetic() || character == b'_' {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let preceded_by_method_dot = start > 0 && bytes[start - 1] == b'.';
            if !preceded_by_method_dot {
                identifiers.insert(&body[start..index]);
            }
        } else {
            index += 1;
        }
    }
    identifiers
}

fn audit_command_threading(source: &str) -> Vec<Finding> {
    let items = parse_items(source);
    let mut helpers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for item in items.iter().filter(|item| !item.is_command) {
        helpers.entry(&item.name).or_default().push(&item.body);
    }
    let allowlist: BTreeSet<&str> = SYNC_COMMAND_ALLOWLIST
        .iter()
        .map(|(name, _)| *name)
        .collect();

    let mut findings = Vec::new();
    for command in items
        .iter()
        .filter(|item| item.is_command && !item.is_async && !allowlist.contains(item.name.as_str()))
    {
        let mut visited = BTreeSet::new();
        let mut frontier: Vec<(Vec<String>, &str)> = vec![(Vec::new(), command.body.as_str())];
        'walk: for depth in 0..=HELPER_WALK_DEPTH {
            let mut next = Vec::new();
            for (via, body) in frontier {
                let identifiers = free_identifiers(body);
                if let Some(token) = LEASE_OR_PROJECT_IO_TOKENS
                    .iter()
                    .find(|token| identifiers.contains(**token))
                {
                    findings.push(Finding {
                        command: command.name.clone(),
                        token,
                        via,
                    });
                    break 'walk;
                }
                if depth == HELPER_WALK_DEPTH {
                    continue;
                }
                for identifier in identifiers {
                    if identifier == command.name || !visited.insert(identifier) {
                        continue;
                    }
                    for helper_body in helpers.get(identifier).into_iter().flatten() {
                        let mut path = via.clone();
                        path.push(identifier.to_string());
                        next.push((path, *helper_body));
                    }
                }
            }
            frontier = next;
        }
    }
    findings
}

#[test]
fn synchronous_tauri_commands_do_not_reach_leases_or_project_io() {
    let findings = audit_command_threading(include_str!("main.rs"));
    let report = findings
        .iter()
        .map(|finding| {
            format!(
                "{} -> {} (via {})",
                finding.command,
                finding.token,
                if finding.via.is_empty() {
                    "its own body".to_string()
                } else {
                    finding.via.join(" -> ")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        findings.is_empty(),
        "synchronous Tauri commands that must move off the main thread:\n{report}"
    );
}

#[test]
fn allowlisted_commands_are_still_synchronous_commands() {
    let items = parse_items(include_str!("main.rs"));
    for (name, reason) in SYNC_COMMAND_ALLOWLIST {
        assert!(!reason.is_empty());
        assert!(
            items
                .iter()
                .any(|item| item.name == *name && item.is_command && !item.is_async),
            "allowlisted command `{name}` is no longer a synchronous command; drop it"
        );
    }
}

#[test]
fn audit_flags_a_synthetic_synchronous_command_that_loads_the_project() {
    let source = r#"
#[tauri::command]
fn direct_load(project_dir: String) -> Result<(), String> {
    load_split_project(&project_dir).map(drop)
}

#[tauri::command]
#[expect(clippy::too_many_arguments, reason = "fixture")]
fn helper_load(project_dir: String) -> Result<(), String> {
    resolve_and_load(&project_dir)
}

fn resolve_and_load(project_dir: &str) -> Result<(), String> {
    let project = load_split_project(project_dir)?;
    Ok(())
}

#[tauri::command]
async fn async_load(project_dir: String) -> Result<(), String> {
    run_blocking_command("load", move || resolve_and_load(&project_dir)).await
}

#[tauri::command]
fn method_named_like_a_helper(state: State) -> bool {
    state.resolve_and_load()
}

#[cfg(test)]
mod tests {
    #[tauri::command]
    fn test_only(project_dir: String) {
        load_split_project(&project_dir);
    }
}
"#;

    let findings = audit_command_threading(source);

    assert_eq!(
        findings,
        vec![
            Finding {
                command: "direct_load".to_string(),
                token: "load_split_project",
                via: Vec::new(),
            },
            Finding {
                command: "helper_load".to_string(),
                token: "load_split_project",
                via: vec!["resolve_and_load".to_string()],
            },
        ]
    );
}
