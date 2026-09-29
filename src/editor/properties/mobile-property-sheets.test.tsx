import { describe, expect, it, vi } from "vitest";
import type { SelectionKind } from "@/lib/preview/selection-kind";
import { mobilePropertyTools, propertySheetId } from "./mobile-property-sheets";
import { propertyTabsForKind } from "./property-tabs";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function tools(kind: SelectionKind, hasAudio = true, generated = false) {
  return mobilePropertyTools({ kind, name: "item", tabs: propertyTabsForKind(kind, hasAudio, generated) }).map((tool) => `${tool.label}:${tool.tab.id}`);
}

describe("mobilePropertyTools", () => {
  it("maps each selection kind's clip tools to the Properties tab their sheet shows", () => {
    expect(tools("visual")).toEqual(["Speed:speed", "Volume:audio", "Animation:animation", "Effects:video", "Adjust:video", "AI:ai"]);
    expect(tools("audio")).toEqual(["Speed:speed", "Volume:basic"]);
    expect(tools("audio", true, true)).toEqual(["Speed:speed", "Volume:basic", "AI:ai"]);
    expect(tools("text")).toEqual(["Text:text", "Style:style", "Animation:animation"]);
    expect(tools("caption")).toEqual(["Text:text", "Style:style", "Animation:animation"]);
    expect(tools("template")).toEqual(["Content:content", "Style:style", "Animation:animation", "Effects:effects"]);
    expect(tools("multiple")).toEqual(["Adjust:common"]);
  });

  it("drops a tool whose tab the selection lacks", () => {
    expect(tools("visual", false)).not.toContain("Volume:audio");
    expect(mobilePropertyTools(null)).toEqual([]);
    expect(propertySheetId("adjust")).toBe("property:adjust");
  });
});
