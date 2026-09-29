import { afterEach, describe, expect, it, vi } from "vitest";

import {
  BackendUnavailableError,
  FixtureOperationUnsupportedError,
} from "./backend-transport";
import { createSampleProject } from "../sample-project";
import { createFixtureTransport } from "./fixture-bootstrap";

describe("fixture runtime bootstrap", () => {
  afterEach(() => {
    delete window.__EDITOR_FIXTURE_DRIVER__;
  });

  it("installs deterministic base operations and a neutral event driver", async () => {
    const marker = {
      enabled: true,
      preferences: { schemaVersion: 2 },
      exportCapabilities: [{ profile: "webm-vp9" }],
    } as const;
    const transport = await createFixtureTransport(marker);

    await expect(transport.request("get_app_preferences"))
      .resolves.toEqual(marker.preferences);
    await expect(transport.request("get_export_profile_availability_report"))
      .resolves.toEqual(marker.exportCapabilities);
    await expect(transport.request("sync_native_menu_state"))
      .resolves.toBeUndefined();
    await expect(transport.request("get_platform_info"))
      .resolves.toEqual({ platform: "macos" });
    await expect(transport.request("materialize_sample_project_media"))
      .rejects.toBeInstanceOf(BackendUnavailableError);
    await expect(transport.request("save_split_project_to_folder"))
      .rejects.toBeInstanceOf(BackendUnavailableError);

    const listener = vi.fn();
    const unlisten = await transport.listen("editor://event", listener);
    window.__EDITOR_FIXTURE_DRIVER__?.emit("editor://event", { sequence: 1 });
    expect(listener).toHaveBeenCalledWith({ sequence: 1 });
    unlisten();
    window.__EDITOR_FIXTURE_DRIVER__?.emit("editor://event", { sequence: 2 });
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("lets a fixture opt into a Linux host presentation", async () => {
    const transport = await createFixtureTransport({
      enabled: true,
      platform: "linux",
    });

    await expect(transport.request("get_platform_info"))
      .resolves.toEqual({ platform: "linux" });
  });

  it("adds the background task handlers only when the marker asks for them", async () => {
    const plain = await createFixtureTransport({ enabled: true, settingsFixtureId: "settings-ai-models-no-project" });
    await expect(plain.request("get_temporal_worker_environment_report")).rejects.toBeInstanceOf(BackendUnavailableError);

    const tasks = await createFixtureTransport({ enabled: true, settingsFixtureId: "settings-ai-models-no-project", tasksFixture: true });
    await expect(tasks.request<{ ready: boolean }>("get_temporal_worker_environment_report")).resolves.toMatchObject({ ready: false });
    const saved = await tasks.request<{ project: { jobs: { id: string }[] } }>("save_split_project_to_folder", { projectDir: "/tmp/sample", project: createSampleProject() });
    expect(saved.project.jobs.map((job) => job.id)).toContain("fixture-transcribe-media-1");
  });

  it("adds the export handlers, with task handlers seeded only alongside tasksFixture", async () => {
    const plain = await createFixtureTransport({ enabled: true, settingsFixtureId: "settings-ai-models-no-project", exportCapabilities: [] });
    await expect(plain.request("get_export_profile_availability_report")).resolves.toEqual([]);
    await expect(plain.request("export_nle_xml_to_split_project_folder")).rejects.toBeInstanceOf(FixtureOperationUnsupportedError);

    const exports = await createFixtureTransport({ enabled: true, settingsFixtureId: "settings-ai-models-no-project", exportCapabilities: [], exportFixture: true });
    await expect(exports.request<{ profile: string; available: boolean }[]>("get_export_profile_availability_report")).resolves.toContainEqual(
      expect.objectContaining({ profile: "proResMov", available: false }),
    );
    const saved = await exports.request<{ project: { jobs: { id: string }[] } }>("save_split_project_to_folder", { projectDir: "/tmp/sample", project: createSampleProject() });
    expect(saved.project.jobs.map((job) => job.id)).not.toContain("fixture-transcribe-media-1");
    await expect(exports.request("get_temporal_worker_environment_report")).resolves.toMatchObject({ ready: false });

    const both = await createFixtureTransport({ enabled: true, settingsFixtureId: "settings-ai-models-no-project", exportFixture: true, tasksFixture: true });
    const seeded = await both.request<{ project: { jobs: { id: string }[] } }>("save_split_project_to_folder", { projectDir: "/tmp/sample", project: createSampleProject() });
    expect(seeded.project.jobs.map((job) => job.id)).toContain("fixture-transcribe-media-1");
  });

  it("adds the editor panel handlers only when the marker asks for them", async () => {
    const plain = await createFixtureTransport({ enabled: true });
    await expect(plain.request("list_visual_effect_catalog")).rejects.toBeInstanceOf(BackendUnavailableError);

    const panels = await createFixtureTransport({ enabled: true, settingsFixtureId: "settings-ai-models-no-project", panelFixtures: true });
    await expect(panels.request<{ effects: unknown[] }>("list_visual_effect_catalog")).resolves.toMatchObject({ effectCount: 3 });
    await expect(panels.request<{ project: { mediaSilenceRanges?: unknown[] } }>("save_split_project_to_folder", { project: createSampleProject() }))
      .resolves.toMatchObject({ project: { mediaSilenceRanges: [expect.anything(), expect.anything()] } });
  });

  it("reports the AI agent unavailable unless the marker asks for conversation handlers", async () => {
    const plain = await createFixtureTransport({ enabled: true });
    await expect(plain.request("start_codex_conversation_edit_for_project", { project: createSampleProject() })).rejects.toBeInstanceOf(BackendUnavailableError);

    const conversation = await createFixtureTransport({ enabled: true, conversationFixture: true });
    const request = { prompt: "Generate three lab shots", focus: { mediaIds: [], timelineItemIds: [] }, createdAt: "2026-09-15T10:00:00.000Z" };
    await expect(conversation.request("start_codex_conversation_edit_for_project", { project: createSampleProject(), request }))
      .resolves.toMatchObject({ preparedProposal: { risk: { level: "review" } } });
  });

  it("shares the committed sample between the conversation and export fixtures", async () => {
    const transport = await createFixtureTransport({ enabled: true, settingsFixtureId: "settings-ai-models-no-project", exportCapabilities: [], conversationFixture: true, exportFixture: true });
    await transport.request("save_split_project_to_folder", { projectDir: "/tmp/sample", project: createSampleProject() });
    const captured = await transport.request<{ project: { jobs: { id: string }[] } }>("capture_canonical_preview_frame_in_split_project_folder", {
      projectDir: "/tmp/sample",
      playheadSeconds: 1,
      jobId: "frame-1",
      updatedAt: "2026-09-15T10:00:00.000Z",
    });
    expect(captured.project.jobs.map((job) => job.id)).toContain("frame-1");
    const loaded = await transport.request<{ schemaVersion: number; jobs: { id: string }[] }>("load_split_project_from_folder", { projectDir: "/tmp/sample" });
    expect(loaded.schemaVersion).toBe(2);
    expect(loaded.jobs.map((job) => job.id)).toContain("frame-1");
    await expect(transport.request("load_agent_sessions_from_split_project_folder", { projectDir: "/tmp/sample" })).resolves.toMatchObject({ sessions: [] });
  });

  it("distinguishes recognized unavailable operations from contract mistakes", async () => {
    const transport = await createFixtureTransport({
      enabled: true,
      preferences: { schemaVersion: 2 },
      exportCapabilities: [],
    });

    await expect(transport.request("load_split_project_from_folder"))
      .rejects.toBeInstanceOf(BackendUnavailableError);
    // Without a settings fixture, startup model readiness reads as a missing backend, not a contract mistake.
    await expect(transport.request("list_transcription_models"))
      .rejects.toBeInstanceOf(BackendUnavailableError);
    await expect(transport.request("load_agent_sessions_from_split_project_folder"))
      .rejects.toBeInstanceOf(BackendUnavailableError);
    await expect(transport.request("not_a_real_backend_operation"))
      .rejects.toBeInstanceOf(FixtureOperationUnsupportedError);
  });
});
