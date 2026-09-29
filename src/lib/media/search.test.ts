import { describe, expect, it } from "vitest";
import type { ProjectMediaSearchResult, ProjectMediaSearchScope } from "@/lib/project";
import { fixtureGeneratedAsset, fixtureMedia, fixtureProject } from "@/test-utils/editor-fixtures";
import {
  generatedAssetMatchesSearch,
  indexedSearchNeedsRebuild,
  indexedSearchStatusLabels,
  isSearchableText,
  mediaIdsFromIndexedSearch,
  mediaMatchesSearch,
  normalizeSearchText,
  searchScopeUsesLocalGeneratedAssets,
  searchScopeUsesLocalMedia,
  stringField,
} from "@/lib/media/search";

const queries = ["", "  ", "INPUT", "voice over", "voiceover", "émoji", "source footage", "video"];

function baseSearchResult(overrides: Partial<ProjectMediaSearchResult> = {}): ProjectMediaSearchResult {
  return {
    query: "edison",
    limit: 20,
    visualStatus: "ready",
    spokenStatus: "ready",
    groups: { spoken: [], visual: [], metadata: [], generated: [] },
    results: [],
    returned: 0,
    ...overrides,
  };
}

const indexStatuses: Array<[string, ProjectMediaSearchResult]> = [
  [
    "fresh",
    baseSearchResult({
      indexStatus: { stored: true, source: "stored", schemaVersion: 1, projectUpdatedAt: "2026-07-12T00:00:00Z" },
      semanticEncoder: {
        status: "installed",
        model: { id: "clip-vit", version: "1", dimensions: 512 },
        manifestConfigured: true,
        licenseReviewed: true,
        hashVerified: true,
        message: "ok",
      },
    }),
  ],
  [
    "stale",
    baseSearchResult({
      indexStatus: { stored: false, source: "memory", reason: "stale", schemaVersion: 1, projectUpdatedAt: "x" },
    }),
  ],
  [
    "building",
    baseSearchResult({
      spokenStatus: "indexing",
      visualStatus: "indexing",
      indexStatus: { stored: false, reason: "missing", schemaVersion: 1, projectUpdatedAt: "x" },
      semanticEncoder: {
        status: "installed",
        model: null,
        manifestConfigured: true,
        licenseReviewed: true,
        hashVerified: true,
        message: "ok",
      },
    }),
  ],
  [
    "failed",
    baseSearchResult({
      spokenStatus: "noTranscripts",
      visualStatus: "failed",
      indexStatus: {
        stored: false,
        reason: "unavailable",
        schemaVersion: 1,
        projectUpdatedAt: "x",
        storedError: "disk full",
      },
    }),
  ],
  [
    "not persisted",
    baseSearchResult({
      visualStatus: "notInstalled",
      indexStatus: { stored: false, reason: "notPersisted", schemaVersion: 1, projectUpdatedAt: "x" },
    }),
  ],
  ["no index status", baseSearchResult({ visualStatus: "unavailable" })],
];

