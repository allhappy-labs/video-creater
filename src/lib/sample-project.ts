import type { VideoProject } from "./project";
import { sampleTimeline, type Timeline } from "./timeline";

export const sampleProjectDir = "/tmp/video-creater-editor-project";
export const sampleProjectBrowserDir = "browser://bundled-sample-project";
const sampleProjectBrowserRoot = "";

export function sampleProjectBrowserPreviewUrl(
  projectDir: string,
  relativePath: string,
) {
  if (projectDir !== sampleProjectBrowserDir) return null;
  const parts = relativePath
    .replace(/\\/g, "/")
    .split("/")
    .filter((part) => part.length > 0 && part !== ".");
  if (parts.length === 0 || parts.some((part) => part === "..")) return null;
  return `${sampleProjectBrowserRoot}/${parts.map(encodeURIComponent).join("/")}`;
}

function createDefaultSampleTimeline(): Timeline {
  const timeline = structuredClone(sampleTimeline);
  timeline.durationSeconds = 8;
  timeline.tracks = timeline.tracks.map((track) =>
    track.kind === "video"
      ? {
          ...track,
          items: [
            // Rendering requires every video clip to select an explicit source range.
            ...track.items.map((item) => ({
              ...item,
              properties: {
                sourceIn: 0,
                sourceOut: item.durationSeconds,
                ...item.properties,
              },
            })),
            {
              id: "sample-generated-clip",
              kind: "video_clip",
              startSeconds: 4,
              durationSeconds: 4,
              source: { type: "media", mediaId: "sample-generated-output" },
              label: "Restored Edison alternate",
              properties: {
                sourceIn: 0,
                sourceOut: 4,
                generatedAssetId: "sample-generated-shot",
                reason: "bundled local restoration demonstration",
              },
            },
          ],
        }
      : track.kind === "caption"
        ? {
            ...track,
            items: track.items.map((item) => {
              const wordIndex = item.id === "caption-2" ? 2 : 0;
              return {
                ...item,
                properties: {
                  ...item.properties,
                  transcriptId: "transcript-media-1",
                  wordIndex,
                },
              };
            }),
          }
        : track.kind === "audio"
          ? {
              ...track,
              items: track.items.map((item) =>
                item.id === "music-bed" && item.source.type === "media"
                  ? { ...item, source: { type: "media", mediaId: "media-voiceover" } }
                  : item,
              ),
            }
          : track,
  );
  return timeline;
}

export function createSampleProject(): VideoProject {
  return {
    schemaVersion: 1,
    id: "project-sample",
    name: "Edison Restoration Demo",
    createdAt: "2026-06-13T00:00:00Z",
    updatedAt: "2026-07-12T00:00:00Z",
    media: [
      {
        id: "media-1",
        relativePath: "media/input.mp4",
        kind: "video",
        durationSeconds: 4,
        width: 640,
        height: 360,
        fps: 24,
        folderId: "folder-source",
      },
      {
        id: "media-voiceover",
        relativePath: "media/voiceover.m4a",
        kind: "audio",
        durationSeconds: 4,
        width: null,
        height: null,
        fps: null,
        folderId: "folder-audio",
      },
      {
        id: "sample-generated-output",
        relativePath: "sample/generated/product-reveal.mp4",
        kind: "generated",
        durationSeconds: 4,
        width: 640,
        height: 360,
        fps: 24,
        folderId: "folder-generated",
      },
    ],
    mediaFolders: [
      { id: "folder-source", name: "Source footage", parentId: null },
      { id: "folder-generated", name: "Generated selects", parentId: null },
      { id: "folder-audio", name: "Audio", parentId: null },
    ],
    generatedAssets: [
      {
        schemaVersion: 1,
        id: "sample-generated-shot",
        kind: "generated",
        status: "completed",
        name: "Bundled Edison restoration",
        prompt: "Restore the public-domain newsreel with a warmer high-contrast treatment",
        model: { provider: "local", id: "bundled-edison-restoration" },
        references: {
          mediaIds: ["media-1"],
          firstFrameMediaId: "media-1",
          lastFrameMediaId: null,
        },
        settings: {
          width: 640,
          height: 360,
          durationSeconds: 4,
          fps: 24,
          aspectRatio: "16:9",
        },
        outputs: [
          {
            mediaId: "sample-generated-output",
            relativePath: "sample/generated/product-reveal.mp4",
            width: 640,
            height: 360,
            durationSeconds: 4,
            fps: 24,
          },
        ],
        createdAt: "2026-07-12T00:00:00Z",
        parentAssetId: null,
        retryOfAssetId: null,
      },
    ],
    renderReports: [],
    transcripts: [
      {
        id: "transcript-media-1",
        mediaId: "media-1",
        engine: "parakeet",
        rawArtifactPath: null,
        repairs: [],
        segments: [
          { text: "Original caption", startSeconds: 0.65, endSeconds: 2 },
          { text: "Second clean split", startSeconds: 2.15, endSeconds: 3.35 },
        ],
        words: [
          { text: "Original", startSeconds: 0.65, endSeconds: 1.05, confidence: 0.98, speaker: null },
          { text: "caption", startSeconds: 1.1, endSeconds: 2, confidence: 0.97, speaker: null },
          { text: "Second", startSeconds: 2.15, endSeconds: 2.7, confidence: 0.96, speaker: null },
          { text: "split", startSeconds: 2.75, endSeconds: 3.35, confidence: 0.95, speaker: null },
        ],
      },
    ],
    timeline: createDefaultSampleTimeline(),
    renderSettings: {
      width: 1920,
      height: 1080,
      fps: 24,
      loudnessLufs: -14,
      captions: "burn_in",
    },
    codexThreadId: null,
    jobs: [
      {
        id: "sample-generated-shot",
        kind: "generate_media",
        status: "completed",
        updatedAt: "2026-07-12T00:00:00Z",
        workflow: {
          workflowId: "video-creater/project-sample/bundled-media/sample-generated-shot",
          workflowType: "VideoCreaterBundledSampleWorkflow",
          taskQueue: "local",
          runId: "bundled-sample",
          activityTypes: ["CopyBundledSampleMedia"],
        },
        startRequest: {
          workflowId: "video-creater/project-sample/bundled-media/sample-generated-shot",
          workflowType: "VideoCreaterBundledSampleWorkflow",
          taskQueue: "local",
          input: {
            projectId: "project-sample",
            projectDir: sampleProjectDir,
            assetId: "sample-generated-shot",
            jobId: "sample-generated-shot",
            mockMode: true,
          },
          searchAttributes: {
            projectId: "project-sample",
            jobId: "sample-generated-shot",
            workflowKind: "generate_media",
          },
          activityTypes: ["CopyBundledSampleMedia"],
          idReusePolicy: "rejectDuplicate",
        },
      },
    ],
  };
}
