import { describe, expect, it } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import {
  applyProjectActionsLocally,
  mergeCodexProjectMetadata,
  mergeProjectMediaAnalysis,
  projectHistorySnapshot,
  projectSnapshotsEqual,
} from "@/lib/agent/project-merge";

function mergeSummary(result: VideoProject, current: VideoProject, incoming: VideoProject) {
  return {
    isCurrent: result === current,
    isIncoming: result === incoming,
    id: result.id,
    name: result.name,
    contentRevision: result.contentRevision,
    hasContentRevision: "contentRevision" in result,
    codexThreadId: result.codexThreadId,
    mediaAnalysis: result.mediaAnalysis,
    hasMediaAnalysis: "mediaAnalysis" in result,
    jobs: result.jobs.map((job) => `${job.id}:${job.status}:${job.updatedAt}`),
    timelineTrackCount: result.timeline.tracks.length,
  };
}

function currentProject(): VideoProject {
  const project = fixtureProject();
  project.name = "Current";
  project.contentRevision = 4;
  project.codexThreadId = "thread-current";
  project.mediaAnalysis = [{ mediaId: "media-1", sourceIn: 0, sourceOut: 1, label: "Current" }];
  project.jobs = [
    { id: "job-shared", kind: "codex_edit", status: "running", updatedAt: "2026-07-15T12:00:00Z" },
    { id: "job-current", kind: "render", status: "queued", updatedAt: "2026-07-15T11:00:00Z" },
  ];
  return project;
}

function incomingProject(): VideoProject {
  const project = fixtureProject();
  project.name = "Incoming";
  project.timeline.tracks = [];
  project.contentRevision = 2;
  project.codexThreadId = "thread-incoming";
  project.mediaAnalysis = [{ mediaId: "media-1", sourceIn: 2, sourceOut: 3, label: "Incoming" }];
  project.jobs = [
    { id: "job-shared", kind: "codex_edit", status: "completed", updatedAt: "2026-07-15T12:05:00Z" },
    { id: "job-incoming", kind: "generate_media", status: "running", updatedAt: "2026-07-15T12:01:00Z" },
  ];
  return project;
}

function withoutMetadata(project: VideoProject): VideoProject {
  delete project.contentRevision;
  delete project.mediaAnalysis;
  project.codexThreadId = null;
  project.jobs = [];
  return project;
}

