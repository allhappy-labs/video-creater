import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TimelineTrack } from "@/lib/timeline";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";
import { TrackHeader } from "./track-header";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderHeader(kind: TimelineTrack["kind"], name: string, patch: Partial<TimelineTrack> = {}) {
  const project = fixtureProject();
  const track = { ...fixtureTrack(project, kind), ...patch };
  const rendered = renderWithEditorStore(<TrackHeader track={track} name={name} height={34} />, { project });
  const applyActions = vi.fn(async () => null);
  act(() => rendered.store.setState({ applyActions }));
  return { ...rendered, track, applyActions };
}

describe("TrackHeader", () => {
  beforeEach(() => window.localStorage.clear());

  it("shows the kind icon and the short display name", () => {
    const { container } = renderHeader("video", "Video 1");
    const group = screen.getByRole("group", { name: "Video 1 track" });
    expect(group).toHaveTextContent("Video 1");
    expect(container.querySelector("svg.lucide-film")).toBeInTheDocument();
  });

  it("hides and shows visual tracks with setTrackEnabled", () => {
    const { applyActions, track } = renderHeader("overlay", "Text 1");
    fireEvent.click(screen.getByRole("button", { name: "Hide Text 1" }));
    expect(applyActions).toHaveBeenCalledWith([{ type: "setTrackEnabled", trackId: track.id, enabled: false }]);
  });

  it("offers Show for a hidden visual track", () => {
    const { applyActions, track } = renderHeader("video", "Video 1", { enabled: false });
    fireEvent.click(screen.getByRole("button", { name: "Show Video 1" }));
    expect(applyActions).toHaveBeenCalledWith([{ type: "setTrackEnabled", trackId: track.id, enabled: true }]);
  });

  it("mutes and unmutes audio tracks with setTrackEnabled", () => {
    const { applyActions, track, container } = renderHeader("audio", "Audio 1");
    expect(container.querySelector("svg.lucide-music")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Hide Audio 1" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Mute Audio 1" }));
    expect(applyActions).toHaveBeenCalledWith([{ type: "setTrackEnabled", trackId: track.id, enabled: false }]);
  });

  it("offers Unmute for a muted audio track", () => {
    renderHeader("audio", "Audio 1", { enabled: false });
    expect(screen.getByRole("button", { name: "Unmute Audio 1" })).toBeInTheDocument();
  });

  it("locks and unlocks every track kind with setTrackLocked", () => {
    const locked = renderHeader("caption", "Captions", { locked: true });
    fireEvent.click(screen.getByRole("button", { name: "Unlock Captions" }));
    expect(locked.applyActions).toHaveBeenCalledWith([{ type: "setTrackLocked", trackId: locked.track.id, locked: false }]);
    locked.unmount();

    const unlocked = renderHeader("hyperframe_scene", "Graphics 1");
    fireEvent.click(screen.getByRole("button", { name: "Lock Graphics 1" }));
    expect(unlocked.applyActions).toHaveBeenCalledWith([{ type: "setTrackLocked", trackId: unlocked.track.id, locked: true }]);
  });
});
