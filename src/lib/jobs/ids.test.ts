import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  generatedCodexEditJobId,
  generatedMediaExportJobId,
  generatedNleExportJobId,
  generatedRenderAttemptId,
  generatedRenderJobId,
  generatedSaveRangeJobId,
  generatedTranscribeMediaJobId,
  videoExportJobIdProfile,
} from "@/lib/jobs/ids";

describe("job id characterization", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-13T00:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("derives time-based job ids", () => {
    expect({
      codexEdit: generatedCodexEditJobId(),
      nlePremiere: generatedNleExportJobId("premiereXmeml"),
      nleDavinci: generatedNleExportJobId("davinciFcpxml"),
      renderDraft: generatedRenderJobId("draft"),
      renderFinal: generatedRenderJobId("final"),
      saveRange: generatedSaveRangeJobId(),
      mediaExport: (["webm", "mp4H264", "mp4H265", "proResMov", "palmierProject"] as const).map(
        (profile) => generatedMediaExportJobId(profile),
      ),
    }).toMatchInlineSnapshot(`
      {
        "codexEdit": "codex-edit-mtz1s000",
        "mediaExport": [
          "export-webm-mtz1s000",
          "export-mp4H264-mtz1s000",
          "export-mp4H265-mtz1s000",
          "export-proResMov-mtz1s000",
          "export-palmierProject-mtz1s000",
        ],
        "nleDavinci": "nle-export-davinci-mtz1s000",
        "nlePremiere": "nle-export-premiere-mtz1s000",
        "renderDraft": "render-draft-mtz1s000",
        "renderFinal": "render-final-mtz1s000",
        "saveRange": "save-range-mtz1s000",
      }
    `);
  });

  it("reads the video profile back from media export job ids", () => {
    expect(videoExportJobIdProfile(generatedMediaExportJobId("mp4H264"))).toBe("mp4H264");
    expect(videoExportJobIdProfile("export-proResMov-mu2aawcr")).toBe("proResMov");
    expect(videoExportJobIdProfile(generatedMediaExportJobId("palmierProject"))).toBeNull();
    expect(videoExportJobIdProfile(generatedRenderJobId("draft"))).toBeNull();
    expect(videoExportJobIdProfile(generatedSaveRangeJobId())).toBeNull();
    expect(videoExportJobIdProfile("export-mp4h264-mu2aawcr")).toBeNull();
  });

  it("slugs transcribe media job ids", () => {
    expect(
      ["media-1", "  Media 1 ", "--Clip__Name.MOV--", "***", ""].map((mediaId) =>
        generatedTranscribeMediaJobId(mediaId),
      ),
    ).toMatchInlineSnapshot(`
      [
        "transcribe-media-1-mtz1s000",
        "transcribe-media-1-mtz1s000",
        "transcribe-clip-name-mov-mtz1s000",
        "transcribe-media-mtz1s000",
        "transcribe-media-mtz1s000",
      ]
    `);
  });

  it("derives render attempt ids from randomUUID with a time and Math.random fallback", () => {
    vi.spyOn(globalThis.crypto, "randomUUID").mockReturnValue("00000000-0000-4000-8000-000000000000");
    expect(generatedRenderAttemptId()).toMatchInlineSnapshot(`"render-attempt/00000000-0000-4000-8000-000000000000"`);

    vi.spyOn(Math, "random").mockReturnValue(0.25);
    vi.stubGlobal("crypto", {});
    expect(generatedRenderAttemptId()).toMatchInlineSnapshot(`"render-attempt/1789257600000-0.25"`);
    vi.stubGlobal("crypto", undefined);
    expect(generatedRenderAttemptId()).toMatchInlineSnapshot(`"render-attempt/1789257600000-0.25"`);
  });
});
