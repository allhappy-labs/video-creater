import { describe, expect, it } from "vitest";
import type { ExportProfileAvailability } from "@/lib/project";
import { draftExportDimensions, fallbackExportProfileAvailability, mediaExportOutputPath } from "./profiles";
import {
  defaultExportChoices,
  estimatedExportSize,
  exportChoiceOptions,
  exportChoicesFromPreset,
  exportFrameRateOptions,
  exportPlan,
  replanExport,
  type ExportChoices,
  type ExportPlanInput,
} from "./export-plan";

function available(overrides: Partial<Record<ExportProfileAvailability["profile"], boolean>> = {}): ExportProfileAvailability[] {
  return fallbackExportProfileAvailability.map((profile) => {
    const on = overrides[profile.profile] ?? true;
    return {
      ...profile,
      available: on,
      policyStatus: on ? "approved" : "policyGated",
      unavailableReason: on ? null : `${profile.label} is not approved in this build.`,
      qualityAvailability: { draft: on, final: on },
      qualityUnavailableReasons: on ? {} : { draft: "Not approved.", final: "Not approved." },
    };
  });
}

const project = { id: "project-1", schemaVersion: 2, renderSettings: { width: 1920, height: 1080, fps: 30 } };

function choices(overrides: Partial<ExportChoices> = {}): ExportChoices {
  return { name: "Edison intro", format: "mp4", resolution: "1080p", quality: "high", codec: "h264", fps: null, directory: null, ...overrides };
}

function input(overrides: Partial<ExportPlanInput> = {}): ExportPlanInput {
  return { choices: choices(), profiles: available(), project, projectDir: "/projects/edison", jobId: "export-1", ...overrides };
}

