import { describe, expect, it } from "vitest";
import type { GeneratedAsset, VideoProject } from "@/lib/project";
import { sampleProjectBrowserDir } from "@/lib/sample-project";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { requiredValue } from "@/test-utils/required";
import { codexToolDisplayName } from "@/lib/agent/selection-context";
import {
  agentMentionTargets,
  agentSelectedGeneratedReferences,
  agentSelectedMediaContext,
  agentSelectedTimelineClipContext,
  agentSetupSnippet,
  projectFolderLabel,
  projectProfileInitial,
  trackForTimelineItem,
  type AgentSetupClient,
} from "@/lib/agent/selection-context";
import {
  generatedAssetForTimelineItem,
  generatedReferenceMediaIds,
  uniqueStringValues,
} from "@/lib/generation/assets";
import {
  generatedAssetPlacementContextLabel,
  mediaTimelineAction,
  timelineGenerationTrackForKind,
} from "@/lib/generation/timeline-placement";
import { mediaFolderPathLabel } from "@/lib/media/folder-tree";
import { previewUrlForMedia, safeProjectMediaPath } from "@/lib/media/preview-source";

const extraOverlayItems: TimelineItem[] = [
  {
    id: "template-overlay",
    kind: "overlay",
    startSeconds: 1,
    durationSeconds: 2,
    source: { type: "text", text: "Opening Hook" },
    label: "Template overlay",
    properties: {
      templateId: "kinetic-lower-third-v1",
      templateFields: { headline: "Opening Hook", subline: 42, extra: null },
      opacity: 0.8,
      fadeInSeconds: 0.25,
    },
  },
  {
    id: "unknown-template-overlay",
    kind: "overlay",
    startSeconds: 3,
    durationSeconds: 1,
    source: { type: "text", text: "Unknown" },
    label: "Unknown template overlay",
    properties: { templateId: "missing-template", templateFields: ["not", "a", "record"] },
  },
  {
    id: "text-overlay",
    kind: "overlay",
    startSeconds: 4,
    durationSeconds: 1,
    source: { type: "text", text: "Plain text" },
    label: "Text overlay",
    properties: {},
  },
];

const replacementAsset: GeneratedAsset = {
  schemaVersion: 1,
  id: "generated-replacement",
  kind: "video",
  status: "completed",
  name: "  ",
  targetFolderId: "folder-child",
  placementIntent: "replace:item-1",
  prompt: "Replace the opening shot",
  model: { provider: "fal", id: "wan" },
  references: {
    mediaIds: ["media-voiceover", "media-1", "media-missing"],
    firstFrameMediaId: "media-1",
    lastFrameMediaId: "media-1",
    referenceImageMediaRefs: ["media-image-ref"],
    referenceVideoMediaRefs: ["media-1"],
    referenceAudioMediaRefs: ["media-voiceover"],
  },
  settings: { width: 1280, height: 720, durationSeconds: 4, fps: 24, aspectRatio: "16:9" },
  outputs: [
    {
      mediaId: "generated-replacement-output",
      relativePath: "generated/replacement/output.mp4",
      width: 1280,
      height: 720,
      durationSeconds: 4,
      fps: 24,
    },
  ],
  createdAt: "2026-07-13T00:00:00Z",
  parentAssetId: null,
  retryOfAssetId: null,
};

function selectionProject(): VideoProject {
  const project = fixtureProject();
  project.media.push({
    id: "generated-replacement-output",
    relativePath: "generated/replacement/output.mp4",
    kind: "video",
    durationSeconds: 0,
    width: 1280,
    height: 720,
    fps: 24,
    folderId: "folder-child",
  });
  project.mediaFolders = [
    ...(project.mediaFolders ?? []),
    { id: "folder-child", name: "Replacements", parentId: "folder-generated" },
  ];
  project.generatedAssets.push(replacementAsset);
  const overlayTrack = requiredValue(
    project.timeline.tracks.find((track) => track.kind === "overlay"),
    "overlay track",
  );
  overlayTrack.items.push(...extraOverlayItems);
  const audioTrack = requiredValue(
    project.timeline.tracks.find((track) => track.kind === "audio"),
    "audio track",
  );
  audioTrack.locked = true;
  audioTrack.enabled = false;
  const videoTrack = requiredValue(
    project.timeline.tracks.find((track) => track.kind === "video"),
    "video track",
  );
  videoTrack.items.push({
    id: "replacement-clip",
    kind: "generated_clip",
    startSeconds: 8,
    durationSeconds: 4,
    source: { type: "media", mediaId: "generated-replacement-output" },
    label: "Replacement clip",
    properties: { sourceIn: 0.5, sourceOut: 4.5, volumeDb: -3, fadeOutSeconds: 1 },
  });
  return project;
}