describe("media search characterization", () => {
  it("normalizes query text", () => {
    expect(queries.map((query) => normalizeSearchText(query))).toMatchInlineSnapshot(`
      [
        "",
        "",
        "input",
        "voice over",
        "voiceover",
        "émoji",
        "source footage",
        "video",
      ]
    `);
    expect(["", "  ", "text", null, undefined].map((value) => isSearchableText(value))).toMatchInlineSnapshot(`
      [
        false,
        true,
        true,
        false,
        false,
      ]
    `);
  });

  it("matches media by id, name, path, kind, and folder label", () => {
    const project = fixtureProject();
    const media = [
      ...project.media,
      { ...fixtureMedia(project, "video"), id: "media-emoji", name: "Émoji reel", relativePath: "media/emoji.mov" },
    ];
    const folderLabels: Record<string, string | null> = {
      "media-1": "Source footage",
      "media-voiceover": "Audio",
      "sample-generated-output": null,
      "media-emoji": null,
    };
    expect(
      queries.map((query) => {
        const normalized = normalizeSearchText(query);
        return [
          query,
          media
            .filter((asset) => mediaMatchesSearch(asset, folderLabels[asset.id] ?? null, normalized))
            .map((asset) => asset.id),
        ];
      }),
    ).toMatchInlineSnapshot(`
      [
        [
          "",
          [
            "media-1",
            "media-voiceover",
            "sample-generated-output",
            "media-emoji",
          ],
        ],
        [
          "  ",
          [
            "media-1",
            "media-voiceover",
            "sample-generated-output",
            "media-emoji",
          ],
        ],
        [
          "INPUT",
          [
            "media-1",
          ],
        ],
        [
          "voice over",
          [],
        ],
        [
          "voiceover",
          [
            "media-voiceover",
          ],
        ],
        [
          "émoji",
          [
            "media-emoji",
          ],
        ],
        [
          "source footage",
          [
            "media-1",
          ],
        ],
        [
          "video",
          [
            "media-1",
            "media-emoji",
          ],
        ],
      ]
    `);
  });

  it("matches generated assets by metadata, placement, references, and lineage", () => {
    const base = fixtureGeneratedAsset(fixtureProject());
    const assets = [
      base,
      { ...base, id: "timeline-asset", placementIntent: "timeline" as const, name: null },
      { ...base, id: "library-asset", placementIntent: "library" as const, parentAssetId: "parent-1" },
      { ...base, id: "replace-asset", placementIntent: "replace:item-1" as const, retryOfAssetId: "retry-1" },
      { ...base, id: "empty-refs", references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null } },
    ];
    const generatedQueries = [
      "",
      "timeline target",
      "library",
      "replacement target",
      "replace:item-1",
      "references media-1",
      "variation of parent-1",
      "retry of retry-1",
      "variation of parent-1 - retry of retry-1",
      "generated selects",
      "sample/generated",
      "LOCAL",
      "completed",
      "nothing-matches",
    ];
    expect(
      generatedQueries.map((query) => [
        query,
        assets
          .filter((asset) =>
            generatedAssetMatchesSearch(asset, normalizeSearchText(query), asset.id === base.id ? "Generated selects" : null),
          )
          .map((asset) => asset.id),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "",
          [
            "sample-generated-shot",
            "timeline-asset",
            "library-asset",
            "replace-asset",
            "empty-refs",
          ],
        ],
        [
          "timeline target",
          [
            "timeline-asset",
          ],
        ],
        [
          "library",
          [
            "library-asset",
          ],
        ],
        [
          "replacement target",
          [
            "replace-asset",
          ],
        ],
        [
          "replace:item-1",
          [
            "replace-asset",
          ],
        ],
        [
          "references media-1",
          [
            "sample-generated-shot",
            "timeline-asset",
            "library-asset",
            "replace-asset",
          ],
        ],
        [
          "variation of parent-1",
          [
            "library-asset",
          ],
        ],
        [
          "retry of retry-1",
          [
            "replace-asset",
          ],
        ],
        [
          "variation of parent-1 - retry of retry-1",
          [],
        ],
        [
          "generated selects",
          [
            "sample-generated-shot",
          ],
        ],
        [
          "sample/generated",
          [
            "sample-generated-shot",
            "timeline-asset",
            "library-asset",
            "replace-asset",
            "empty-refs",
          ],
        ],
        [
          "LOCAL",
          [
            "sample-generated-shot",
            "timeline-asset",
            "library-asset",
            "replace-asset",
            "empty-refs",
          ],
        ],
        [
          "completed",
          [
            "sample-generated-shot",
            "timeline-asset",
            "library-asset",
            "replace-asset",
            "empty-refs",
          ],
        ],
        [
          "nothing-matches",
          [],
        ],
      ]
    `);
  });

  it("reads string fields", () => {
    const record = { text: "value", blank: "  ", empty: "", number: 1, nil: null };
    expect(["text", "blank", "empty", "number", "nil", "missing"].map((key) => stringField(record, key))).toMatchInlineSnapshot(`
      [
        "value",
        null,
        null,
        null,
        null,
        null,
      ]
    `);
  });

  it("collects media ids from indexed search results", () => {
    const result = baseSearchResult({
      groups: {
        spoken: [{ mediaId: "media-1" }, { mediaId: "  " }, { text: "no media id" }],
        visual: [{ mediaId: "media-2" }, { mediaId: 42 }, { mediaId: "media-1" }],
        metadata: [{ mediaId: "media-3" }, {}],
        generated: [
          { mediaIds: ["media-4", "", "  ", 7, null, "media-2"] },
          { mediaIds: "media-5" },
          {},
        ],
      },
    });
    expect(Array.from(mediaIdsFromIndexedSearch(result))).toMatchInlineSnapshot(`
      [
        "media-1",
        "media-2",
        "media-3",
        "media-4",
      ]
    `);
    expect(Array.from(mediaIdsFromIndexedSearch(baseSearchResult()))).toMatchInlineSnapshot(`[]`);
    expect(Array.from(mediaIdsFromIndexedSearch(null))).toMatchInlineSnapshot(`[]`);
  });

  it("labels indexed search status and rebuild need", () => {
    expect(
      indexStatuses.map(([name, result]) => [
        name,
        indexedSearchStatusLabels(result),
        indexedSearchNeedsRebuild(result),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "fresh",
          [
            "Spoken ready",
            "Visual ready",
            "Semantic clip-vit ready",
            "Index current",
          ],
          false,
        ],
        [
          "stale",
          [
            "Spoken ready",
            "Visual ready",
            "Semantic encoder not installed",
            "Index rebuilding",
          ],
          true,
        ],
        [
          "building",
          [
            "Spoken indexing",
            "Visual indexing",
            "Semantic encoder ready",
            "Index not saved",
          ],
          true,
        ],
        [
          "failed",
          [
            "No transcripts",
            "Visual search failed",
            "Semantic encoder not installed",
            "Index unavailable",
          ],
          false,
        ],
        [
          "not persisted",
          [
            "Spoken ready",
            "Visual search not installed",
            "Semantic encoder not installed",
            "Index in memory",
          ],
          false,
        ],
        [
          "no index status",
          [
            "Spoken ready",
            "Visual unavailable",
            "Semantic encoder not installed",
          ],
          false,
        ],
      ]
    `);
    expect([indexedSearchStatusLabels(null), indexedSearchNeedsRebuild(null)]).toMatchInlineSnapshot(`
      [
        [],
        false,
      ]
    `);
  });

  it("maps search scopes to local media and generated assets", () => {
    const scopes: ProjectMediaSearchScope[] = ["visual", "spoken", "both", "metadata", "generated"];
    expect(
      scopes.map((scope) => [scope, searchScopeUsesLocalMedia(scope), searchScopeUsesLocalGeneratedAssets(scope)]),
    ).toMatchInlineSnapshot(`
      [
        [
          "visual",
          false,
          false,
        ],
        [
          "spoken",
          false,
          false,
        ],
        [
          "both",
          true,
          true,
        ],
        [
          "metadata",
          true,
          false,
        ],
        [
          "generated",
          false,
          true,
        ],
      ]
    `);
  });
});