describe("exportPlan", () => {
  it("maps MP4 / 1080p / High onto the H.264 profile at 1920×1080", () => {
    const plan = exportPlan(input());
    expect(plan).toMatchObject({ profile: "mp4H264", quality: "final", width: 1920, height: 1080, fps: 30, codecLabel: "H.264", extension: "mp4", blockedReason: null });
    const output = { fileName: "Edison intro", directory: null };
    expect(plan.render).toEqual({ projectDir: "/projects/edison", projectId: "project-1", profile: "mp4H264", quality: "final", width: 1920, height: 1080, jobId: "export-1", output });
    expect(plan.temporal).toEqual({ projectId: "project-1", projectDir: "/projects/edison", jobId: "export-1", profile: "mp4H264", quality: "final", width: 1920, height: 1080, outputPath: plan.outputPath, output });
  });

  it("sends the chosen export folder with the file name", () => {
    const plan = exportPlan(input({ choices: choices({ directory: "/home/me/Movies" }) }));
    expect(plan.render.output).toEqual({ fileName: "Edison intro", directory: "/home/me/Movies" });
    expect(plan.temporal.output).toEqual({ fileName: "Edison intro", directory: "/home/me/Movies" });
  });

  it("exports Master as Final quality with the Master encode tier", () => {
    const plan = exportPlan(input({ choices: choices({ quality: "master" }) }));
    expect(plan).toMatchObject({ quality: "final", encodeTier: "master", codecLabel: "H.264", blockedReason: null });
    expect(plan.render).toMatchObject({ quality: "final", encodeTier: "master" });
    expect(plan.temporal).toMatchObject({ quality: "final", encodeTier: "master" });
    expect(exportPlan(input()).render).not.toHaveProperty("encodeTier");
    expect(exportPlan(input({ choices: choices({ format: "prores", quality: "master" }) })).blockedReason).toBe(
      "Master quality is for MP4 and WebM exports; ProRes already exports at mastering quality.",
    );
  });

  it("renders at a chosen frame rate and sends no rate when it is the timeline's", () => {
    const at25 = exportPlan(input({ choices: choices({ fps: 25 }) }));
    expect(at25.fps).toBe(25);
    expect(at25.render.fps).toBe(25);
    expect(at25.temporal.fps).toBe(25);
    const atTimeline = exportPlan(input({ choices: choices({ fps: 30 }) }));
    expect(atTimeline.fps).toBe(30);
    expect(atTimeline.render).not.toHaveProperty("fps");
    expect(atTimeline.temporal).not.toHaveProperty("fps");
    expect(exportPlan(input()).render).not.toHaveProperty("fps");
  });

  it("sends Draft's 24 fps cap explicitly so the render matches the summary", () => {
    const draft = exportPlan(input({ choices: choices({ quality: "draft" }) }));
    expect(draft.fps).toBe(24);
    expect(draft.render.fps).toBe(24);
    expect(draft.temporal.fps).toBe(24);
    const slowTimeline = { ...project, renderSettings: { width: 1920, height: 1080, fps: 24 } };
    expect(exportPlan(input({ project: slowTimeline, choices: choices({ quality: "draft" }) })).render).not.toHaveProperty("fps");
  });

  it("blocks a file name the backend would refuse", () => {
    expect(exportPlan(input({ choices: choices({ name: "a/b" }) })).blockedReason).toBe("Export names can't contain slashes.");
  });

  it("takes output paths from mediaExportOutputPath", () => {
    const plan = exportPlan(input({ choices: choices({ format: "prores" }) }));
    expect(plan.outputPath).toBe(mediaExportOutputPath("project-1", "proResMov", "mov", "export-1"));
  });

  it("keeps the timeline aspect ratio for each resolution", () => {
    const vertical = { ...project, renderSettings: { width: 1080, height: 1920, fps: 24 } };
    expect(exportPlan(input({ project: vertical, choices: choices({ resolution: "720p" }) }))).toMatchObject({ width: 720, height: 1280 });
    expect(exportPlan(input({ choices: choices({ resolution: "4k" }) }))).toMatchObject({ width: 3840, height: 2160 });
  });

  it("renders Draft quality at draftExportDimensions and at most 24 fps", () => {
    const plan = exportPlan(input({ choices: choices({ quality: "draft", resolution: "4k" }) }));
    expect(plan).toMatchObject({ quality: "draft", ...draftExportDimensions(3840, 2160), fps: 24 });
  });

  it("applies the Advanced H.265 codec to MP4 only", () => {
    expect(exportPlan(input({ choices: choices({ codec: "h265" }) }))).toMatchObject({ profile: "mp4H265", codecLabel: "H.265", extension: "mp4" });
    expect(exportPlan(input({ choices: choices({ format: "webm", codec: "h265" }) }))).toMatchObject({ profile: "webm", codecLabel: "VP8", extension: "webm" });
  });

  it("returns the disabled reason for an unavailable profile or quality", () => {
    const gated = available({ mp4H265: false });
    expect(exportPlan(input({ profiles: gated, choices: choices({ codec: "h265" }) })).blockedReason).toBe("MP4 / H.265 is not approved in this build.");
    const noDraft = available().map((profile) =>
      profile.profile === "mp4H264" ? { ...profile, qualityAvailability: { draft: false, final: true }, qualityUnavailableReasons: { draft: "Draft H.264 needs the software encoder." } } : profile,
    );
    expect(exportPlan(input({ profiles: noDraft, choices: choices({ quality: "draft" }) })).blockedReason).toBe("Draft H.264 needs the software encoder.");
  });

  it("blocks projects that are not split project folders", () => {
    expect(exportPlan(input({ projectDir: " " })).blockedReason).toBe("Save as a schema-v2 split project to export media.");
    expect(exportPlan(input({ project: { ...project, schemaVersion: 1 } })).blockedReason).toBe("Save as a schema-v2 split project to export media.");
  });

  it("blocks an empty name", () => {
    expect(exportPlan(input({ choices: choices({ name: "  " }) })).blockedReason).toBe("Give the export a name.");
  });

  it("re-plans a stored export under a new job id", () => {
    const plan = exportPlan(input());
    const retry = replanExport(plan, "export-2");
    expect(retry).toMatchObject({ jobId: "export-2", render: { jobId: "export-2" }, temporal: { jobId: "export-2", outputPath: mediaExportOutputPath("project-1", "mp4H264", "mp4", "export-2") } });
    expect(replanExport({ profile: "mp4H264" }, "export-2")).toBeNull();
  });

  it("keeps the folder, frame rate and Master tier when re-planning", () => {
    const plan = exportPlan(input({ choices: choices({ quality: "master", fps: 25, directory: "/home/me/Movies" }) }));
    const retry = replanExport(plan, "export-2");
    const expected = { fps: 25, encodeTier: "master", output: { fileName: "Edison intro", directory: "/home/me/Movies" } };
    expect(retry?.render).toMatchObject(expected);
    expect(retry?.temporal).toMatchObject(expected);
  });
});

