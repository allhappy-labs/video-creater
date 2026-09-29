import { afterEach, describe, expect, it } from "vitest";

import type { FileDropEvent } from "../file-drop";
import { listenForBrowserFileDrops } from "./browser-file-drop";

interface FakeTransfer {
  types: string[];
  files: Array<{ name: string; path?: string }>;
  dropEffect: string;
  getData: (format: string) => string;
}

function transfer(overrides: Partial<FakeTransfer> = {}): FakeTransfer {
  return { types: ["Files"], files: [], dropEffect: "none", getData: () => "", ...overrides };
}

function dragEvent(type: string, dataTransfer: FakeTransfer, relatedTarget: EventTarget | null = null) {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperty(event, "dataTransfer", { value: dataTransfer });
  Object.defineProperty(event, "relatedTarget", { value: relatedTarget });
  return event;
}

function setup(options?: { fileNameFallback?: boolean }) {
  const target = document.createElement("section");
  const child = document.createElement("div");
  target.append(child);
  document.body.append(target);
  const events: FileDropEvent[] = [];
  const stop = listenForBrowserFileDrops(target, (event) => events.push(event), options);
  return { target, child, events, stop };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("browser file drops", () => {
  it("marks external file drags as copy targets", () => {
    const { target, events } = setup();
    const data = transfer();
    const over = dragEvent("dragover", data);

    target.dispatchEvent(dragEvent("dragenter", data));
    target.dispatchEvent(over);

    expect(events).toEqual([{ type: "over" }, { type: "over" }]);
    expect(over.defaultPrevented).toBe(true);
    expect(data.dropEffect).toBe("copy");
  });

  it("ignores drags without files, such as internal media payloads", () => {
    const { target, events } = setup();
    const over = dragEvent("dragover", transfer({ types: ["application/x-video-creater-media-id"] }));

    target.dispatchEvent(over);

    expect(events).toEqual([]);
    expect(over.defaultPrevented).toBe(false);
  });

  it("clears only when the drag leaves the drop zone", () => {
    const { target, child, events } = setup();

    target.dispatchEvent(dragEvent("dragleave", transfer(), child));
    expect(events).toEqual([]);

    target.dispatchEvent(dragEvent("dragleave", transfer(), document.body));
    expect(events).toEqual([{ type: "leave" }]);
  });

  it("converts dropped HTML5 files with filesystem paths", () => {
    const { target, events } = setup();
    const file = { name: "clip.mp4", path: "/tmp/clip.mp4" };
    const drop = dragEvent("drop", transfer({ files: [file] }));

    target.dispatchEvent(drop);

    expect(events).toEqual([{ type: "drop", paths: ["/tmp/clip.mp4"], files: [file] }]);
    expect(drop.defaultPrevented).toBe(true);
  });

  it("converts file URI lists when files expose no path", () => {
    const { target, events } = setup();
    const file = { name: "voice one.wav" };
    const data = transfer({
      files: [file],
      getData: (format) => (format === "text/uri-list" ? "file:///tmp/voice%20one.wav\n" : ""),
    });

    target.dispatchEvent(dragEvent("drop", data));

    expect(events).toEqual([{ type: "drop", paths: ["/tmp/voice one.wav"], files: [file] }]);
  });

  it("keeps browser file bytes and adds name paths only for fixture fallback", () => {
    const plain = setup();
    const plainFile = { name: "still.png" };
    const plainDrop = dragEvent("drop", transfer({ files: [plainFile] }));
    plain.target.dispatchEvent(plainDrop);
    expect(plain.events).toEqual([{ type: "drop", paths: [], files: [plainFile] }]);
    expect(plainDrop.defaultPrevented).toBe(true);

    const fixture = setup({ fileNameFallback: true });
    const fixtureFile = { name: "still.png" };
    fixture.target.dispatchEvent(dragEvent("drop", transfer({ files: [fixtureFile] })));
    expect(fixture.events).toEqual([{ type: "drop", paths: ["still.png"], files: [fixtureFile] }]);
  });

  it("stops listening", () => {
    const { target, events, stop } = setup();
    stop();

    target.dispatchEvent(dragEvent("dragover", transfer()));

    expect(events).toEqual([]);
  });
});