describe("agent selection context characterization", () => {
  it("builds selected media context for every fixture media", () => {
    const project = selectionProject();
    expect({
      media: project.media.map((media) => agentSelectedMediaContext(project, media.id, "/tmp/project")),
      bundledPreviews: agentSelectedMediaContext(project, "generated-replacement-output", sampleProjectBrowserDir),
      defaultProjectDir: agentSelectedMediaContext(project, "sample-generated-output"),
      nullMedia: agentSelectedMediaContext(project, null),
      missingMedia: agentSelectedMediaContext(project, "media-missing"),
    }).toMatchInlineSnapshot(`
      {
        "bundledPreviews": {
          "canInsertOnTimeline": true,
          "canQueueReferencedGeneration": true,
          "canQueueUpscale": true,
          "generatedAssetId": "generated-replacement",
          "kind": "video",
          "label": "output.mp4",
          "mediaId": "generated-replacement-output",
          "modelLabel": "fal/wan",
          "placementIntent": "replace:item-1",
          "placementLabel": "Replacement target",
          "prompt": "Replace the opening shot",
          "referenceIds": [
            "media-1",
            "media-voiceover",
            "media-missing",
            "media-image-ref",
          ],
          "references": [
            {
              "kind": "video",
              "label": "input.mp4",
              "mediaId": "media-1",
              "previewUrl": "/media/input.mp4",
              "role": "First frame",
            },
            {
              "kind": "audio",
              "label": "voiceover.m4a",
              "mediaId": "media-voiceover",
              "previewUrl": "/media/voiceover.m4a",
              "role": "Reference",
            },
            {
              "kind": "media",
              "label": "media-missing",
              "mediaId": "media-missing",
              "previewUrl": null,
              "role": "Reference",
            },
            {
              "kind": "media",
              "label": "media-image-ref",
              "mediaId": "media-image-ref",
              "previewUrl": null,
              "role": "Reference",
            },
          ],
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 4,
            "fps": 24,
            "height": 720,
            "width": 1280,
          },
          "targetFolderId": "folder-child",
          "targetFolderLabel": "Generated selects / Replacements",
        },
        "defaultProjectDir": {
          "canInsertOnTimeline": true,
          "canQueueReferencedGeneration": true,
          "canQueueUpscale": true,
          "generatedAssetId": "sample-generated-shot",
          "kind": "generated",
          "label": "product-reveal.mp4",
          "mediaId": "sample-generated-output",
          "modelLabel": "local/bundled-edison-restoration",
          "placementIntent": null,
          "placementLabel": null,
          "prompt": "Restore the public-domain newsreel with a warmer high-contrast treatment",
          "referenceIds": [
            "media-1",
          ],
          "references": [
            {
              "kind": "video",
              "label": "input.mp4",
              "mediaId": "media-1",
              "previewUrl": null,
              "role": "First frame",
            },
          ],
          "settings": {
            "aspectRatio": "16:9",
            "durationSeconds": 4,
            "fps": 24,
            "height": 360,
            "width": 640,
          },
          "targetFolderId": null,
          "targetFolderLabel": null,
        },
        "media": [
          {
            "canInsertOnTimeline": true,
            "canQueueReferencedGeneration": true,
            "canQueueUpscale": true,
            "generatedAssetId": null,
            "kind": "video",
            "label": "input.mp4",
            "mediaId": "media-1",
            "modelLabel": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "referenceIds": [],
            "references": [],
            "settings": null,
            "targetFolderId": null,
            "targetFolderLabel": null,
          },
          {
            "canInsertOnTimeline": false,
            "canQueueReferencedGeneration": false,
            "canQueueUpscale": false,
            "generatedAssetId": null,
            "kind": "audio",
            "label": "voiceover.m4a",
            "mediaId": "media-voiceover",
            "modelLabel": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "referenceIds": [],
            "references": [],
            "settings": null,
            "targetFolderId": null,
            "targetFolderLabel": null,
          },
          {
            "canInsertOnTimeline": true,
            "canQueueReferencedGeneration": true,
            "canQueueUpscale": true,
            "generatedAssetId": "sample-generated-shot",
            "kind": "generated",
            "label": "product-reveal.mp4",
            "mediaId": "sample-generated-output",
            "modelLabel": "local/bundled-edison-restoration",
            "placementIntent": null,
            "placementLabel": null,
            "prompt": "Restore the public-domain newsreel with a warmer high-contrast treatment",
            "referenceIds": [
              "media-1",
            ],
            "references": [
              {
                "kind": "video",
                "label": "input.mp4",
                "mediaId": "media-1",
                "previewUrl": null,
                "role": "First frame",
              },
            ],
            "settings": {
              "aspectRatio": "16:9",
              "durationSeconds": 4,
              "fps": 24,
              "height": 360,
              "width": 640,
            },
            "targetFolderId": null,
            "targetFolderLabel": null,
          },
          {
            "canInsertOnTimeline": true,
            "canQueueReferencedGeneration": true,
            "canQueueUpscale": true,
            "generatedAssetId": "generated-replacement",
            "kind": "video",
            "label": "output.mp4",
            "mediaId": "generated-replacement-output",
            "modelLabel": "fal/wan",
            "placementIntent": "replace:item-1",
            "placementLabel": "Replacement target",
            "prompt": "Replace the opening shot",
            "referenceIds": [
              "media-1",
              "media-voiceover",
              "media-missing",
              "media-image-ref",
            ],
            "references": [
              {
                "kind": "video",
                "label": "input.mp4",
                "mediaId": "media-1",
                "previewUrl": null,
                "role": "First frame",
              },
              {
                "kind": "audio",
                "label": "voiceover.m4a",
                "mediaId": "media-voiceover",
                "previewUrl": null,
                "role": "Reference",
              },
              {
                "kind": "media",
                "label": "media-missing",
                "mediaId": "media-missing",
                "previewUrl": null,
                "role": "Reference",
              },
              {
                "kind": "media",
                "label": "media-image-ref",
                "mediaId": "media-image-ref",
                "previewUrl": null,
                "role": "Reference",
              },
            ],
            "settings": {
              "aspectRatio": "16:9",
              "durationSeconds": 4,
              "fps": 24,
              "height": 720,
              "width": 1280,
            },
            "targetFolderId": "folder-child",
            "targetFolderLabel": "Generated selects / Replacements",
          },
        ],
        "missingMedia": null,
        "nullMedia": null,
      }
    `);
  });

  it("builds selected timeline clip context for every fixture item", () => {
    const project = selectionProject();
    const items = project.timeline.tracks.flatMap((track) => track.items);
    expect({
      items: items.map((item) => agentSelectedTimelineClipContext(project, item, sampleProjectBrowserDir)),
      nullItem: agentSelectedTimelineClipContext(project, null),
      detachedItem: agentSelectedTimelineClipContext(project, {
        id: "detached",
        kind: "caption",
        startSeconds: 0,
        durationSeconds: 1,
        source: { type: "text", text: "Detached caption" },
        label: "Detached",
        properties: {},
      }),
    }).toMatchInlineSnapshot(`
      {
        "detachedItem": {
          "durationSeconds": 1,
          "fadeInSeconds": null,
          "fadeOutSeconds": null,
          "generatedAssetId": null,
          "itemId": "detached",
          "kind": "caption",
          "label": "Detached",
          "modelLabel": null,
          "opacity": null,
          "placementIntent": null,
          "placementLabel": null,
          "prompt": null,
          "references": [],
          "settings": null,
          "sourceIn": null,
          "sourceMediaId": null,
          "sourceOut": null,
          "targetFolderId": null,
          "targetFolderLabel": null,
          "templateFieldLabels": null,
          "templateFields": null,
          "templateId": null,
          "text": "Detached caption",
          "timelineStartSeconds": 0,
          "trackEnabled": true,
          "trackId": null,
          "trackLocked": false,
          "trackName": null,
          "volumeDb": null,
        },
        "items": [
          {
            "durationSeconds": 4,
            "fadeInSeconds": null,
            "fadeOutSeconds": null,
            "generatedAssetId": null,
            "itemId": "item-1",
            "kind": "video_clip",
            "label": "Opening clip",
            "modelLabel": null,
            "opacity": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "references": [],
            "settings": null,
            "sourceIn": 0,
            "sourceMediaId": "media-1",
            "sourceOut": 4,
            "targetFolderId": null,
            "targetFolderLabel": null,
            "templateFieldLabels": null,
            "templateFields": null,
            "templateId": null,
            "text": null,
            "timelineStartSeconds": 0,
            "trackEnabled": true,
            "trackId": "track-video",
            "trackLocked": false,
            "trackName": "Video",
            "volumeDb": null,
          },
          {
            "durationSeconds": 4,
            "fadeInSeconds": null,
            "fadeOutSeconds": null,
            "generatedAssetId": "sample-generated-shot",
            "itemId": "sample-generated-clip",
            "kind": "video_clip",
            "label": "Restored Edison alternate",
            "modelLabel": "local/bundled-edison-restoration",
            "opacity": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": "Restore the public-domain newsreel with a warmer high-contrast treatment",
            "references": [
              {
                "kind": "video",
                "label": "input.mp4",
                "mediaId": "media-1",
                "previewUrl": "/media/input.mp4",
                "role": "First frame",
              },
            ],
            "settings": {
              "aspectRatio": "16:9",
              "durationSeconds": 4,
              "fps": 24,
              "height": 360,
              "width": 640,
            },
            "sourceIn": 0,
            "sourceMediaId": "sample-generated-output",
            "sourceOut": 4,
            "targetFolderId": null,
            "targetFolderLabel": null,
            "templateFieldLabels": null,
            "templateFields": null,
            "templateId": null,
            "text": null,
            "timelineStartSeconds": 4,
            "trackEnabled": true,
            "trackId": "track-video",
            "trackLocked": false,
            "trackName": "Video",
            "volumeDb": null,
          },
          {
            "durationSeconds": 4,
            "fadeInSeconds": null,
            "fadeOutSeconds": 1,
            "generatedAssetId": "generated-replacement",
            "itemId": "replacement-clip",
            "kind": "generated_clip",
            "label": "Replacement clip",
            "modelLabel": "fal/wan",
            "opacity": null,
            "placementIntent": "replace:item-1",
            "placementLabel": "Replacement target",
            "prompt": "Replace the opening shot",
            "references": [
              {
                "kind": "video",
                "label": "input.mp4",
                "mediaId": "media-1",
                "previewUrl": "/media/input.mp4",
                "role": "First frame",
              },
              {
                "kind": "audio",
                "label": "voiceover.m4a",
                "mediaId": "media-voiceover",
                "previewUrl": "/media/voiceover.m4a",
                "role": "Reference",
              },
              {
                "kind": "media",
                "label": "media-missing",
                "mediaId": "media-missing",
                "previewUrl": null,
                "role": "Reference",
              },
              {
                "kind": "media",
                "label": "media-image-ref",
                "mediaId": "media-image-ref",
                "previewUrl": null,
                "role": "Reference",
              },
            ],
            "settings": {
              "aspectRatio": "16:9",
              "durationSeconds": 4,
              "fps": 24,
              "height": 720,
              "width": 1280,
            },
            "sourceIn": 0.5,
            "sourceMediaId": "generated-replacement-output",
            "sourceOut": 4.5,
            "targetFolderId": "folder-child",
            "targetFolderLabel": "Generated selects / Replacements",
            "templateFieldLabels": null,
            "templateFields": null,
            "templateId": null,
            "text": null,
            "timelineStartSeconds": 8,
            "trackEnabled": true,
            "trackId": "track-video",
            "trackLocked": false,
            "trackName": "Video",
            "volumeDb": -3,
          },
          {
            "durationSeconds": 2,
            "fadeInSeconds": 0.25,
            "fadeOutSeconds": null,
            "generatedAssetId": null,
            "itemId": "template-overlay",
            "kind": "overlay",
            "label": "Template overlay",
            "modelLabel": null,
            "opacity": 0.8,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "references": [],
            "settings": null,
            "sourceIn": null,
            "sourceMediaId": null,
            "sourceOut": null,
            "targetFolderId": null,
            "targetFolderLabel": null,
            "templateFieldLabels": {
              "headline": "Headline",
              "subline": "Subline",
            },
            "templateFields": {
              "extra": "",
              "headline": "Opening Hook",
              "subline": "42",
            },
            "templateId": "kinetic-lower-third-v1",
            "text": "Opening Hook",
            "timelineStartSeconds": 1,
            "trackEnabled": true,
            "trackId": "track-overlays",
            "trackLocked": false,
            "trackName": "Overlays",
            "volumeDb": null,
          },
          {
            "durationSeconds": 1,
            "fadeInSeconds": null,
            "fadeOutSeconds": null,
            "generatedAssetId": null,
            "itemId": "unknown-template-overlay",
            "kind": "overlay",
            "label": "Unknown template overlay",
            "modelLabel": null,
            "opacity": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "references": [],
            "settings": null,
            "sourceIn": null,
            "sourceMediaId": null,
            "sourceOut": null,
            "targetFolderId": null,
            "targetFolderLabel": null,
            "templateFieldLabels": null,
            "templateFields": {},
            "templateId": "missing-template",
            "text": "Unknown",
            "timelineStartSeconds": 3,
            "trackEnabled": true,
            "trackId": "track-overlays",
            "trackLocked": false,
            "trackName": "Overlays",
            "volumeDb": null,
          },
          {
            "durationSeconds": 1,
            "fadeInSeconds": null,
            "fadeOutSeconds": null,
            "generatedAssetId": null,
            "itemId": "text-overlay",
            "kind": "overlay",
            "label": "Text overlay",
            "modelLabel": null,
            "opacity": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "references": [],
            "settings": null,
            "sourceIn": null,
            "sourceMediaId": null,
            "sourceOut": null,
            "targetFolderId": null,
            "targetFolderLabel": null,
            "templateFieldLabels": null,
            "templateFields": null,
            "templateId": null,
            "text": "Plain text",
            "timelineStartSeconds": 4,
            "trackEnabled": true,
            "trackId": "track-overlays",
            "trackLocked": false,
            "trackName": "Overlays",
            "volumeDb": null,
          },
          {
            "durationSeconds": 1.35,
            "fadeInSeconds": null,
            "fadeOutSeconds": null,
            "generatedAssetId": null,
            "itemId": "caption-1",
            "kind": "caption",
            "label": "Caption 1",
            "modelLabel": null,
            "opacity": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "references": [],
            "settings": null,
            "sourceIn": 0.65,
            "sourceMediaId": null,
            "sourceOut": 2,
            "targetFolderId": null,
            "targetFolderLabel": null,
            "templateFieldLabels": null,
            "templateFields": null,
            "templateId": null,
            "text": "Original caption text",
            "timelineStartSeconds": 0.65,
            "trackEnabled": true,
            "trackId": "track-captions",
            "trackLocked": false,
            "trackName": "Captions",
            "volumeDb": null,
          },
          {
            "durationSeconds": 1.2,
            "fadeInSeconds": null,
            "fadeOutSeconds": null,
            "generatedAssetId": null,
            "itemId": "caption-2",
            "kind": "caption",
            "label": "Caption 2",
            "modelLabel": null,
            "opacity": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "references": [],
            "settings": null,
            "sourceIn": 2.15,
            "sourceMediaId": null,
            "sourceOut": 3.35,
            "targetFolderId": null,
            "targetFolderLabel": null,
            "templateFieldLabels": null,
            "templateFields": null,
            "templateId": null,
            "text": "Second clean split",
            "timelineStartSeconds": 2.15,
            "trackEnabled": true,
            "trackId": "track-captions",
            "trackLocked": false,
            "trackName": "Captions",
            "volumeDb": null,
          },
          {
            "durationSeconds": 4,
            "fadeInSeconds": null,
            "fadeOutSeconds": 0.75,
            "generatedAssetId": null,
            "itemId": "music-bed",
            "kind": "audio_clip",
            "label": "Music bed",
            "modelLabel": null,
            "opacity": null,
            "placementIntent": null,
            "placementLabel": null,
            "prompt": null,
            "references": [],
            "settings": null,
            "sourceIn": 0,
            "sourceMediaId": "media-voiceover",
            "sourceOut": 4,
            "targetFolderId": null,
            "targetFolderLabel": null,
            "templateFieldLabels": null,
            "templateFields": null,
            "templateId": null,
            "text": null,
            "timelineStartSeconds": 0,
            "trackEnabled": false,
            "trackId": "track-audio",
            "trackLocked": true,
            "trackName": "Audio",
            "volumeDb": null,
          },
        ],
        "nullItem": null,
      }
    `);
  });

  it("collects generated references", () => {
    const project = selectionProject();
    expect({
      replacement: agentSelectedGeneratedReferences(project, replacementAsset, sampleProjectBrowserDir),
      sample: agentSelectedGeneratedReferences(project, requiredValue(project.generatedAssets[0], "sample asset")),
      referenceIds: generatedReferenceMediaIds(replacementAsset.references),
      unique: uniqueStringValues(["a", null, "b", undefined, "", "a"]),
    }).toMatchInlineSnapshot(`
      {
        "referenceIds": [
          "media-voiceover",
          "media-1",
          "media-missing",
          "media-image-ref",
        ],
        "replacement": [
          {
            "kind": "video",
            "label": "input.mp4",
            "mediaId": "media-1",
            "previewUrl": "/media/input.mp4",
            "role": "First frame",
          },
          {
            "kind": "audio",
            "label": "voiceover.m4a",
            "mediaId": "media-voiceover",
            "previewUrl": "/media/voiceover.m4a",
            "role": "Reference",
          },
          {
            "kind": "media",
            "label": "media-missing",
            "mediaId": "media-missing",
            "previewUrl": null,
            "role": "Reference",
          },
          {
            "kind": "media",
            "label": "media-image-ref",
            "mediaId": "media-image-ref",
            "previewUrl": null,
            "role": "Reference",
          },
        ],
        "sample": [
          {
            "kind": "video",
            "label": "input.mp4",
            "mediaId": "media-1",
            "previewUrl": null,
            "role": "First frame",
          },
        ],
        "unique": [
          "a",
          "b",
        ],
      }
    `);
  });

  it("lists mention targets for the fixture project", () => {
    expect({
      fixture: agentMentionTargets(fixtureProject()),
      withReplacement: agentMentionTargets(selectionProject()),
    }).toMatchInlineSnapshot(`
      {
        "fixture": [
          {
            "canInsertOnTimeline": true,
            "kind": "video",
            "label": "input.mp4",
            "mediaId": "media-1",
          },
          {
            "canInsertOnTimeline": true,
            "kind": "audio",
            "label": "voiceover.m4a",
            "mediaId": "media-voiceover",
          },
          {
            "canInsertOnTimeline": true,
            "description": "product-reveal.mp4",
            "kind": "generated",
            "label": "Bundled Edison restoration",
            "mediaId": "sample-generated-output",
            "searchTerms": [
              "sample-generated-shot",
              "Bundled Edison restoration",
              "Restore the public-domain newsreel with a warmer high-contrast treatment",
              "sample/generated/product-reveal.mp4",
              "",
              "",
              "",
              "",
              "local",
              "bundled-edison-restoration",
              "media-1",
              "",
              "media-1",
            ],
          },
        ],
        "withReplacement": [
          {
            "canInsertOnTimeline": true,
            "kind": "video",
            "label": "input.mp4",
            "mediaId": "media-1",
          },
          {
            "canInsertOnTimeline": false,
            "kind": "audio",
            "label": "voiceover.m4a",
            "mediaId": "media-voiceover",
          },
          {
            "canInsertOnTimeline": true,
            "description": "product-reveal.mp4",
            "kind": "generated",
            "label": "Bundled Edison restoration",
            "mediaId": "sample-generated-output",
            "searchTerms": [
              "sample-generated-shot",
              "Bundled Edison restoration",
              "Restore the public-domain newsreel with a warmer high-contrast treatment",
              "sample/generated/product-reveal.mp4",
              "",
              "",
              "",
              "",
              "local",
              "bundled-edison-restoration",
              "media-1",
              "",
              "media-1",
            ],
          },
          {
            "canInsertOnTimeline": true,
            "description": "output.mp4 | Replacement target | Generated selects / Replacements",
            "kind": "video",
            "label": "output.mp4",
            "mediaId": "generated-replacement-output",
            "searchTerms": [
              "generated-replacement",
              "  ",
              "Replace the opening shot",
              "generated/replacement/output.mp4",
              "replace:item-1",
              "folder-child",
              "Replacement target",
              "Generated selects / Replacements",
              "fal",
              "wan",
              "media-1",
              "media-1",
              "media-voiceover",
              "media-1",
              "media-missing",
            ],
          },
        ],
      }
    `);
  });

  it("resolves dependencies used by the selection context", () => {
    const project = selectionProject();
    const items = project.timeline.tracks.flatMap((track) => track.items);
    const lockedVideo = selectionProject();
    for (const track of lockedVideo.timeline.tracks) track.locked = true;
    const cycleFolders = [
      { id: "folder-a", name: "A", parentId: "folder-b" },
      { id: "folder-b", name: "B", parentId: "folder-a" },
      { id: "folder-orphan", name: "Orphan", parentId: "folder-gone" },
    ];
    expect({
      tracksForItems: items.map((item) => [item.id, trackForTimelineItem(project, item.id)?.id ?? null]),
      missingTrack: trackForTimelineItem(project, "missing"),
      generatedAssetsForItems: items.map((item) => [
        item.id,
        generatedAssetForTimelineItem(project, item)?.id ?? null,
      ]),
      placementLabels: [
        "timeline",
        "library",
        "replace:item-1",
        "replace:",
        "unknown",
        null,
        undefined,
      ].map((intent) => generatedAssetPlacementContextLabel(intent as GeneratedAsset["placementIntent"])),
      folderLabels: [
        mediaFolderPathLabel(project.mediaFolders ?? [], "folder-child"),
        mediaFolderPathLabel(project.mediaFolders ?? [], "folder-source"),
        mediaFolderPathLabel(project.mediaFolders ?? [], "folder-missing"),
        mediaFolderPathLabel(project.mediaFolders ?? [], null),
        mediaFolderPathLabel(cycleFolders, "folder-a"),
        mediaFolderPathLabel(cycleFolders, "folder-orphan"),
      ],
      generationTracks: {
        video: timelineGenerationTrackForKind(project, "video")?.id ?? null,
        lockedAudio: timelineGenerationTrackForKind(project, "audio")?.id ?? null,
        allLocked: timelineGenerationTrackForKind(lockedVideo, "video")?.id ?? null,
      },
      mediaTimelineActions: [...project.media.map((media) => media.id), "media-missing"].map((mediaId) => [
        mediaId,
        mediaTimelineAction(project, mediaId),
      ]),
      previewUrls: [
        [sampleProjectBrowserDir, "media/input.mp4"],
        [sampleProjectBrowserDir, "../escape.mp4"],
        ["/tmp/project", "media/input.mp4"],
        ["", "media/input.mp4"],
      ].map(([projectDir = "", relativePath = ""]) => previewUrlForMedia(projectDir, relativePath)),
      safePaths: [
        ["/tmp/project/", "media/a.mp4"],
        ["C:\\projects\\demo\\", "media\\.\\a.mp4"],
        ["/tmp/project", "/etc/passwd"],
        ["/tmp/project", "../x.mp4"],
        ["/tmp/project", "media/../x.mp4"],
        ["/tmp/project", "file:///x"],
        ["/tmp/project", "C:\\x.mp4"],
        ["/tmp/project", "./"],
        ["  ", "media/a.mp4"],
      ].map(([projectDir = "", relativePath = ""]) => safeProjectMediaPath(projectDir, relativePath)),
    }).toMatchInlineSnapshot(`
      {
        "folderLabels": [
          "Generated selects / Replacements",
          "Source footage",
          null,
          null,
          "B / A",
          "Orphan",
        ],
        "generatedAssetsForItems": [
          [
            "item-1",
            null,
          ],
          [
            "sample-generated-clip",
            "sample-generated-shot",
          ],
          [
            "replacement-clip",
            "generated-replacement",
          ],
          [
            "template-overlay",
            null,
          ],
          [
            "unknown-template-overlay",
            null,
          ],
          [
            "text-overlay",
            null,
          ],
          [
            "caption-1",
            null,
          ],
          [
            "caption-2",
            null,
          ],
          [
            "music-bed",
            null,
          ],
        ],
        "generationTracks": {
          "allLocked": null,
          "lockedAudio": null,
          "video": "track-video",
        },
        "mediaTimelineActions": [
          [
            "media-1",
            {
              "action": {
                "items": [
                  {
                    "durationSeconds": 4,
                    "id": "timeline-media-1",
                    "kind": "video_clip",
                    "label": "input.mp4",
                    "properties": {
                      "sourceIn": 0,
                      "sourceOut": 4,
                    },
                    "source": {
                      "mediaId": "media-1",
                      "type": "media",
                    },
                    "startSeconds": 12,
                  },
                ],
                "targetTrackId": "track-video",
                "type": "addItems",
              },
              "itemId": "timeline-media-1",
            },
          ],
          [
            "media-voiceover",
            null,
          ],
          [
            "sample-generated-output",
            {
              "action": {
                "items": [
                  {
                    "durationSeconds": 4,
                    "id": "timeline-sample-generated-output",
                    "kind": "video_clip",
                    "label": "product-reveal.mp4",
                    "properties": {
                      "sourceIn": 0,
                      "sourceOut": 4,
                    },
                    "source": {
                      "mediaId": "sample-generated-output",
                      "type": "media",
                    },
                    "startSeconds": 12,
                  },
                ],
                "targetTrackId": "track-video",
                "type": "addItems",
              },
              "itemId": "timeline-sample-generated-output",
            },
          ],
          [
            "generated-replacement-output",
            {
              "action": {
                "items": [
                  {
                    "durationSeconds": 4,
                    "id": "timeline-generated-replacement-output",
                    "kind": "video_clip",
                    "label": "output.mp4",
                    "properties": {
                      "sourceIn": 0,
                      "sourceOut": 4,
                    },
                    "source": {
                      "mediaId": "generated-replacement-output",
                      "type": "media",
                    },
                    "startSeconds": 12,
                  },
                ],
                "targetTrackId": "track-video",
                "type": "addItems",
              },
              "itemId": "timeline-generated-replacement-output",
            },
          ],
          [
            "media-missing",
            null,
          ],
        ],
        "missingTrack": null,
        "placementLabels": [
          "Timeline target",
          "Library",
          "Replacement target",
          "Replacement target",
          null,
          null,
          null,
        ],
        "previewUrls": [
          "/media/input.mp4",
          null,
          null,
          null,
        ],
        "safePaths": [
          "/tmp/project/media/a.mp4",
          "C:/projects/demo/media/a.mp4",
          null,
          null,
          null,
          null,
          null,
          null,
          null,
        ],
        "tracksForItems": [
          [
            "item-1",
            "track-video",
          ],
          [
            "sample-generated-clip",
            "track-video",
          ],
          [
            "replacement-clip",
            "track-video",
          ],
          [
            "template-overlay",
            "track-overlays",
          ],
          [
            "unknown-template-overlay",
            "track-overlays",
          ],
          [
            "text-overlay",
            "track-overlays",
          ],
          [
            "caption-1",
            "track-captions",
          ],
          [
            "caption-2",
            "track-captions",
          ],
          [
            "music-bed",
            "track-audio",
          ],
        ],
      }
    `);
  });

  it("builds agent setup snippets for every client", () => {
    const clients: AgentSetupClient[] = ["codex", "claudeCode", "claudeDesktop", "cursor"];
    expect({
      snippets: Object.fromEntries(clients.map((client) => [client, agentSetupSnippet(client, "/tmp/project")])),
      blankProjectDir: agentSetupSnippet("codex", "   "),
    }).toMatchInlineSnapshot(`
      {
        "blankProjectDir": "Video Creater agent setup
      Project folder: project manifest
      Structured proposals only: inspect text project files and return proposal-shaped edits.
      Rust validates project actions before canonical state changes.
      Editable files: video-creater.project.json, timeline.json, media/index.json, templates, queue records.
      Codex command: codex app-server --stdio
      Use the project folder as cwd and ask Codex for validated timeline, media, render, or generation actions.",
        "snippets": {
          "claudeCode": "Video Creater agent setup
      Project folder: /tmp/project
      Structured proposals only: inspect text project files and return proposal-shaped edits.
      Rust validates project actions before canonical state changes.
      Editable files: video-creater.project.json, timeline.json, media/index.json, templates, queue records.
      Claude Code: open this project folder as the workspace and inspect split project files before proposing edits.
      Start with timeline.json, media/index.json, templates, and queue records; return structured proposal JSON only.",
          "claudeDesktop": "Video Creater agent setup
      Project folder: /tmp/project
      Structured proposals only: inspect text project files and return proposal-shaped edits.
      Rust validates project actions before canonical state changes.
      Editable files: video-creater.project.json, timeline.json, media/index.json, templates, queue records.
      Claude Desktop: add this folder as project context, then request structured proposal JSON only.
      Start by reading timeline.json, media/index.json, templates, and recent queue records before proposing changes.",
          "codex": "Video Creater agent setup
      Project folder: /tmp/project
      Structured proposals only: inspect text project files and return proposal-shaped edits.
      Rust validates project actions before canonical state changes.
      Editable files: video-creater.project.json, timeline.json, media/index.json, templates, queue records.
      Codex command: codex app-server --stdio
      Use the project folder as cwd and ask Codex for validated timeline, media, render, or generation actions.",
          "cursor": "Video Creater agent setup
      Project folder: /tmp/project
      Structured proposals only: inspect text project files and return proposal-shaped edits.
      Rust validates project actions before canonical state changes.
      Editable files: video-creater.project.json, timeline.json, media/index.json, templates, queue records.
      Cursor: open the project folder, inspect media/index.json and timeline.json, then draft proposal-shaped edits.
      Do not directly rewrite canonical project files unless the editor asks for a manual text-file repair.",
        },
      }
    `);
  });

  it("labels project folders, profile initials, and Codex tools", () => {
    expect({
      folders: [projectFolderLabel(" /tmp/project "), projectFolderLabel(""), projectFolderLabel("   ")],
      initials: [
        projectProfileInitial("edison demo"),
        projectProfileInitial("  ünicode"),
        projectProfileInitial("   "),
        projectProfileInitial("1st cut"),
      ],
      tools: [
        "video_creater.inspect_timeline",
        "video_creater.add_clips",
        "video_creater.split_clips",
        "video_creater.set_clip_properties",
        "video_creater.export_project",
        "video_creater.search_media",
        "video_creater.unknown_tool",
        "",
      ].map(codexToolDisplayName),
    }).toMatchInlineSnapshot(`
      {
        "folders": [
          "/tmp/project",
          "project manifest",
          "project manifest",
        ],
        "initials": [
          "E",
          "Ü",
          "V",
          "1",
        ],
        "tools": [
          "Inspect Timeline",
          "Add Clips",
          "Split Clips",
          "Set Clip Properties",
          "Export Project",
          "Search Media",
          "video_creater.unknown_tool",
          "",
        ],
      }
    `);
  });
});