describe("exportChoiceOptions", () => {
  it("labels WebM as WebM and keeps an unavailable ProRes listed with its reason", () => {
    const formats = exportChoiceOptions(available(), choices()).format;
    expect(formats.map((option) => option.label)).toEqual(["MP4", "ProRes", "WebM"]);
    const withoutProRes = exportChoiceOptions(available({ proResMov: false }), choices()).format;
    expect(withoutProRes).toContainEqual({ value: "prores", label: "ProRes", disabledReason: "ProRes MOV is not approved in this build." });
  });

  it("disables unavailable options with their reasons", () => {
    const options = exportChoiceOptions(available({ mp4H264: false, mp4H265: false, webm: false }), choices());
    expect(options.format).toEqual([
      { value: "mp4", label: "MP4", disabledReason: "MP4 / H.264 is not approved in this build." },
      { value: "prores", label: "ProRes", disabledReason: null },
      { value: "webm", label: "WebM", disabledReason: "Legacy WebM is not approved in this build." },
    ]);
    // An unavailable format is disabled as a whole; its qualities carry no reason of their own.
    expect(options.quality.find((option) => option.value === "master")?.disabledReason).toBeNull();
    expect(options.codec.map((option) => option.disabledReason)).toEqual(["MP4 / H.264 is not approved in this build.", "MP4 / H.265 is not approved in this build."]);
    const webm = exportChoiceOptions(available(), choices({ format: "webm" }));
    expect(webm.codec.every((option) => option.disabledReason === "WebM always uses VP8.")).toBe(true);
  });

  it("disables qualities the chosen profile can't render", () => {
    const noDraft = available().map((profile) => ({ ...profile, qualityAvailability: { draft: false, final: true }, qualityUnavailableReasons: { draft: "No draft encoder." } }));
    const quality = exportChoiceOptions(noDraft, choices()).quality;
    expect(quality.find((option) => option.value === "draft")?.disabledReason).toBe("No draft encoder.");
    expect(quality.find((option) => option.value === "high")?.disabledReason).toBeNull();
  });

  it("enables Master for MP4 and WebM and explains why ProRes has no Master", () => {
    const master = (profiles: ExportProfileAvailability[], format: ExportChoices["format"]) =>
      exportChoiceOptions(profiles, choices({ format })).quality.find((option) => option.value === "master")?.disabledReason;
    expect(master(available(), "mp4")).toBeNull();
    expect(master(available(), "webm")).toBeNull();
    expect(master(available(), "prores")).toBe("Master quality is for MP4 and WebM exports; ProRes already exports at mastering quality.");
    const noFinal = available().map((profile) => ({ ...profile, qualityAvailability: { draft: true, final: false }, qualityUnavailableReasons: { final: "Final H.264 needs the encoder." } }));
    expect(master(noFinal, "mp4")).toBe("Final H.264 needs the encoder.");
  });
});