describe("project merge characterization", () => {
  it("merges Codex project metadata", () => {
    const current = currentProject();
    const incoming = incomingProject();
    const otherProject = { ...incomingProject(), id: "project-other" };
    const bareIncoming = withoutMetadata(incomingProject());
    const bareCurrent = withoutMetadata(currentProject());
    expect({
      overlapping: mergeSummary(mergeCodexProjectMetadata(current, incoming), current, incoming),
      differentProject: mergeSummary(mergeCodexProjectMetadata(current, otherProject), current, otherProject),
      incomingWithoutMetadata: mergeSummary(mergeCodexProjectMetadata(current, bareIncoming), current, bareIncoming),
      currentWithoutMetadata: mergeSummary(mergeCodexProjectMetadata(bareCurrent, incoming), bareCurrent, incoming),
      neitherHasMetadata: mergeSummary(
        mergeCodexProjectMetadata(bareCurrent, withoutMetadata(incomingProject())),
        bareCurrent,
        bareIncoming,
      ),
    }).toMatchInlineSnapshot(`
      {
        "currentWithoutMetadata": {
          "codexThreadId": "thread-incoming",
          "contentRevision": 2,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [
            "job-shared:completed:2026-07-15T12:05:00Z",
            "job-incoming:running:2026-07-15T12:01:00Z",
          ],
          "mediaAnalysis": [
            {
              "label": "Incoming",
              "mediaId": "media-1",
              "sourceIn": 2,
              "sourceOut": 3,
            },
          ],
          "name": "Current",
          "timelineTrackCount": 5,
        },
        "differentProject": {
          "codexThreadId": "thread-incoming",
          "contentRevision": 2,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-other",
          "isCurrent": false,
          "isIncoming": true,
          "jobs": [
            "job-shared:completed:2026-07-15T12:05:00Z",
            "job-incoming:running:2026-07-15T12:01:00Z",
          ],
          "mediaAnalysis": [
            {
              "label": "Incoming",
              "mediaId": "media-1",
              "sourceIn": 2,
              "sourceOut": 3,
            },
          ],
          "name": "Incoming",
          "timelineTrackCount": 0,
        },
        "incomingWithoutMetadata": {
          "codexThreadId": "thread-current",
          "contentRevision": 4,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [
            "job-shared:running:2026-07-15T12:00:00Z",
            "job-current:queued:2026-07-15T11:00:00Z",
          ],
          "mediaAnalysis": [
            {
              "label": "Current",
              "mediaId": "media-1",
              "sourceIn": 0,
              "sourceOut": 1,
            },
          ],
          "name": "Current",
          "timelineTrackCount": 5,
        },
        "neitherHasMetadata": {
          "codexThreadId": null,
          "contentRevision": undefined,
          "hasContentRevision": false,
          "hasMediaAnalysis": false,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [],
          "mediaAnalysis": undefined,
          "name": "Current",
          "timelineTrackCount": 5,
        },
        "overlapping": {
          "codexThreadId": "thread-incoming",
          "contentRevision": 2,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [
            "job-shared:completed:2026-07-15T12:05:00Z",
            "job-current:queued:2026-07-15T11:00:00Z",
            "job-incoming:running:2026-07-15T12:01:00Z",
          ],
          "mediaAnalysis": [
            {
              "label": "Incoming",
              "mediaId": "media-1",
              "sourceIn": 2,
              "sourceOut": 3,
            },
          ],
          "name": "Current",
          "timelineTrackCount": 5,
        },
      }
    `);
  });

  it("merges project media analysis", () => {
    const current = currentProject();
    const incoming = incomingProject();
    const newerIncoming = { ...incomingProject(), contentRevision: 9 };
    const otherProject = { ...incomingProject(), id: "project-other" };
    const bareIncoming = withoutMetadata(incomingProject());
    const bareCurrent = withoutMetadata(currentProject());
    expect({
      overlappingOlderIncoming: mergeSummary(mergeProjectMediaAnalysis(current, incoming), current, incoming),
      overlappingNewerIncoming: mergeSummary(
        mergeProjectMediaAnalysis(current, newerIncoming),
        current,
        newerIncoming,
      ),
      differentProject: mergeSummary(mergeProjectMediaAnalysis(current, otherProject), current, otherProject),
      incomingWithoutMetadata: mergeSummary(mergeProjectMediaAnalysis(current, bareIncoming), current, bareIncoming),
      currentWithoutMetadata: mergeSummary(mergeProjectMediaAnalysis(bareCurrent, incoming), bareCurrent, incoming),
      neitherHasMetadata: mergeSummary(
        mergeProjectMediaAnalysis(bareCurrent, withoutMetadata(incomingProject())),
        bareCurrent,
        bareIncoming,
      ),
    }).toMatchInlineSnapshot(`
      {
        "currentWithoutMetadata": {
          "codexThreadId": null,
          "contentRevision": 2,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [],
          "mediaAnalysis": [
            {
              "label": "Incoming",
              "mediaId": "media-1",
              "sourceIn": 2,
              "sourceOut": 3,
            },
          ],
          "name": "Current",
          "timelineTrackCount": 5,
        },
        "differentProject": {
          "codexThreadId": "thread-incoming",
          "contentRevision": 2,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-other",
          "isCurrent": false,
          "isIncoming": true,
          "jobs": [
            "job-shared:completed:2026-07-15T12:05:00Z",
            "job-incoming:running:2026-07-15T12:01:00Z",
          ],
          "mediaAnalysis": [
            {
              "label": "Incoming",
              "mediaId": "media-1",
              "sourceIn": 2,
              "sourceOut": 3,
            },
          ],
          "name": "Incoming",
          "timelineTrackCount": 0,
        },
        "incomingWithoutMetadata": {
          "codexThreadId": "thread-current",
          "contentRevision": 4,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [
            "job-shared:running:2026-07-15T12:00:00Z",
            "job-current:queued:2026-07-15T11:00:00Z",
          ],
          "mediaAnalysis": [
            {
              "label": "Current",
              "mediaId": "media-1",
              "sourceIn": 0,
              "sourceOut": 1,
            },
          ],
          "name": "Current",
          "timelineTrackCount": 5,
        },
        "neitherHasMetadata": {
          "codexThreadId": null,
          "contentRevision": undefined,
          "hasContentRevision": false,
          "hasMediaAnalysis": false,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [],
          "mediaAnalysis": undefined,
          "name": "Current",
          "timelineTrackCount": 5,
        },
        "overlappingNewerIncoming": {
          "codexThreadId": "thread-current",
          "contentRevision": 9,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [
            "job-shared:running:2026-07-15T12:00:00Z",
            "job-current:queued:2026-07-15T11:00:00Z",
          ],
          "mediaAnalysis": [
            {
              "label": "Incoming",
              "mediaId": "media-1",
              "sourceIn": 2,
              "sourceOut": 3,
            },
          ],
          "name": "Current",
          "timelineTrackCount": 5,
        },
        "overlappingOlderIncoming": {
          "codexThreadId": "thread-current",
          "contentRevision": 4,
          "hasContentRevision": true,
          "hasMediaAnalysis": true,
          "id": "project-sample",
          "isCurrent": false,
          "isIncoming": false,
          "jobs": [
            "job-shared:running:2026-07-15T12:00:00Z",
            "job-current:queued:2026-07-15T11:00:00Z",
          ],
          "mediaAnalysis": [
            {
              "label": "Incoming",
              "mediaId": "media-1",
              "sourceIn": 2,
              "sourceOut": 3,
            },
          ],
          "name": "Current",
          "timelineTrackCount": 5,
        },
      }
    `);
  });

  it("applies a batch of project actions locally in order", () => {
    const project = fixtureProject();
    const actions: ProjectAction[] = [
      {
        type: "addItems",
        targetTrackId: "track-overlays",
        items: [
          {
            id: "overlay-local",
            kind: "overlay",
            startSeconds: 1,
            durationSeconds: 2,
            source: { type: "text", text: "Local overlay" },
            label: "Local overlay",
            properties: {},
          },
        ],
      },
      { type: "editCaptionText", itemId: "caption-1", text: "Edited locally" },
      { type: "removeItems", itemIds: ["music-bed"] },
      { type: "addItems", targetTrackId: "track-missing", items: [] },
    ];
    const next = applyProjectActionsLocally(project, actions);
    expect({
      unchangedForNoActions: applyProjectActionsLocally(project, []) === project,
      tracks: next.timeline.tracks.map((track) => ({
        id: track.id,
        items: track.items.map((item) => ({
          id: item.id,
          startSeconds: item.startSeconds,
          durationSeconds: item.durationSeconds,
          text: item.source.type === "text" ? item.source.text : null,
        })),
      })),
      originalCaptionText: project.timeline.tracks
        .find((track) => track.id === "track-captions")
        ?.items.map((item) => (item.source.type === "text" ? item.source.text : null)),
    }).toMatchInlineSnapshot(`
      {
        "originalCaptionText": [
          "Original caption text",
          "Second clean split",
        ],
        "tracks": [
          {
            "id": "track-video",
            "items": [
              {
                "durationSeconds": 4,
                "id": "item-1",
                "startSeconds": 0,
                "text": null,
              },
              {
                "durationSeconds": 4,
                "id": "sample-generated-clip",
                "startSeconds": 4,
                "text": null,
              },
            ],
          },
          {
            "id": "track-scenes",
            "items": [],
          },
          {
            "id": "track-overlays",
            "items": [
              {
                "durationSeconds": 2,
                "id": "overlay-local",
                "startSeconds": 1,
                "text": "Local overlay",
              },
            ],
          },
          {
            "id": "track-captions",
            "items": [
              {
                "durationSeconds": 1.35,
                "id": "caption-1",
                "startSeconds": 0.65,
                "text": "Edited locally",
              },
              {
                "durationSeconds": 1.2,
                "id": "caption-2",
                "startSeconds": 2.15,
                "text": "Second clean split",
              },
            ],
          },
          {
            "id": "track-audio",
            "items": [],
          },
        ],
        "unchangedForNoActions": true,
      }
    `);
  });

  it("snapshots project history and compares snapshots by serialized JSON", () => {
    const project = fixtureProject();
    const snapshot = projectHistorySnapshot(project);
    const reordered = { ...project, name: project.name };
    const keyOrderChanged = Object.fromEntries(Object.entries(project).reverse()) as unknown as VideoProject;
    const withUndefinedField = { ...project, contentRevision: undefined } as unknown as VideoProject;
    const renamed = { ...project, name: "Renamed" };
    expect({
      snapshotIsCopy: snapshot !== project && snapshot.timeline !== project.timeline,
      snapshotEqual: projectSnapshotsEqual(snapshot, project),
      shallowCopyEqual: projectSnapshotsEqual(reordered, project),
      keyOrderChangedEqual: projectSnapshotsEqual(keyOrderChanged, project),
      undefinedFieldEqual: projectSnapshotsEqual(withUndefinedField, project),
      renamedEqual: projectSnapshotsEqual(renamed, project),
    }).toMatchInlineSnapshot(`
      {
        "keyOrderChangedEqual": false,
        "renamedEqual": false,
        "shallowCopyEqual": true,
        "snapshotEqual": true,
        "snapshotIsCopy": true,
        "undefinedFieldEqual": true,
      }
    `);
  });
});
