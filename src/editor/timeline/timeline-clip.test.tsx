import "@testing-library/jest-dom/vitest";
import { fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureGeneratedAsset, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";
import { requiredValue } from "@/test-utils/required";
import { TimelineClip } from "./timeline-clip";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

interface ClipOptions {
  readonly selected?: boolean;
  readonly project?: VideoProject;
  readonly onSelect?: (item: TimelineItem, additive: boolean) => void;
}

function renderClip(item: TimelineItem, options: ClipOptions = {}) {
  return renderWithEditorStore(
    <div role="listbox" aria-label="Lane">
      <TimelineClip
        item={item}
        pixelsPerSecond={80}
        rowHeight={58}
        selected={options.selected ?? false}
        dimmed={false}
        tabIndex={0}
        onSelect={options.onSelect ?? vi.fn()}
        onKeyDown={vi.fn()}
        onFocus={vi.fn()}
      />
    </div>,
    { project: options.project ?? fixtureProject(), projectDir: "browser://bundled-sample-project" },
  );
}

function generatedClip(project: VideoProject): TimelineItem {
  return requiredValue(
    fixtureTrack(project, "video").items.find((entry) => entry.properties.generatedAssetId === fixtureGeneratedAsset(project).id),
    "sample generated clip",
  );
}

function item(kind: TimelineItem["kind"], patch: Partial<TimelineItem> = {}): TimelineItem {
  return {
    id: `${kind}-1`,
    kind,
    startSeconds: 1.5,
    durationSeconds: 2.25,
    source: { type: "media", mediaId: "media-1" },
    label: `${kind} label`,
    properties: {},
    ...patch,
  };
}

describe("TimelineClip", () => {
  beforeEach(() => window.localStorage.clear());

  it("is an option named by label, start timecode and duration, placed by pixels per second", () => {
    renderClip(item("video_clip", { label: "Opening clip" }));
    const option = screen.getByRole("option", { name: "Opening clip, 00:00:01.500, 00:00:02.250" });
    expect(option).toHaveStyle({ left: "120px", width: "180px", top: "3px", height: "52px" });
    expect(option).toHaveAttribute("aria-selected", "false");
  });

  it.each([
    ["video_clip", "bg-clip-video"],
    ["image_clip", "bg-clip-video"],
    ["overlay", "bg-clip-text"],
    ["caption", "bg-clip-caption"],
    ["audio_clip", "bg-clip-audio"],
    ["hyperframe_scene", "bg-clip-graphics"],
  ] as const)("fills %s clips with %s", (kind, fillClass) => {
    renderClip(item(kind));
    expect(screen.getByTestId("clip-body")).toHaveClass(fillClass);
  });

  it("fills template overlays with the graphics color", () => {
    renderClip(item("overlay", { properties: { templateId: "lower-third" } }));
    expect(screen.getByTestId("clip-body")).toHaveClass("bg-clip-graphics");
  });

  it("shows the filmstrip and an audio strip for video and a waveform for audio", () => {
    const video = renderClip(item("video_clip"));
    expect(screen.getByTestId("clip-filmstrip")).toBeInTheDocument();
    expect(screen.getByTestId("clip-waveform")).toBeInTheDocument();
    video.unmount();

    renderClip(item("audio_clip"));
    expect(screen.queryByTestId("clip-filmstrip")).not.toBeInTheDocument();
    expect(screen.getByTestId("clip-waveform")).toBeInTheDocument();
  });

  it("marks a reversed clip and describes it as playing in reverse", () => {
    const forward = renderClip(item("video_clip"));
    expect(screen.queryByTestId("clip-reversed-mark")).not.toBeInTheDocument();
    expect(screen.getByRole("option")).not.toHaveAccessibleDescription();
    forward.unmount();

    renderClip(item("audio_clip", { label: "Music bed", properties: { reverse: true } }));
    expect(screen.getByTestId("clip-reversed-mark")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Music bed, 00:00:01.500, 00:00:02.250" })).toHaveAccessibleDescription("Plays in reverse");
  });

  it("shows the text of text clips next to the kind icon", () => {
    const { container } = renderClip(item("caption", { source: { type: "text", text: "The first recorded voice" } }));
    expect(screen.getByTestId("clip-body")).toHaveTextContent("The first recorded voice");
    expect(container.querySelector("svg.lucide-captions")).toBeInTheDocument();
  });

  it("outlines the selected clip", () => {
    renderClip(item("video_clip"), { selected: true });
    expect(screen.getByRole("option")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByTestId("clip-body")).toHaveClass("outline-foreground");
  });

  it("selects on click, additively with a modifier", () => {
    const onSelect = vi.fn();
    const clip = item("video_clip");
    renderClip(clip, { onSelect });
    fireEvent.click(screen.getByRole("option"));
    fireEvent.click(screen.getByRole("option"), { shiftKey: true });
    expect(onSelect.mock.calls).toEqual([[clip, false], [clip, true]]);
  });

  it("leaves pointer clicks to the pointer handler and dims while dragged", () => {
    const onSelect = vi.fn();
    const onPointerDown = vi.fn();
    const clip = item("video_clip");
    renderWithEditorStore(
      <div role="listbox" aria-label="Lane">
        <TimelineClip
          item={clip}
          pixelsPerSecond={80}
          rowHeight={58}
          selected={false}
          dragging
          placement={{ startSeconds: 2, durationSeconds: 1 }}
          dimmed={false}
          tabIndex={0}
          onPointerDown={onPointerDown}
          onSelect={onSelect}
          onKeyDown={vi.fn()}
          onFocus={vi.fn()}
        />
      </div>,
      { project: fixtureProject(), projectDir: "browser://bundled-sample-project" },
    );
    const option = screen.getByRole("option");
    expect(option).toHaveStyle({ left: "160px", width: "80px" });
    expect(option).toHaveClass("opacity-40");
    fireEvent.pointerDown(option, { button: 0 });
    fireEvent.click(option, { detail: 1 });
    expect(onPointerDown).toHaveBeenCalledTimes(1);
    expect(onSelect).not.toHaveBeenCalled();
    fireEvent.click(option, { detail: 0 });
    expect(onSelect).toHaveBeenCalledWith(clip, false);
  });

  it("takes focus on mouse down without scrolling the timeline", () => {
    renderClip(item("video_clip"));
    const option = screen.getByRole("option");
    const focus = vi.spyOn(option, "focus");
    expect(fireEvent.mouseDown(option, { button: 0 })).toBe(false);
    expect(focus).toHaveBeenCalledWith({ preventScroll: true });
  });

  it("marks failed generations with a corner mark and a tooltip", async () => {
    const project = fixtureProject();
    fixtureGeneratedAsset(project).status = "failed";
    renderClip(generatedClip(project), { project });

    const message = "Generation failed: Bundled Edison restoration";
    expect(screen.getByRole("option")).toHaveAccessibleDescription(message);
    fireEvent.focus(screen.getByTestId("clip-failure-mark"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent(message);
    expect(screen.queryByTestId("clip-generation-progress")).not.toBeInTheDocument();
  });

  it("shows a progress fill while a generation runs, without status badges", () => {
    const project = fixtureProject();
    fixtureGeneratedAsset(project).status = "running";
    renderClip(generatedClip(project), { project });
    expect(screen.getByTestId("clip-generation-progress")).toBeInTheDocument();
    expect(screen.queryByTestId("clip-failure-mark")).not.toBeInTheDocument();
    expect(within(screen.getByTestId("clip-body")).queryByText(/^(AI|Running|Queued|Generating)$/i)).not.toBeInTheDocument();
  });

  it("hatches dead-air ranges on audio-bearing clips", () => {
    const project = fixtureProject();
    project.mediaSilenceRanges = [
      { mediaId: "media-voiceover", sourceIn: 0, sourceOut: 1, confidence: 0.9 },
      { mediaId: "media-voiceover", sourceIn: 3, sourceOut: 4, confidence: 0.9 },
    ];
    const audio = item("audio_clip", {
      startSeconds: 0,
      durationSeconds: 4,
      source: { type: "media", mediaId: "media-voiceover" },
      properties: { sourceIn: 0, sourceOut: 4 },
    });
    renderClip(audio, { project });
    const hatches = screen.getAllByTestId("dead-air-hatch");
    expect(hatches).toHaveLength(2);
    expect(hatches[0]).toHaveStyle({ left: "0%", width: "25%" });
    expect(hatches[1]).toHaveStyle({ left: "75%", width: "25%" });
  });

  it("does not hatch clips without audio", () => {
    const project = fixtureProject();
    project.mediaSilenceRanges = [{ mediaId: "media-1", sourceIn: 0, sourceOut: 1, confidence: 0.9 }];
    renderClip(item("image_clip"), { project });
    expect(screen.queryByTestId("dead-air-hatch")).not.toBeInTheDocument();
  });
});
