import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const repoRoot = process.cwd();
const inventoryPath = join(repoRoot, "src-tauri/src/app_service/operation.rs");
const webHostRegistryPath = join(repoRoot, "src-tauri/src/web_host/registry.rs");

function productionTauriHandlers(): string[] {
  const source = readFileSync(join(repoRoot, "src-tauri/src/main.rs"), "utf8");
  const marker = ".invoke_handler(tauri::generate_handler![";
  const start = source.indexOf(marker);
  assert.notEqual(start, -1, "production Tauri handler registry is missing");
  const end = source.indexOf("        ])\n        .run(", start);
  assert.notEqual(end, -1, "production Tauri handler registry terminator is missing");
  const block = source.slice(start + marker.length, end);
  return [...block.matchAll(/^\s*(?:[a-z_][a-z0-9_]*::)?([a-z_][a-z0-9_]*),?\s*$/gim)]
    .map((match) => match[1])
    .sort();
}

function sourceFiles(root: string): string[] {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    if (!/\.(?:ts|tsx)$/.test(entry.name) || /\.(?:test|spec)\./.test(entry.name)) return [];
    return [path];
  });
}

function frontendRequestLiterals(): string[] {
  const operations = new Set<string>();
  const pattern = /backendRequest(?:<[^;]{0,600}?>)?\(\s*["']([^"']+)["']/gs;
  for (const path of sourceFiles(join(repoRoot, "src"))) {
    const source = readFileSync(path, "utf8");
    for (const match of source.matchAll(pattern)) operations.add(match[1]);
  }
  return [...operations].sort();
}

function inventoryEntries(): Map<string, string[]> {
  const source = existsSync(inventoryPath) ? readFileSync(inventoryPath, "utf8") : "";
  const entries = new Map<string, string[]>();
  const pattern = /operation!\(\s*"([^"]+)"\s*,\s*([^\n]+)\),/g;
  for (const match of source.matchAll(pattern)) {
    assert.equal(entries.has(match[1]), false, `duplicate operation inventory row: ${match[1]}`);
    entries.set(match[1], match[2].split(",").map((field) => field.trim()));
  }
  return entries;
}

function webHostOnlyOperations(): string[] {
  const source = readFileSync(webHostRegistryPath, "utf8");
  const block = source.match(/const WEB_HOST_OPERATION_INVENTORY:[\s\S]+?^\];/m)?.[0] ?? "";
  return [...block.matchAll(/name:\s*"([^"]+)"/g)].map((match) => match[1]).sort();
}

test("every production backend operation has one complete classification", () => {
  const handlers = productionTauriHandlers();
  const frontend = frontendRequestLiterals();
  const inventory = inventoryEntries();
  const webHostOnly = webHostOnlyOperations();
  const classified = new Set([...inventory.keys(), ...webHostOnly]);
  const required = [...new Set([...handlers, ...frontend])].sort();

  assert.deepEqual(
    required.filter((operation) => !classified.has(operation)),
    [],
    "unclassified production backend operations",
  );
  assert.deepEqual(
    webHostOnly.filter((operation) => handlers.includes(operation)),
    [],
    "web-host-only operations must not duplicate Tauri handlers",
  );
  assert.equal(new Set(webHostOnly).size, webHostOnly.length, "duplicate web-host-only operation");
  for (const operation of webHostOnly) assert.match(operation, /^remote_[a-z0-9_]+$/);
  assert.deepEqual(
    [...inventory.keys()].filter((operation) => !handlers.includes(operation)).sort(),
    [],
    "inventory rows must refer to production Tauri handlers",
  );
  for (const [operation, fields] of inventory) {
    assert.equal(fields.length, 7, `${operation} must classify support, auth, mutation, project, revision, request limit, and browser replacement`);
    assert.match(fields[0], /^(Remote|DesktopOnly|Internal|Removed)$/);
    assert.match(fields[1], /^(Public|Session|ProjectRead|ProjectWrite|HostAdmin)$/);
    assert.match(fields[2], /^(Read|ProjectMutation|HostMutation|JobMutation|ExternalMutation)$/);
    assert.match(fields[3], /^(true|false)$/);
    assert.match(fields[4], /^(true|false)$/);
    assert.match(fields[5], /^\d+$/);
    assert.match(fields[6], /^(None|Some\("[^"]+"\))$/);
  }
});

test("desktop-only operations name a browser replacement", () => {
  const inventory = inventoryEntries();
  const missing = [...inventory]
    .filter(([, fields]) => fields[0] === "DesktopOnly" && fields[6] === "None")
    .map(([operation]) => operation);
  assert.deepEqual(missing, []);
});
