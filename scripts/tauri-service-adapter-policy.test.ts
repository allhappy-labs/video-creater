import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync("src-tauri/src/main.rs", "utf8");

function functionBody(name: string): string {
  const start = source.search(new RegExp(`(?:async\\s+)?fn\\s+${name}(?:<[^>]+>)?\\s*\\(`));
  assert.notEqual(start, -1, `missing Tauri adapter ${name}`);
  const brace = source.indexOf("{", start);
  let depth = 0;
  for (let index = brace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) return source.slice(brace, index + 1);
  }
  throw new Error(`unterminated Tauri adapter ${name}`);
}

test("extracted project commands delegate through ProjectService", () => {
  const adapters = [
    "create_empty_project",
    "save_split_project_to_folder_impl",
    "load_split_project_from_folder_blocking",
    "validate_split_project_folder_blocking",
    "load_app_server_conversation_history_from_split_project_folder_blocking",
    "load_agent_sessions_from_split_project_folder_blocking",
    "apply_project_action_to_split_project_folder_blocking",
    "apply_project_actions_to_split_project_folder_blocking",
    "update_project_settings_in_split_project_folder_blocking",
  ];
  for (const adapter of adapters) {
    assert.match(functionBody(adapter), /desktop_project_service\(\)/, `${adapter} bypasses ProjectService`);
  }
});

test("project command adapters do not perform raw filesystem or JSON orchestration", () => {
  const adapters = [
    "save_split_project_to_folder_impl",
    "validate_split_project_folder_blocking",
    "load_app_server_conversation_history_from_split_project_folder_blocking",
    "load_agent_sessions_from_split_project_folder_blocking",
    "apply_project_action_to_split_project_folder_blocking",
    "apply_project_actions_to_split_project_folder_blocking",
    "update_project_settings_in_split_project_folder_blocking",
  ];
  for (const adapter of adapters) {
    const body = functionBody(adapter);
    assert.doesNotMatch(body, /\bfs::|serde_json::|std::process::Command/, `${adapter} contains orchestration`);
  }
});
