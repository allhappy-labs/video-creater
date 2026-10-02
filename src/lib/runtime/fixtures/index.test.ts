import { afterEach, describe, expect, it } from "vitest";
import type { ImportMediaResult, ProjectActionWriteResult, VideoProject } from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import { fixtureMediaChooserOperation } from "../adapters/tauri-dialog";
import { BackendUnavailableError, FixtureOperationUnsupportedError } from "../backend-transport";
import { createFixtureTransport } from "../fixture-bootstrap";
import { fixtureDomainOperations } from "./index";

const projectDir = "/tmp/video-creater-editor-project";

/** Every command the acceptance fixture answers, by domain. */
const acceptanceOperations = {
  project: [
    "materialize_sample_project_media",
    "save_split_project_to_folder",
    "load_split_project_from_folder",
    "read_project_snapshot_from_split_project_folder",
    "apply_project_actions_to_split_project_folder",
    "apply_project_action_to_split_project_folder",
    "update_project_settings_in_split_project_folder",
    fixtureMediaChooserOperation,
    "import_media_to_project",
    "prepare_project_preview",
    "cache_timeline_filmstrip_in_split_project_folder",
    "list_shader_background_templates",
    "search_project_media",
    "rebuild_project_search_index",
  ],
  panels: ["list_visual_effect_catalog"],
  speech: [
    "build_temporal_job_summary",
    "build_temporal_transcribe_media_start_request",
    "build_temporal_start_result_action",
    "start_temporal_workflow",
    "get_project_speaker_registry",
    "rename_project_speaker",
  ],
  generation: [
    "list_generation_model_catalog",
    "build_temporal_generate_media_start_request",
    "run_generate_media_in_process",
    "cancel_generate_media_in_process",
    "build_temporal_generate_media_failure_actions",
    "complete_mock_generated_asset_in_split_project_folder",
  ],
  conversation: [
    "start_codex_conversation_edit_for_project",
    "cancel_codex_conversation_edit_for_project",
    "apply_codex_conversation_proposal",
    "undo_latest_codex_conversation_edit",
    "capture_canonical_preview_frame_in_split_project_folder",
    "load_agent_sessions_from_split_project_folder",
    "apply_agent_session_action_to_split_project_folder",
    "load_app_server_conversation_history_from_split_project_folder",
  ],
  export: [
    "get_export_profile_availability_report",
    "open_export_directory_dialog",
    "render_media_to_split_project_folder",
    "load_render_attempt_in_split_project_folder",
    "recover_render_attempt_in_split_project_folder",
    "cancel_render_job_in_split_project_folder",
    "export_nle_xml_to_split_project_folder",
    "export_palmier_project_package_to_split_project_folder",
  ],
  tasks: [
    "get_temporal_worker_environment_report",
    "reveal_export_artifact_in_split_project_folder",
    "load_render_pipeline_report_from_split_project_folder",
    "run_preview_render_comparison_request_in_split_project_folder",
    "load_job_progress_from_split_project_folder",
    "reconcile_temporal_jobs_in_split_project_folder",
  ],
} as const;

describe("fixture domain registration", () => {
  afterEach(() => {
    delete window.__EDITOR_FIXTURE_DRIVER__;
  });

  it("registers every domain handler under acceptanceFixture", () => {
    const operations = fixtureDomainOperations({ acceptanceFixture: true }, { turnDelayMs: 0 });
    const expected = Object.values(acceptanceOperations).flat();
    expect(expected.filter((operation) => !operations.has(operation))).toEqual([]);
    expect([...operations.keys()].sort()).toEqual([...expected].sort());
  });

  it("keeps the earlier markers to their own domains plus the folder core", () => {
    expect(fixtureDomainOperations({}, { turnDelayMs: 0 }).size).toBe(0);
    expect([...fixtureDomainOperations({ panelFixtures: true }, { turnDelayMs: 0 }).keys()].sort()).toEqual(["list_visual_effect_catalog", "save_split_project_to_folder"]);
    const conversation = fixtureDomainOperations({ conversationFixture: true }, { turnDelayMs: 0 });
    expect(conversation.has("apply_codex_conversation_proposal")).toBe(true);
    expect(conversation.has("apply_project_actions_to_split_project_folder")).toBe(true);
    expect(conversation.has("import_media_to_project")).toBe(false);
    expect(conversation.has("start_temporal_workflow")).toBe(false);
    expect(conversation.has("render_media_to_split_project_folder")).toBe(false);
  });

  it("opens the untranscribed sample folder backed and imports through the fixture chooser; unknown commands still fail loudly", async () => {
    const transport = await createFixtureTransport({ enabled: true, acceptanceFixture: true });
    await expect(transport.request("materialize_sample_project_media", { projectDir })).resolves.toBeUndefined();
    await expect(transport.request("load_split_project_from_folder", { projectDir })).rejects.toBe("This folder has no saved project yet.");
    const { project } = await transport.request<ProjectActionWriteResult>("save_split_project_to_folder", { projectDir, project: createSampleProject(), expectedRevision: 0 });
    expect(project).toMatchObject({ schemaVersion: 2, contentRevision: 1, transcripts: [] });
    expect(project.mediaSilenceRanges).toHaveLength(2);
    expect(project.jobs.map((job) => job.id)).not.toContain("fixture-transcribe-media-1");

    const paths = await transport.request<string[]>(fixtureMediaChooserOperation, { title: "Import media", filters: [{ name: "Media", extensions: ["webm"] }] });
    const imported = await transport.request<ImportMediaResult>("import_media_to_project", { projectDir, project, sourcePaths: paths });
    expect(imported.imported.map((media) => media.id)).toEqual(["media-fixture-import-1"]);
    await expect(transport.request<VideoProject>("load_split_project_from_folder", { projectDir })).resolves.toMatchObject({ contentRevision: 2 });

    await expect(transport.request("not_a_real_backend_operation")).rejects.toBeInstanceOf(FixtureOperationUnsupportedError);
    await expect(transport.request("create_matte_in_split_project_folder", { projectDir })).rejects.toBeInstanceOf(FixtureOperationUnsupportedError);
  });

  it("seeds tasks into the acceptance sample only alongside tasksFixture, and leaves the chooser unavailable without it", async () => {
    const seeded = await createFixtureTransport({ enabled: true, acceptanceFixture: true, tasksFixture: true });
    const { project } = await seeded.request<ProjectActionWriteResult>("save_split_project_to_folder", { projectDir, project: createSampleProject() });
    expect(project.jobs.map((job) => job.id)).toContain("fixture-transcribe-media-1");

    const plain = await createFixtureTransport({ enabled: true, conversationFixture: true });
    await expect(plain.request(fixtureMediaChooserOperation, { title: "Import media", filters: [] })).rejects.toBeInstanceOf(BackendUnavailableError);
    await expect(plain.request("import_media_to_project", { projectDir })).rejects.toBeInstanceOf(BackendUnavailableError);
  });
});
