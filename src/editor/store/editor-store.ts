import { createStore, type StoreApi } from "zustand/vanilla";
import type { VideoProject } from "@/lib/project";
import { createAgentSlice, type AgentSlice } from "./agent-slice";
import { createJobsSlice, type JobsSlice } from "./jobs-slice";
import { createPlaybackSlice, type PlaybackSlice } from "./playback-slice";
import { createProjectSlice, type ProjectSlice } from "./project-slice";
import { createSelectionSlice, type SelectionSlice } from "./selection-slice";
import { createTimelineViewSlice, type TimelineViewSlice } from "./timeline-view-slice";
import { createUiSlice, type UiSlice } from "./ui-slice";

export type EditorState = ProjectSlice & SelectionSlice & PlaybackSlice & UiSlice & TimelineViewSlice & JobsSlice & AgentSlice;

export type EditorSliceCreator<T> = (
  set: StoreApi<EditorState>["setState"],
  get: StoreApi<EditorState>["getState"],
) => T;

export interface EditorStoreInit {
  readonly projectDir: string;
  readonly project: VideoProject;
}

export type EditorStore = StoreApi<EditorState>;

export function createEditorStore(init: EditorStoreInit): EditorStore {
  const store: EditorStore = createStore<EditorState>()((set, get) => ({
    ...createProjectSlice(init)(set, get),
    ...createSelectionSlice(set, get),
    ...createPlaybackSlice(set, get),
    ...createUiSlice(set, get),
    ...createTimelineViewSlice(init.projectDir)(set, get),
    // The jobs slice reaches the store lazily: services it routes to are bound to the store.
    ...createJobsSlice(init, (): EditorStore => store)(set, get),
    ...createAgentSlice()(set, get),
  }));

  // pruneSelection only writes when ids were dropped and never touches `project`, so the
  // nested notification it triggers sees an unchanged project and stops. syncJobs only
  // re-derives tasks when their inputs changed, so its own `tasks` write stops there too, and
  // syncAgentContext only writes the context chip when its label or items changed.
  store.subscribe((state, previous) => {
    if (state.project !== previous.project) state.pruneSelection(state.project);
    state.syncJobs();
    state.syncAgentContext();
  });
  return store;
}
