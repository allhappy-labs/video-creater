import { expect, test } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

test("prepared lower media stays beneath upper source media and above the canvas background", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openSampleEditor(page);
  await page.route(/\/__editor-fixture-media\/.*(lower|upper)\.png$/, (route) => route.fulfill({
    contentType: "image/svg+xml",
    body: `<svg xmlns="http://www.w3.org/2000/svg" width="160" height="90"><rect width="160" height="90" fill="${route.request().url().endsWith("upper.png") ? "#0000ff" : "#ff0000"}"/></svg>`,
  }));
  await page.evaluate(async () => {
    const load = (path: string) => import(path);
    const { default: React } = await load("/node_modules/.vite/deps/react.js");
    const { default: { createRoot } } = await load("/node_modules/.vite/deps/react-dom_client.js");
    const { createSampleProject } = await load("/src/lib/sample-project.ts");
    const { createEditorStore } = await load("/src/editor/store/editor-store.ts");
    const { EditorStoreProvider } = await load("/src/editor/store/editor-store-context.tsx");
    const { TooltipProvider } = await load("/src/components/ui/tooltip.tsx");
    const { TimelinePreview } = await load("/src/editor/preview/timeline-preview.tsx");
    const project = createSampleProject();
    const original = project.timeline.tracks.find((track: { kind: string }) => track.kind === "video");
    project.timeline.tracks = [{ ...original, items: [{ ...original.items[0], kind: "image_clip", properties: { blendMode: "add" } }] },
      { ...original, id: "upper", name: "Upper", items: [{ ...original.items[0], id: "upper-item", kind: "image_clip", source: { type: "media", mediaId: "upper-image" }, properties: {} }] }];
    project.media = [{ ...project.media[0], kind: "image", relativePath: "lower.png" },
      { ...project.media[0], id: "upper-image", kind: "image", relativePath: "upper.png" }];
    project.timelines = [];
    const store = createEditorStore({ projectDir: "/stacking", project });
    store.getState().setCanonicalPreparation({ sourceProject: project, status: "ready", result: {
      project: structuredClone(project), reports: [], frameSequences: [{ itemId: original.items[0].id, preparedMediaId: project.media[0].id,
        startSeconds: 0, durationSeconds: 4, fps: 1, framePaths: ["lower.png"] }],
    } });
    const host = document.createElement("div");
    host.setAttribute("data-testid", "stacking-preview");
    host.style.cssText = "position:fixed;inset:20px;width:800px;height:500px;display:flex;z-index:9999;background:#101010";
    document.body.append(host);
    createRoot(host).render(React.createElement(TooltipProvider, null,
      React.createElement(EditorStoreProvider, { store }, React.createElement(TimelinePreview, { fullscreen: false, onToggleFullscreen() {}, onOpenSource() {} }))));
  });
  const preview = page.getByTestId("stacking-preview");
  const canvas = preview.getByTestId("preview-canvas");
  await expect(preview.getByTestId("canonical-prepared-preview-frame")).toBeVisible();
  await expect.poll(() => preview.locator("img").evaluateAll((images) => images.every((image) => (image as HTMLImageElement).complete && (image as HTMLImageElement).naturalWidth > 0))).toBe(true);

  const centerPixel = async () => {
    const screenshot = (await canvas.screenshot()).toString("base64");
    return page.evaluate(async (base64) => {
      const image = new Image();
      image.src = `data:image/png;base64,${base64}`;
      await image.decode();
      const sample = document.createElement("canvas");
      sample.width = image.width; sample.height = image.height;
      const context = sample.getContext("2d")!;
      context.drawImage(image, 0, 0);
      return [...context.getImageData(Math.floor(image.width / 2), Math.floor(image.height / 2), 1, 1).data].slice(0, 3);
    }, screenshot);
  };
  expect(await centerPixel()).toEqual([0, 0, 255]);
  await preview.locator('[data-item-id="upper-item"]').evaluate((element) => { (element as HTMLElement).style.visibility = "hidden"; });
  expect(await centerPixel()).toEqual([255, 0, 0]);
});