describe("export choices", () => {
  it("defaults to the first available format and H.264", () => {
    expect(defaultExportChoices(available(), "Edison")).toEqual({ name: "Edison", format: "mp4", resolution: "1080p", quality: "high", codec: "h264", fps: null, directory: null });
    expect(defaultExportChoices(available({ mp4H264: false, mp4H265: false }), "Edison")).toMatchObject({ format: "webm" });
    expect(defaultExportChoices(available({ mp4H264: false }), "Edison")).toMatchObject({ format: "mp4", codec: "h265" });
  });

  it("presets choices from an earlier job", () => {
    const base = defaultExportChoices(available(), "Edison");
    expect(exportChoicesFromPreset(base, { profile: "mp4H265", quality: "draft" })).toMatchObject({ format: "mp4", codec: "h265", quality: "draft" });
    expect(exportChoicesFromPreset(base, { profile: "webm", quality: "final" })).toMatchObject({ format: "webm", quality: "high" });
    expect(exportChoicesFromPreset(base, { profile: "unknown", quality: null })).toEqual(base);
    expect(exportChoicesFromPreset(base, { profile: "unknown", quality: null, settings: null })).toEqual(base);
  });

  it("restores every recorded export setting", () => {
    const base = defaultExportChoices(available(), "Edison");
    const settings = {
      profile: "webm" as const,
      quality: "final" as const,
      width: 3840,
      height: 2160,
      fps: 25,
      encodeTier: "master" as const,
      output: { fileName: "Edison master", directory: "/home/me/Movies" },
    };
    expect(exportChoicesFromPreset(base, { profile: "mp4H264", quality: "draft", settings })).toEqual({
      name: "Edison master",
      format: "webm",
      codec: "h264",
      quality: "master",
      resolution: "4k",
      fps: 25,
      directory: "/home/me/Movies",
    });
    const portraitDraft = { ...settings, profile: "mp4H265" as const, quality: "draft" as const, width: 720, height: 1280, fps: null, encodeTier: "standard" as const, output: { fileName: "Edison" } };
    expect(exportChoicesFromPreset(base, { profile: null, quality: null, settings: portraitDraft })).toMatchObject({ format: "mp4", codec: "h265", quality: "draft", resolution: "720p", fps: null, directory: null, name: "Edison" });
  });

  it("Retry after a Draft export restores the chosen resolution and frame rate, not Draft's reduced render", () => {
    const base = defaultExportChoices(available(), "Edison");
    const draft = exportPlan(input({ choices: choices({ quality: "draft", resolution: "1080p", fps: null }) }));
    expect(draft).toMatchObject({ width: 1280, height: 720, fps: 24 });
    expect(draft.settings).toEqual({ profile: "mp4H264", quality: "draft", width: 1920, height: 1080, output: { fileName: "Edison intro", directory: null } });

    const restored = exportChoicesFromPreset(base, { profile: null, quality: null, settings: draft.settings });
    expect(restored).toMatchObject({ quality: "draft", resolution: "1080p", fps: null });
    expect(exportPlan(input({ choices: { ...restored, quality: "high" } }))).toMatchObject({ width: 1920, height: 1080, fps: 30 });

    const at25 = exportPlan(input({ choices: choices({ quality: "draft", resolution: "4k", fps: 25 }) }));
    expect(at25.settings).toMatchObject({ width: 3840, height: 2160, fps: 25 });
  });

  it("lists the timeline rate first, then common frame rates", () => {
    const options = exportFrameRateOptions(30);
    expect(options[0]).toEqual({ value: null, label: "Timeline (30 fps)" });
    expect(options.slice(1).map((option) => option.value)).toEqual([23.976, 24, 25, 29.97, 30, 50, 59.94, 60]);
    expect(options.find((option) => option.value === 29.97)?.label).toBe("29.97 fps");
    expect(exportFrameRateOptions(29.97)[0]?.label).toBe("Timeline (29.97 fps)");
  });
});

describe("estimatedExportSize", () => {
  it("grows with duration, resolution and the mastering codec", () => {
    const hd = exportPlan(input());
    const size = estimatedExportSize(hd, 60);
    expect(size).toBeGreaterThan(40_000_000);
    expect(size).toBeLessThan(60_000_000);
    expect(estimatedExportSize(hd, 120)).toBe(size * 2);
    expect(estimatedExportSize(exportPlan(input({ choices: choices({ resolution: "4k" }) })), 60)).toBeGreaterThan(size);
    expect(estimatedExportSize(exportPlan(input({ choices: choices({ quality: "draft" }) })), 60)).toBeLessThan(size);
    expect(estimatedExportSize(exportPlan(input({ choices: choices({ format: "prores" }) })), 60)).toBeGreaterThan(size * 10);
    expect(estimatedExportSize(hd, Number.NaN)).toBe(0);
  });

  it("estimates Master at twice the Final bitrate within 12–60 Mbps", () => {
    const high = exportPlan(input());
    const master = exportPlan(input({ choices: choices({ quality: "master" }) }));
    const audioBytes = (192 * 1_000) / 8;
    expect(estimatedExportSize(high, 1)).toBe((6_000 * 1_000) / 8 + audioBytes);
    expect(estimatedExportSize(master, 1)).toBe((12_000 * 1_000) / 8 + audioBytes);
    const master4k60 = exportPlan(input({ project: { ...project, renderSettings: { width: 1920, height: 1080, fps: 60 } }, choices: choices({ quality: "master", resolution: "4k" }) }));
    expect(estimatedExportSize(master4k60, 1)).toBe((48_000 * 1_000) / 8 + audioBytes);
  });
});
