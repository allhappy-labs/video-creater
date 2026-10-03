import type { BackendInput } from "../backend-transport";

export type RemoteOperationClass = "read" | "mutation" | "job" | "cancellation";
export interface RemoteDeadlines {
  readonly queue: number;
  readonly lease: number;
  readonly read: number;
  readonly mutation: number;
  readonly job: number;
  readonly cancellation: number;
  readonly ticket: number;
}
export const defaultRemoteDeadlines: RemoteDeadlines = {
  queue: 30_000, lease: 10_000, read: 15_000, mutation: 30_000,
  // Legacy synchronous render/analysis requests still need time to finish.
  job: 30 * 60_000, cancellation: 15_000, ticket: 10_000,
};

const snapshotReads = new Set([
  "load_job_progress_from_split_project_folder",
  "load_render_attempt_in_split_project_folder",
  "read_project_snapshot_from_split_project_folder",
]);
// These Rust Read operations were previously classified as browser mutations.
// Only their false local markers can be migrated; accepted edit markers stay fenced.
const legacyReadMarkers = new Set([
  "capture_canonical_preview_frame_in_split_project_folder", "prepare_project_preview",
  "mcp_client_configuration", "preview_storage_cleanup",
]);
const reads = new Set([
  "remote_list_projects",
  "load_split_project_from_folder", "load_agent_sessions_from_split_project_folder",
  "load_app_server_conversation_history_from_split_project_folder", "load_render_pipeline_report_from_split_project_folder",
  "search_project_media", "validate_split_project_folder",
  ...legacyReadMarkers,
]);
const longJobs = new Set([
  "render_media_to_split_project_folder", "render_webm_to_split_project_folder", "prepare_project_preview",
  "capture_canonical_preview_frame_in_split_project_folder", "cache_timeline_filmstrip_in_split_project_folder",
  "analyze_media_for_edit_in_split_project_folder", "analyze_project_speech", "run_generate_media_in_process",
  "generate_one_click_edit_for_project", "generate_spoken_semantic_multi_source_edit_for_project",
  "export_nle_xml_to_split_project_folder", "export_palmier_project_package_to_split_project_folder",
  "extract_visual_frame_cache_in_split_project_folder", "caption_visual_frame_cache_in_split_project_folder",
  "run_preview_render_comparison_request_in_split_project_folder", "download_production_speech_models", "download_transcription_model",
]);

export function remoteOperationClass(operation: string, input: BackendInput): RemoteOperationClass {
  if (operation.startsWith("remote_build_temporal_")) return "read";
  if (operation.startsWith("cancel_")) return "cancellation";
  // Recovery authorizes a fresh durable attempt; it is a short write, never a status read.
  if (operation === "recover_render_attempt_in_split_project_folder") return "mutation";
  if (operation === "render_media_to_split_project_folder" && input.admissionProtocol === 1) return "mutation";
  if (snapshotReads.has(operation) || reads.has(operation) || operation.startsWith("get_") || operation.startsWith("list_")) return "read";
  if (longJobs.has(operation)) return "job";
  return "mutation";
}

/** Native preview reads can take as long as jobs without becoming durable mutations. */
export function remoteOperationDeadlineClass(operation: string, input: BackendInput): RemoteOperationClass {
  const operationClass = remoteOperationClass(operation, input);
  return operationClass === "read" && longJobs.has(operation) ? "job" : operationClass;
}

export function remoteOperationHadFalseMutationMarker(operation: string): boolean {
  return legacyReadMarkers.has(operation);
}

/** Only readers whose host implementation avoids recovery/mutation gates bypass the FIFO. */
export function remoteOperationBypassesQueue(operation: string): boolean {
  return snapshotReads.has(operation) || operation === "prepare_project_preview" || operation.startsWith("cancel_");
}
