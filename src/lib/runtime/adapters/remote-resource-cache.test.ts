import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cacheMediaTicketsForResult,
  clearRemoteResourceUrlsForTests,
  remoteResourceUrl,
  remoteMediaReadiness,
  retryRemoteMediaTickets,
  subscribeRemoteMediaReadiness,
  refreshRemoteMediaUrl,
  ensureRemoteMediaPaths,
} from "./remote-resource-cache";

describe("remote resource cache", () => {
  beforeEach(clearRemoteResourceUrlsForTests);

  it("keeps newer frame tickets when an older preparation response arrives late", async () => {
    const fetcher = async (_url: RequestInfo | URL, init?: RequestInit) => {
      const { relativePaths } = JSON.parse(String(init?.body)) as { relativePaths: string[] };
      return new Response(JSON.stringify({ urls: Object.fromEntries(relativePaths.map((path) => [`p/${path}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
    };
    const prepared = (revision: number) => ({ project: { contentRevision: revision, media: [{ relativePath: `cache/${revision}.mov` }] }, frameSequences: [{ framePaths: [`cache/${revision}.png`] }] });
    await cacheMediaTicketsForResult("p", prepared(2), "csrf", fetcher);
    await ensureRemoteMediaPaths("p", ["cache/2.png"]);
    await cacheMediaTicketsForResult("p", prepared(1), "csrf", fetcher);
    expect(remoteResourceUrl("p/cache/2.png")).not.toBe("");
    expect(remoteResourceUrl("p/cache/1.mov")).toBe("");
  });

  it("retires old prepared frames before renewing tickets after several edits", async () => {
    vi.useFakeTimers();
    try {
      const batches: string[][] = [];
      const fetcher = async (_url: RequestInfo | URL, init?: RequestInit) => {
        const { relativePaths } = JSON.parse(String(init?.body)) as { relativePaths: string[] };
        batches.push(relativePaths);
        return new Response(JSON.stringify({ urls: Object.fromEntries(relativePaths.map((path) => [`p/${path}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
      };
      for (const revision of [1, 2, 3]) {
        const path = `cache/precompose/revision-${revision}/frames/0.png`;
        await cacheMediaTicketsForResult("p", { project: { media: [{ relativePath: `cache/precompose/revision-${revision}/intermediate.mov` }] }, frameSequences: [{ framePaths: [path] }] }, "csrf", fetcher);
        await ensureRemoteMediaPaths("p", [path]);
      }
      batches.length = 0;
      vi.setSystemTime(Date.now() + 5 * 60_000);
      await retryRemoteMediaTickets("p");
      expect(batches.flat().sort()).toEqual(["cache/precompose/revision-3/frames/0.png", "cache/precompose/revision-3/intermediate.mov"]);
    } finally { vi.useRealTimers(); }
  });

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
    expect(remoteResourceUrl("opaque-project/media/missing.mp4")).toBe("");
  });

  it("exposes failed authorization as recoverable media state and retries only tickets", async () => {
    let unavailable = true;
    const fetcher = vi.fn(async (_url: RequestInfo | URL) => unavailable
      ? new Response(JSON.stringify({ message: "Media temporarily unavailable" }), { status: 503 })
      : new Response(JSON.stringify({ urls: { "p/media/a.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef" } })));
    await cacheMediaTicketsForResult("p", { media: [{ relativePath: "media/a.mp4" }] }, "csrf", fetcher);
    expect(remoteMediaReadiness("p")).toMatchObject({ status: "failed", message: "Media temporarily unavailable" });
    expect(remoteResourceUrl("p/media/a.mp4")).toBe("");
    unavailable = false;
    await retryRemoteMediaTickets("p");
    expect(remoteMediaReadiness("p").status).toBe("ready");
    expect(remoteResourceUrl("p/media/a.mp4")).toBe("/api/v1/media/0123456789abcdef0123456789abcdef");
    expect(fetcher.mock.calls.every(([url]) => url === "/api/v1/resource-tickets/media")).toBe(true);
  });

  it("marks incomplete or unsafe ticket replies failed without accepting another project's URLs", async () => {
    await cacheMediaTicketsForResult("p", { media: [{ relativePath: "media/a.mp4" }] }, "csrf", async () => new Response(JSON.stringify({ urls: {
      "other/media/a.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef",
      "p/media/a.mp4": "https://external.test/private",
    } })));
    expect(remoteMediaReadiness("p").status).toBe("failed");
    expect(remoteResourceUrl("other/media/a.mp4")).toBe("");
    expect(remoteResourceUrl("p/media/a.mp4")).toBe("");
  });

  it("reuses live resource tickets and reacquires expired resources", async () => {
    vi.useFakeTimers();
    try {
      const fetcher = vi.fn(async () => new Response(JSON.stringify({ urls: { "p/media/a.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef" } })));
      const result = { media: [{ relativePath: "media/a.mp4" }] };
      await cacheMediaTicketsForResult("p", result, "csrf", fetcher);
      await cacheMediaTicketsForResult("p", result, "csrf", fetcher);
      expect(fetcher).toHaveBeenCalledTimes(1);
      vi.setSystemTime(Date.now() + 5 * 60_000);
      expect(remoteResourceUrl("p/media/a.mp4")).toBe("");
      await cacheMediaTicketsForResult("p", result, "csrf", fetcher);
      expect(fetcher).toHaveBeenCalledTimes(2);
    } finally { vi.useRealTimers(); }
  });

  it("coalesces pending resources and retains failures from concurrent result batches", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const fetcher = vi.fn(async (_url: RequestInfo | URL, init?: RequestInit) => {
      const { relativePaths } = JSON.parse(String(init?.body)) as { relativePaths: string[] };
      if (relativePaths.includes("media/a.mp4")) await gate;
      return relativePaths.includes("media/a.mp4")
        ? new Response(JSON.stringify({ message: "First media unavailable" }), { status: 503 })
        : new Response(JSON.stringify({ urls: { "p/media/b.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef" } }));
    });
    const first = cacheMediaTicketsForResult("p", { relativePath: "media/a.mp4" }, "csrf", fetcher);
    const same = cacheMediaTicketsForResult("p", { relativePath: "media/a.mp4" }, "csrf", fetcher);
    await cacheMediaTicketsForResult("p", { relativePath: "media/b.mp4" }, "csrf", fetcher);
    release();
    await Promise.all([first, same]);
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(remoteMediaReadiness("p")).toMatchObject({ status: "failed", message: "First media unavailable" });
    expect(remoteResourceUrl("p/media/b.mp4")).not.toBe("");
  });

  it("bounds request batches to the host limit while authorizing canonical frame paths", async () => {
    const requests: string[][] = [];
    const fetcher = async (_url: RequestInfo | URL, init?: RequestInit) => {
      const { relativePaths } = JSON.parse(String(init?.body)) as { relativePaths: string[] };
      requests.push(relativePaths);
      return new Response(JSON.stringify({ urls: Object.fromEntries(relativePaths.map((path) => [`p/${path}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
    };
    await cacheMediaTicketsForResult("p", { framePaths: Array.from({ length: 1100 }, (_, i) => `frames/${i}.png`) }, "csrf", fetcher);
    expect(requests).toHaveLength(2);
    expect(requests.every((paths) => paths.length <= 1024)).toBe(true);
    expect(remoteMediaReadiness("p").status).toBe("ready");
  });

  it("refreshes active media before the actual server expiry and bounds automatic load-error retries", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const unsubscribe = subscribeRemoteMediaReadiness(() => undefined, "p");
    try {
      let mint = 0;
      const fetcher = vi.fn(async () => {
        mint += 1;
        return new Response(JSON.stringify({ urls: { "p/a.mp4": `/api/v1/media/${mint.toString(16).padStart(32, "0")}` }, expiresAt: { "p/a.mp4": Date.now() / 1000 + 20 } }));
      });
      await cacheMediaTicketsForResult("p", { relativePath: "a.mp4" }, "csrf", fetcher);
      await vi.advanceTimersByTimeAsync(5000);
      expect(mint).toBe(2);
      const refreshed = remoteResourceUrl("p/a.mp4");
      expect(refreshed).toBe("/api/v1/media/00000000000000000000000000000002");
      expect(refreshRemoteMediaUrl(refreshed)).toBe(true);
      await vi.advanceTimersByTimeAsync(0);
      expect(mint).toBe(3);
      expect(refreshRemoteMediaUrl(remoteResourceUrl("p/a.mp4"))).toBe(false);
      expect(refreshRemoteMediaUrl(remoteResourceUrl("p/a.mp4"), true)).toBe(true);
      await vi.advanceTimersByTimeAsync(0);
      expect(mint).toBe(4);
    } finally { unsubscribe(); clearRemoteResourceUrlsForTests(); vi.useRealTimers(); }
  });

  it("forgets failed resources removed by a later canonical project", async () => {
    await cacheMediaTicketsForResult("p", { media: [{ relativePath: "removed.mp4" }] }, "csrf", async () => new Response("{}", { status: 503 }));
    await cacheMediaTicketsForResult("p", { media: [{ relativePath: "kept.mp4" }] }, "csrf", async () => new Response(JSON.stringify({ urls: { "p/kept.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef" } })));
    expect(remoteMediaReadiness("p").status).toBe("ready");
  });

  it("forgets failed resources when the canonical project removes all media", async () => {
    const fetcher = vi.fn(async () => new Response("{}", { status: 503 }));
    await cacheMediaTicketsForResult("p", { media: [{ relativePath: "removed.mp4" }] }, "csrf", fetcher);
    expect(remoteMediaReadiness("p").status).toBe("failed");
    await cacheMediaTicketsForResult("p", { media: [] }, "csrf", fetcher);
    expect(remoteMediaReadiness("p").status).toBe("ready");
    await retryRemoteMediaTickets("p");
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it("evicts inactive projects and rejects retired-session ticket completions", async () => {
    const fetcher = async (_url: RequestInfo | URL, init?: RequestInit) => {
      const { projectId } = JSON.parse(String(init?.body)) as { projectId: string };
      return new Response(JSON.stringify({ urls: { [`${projectId}/a.mp4`]: "/api/v1/media/0123456789abcdef0123456789abcdef" } }));
    };
    for (let i = 0; i < 9; i += 1) await cacheMediaTicketsForResult(`p${i}`, { relativePath: "a.mp4" }, "csrf", fetcher);
    expect(remoteResourceUrl("p0/a.mp4")).toBe("");
    expect(remoteResourceUrl("p8/a.mp4")).not.toBe("");
    let release!: (response: Response) => void;
    const pending = cacheMediaTicketsForResult("pending", { relativePath: "a.mp4" }, "csrf", async () => new Promise<Response>((resolve) => { release = resolve; }));
    await Promise.resolve();
    clearRemoteResourceUrlsForTests();
    release(new Response(JSON.stringify({ urls: { "pending/a.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef" } })));
    await pending;
    expect(remoteResourceUrl("pending/a.mp4")).toBe("");
    expect(remoteMediaReadiness("pending").status).toBe("idle");
  });

  it("bounds retained identities for a project beyond the preview cache budget", async () => {
    await cacheMediaTicketsForResult("p", { framePaths: Array.from({ length: 8200 }, (_, i) => `frames/${i.toString().padStart(5, "0")}.png`) }, "csrf", async (_url, init) => {
      const { relativePaths } = JSON.parse(String(init?.body)) as { relativePaths: string[] };
      return new Response(JSON.stringify({ urls: Object.fromEntries(relativePaths.map((path) => [`p/${path}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
    });
    expect(remoteResourceUrl("p/frames/00000.png")).toBe("");
    expect(remoteResourceUrl("p/frames/08199.png")).not.toBe("");
    expect(remoteMediaReadiness("p")).toMatchObject({ status: "failed", message: "This project exceeds the media preview cache limit." });
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

it("preserves original source tickets when a transient prepared project returns", async () => {
  clearRemoteResourceUrlsForTests();
  const fetcher = async (_url: RequestInfo | URL, init?: RequestInit) => {
    const { relativePaths } = JSON.parse(String(init?.body)) as { relativePaths: string[] };
    return new Response(JSON.stringify({ urls: Object.fromEntries(relativePaths.map((path) => [`p/${path}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
  };
  await cacheMediaTicketsForResult("p", { media: [{ relativePath: "original.lottie" }] }, "csrf", fetcher);
  await cacheMediaTicketsForResult("p", { project: { media: [{ relativePath: "cache/prepared.mov" }] }, frameSequences: [] }, "csrf", fetcher);
  expect(remoteResourceUrl("p/original.lottie")).not.toBe("");
});

it("loads prepared animation tickets on demand without exhausting the frame cache", async () => {
  clearRemoteResourceUrlsForTests();
  const requested: string[] = [];
  await cacheMediaTicketsForResult("p", {
    project: { media: [{ relativePath: "cache/prepared.mov" }] },
    frameSequences: [{ framePaths: Array.from({ length: 9000 }, (_, i) => `frames/${i}.png`) }],
  }, "csrf", async (_url, init) => {
    const { relativePaths } = JSON.parse(String(init?.body)) as { relativePaths: string[] };
    requested.push(...relativePaths);
    return new Response(JSON.stringify({ urls: Object.fromEntries(relativePaths.map((path) => [`p/${path}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
  });
  expect(requested).toEqual(["cache/prepared.mov"]);
  await ensureRemoteMediaPaths("p", ["frames/0.png", "frames/1.png"]);
  await ensureRemoteMediaPaths("p", ["frames/8999.png"]);
  expect(remoteResourceUrl("p/frames/0.png")).not.toBe("");
  expect(remoteResourceUrl("p/frames/8999.png")).not.toBe("");
  expect(requested).toHaveLength(4);
  expect(remoteMediaReadiness("p").status).toBe("ready");
});

it("retains canonical sources while an animation advances beyond the cache budget", async () => {
  clearRemoteResourceUrlsForTests();
  const fetcher = async (_url: RequestInfo | URL, init?: RequestInit) => {
    const { relativePaths } = JSON.parse(String(init?.body)) as { relativePaths: string[] };
    return new Response(JSON.stringify({ urls: Object.fromEntries(relativePaths.map((path) => [`p/${path}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
  };
  await cacheMediaTicketsForResult("p", { media: [{ relativePath: "media/source.mp4" }] }, "csrf", fetcher);
  for (let offset = 0; offset < 9000; offset += 1000) {
    await ensureRemoteMediaPaths("p", Array.from({ length: 1000 }, (_, i) => `frames/${offset + i}.png`));
  }
  expect(remoteResourceUrl("p/media/source.mp4")).not.toBe("");
  expect(remoteResourceUrl("p/frames/8999.png")).not.toBe("");
  expect(remoteMediaReadiness("p").status).toBe("ready");
});
