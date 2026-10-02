// @vitest-environment node
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { defaultRemoteDeadlines, remoteOperationBypassesQueue, remoteOperationClass, remoteOperationDeadlineClass } from "./remote-operation-policy";

// Rust's operation inventory owns mutation semantics. Request duration is independent.
const inventory = readFileSync(new URL("../../../../src-tauri/src/app_service/operation.rs", import.meta.url), "utf8");
const remoteOperations = [...inventory.matchAll(/operation!\("([^\"]+)",\s*Remote,\s*\w+,\s*(\w+),/g)]
  .map((match) => ({ operation: match[1]!, mutation: match[2]! }));
const remoteReads = remoteOperations.filter((entry) => entry.mutation === "Read");

it("reads a nonempty authoritative Rust remote operation inventory", () => {
  expect(remoteOperations.length).toBeGreaterThan(50);
  expect(remoteReads.length).toBeGreaterThan(20);
});

it.each(remoteReads)("keeps Rust Read $operation out of durable mutation semantics", ({ operation }) => {
  expect(remoteOperationClass(operation, {})).toBe("read");
});

it.each(remoteOperations.filter((entry) => entry.mutation !== "Read"))("keeps Rust $mutation $operation classified as a mutation", ({ operation }) => {
  expect(remoteOperationClass(operation, {})).not.toBe("read");
});

it("preserves synchronous render's long allowance and short durable admission classification", () => {
  expect(defaultRemoteDeadlines.job).toBe(30 * 60_000);
  expect(remoteOperationClass("render_media_to_split_project_folder", {})).toBe("job");
  expect(remoteOperationClass("render_media_to_split_project_folder", { admissionProtocol: 1 })).toBe("mutation");
});

it("preserves cancellation bypass and the existing narrow reader FIFO bypass", () => {
  expect(remoteOperationClass("cancel_render_job_in_split_project_folder", {})).toBe("cancellation");
  expect(remoteOperationBypassesQueue("cancel_render_job_in_split_project_folder")).toBe(true);
  for (const reader of ["load_job_progress_from_split_project_folder", "load_render_attempt_in_split_project_folder", "read_project_snapshot_from_split_project_folder"]) {
    expect(remoteOperationBypassesQueue(reader)).toBe(true);
  }
  for (const reader of ["capture_canonical_preview_frame_in_split_project_folder", "prepare_project_preview", "preview_storage_cleanup", "mcp_client_configuration"]) {
    expect(remoteOperationBypassesQueue(reader)).toBe(false);
  }
});

it.each(["capture_canonical_preview_frame_in_split_project_folder", "prepare_project_preview"])("keeps the long native deadline for read-only %s", (operation) => {
  expect(remoteOperationClass(operation, {})).toBe("read");
  expect(remoteOperationDeadlineClass(operation, {})).toBe("job");
  expect(defaultRemoteDeadlines[remoteOperationDeadlineClass(operation, {})]).toBe(30 * 60_000);
});

it("selects existing render, admission, ordinary read and cancellation deadlines independently", () => {
  expect(remoteOperationDeadlineClass("render_media_to_split_project_folder", {})).toBe("job");
  expect(remoteOperationDeadlineClass("render_media_to_split_project_folder", { admissionProtocol: 1 })).toBe("mutation");
  expect(remoteOperationDeadlineClass("recover_render_attempt_in_split_project_folder", {})).toBe("mutation");
  expect(remoteOperationDeadlineClass("read_project_snapshot_from_split_project_folder", {})).toBe("read");
  expect(remoteOperationDeadlineClass("cancel_render_job_in_split_project_folder", {})).toBe("cancellation");
});
