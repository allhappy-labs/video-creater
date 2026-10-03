import type { PreparedProjectPreview, VideoProject } from "@/lib/project";
import { playheadForPlayStart } from "@/lib/preview/playback-clock";
import type { EditorSliceCreator } from "./editor-store";

type PreviewSourceSelection = { readonly kind: "timeline" } | { readonly kind: "asset"; readonly mediaId: string };

/**
 * Canonical (backend-prepared) preview frames for one project snapshot. A state only applies while
 * `sourceProject` is the current project object. `unavailable` means there is no backend, so the
 * preview stays DOM-only without an error.
 */
export type CanonicalPreparation =
  | { readonly sourceProject: VideoProject; readonly status: "preparing" }
  | { readonly sourceProject: VideoProject; readonly status: "ready"; readonly result: PreparedProjectPreview }
  | { readonly sourceProject: VideoProject; readonly status: "failed"; readonly message: string }
  | { readonly sourceProject: VideoProject; readonly status: "unavailable" };

const playableAssetKinds: ReadonlySet<string> = new Set(["video", "generated", "audio", "lottie"]);

export interface PlaybackSlice {
  readonly playheadSeconds: number;
  /** Timeline playback in timeline mode; the previewed asset's playback in asset mode. */
  readonly playing: boolean;
  readonly previewSource: PreviewSourceSelection;
  readonly fullscreen: boolean;
  /** Last playhead per timeline id, restored when switching back to that timeline. */
  readonly timelinePlayheads: Readonly<Record<string, number>>;
  /** Null when the project needs no canonical preview. */
  readonly canonicalPreparation: CanonicalPreparation | null;
  /** Bumped by Retry; the preparation hook re-runs when it changes. */
  readonly canonicalRetryToken: number;
  /** Text overlay or caption whose text is being edited inline on the preview canvas. */
  readonly editingTextItemId: string | null;
  seek(seconds: number): void;
  setPlaying(playing: boolean): void;
  togglePlaying(): void;
  previewAsset(mediaId: string): void;
  previewTimeline(): void;
  setFullscreen(fullscreen: boolean): void;
  rememberTimelinePlayhead(timelineId: string, seconds: number): void;
  forgetTimelinePlayhead(timelineId: string): void;
  setCanonicalPreparation(preparation: CanonicalPreparation | null): void;
  retryCanonicalPreparation(): void;
  setEditingTextItemId(itemId: string | null): void;
}

export const createPlaybackSlice: EditorSliceCreator<PlaybackSlice> = (set, get) => ({
  playheadSeconds: 0,
  playing: false,
  previewSource: { kind: "timeline" },
  fullscreen: false,
  timelinePlayheads: {},
  canonicalPreparation: null,
  canonicalRetryToken: 0,
  editingTextItemId: null,
  seek: (seconds) => {
    const duration = get().project.timeline.durationSeconds;
    const next = Number.isFinite(seconds) ? Math.min(Math.max(0, seconds), Math.max(0, duration)) : 0;
    set({ playheadSeconds: next });
  },
  setPlaying: (playing) => set({ playing }),
  togglePlaying: () => {
    const { playing, previewSource, project, playheadSeconds } = get();
    if (playing) {
      set({ playing: false });
      return;
    }
    if (previewSource.kind === "asset") {
      const asset = project.media.find((media) => media.id === previewSource.mediaId);
      if (asset && playableAssetKinds.has(asset.kind)) set({ playing: true });
      return;
    }
    const duration = project.timeline.durationSeconds;
    if (!Number.isFinite(duration) || duration <= 0) return;
    set({ playing: true, playheadSeconds: playheadForPlayStart(playheadSeconds, duration) });
  },
  previewAsset: (mediaId) => set({ previewSource: { kind: "asset", mediaId }, playing: false }),
  previewTimeline: () => set({ previewSource: { kind: "timeline" }, playing: false }),
  setFullscreen: (fullscreen) => set({ fullscreen }),
  rememberTimelinePlayhead: (timelineId, seconds) =>
    set((state) => ({
      timelinePlayheads: { ...state.timelinePlayheads, [timelineId]: Number.isFinite(seconds) ? Math.max(0, seconds) : 0 },
    })),
  forgetTimelinePlayhead: (timelineId) =>
    set((state) => {
      if (!(timelineId in state.timelinePlayheads)) return {};
      return {
        timelinePlayheads: Object.fromEntries(
          Object.entries(state.timelinePlayheads).filter(([id]) => id !== timelineId),
        ),
      };
    }),
  setCanonicalPreparation: (canonicalPreparation) => set({ canonicalPreparation }),
  retryCanonicalPreparation: () => set((state) => ({ canonicalRetryToken: state.canonicalRetryToken + 1 })),
  setEditingTextItemId: (editingTextItemId) => {
    if (get().editingTextItemId !== editingTextItemId) set({ editingTextItemId });
  },
});
