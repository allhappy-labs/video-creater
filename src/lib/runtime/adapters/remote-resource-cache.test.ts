import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cacheMediaTicketsForResult,
  clearRemoteResourceUrlsForTests,
  remoteResourceUrl,
} from "./remote-resource-cache";

describe("remote resource cache", () => {
  beforeEach(clearRemoteResourceUrlsForTests);

  it("mints and caches opaque URLs for canonical project-relative media", async () => {
    const fetcher = vi.fn(async () => new Response(JSON.stringify({
      urls: { "opaque-project/media/clip one.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef" },
    })));

    await cacheMediaTicketsForResult(
      "opaque-project",
      { media: [{ relativePath: "media/clip one.mp4" }] },
      "csrf-1",
      fetcher,
    );

    expect(fetcher).toHaveBeenCalledWith("/api/v1/resource-tickets/media", expect.objectContaining({
      method: "POST",
      credentials: "same-origin",
      body: JSON.stringify({ projectId: "opaque-project", relativePaths: ["media/clip one.mp4"] }),
    }));
    expect(remoteResourceUrl("opaque-project/media/clip one.mp4")).toBe("/api/v1/media/0123456789abcdef0123456789abcdef");
    expect(() => remoteResourceUrl("opaque-project/media/missing.mp4")).toThrow("not authorized");
  });

  it("authorizes a derived preview path returned by the host", async () => {
    const fetcher = vi.fn(async () => new Response(JSON.stringify({ urls: {} })));

    await cacheMediaTicketsForResult(
      "opaque-project",
      {
        previewFrame: "renders/frame/preview.png",
        sourceOutput: "renders/frame/source.mp4",
        evidenceReport: "renders/frame/evidence.json",
      },
      "csrf-1",
      fetcher,
    );

    expect(fetcher).toHaveBeenCalledWith("/api/v1/resource-tickets/media", expect.objectContaining({
      body: JSON.stringify({
        projectId: "opaque-project",
        relativePaths: ["renders/frame/preview.png"],
      }),
    }));
  });
});
