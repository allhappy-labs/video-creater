import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getSkillsHealth, repairBundledSkills } from "@/lib/settings/skills";
import {
  getStorageHealth,
  previewStorageCleanup,
  refreshStorageInventory,
  revealStorageInventoryItem,
  runStorageCleanup,
} from "@/lib/settings/storage";
import type { VideoProject } from "@/lib/project";
import type { SettingsOperation } from "@/lib/settings/operations";
import { ProjectSettings } from "./project-settings";

const operationSnapshot = vi.hoisted(() => ({
  items: [] as SettingsOperation[],
}));

vi.mock("@/lib/settings/skills", () => ({
  getSkillsHealth: vi.fn(),
  repairBundledSkills: vi.fn(),
}));

vi.mock("@/lib/settings/storage", () => ({
  getStorageHealth: vi.fn(),
  previewStorageCleanup: vi.fn(),
  refreshStorageInventory: vi.fn(),
  revealStorageInventoryItem: vi.fn(),
  runStorageCleanup: vi.fn(),
}));

vi.mock("@/lib/settings/use-settings-operations", () => ({
  useSettingsOperations: () => ({
    operations: operationSnapshot.items,
    error: null,
    hydrated: true,
    baselineOperationIds: new Set<string>(),
    baselineTerminalOperationIds: new Set<string>(),
  }),
}));

const project: VideoProject = {
  schemaVersion: 2,
  id: "project-1",
  name: "Interview",
  createdAt: "2026-07-19T00:00:00Z",
  updatedAt: "2026-07-19T00:00:00Z",
  media: [],
  generatedAssets: [],
  renderReports: [],
  transcripts: [],
  timeline: { durationSeconds: 0, tracks: [] },
  renderSettings: {
    width: 1920,
    height: 1080,
    fps: 30,
    loudnessLufs: -14,
    captions: "burn_in",
  },
  codexThreadId: null,
  jobs: [],
};

const readySkills = {
  id: "skills",
  state: "ready" as const,
  items: [
    {
      id: "video-creater-video-pipeline",
      label: "Editing pipeline",
      state: "ready" as const,
      summary: "Bundled skill matches.",
      actionId: null,
      actionLabel: null,
      lastCheckedAt: "2026-07-19T00:00:00Z",
      diagnosticCode: null,
      diagnosticDetail: null,
      provenance: {},
    },
  ],
};

const projectStorage = {
  id: "storage",
  state: "ready" as const,
  items: [
    {
      id: "storage.projectMedia",
      label: "Project media",
      state: "ready" as const,
      summary: "Measured project media.",
      actionId: null,
      actionLabel: null,
      lastCheckedAt: "2026-07-19T00:00:00Z",
      diagnosticCode: null,
      diagnosticDetail: null,
      provenance: {
        scope: "projectMedia",
        path: "/Projects/interview/media",
        bytes: "1200",
      },
    },
    {
      id: "storage.projectRenderArtifacts",
      label: "Project render artifacts",
      state: "ready" as const,
      summary: "Measured project renders.",
      actionId: null,
      actionLabel: null,
      lastCheckedAt: "2026-07-19T00:00:00Z",
      diagnosticCode: null,
      diagnosticDetail: null,
      provenance: {
        scope: "projectRenderArtifacts",
        path: "/Projects/interview/renders",
        bytes: "2400",
      },
    },
    {
      id: "storage.globalModels",
      label: "Global models",
      state: "ready" as const,
      summary: "Measured models.",
      actionId: null,
      actionLabel: null,
      lastCheckedAt: "2026-07-19T00:00:00Z",
      diagnosticCode: null,
      diagnosticDetail: null,
      provenance: { scope: "globalModels", bytes: "9999" },
    },
  ],
};

