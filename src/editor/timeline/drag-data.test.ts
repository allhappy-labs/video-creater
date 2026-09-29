import { describe, expect, it } from "vitest";
import { fakeDataTransfer } from "@/test-utils/data-transfer";
import { assetDragMimeType, hasAssetDragData, readAssetDragData, readDropStartSeconds, writeAssetDragData } from "./drag-data";

describe("asset drag data", () => {
  it("round-trips a typed payload under the asset MIME type", () => {
    const dataTransfer = fakeDataTransfer();
    writeAssetDragData(dataTransfer, { kind: "template", id: "kinetic-lower-third-v1" });
    expect(dataTransfer.types).toEqual([assetDragMimeType]);
    expect(dataTransfer.effectAllowed).toBe("copy");
    expect(hasAssetDragData(dataTransfer)).toBe(true);
    expect(readAssetDragData(dataTransfer)).toEqual({ kind: "template", id: "kinetic-lower-third-v1" });
  });

  it("rejects missing and malformed payloads", () => {
    expect(hasAssetDragData(fakeDataTransfer({ "text/plain": "x" }))).toBe(false);
    expect(readAssetDragData(fakeDataTransfer())).toBeNull();
    expect(readAssetDragData(fakeDataTransfer({ [assetDragMimeType]: "{" }))).toBeNull();
    expect(readAssetDragData(fakeDataTransfer({ [assetDragMimeType]: JSON.stringify({ kind: "effect", id: "x" }) }))).toBeNull();
    expect(readAssetDragData(fakeDataTransfer({ [assetDragMimeType]: JSON.stringify({ kind: "media", id: " " }) }))).toBeNull();
  });

  it("reads an explicit start time and treats empty values as absent", () => {
    const key = "application/x-video-creater-start-seconds";
    expect(readDropStartSeconds(fakeDataTransfer({ [key]: "1.5" }))).toBe(1.5);
    expect(readDropStartSeconds(fakeDataTransfer({ [key]: "" }))).toBeNull();
    expect(readDropStartSeconds(fakeDataTransfer({ [key]: "-2" }))).toBeNull();
    expect(readDropStartSeconds(fakeDataTransfer())).toBeNull();
  });
});
