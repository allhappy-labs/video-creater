import type { VideoProject, VisualEffectCatalog, VisualEffectDescriptor } from "../../project";
import type { FixtureOperationHandler } from "../adapters/fixture-transport";

/**
 * DEV-only backend handlers the editor panel flows need (`panelFixtures: true` on the fixture
 * marker): the effect catalog, and a save that gives the sample its detected silences. With a
 * stateful fixture (`fixtures/index.ts`) the silences are a seed of the shared project store instead.
 */

const sampleProjectId = "project-sample";

/** Three non-color effects with the native catalog's ids, names and parameter ranges. */
const effects: readonly VisualEffectDescriptor[] = [
  {
    id: "blur.gaussian",
    displayName: "Gaussian Blur",
    category: "Blur & Sharpen",
    params: [{ key: "radius", label: "Radius", min: 0, max: 100, defaultValue: 8, unit: "px" }],
    resourceKey: null,
    colorEffect: false,
    controlSchema: null,
  },
  {
    id: "stylize.grain",
    displayName: "Film Grain",
    category: "Stylize",
    params: [
      { key: "amount", label: "Amount", min: 0, max: 1, defaultValue: 0, unit: "" },
      { key: "size", label: "Size", min: 0.5, max: 4, defaultValue: 1.5, unit: "" },
    ],
    resourceKey: null,
    colorEffect: false,
    controlSchema: null,
  },
  {
    id: "stylize.vignette",
    displayName: "Vignette",
    category: "Stylize",
    params: [
      { key: "amount", label: "Amount", min: -1, max: 1, defaultValue: 0, unit: "" },
      { key: "midpoint", label: "Midpoint", min: 0, max: 1, defaultValue: 0.5, unit: "" },
    ],
    resourceKey: null,
    colorEffect: false,
    controlSchema: null,
  },
];

/**
 * Detected silences stored with the sample's source clip. Remove silences pads each side by
 * 0.12 s, so on the timeline these review as 1.0–2.0 s and 2.5–3.3 s.
 */
const sampleSilenceRanges: NonNullable<VideoProject["mediaSilenceRanges"]> = [
  { mediaId: "media-1", sourceIn: 0.88, sourceOut: 2.12, confidence: 0.94, label: null },
  { mediaId: "media-1", sourceIn: 2.38, sourceOut: 3.42, confidence: 0.91, label: null },
];

function effectCatalog(): VisualEffectCatalog {
  return {
    source: "editor-panel-fixture",
    effectCount: effects.length,
    canonicalOrder: effects.map((effect) => effect.id),
    effects: structuredClone([...effects]),
  };
}

/**
 * Like a native split save, the committed project carries the media analysis stored in the
 * folder: the sample's source clip has detected silences unless the project already has some.
 */
export function withSampleSilences(project: VideoProject): VideoProject {
  const hasSourceMedia = project.media.some((media) => media.id === "media-1");
  if (project.id !== sampleProjectId || !hasSourceMedia || (project.mediaSilenceRanges?.length ?? 0) > 0) return project;
  return { ...project, mediaSilenceRanges: structuredClone(sampleSilenceRanges) };
}

export function panelFixtureOperations(): ReadonlyMap<string, FixtureOperationHandler> {
  return new Map<string, FixtureOperationHandler>([
    ["list_visual_effect_catalog", effectCatalog],
    [
      "save_split_project_to_folder",
      (input) => {
        const project = (input as { project?: VideoProject }).project;
        return {
          project: project ? withSampleSilences(project) : project,
          report: { manifestPath: "video-creater.project.json", writtenFiles: [], removedFiles: [] },
        };
      },
    ],
  ]);
}