describe("ProjectSettings", () => {
  beforeEach(() => {
    operationSnapshot.items = [];
    vi.mocked(getSkillsHealth).mockReset();
    vi.mocked(getSkillsHealth).mockResolvedValue(readySkills);
    vi.mocked(repairBundledSkills).mockReset();
    vi.mocked(getStorageHealth).mockReset();
    vi.mocked(getStorageHealth).mockResolvedValue(projectStorage);
    vi.mocked(refreshStorageInventory).mockReset();
    vi.mocked(revealStorageInventoryItem).mockReset();
    vi.mocked(previewStorageCleanup).mockReset();
    vi.mocked(runStorageCleanup).mockReset();
  });

  it("submits one complete project settings action", async () => {
    const onApply = vi.fn();
    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={onApply}
      />,
    );

    fireEvent.change(screen.getByLabelText("Project name"), {
      target: { value: "Interview cut" },
    });
    fireEvent.change(screen.getByLabelText("Project format preset"), {
      target: { value: "uhd" },
    });
    fireEvent.change(screen.getByLabelText("Project frame rate"), {
      target: { value: "24" },
    });
    fireEvent.change(screen.getByLabelText("Project loudness target"), {
      target: { value: "-16" },
    });
    fireEvent.change(screen.getByLabelText("Project caption mode"), {
      target: { value: "mux" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Apply project settings" }));

    await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
    expect(onApply).toHaveBeenCalledWith({
      type: "updateProjectSettings",
      name: "Interview cut",
      renderSettings: {
        width: 3840,
        height: 2160,
        fps: 24,
        loudnessLufs: -16,
        captions: "mux",
      },
    });
  });

  it("rejects a blank name without applying and focuses the invalid field", async () => {
    const onApply = vi.fn();
    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={onApply}
      />,
    );
    await screen.findByText("Bundled project guidance active");

    const name = screen.getByLabelText("Project name");
    fireEvent.change(name, { target: { value: "  " } });
    fireEvent.click(screen.getByRole("button", { name: "Apply project settings" }));

    expect(screen.getByRole("alert")).toHaveTextContent("Project name is required.");
    expect(name).toHaveFocus();
    expect(onApply).not.toHaveBeenCalled();
  });

  it("rejects odd dimensions without applying the action", async () => {
    const onApply = vi.fn();
    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={onApply}
      />,
    );
    await screen.findByText("Bundled project guidance active");

    fireEvent.change(screen.getByLabelText("Project width"), {
      target: { value: "1919" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Apply project settings" }));

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Dimensions must be even values between 2 and 16384.",
    );
    expect(onApply).not.toHaveBeenCalled();
  });

  it("focuses height when width is valid and height is invalid", async () => {
    const onApply = vi.fn();
    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={onApply}
      />,
    );
    await screen.findByText("Bundled project guidance active");

    const width = screen.getByLabelText("Project width");
    const height = screen.getByLabelText("Project height");
    fireEvent.change(width, { target: { value: "1920" } });
    fireEvent.change(height, { target: { value: "1079" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply project settings" }));

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Dimensions must be even values between 2 and 16384.",
    );
    expect(height).toHaveFocus();
    expect(width).not.toHaveFocus();
    expect(onApply).not.toHaveBeenCalled();
  });

  it("shows only active-project storage and can reveal its exact path", async () => {
    vi.mocked(revealStorageInventoryItem).mockResolvedValue();
    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={vi.fn()}
      />,
    );

    expect(await screen.findByText("Project media")).toBeInTheDocument();
    expect(screen.getByText("Project render artifacts")).toBeInTheDocument();
    expect(screen.queryByText("Global models")).not.toBeInTheDocument();
    expect(screen.getByText("/Projects/interview")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Reveal Project media in Finder" }));
    await waitFor(() =>
      expect(revealStorageInventoryItem).toHaveBeenCalledWith(
        "/Projects/interview/media",
        "/Projects/interview",
      ),
    );
  });

  it("reloads project storage only after a queued refresh reaches a terminal state", async () => {
    const queued: SettingsOperation = {
      id: "storage-refresh-1",
      kind: "storageRefresh",
      targetId: "storage.inventory",
      phase: "queued",
      state: "queued",
      completedUnits: 0,
      totalUnits: 1,
      unit: "inventories",
      cancellable: false,
      message: "Storage refresh queued.",
      error: null,
      startedAt: "2026-07-19T00:00:00Z",
      updatedAt: "2026-07-19T00:00:00Z",
    };
    vi.mocked(refreshStorageInventory).mockResolvedValue(queued);
    const view = render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={vi.fn()}
      />,
    );
    await screen.findByText("Project media");
    expect(getStorageHealth).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Refresh project storage" }));
    await waitFor(() => expect(refreshStorageInventory).toHaveBeenCalledTimes(1));
    expect(getStorageHealth).toHaveBeenCalledTimes(1);

    operationSnapshot.items = [
      {
        ...queued,
        phase: "complete",
        state: "succeeded",
        completedUnits: 1,
        message: "Storage refresh completed.",
        updatedAt: "2026-07-19T00:00:01Z",
      },
    ];
    view.rerender(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={vi.fn()}
      />,
    );

    await waitFor(() => expect(getStorageHealth).toHaveBeenCalledTimes(2));
  });

  it("collapses healthy guidance to one compact row", async () => {
    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={vi.fn()}
      />,
    );

    expect(await screen.findByText("Bundled project guidance active")).toBeInTheDocument();
    expect(screen.getByText("Editing pipeline")).not.toBeVisible();
    fireEvent.click(screen.getByText("View guidance details"));
    expect(screen.getByText("Editing pipeline")).toBeVisible();
  });

  it("keeps an invalid project-root diagnosis visible without offering a false repair", async () => {
    vi.mocked(getSkillsHealth).mockResolvedValue({
      id: "skills",
      state: "actionRequired",
      items: [
        {
          id: "skills.root",
          label: "Project skills",
          state: "actionRequired",
          summary: "The configured project repository root is invalid.",
          actionId: null,
          actionLabel: null,
          lastCheckedAt: "2026-07-19T00:00:00Z",
          diagnosticCode: "skills.rootInvalid",
          diagnosticDetail: "Project root does not contain AGENTS.md.",
          provenance: { configuredRoot: "/Projects/interview" },
        },
      ],
    });

    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={vi.fn()}
      />,
    );

    expect(
      await screen.findByText("Project repository guidance is unavailable"),
    ).toBeInTheDocument();
    expect(screen.getByText("Project root does not contain AGENTS.md.")).toBeVisible();
    expect(
      screen.getByText(/Bundled editing guidance remains available to the app/),
    ).toBeVisible();
    expect(screen.queryByText("Bundled project guidance active")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Repair project guidance" }),
    ).not.toBeInTheDocument();
  });

  it("retries guidance verification after the initial health request fails", async () => {
    vi.mocked(getSkillsHealth)
      .mockRejectedValueOnce(new Error("health service unavailable"))
      .mockResolvedValueOnce(readySkills);

    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={vi.fn()}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Project guidance could not be verified: health service unavailable",
    );
    fireEvent.click(screen.getByRole("button", { name: "Retry project guidance" }));

    expect(await screen.findByText("Bundled project guidance active")).toBeInTheDocument();
    expect(getSkillsHealth).toHaveBeenCalledTimes(2);
    expect(screen.getByRole("button", { name: "Verify project guidance" })).toBeEnabled();
  });

  it("previews exact repair paths for damaged bundled guidance", async () => {
    vi.mocked(getSkillsHealth).mockResolvedValue({
      ...readySkills,
      state: "actionRequired",
      items: [
        {
          ...readySkills.items[0]!,
          state: "actionRequired",
          summary: "Bundled skill differs.",
          diagnosticCode: "skills.damaged",
        },
      ],
    });
    vi.mocked(repairBundledSkills)
      .mockResolvedValueOnce({
        preview: {
          skillIds: ["video-creater-video-pipeline"],
          affectedPaths: ["/Projects/interview/.agents/skills/video-creater-video-pipeline/SKILL.md"],
        },
        operation: null,
        report: null,
      })
      .mockResolvedValueOnce({
        preview: {
          skillIds: ["video-creater-video-pipeline"],
          affectedPaths: ["/Projects/interview/.agents/skills/video-creater-video-pipeline/SKILL.md"],
        },
        operation: null,
        report: {
          repairedPaths: ["/Projects/interview/.agents/skills/video-creater-video-pipeline/SKILL.md"],
          backupPaths: [],
          verification: [],
        },
      });
    render(
      <ProjectSettings
        project={project}
        projectDir="/Projects/interview"
        onApply={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Repair project guidance" }));
    expect(await screen.findByRole("dialog", { name: "Confirm guidance repair" })).toHaveTextContent(
      "/Projects/interview/.agents/skills/video-creater-video-pipeline/SKILL.md",
    );
    fireEvent.click(screen.getByRole("button", { name: "Repair confirmed files" }));

    await waitFor(() =>
      expect(repairBundledSkills).toHaveBeenLastCalledWith(
        "/Projects/interview",
        ["video-creater-video-pipeline"],
        ["/Projects/interview/.agents/skills/video-creater-video-pipeline/SKILL.md"],
      ),
    );
  });
});
