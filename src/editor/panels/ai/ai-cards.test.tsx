import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { StrictMode, useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentAssistantMessage } from "@/editor/store/agent-slice";
import { undoConflictReason } from "@/lib/agent/conversation-state";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { agentProject, appliedMessage, failedMessage, reviewMessage } from "@/test-utils/agent-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureGeneratedAsset, fixtureItem } from "@/test-utils/editor-fixtures";
import { AppliedCard } from "./applied-card";
import { FailureCard } from "./failure-card";
import { ReviewCard } from "./review-card";
import { VariationResultCard } from "./variation-result-card";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => `asset://${p}` }));

const captureCommand = "capture_canonical_preview_frame_in_split_project_folder";
let sequence = 0;

function uniqueDir(): string {
  sequence += 1;
  return `/projects/cards-${sequence.toString()}`;
}

function captureCalls() {
  return vi.mocked(backendRequest).mock.calls.filter(([command]) => command === captureCommand);
}

/** Captures resolve with a frame path per playhead, bumping only bookkeeping (a job record). */
function mockCaptures(options: { fail?: boolean } = {}) {
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    if (command !== captureCommand) throw new Error(`unexpected backend call ${command}`);
    if (options.fail) throw new Error("Canonical preview capture did not use the native AVFoundation compositor.");
    const seconds = Number(input?.playheadSeconds);
    return { project: null, playheadSeconds: seconds, previewFrame: `renders/${String(input?.jobId)}/preview-0001.png` };
  });
}

function renderApplied(message: AgentAssistantMessage, project: VideoProject, projectDir = uniqueDir()) {
  const card = message.card;
  if (!card?.result) throw new Error("expected an applied card");
  const rendered = renderWithEditorStore(<AppliedCard message={message} card={{ ...card, result: card.result }} />, { project, projectDir });
  act(() => rendered.store.setState({ mergeLoadedProject: vi.fn(async () => true) }));
  return rendered;
}

async function settle() {
  await act(async () => {
    for (let index = 0; index < 20; index += 1) await Promise.resolve();
  });
}

describe("AppliedCard", () => {
  beforeEach(() => {
    vi.mocked(backendRequest).mockReset();
    mockCaptures();
  });

  it("names the status, facts and actions", async () => {
    const project = agentProject();
    renderApplied(appliedMessage(project, "assistant-a1", "user-a1", { result: { historyEntryId: "a1" } }), project);
    const card = screen.getByRole("article", { name: "Applied to Timeline 1" });
    expect(within(within(card).getByRole("list", { name: "Facts" })).getAllByRole("listitem").map((chip) => chip.textContent)).toContain("0:08 → 0:06");
    expect(within(card).getByRole("button", { name: "Show changes" })).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Undo" })).not.toHaveAttribute("aria-disabled");
    await settle();
  });

  it("captures up to four frames one at a time and plays from a frame", async () => {
    const project = agentProject();
    const resolvers: ((value: unknown) => void)[] = [];
    vi.mocked(backendRequest).mockImplementation(
      (command: string, input?: Record<string, unknown>) =>
        new Promise((resolve) => {
          resolvers.push(() => resolve({ project: null, playheadSeconds: input?.playheadSeconds, previewFrame: `renders/${String(input?.jobId)}/frame.png` }));
          expect(command).toBe(captureCommand);
        }),
    );
    const message = appliedMessage(project, "assistant-a2", "user-a2", {
      result: {
        historyEntryId: "a2",
        impact: { summary: "", beforeDurationSeconds: 8, afterDurationSeconds: 6, affectedItemIds: [], previewTimestamp: 0.5, affectedRanges: [1, 2, 3, 4].map((start) => ({ startSeconds: start, endSeconds: start + 0.5 })) },
      },
    });
    const { store } = renderApplied(message, project);
    await settle();
    expect(captureCalls()).toHaveLength(1);
    for (let index = 0; index < 4; index += 1) {
      await act(async () => {
        resolvers[index]?.(undefined);
        for (let step = 0; step < 20; step += 1) await Promise.resolve();
      });
    }
    expect(captureCalls().map(([, input]) => input?.playheadSeconds)).toEqual([0.5, 1, 2, 3]);
    const jobIds = captureCalls().map(([, input]) => input?.jobId);
    expect(new Set(jobIds).size).toBe(4);
    const frames = within(screen.getByRole("group", { name: "Result preview" })).getAllByRole("button");
    expect(frames.map((frame) => frame.getAttribute("aria-label"))).toEqual(["Play from 0:01", "Play from 0:01", "Play from 0:02", "Play from 0:03"]);
    fireEvent.click(frames[2] as HTMLElement);
    expect(store.getState().playheadSeconds).toBe(2);
    expect(store.getState().playing).toBe(true);

    // A remount reads the cache instead of rendering again.
    const { unmount } = renderApplied(message, project, store.getState().projectDir);
    await settle();
    expect(captureCalls()).toHaveLength(4);
    unmount();
  });

  it("labels frames from an earlier version once the project content changes", async () => {
    const project = agentProject();
    const { store } = renderApplied(appliedMessage(project, "assistant-a3", "user-a3", { result: { historyEntryId: "a3" } }), project);
    await settle();
    expect(screen.queryByText("Earlier version")).not.toBeInTheDocument();

    // Bookkeeping (the capture's job record) bumps the revision without changing content.
    act(() => store.setState({ project: { ...store.getState().project, contentRevision: 6, jobs: [] } }));
    expect(screen.queryByText("Earlier version")).not.toBeInTheDocument();

    const edited = structuredClone(store.getState().project);
    const video = fixtureItem(edited, "video");
    video.label = "Renamed clip";
    act(() => store.setState({ project: { ...edited, contentRevision: 7 } }));
    expect(screen.getByText("Earlier version")).toBeInTheDocument();
  });

  it("falls back to Open viewer and Retry preview when frames can't be captured, keeping the status", async () => {
    mockCaptures({ fail: true });
    const project = agentProject();
    const { store } = renderApplied(appliedMessage(project, "assistant-a4", "user-a4", { result: { historyEntryId: "a4" } }), project);
    await settle();
    const preview = screen.getByRole("group", { name: "Result preview" });
    expect(screen.getByRole("article", { name: "Applied to Timeline 1" })).toBeInTheDocument();
    const attempts = captureCalls().length;
    expect(attempts).toBe(3);

    fireEvent.click(within(preview).getByRole("button", { name: "Open viewer" }));
    expect(store.getState().playheadSeconds).toBe(0.5);

    mockCaptures();
    fireEvent.click(within(preview).getByRole("button", { name: "Retry preview" }));
    await settle();
    expect(captureCalls().length).toBeGreaterThan(attempts);
    expect(within(screen.getByRole("group", { name: "Result preview" })).getAllByRole("button", { name: /^Play from/ })).toHaveLength(3);
  });

  it("disables Undo with the conflict reason", async () => {
    const project = agentProject();
    const { store } = renderApplied(
      appliedMessage(project, "assistant-a5", "user-a5", { result: { historyEntryId: "a5" }, undo: { available: false, reason: undoConflictReason } }),
      project,
    );
    const undoAgentEdit = vi.fn(async () => true);
    act(() => store.setState({ undoAgentEdit }));
    const undo = screen.getByRole("button", { name: "Undo" });
    expect(undo).toHaveAttribute("aria-disabled", "true");
    expect(undo).toHaveAccessibleDescription(undoConflictReason);
    fireEvent.click(undo);
    expect(undoAgentEdit).not.toHaveBeenCalled();
    await settle();
  });

  it("Undo calls the agent undo for the card", async () => {
    const project = agentProject();
    const { store } = renderApplied(appliedMessage(project, "assistant-a6", "user-a6", { result: { historyEntryId: "a6" } }), project);
    const undoAgentEdit = vi.fn(async () => true);
    act(() => store.setState({ undoAgentEdit }));
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(undoAgentEdit).toHaveBeenCalledWith("assistant-a6");
    await settle();
  });

  it("Show changes highlights the affected items and ranges, seeks, and scrolls the timeline", async () => {
    const project = agentProject();
    const { store } = renderApplied(appliedMessage(project, "assistant-a7", "user-a7", { result: { historyEntryId: "a7" } }), project);
    act(() => store.setState({ zoomPercent: 100 }));
    fireEvent.click(screen.getByRole("button", { name: "Show changes" }));
    const state = store.getState();
    const video = fixtureItem(project, "video");
    expect(state.highlightedItemIds).toEqual([video.id]);
    expect(state.highlightedRanges).toEqual([
      { trackIds: ["track-video"], startSeconds: 1, endSeconds: 2 },
      { trackIds: ["track-video"], startSeconds: 3, endSeconds: 3.5 },
    ]);
    expect(state.playheadSeconds).toBe(0.5);
    expect(state.scrollLeft).toBe(32);
    await settle();
  });

  it("shows Undone without actions after an undo", () => {
    const project = agentProject();
    const message = { ...appliedMessage(project, "assistant-a8", "user-a8", { result: { historyEntryId: "a8" } }), status: "undone" as const };
    renderApplied(message, project);
    const card = screen.getByRole("article", { name: "Undone" });
    expect(within(card).queryByRole("button")).not.toBeInTheDocument();
  });
});

let hideReview = () => undefined as void;

/** A review card inside a sheet-like dialog that a test can remove while it holds focus. */
function RemovableReview({ message, card, onFocusLost }: { message: AgentAssistantMessage; card: NonNullable<AgentAssistantMessage["card"]>; onFocusLost(): void }) {
  const [shown, setShown] = useState(true);
  hideReview = () => setShown(false);
  return (
    <div role="dialog" aria-label="AI" tabIndex={-1}>
      {shown && <ReviewCard message={message} card={card} onRevise={() => undefined} onFocusLost={onFocusLost} />}
    </div>
  );
}

describe("ReviewCard", () => {
  beforeEach(() => vi.mocked(backendRequest).mockReset());

  function renderReview(message: AgentAssistantMessage, project: VideoProject) {
    const card = message.card;
    if (!card) throw new Error("expected a card");
    const onRevise = vi.fn();
    const rendered = renderWithEditorStore(<ReviewCard message={message} card={card} onRevise={onRevise} onFocusLost={() => undefined} />, { project });
    return { ...rendered, onRevise };
  }

  it("names the status, placements and actions, and focuses the primary action", () => {
    const project = agentProject();
    const { store } = renderReview(reviewMessage(project, "assistant-r1", "user-r1"), project);
    const card = screen.getByRole("article", { name: "Needs your review" });
    expect(within(card).getByRole("list", { name: "Planned changes" })).toBeInTheDocument();
    const apply = within(card).getByRole("button", { name: "Apply" });
    expect(apply).toHaveFocus();
    const approve = vi.fn(async () => true);
    const reject = vi.fn();
    act(() => store.setState({ approveAgentProposal: approve, rejectAgentProposal: reject }));
    fireEvent.click(apply);
    expect(approve).toHaveBeenCalledWith("assistant-r1");
    fireEvent.click(within(card).getByRole("button", { name: "Dismiss" }));
    expect(reject).toHaveBeenCalledWith("assistant-r1");
  });

  it("labels generation bundles Generate & place and prefills Revise", () => {
    const project = agentProject();
    const generated = fixtureGeneratedAsset(project);
    const actions: ProjectAction[] = [{ type: "recordGeneratedAsset", asset: { ...generated, id: "generated-new", outputs: [], status: "queued" } }];
    const { onRevise } = renderReview(reviewMessage(project, "assistant-r2", "user-r2", { actions }), project);
    expect(screen.getByRole("button", { name: "Generate & place" })).toHaveFocus();
    fireEvent.click(screen.getByRole("button", { name: "Revise" }));
    expect(onRevise).toHaveBeenCalledTimes(1);
  });

  it("keeps focus on the primary action under Strict Mode's effect re-run", async () => {
    const project = agentProject();
    const message = reviewMessage(project, "assistant-r4", "user-r4");
    const card = message.card;
    if (!card) throw new Error("expected a card");
    const onFocusLost = vi.fn();
    renderWithEditorStore(
      <StrictMode>
        <ReviewCard message={message} card={card} onRevise={() => undefined} onFocusLost={onFocusLost} />
      </StrictMode>,
      { project },
    );
    expect(screen.getByRole("button", { name: "Apply" })).toHaveFocus();
    await act(async () => {
      await new Promise((resolve) => requestAnimationFrame(resolve));
    });
    expect(screen.getByRole("button", { name: "Apply" })).toHaveFocus();
    expect(onFocusLost).not.toHaveBeenCalled();
  });

  it.each([
    ["focuses the sheet around the removed card", "AI", true],
    ["moves focus elsewhere", "Elsewhere", false],
  ] as const)("reports lost focus only when the page (or a sheet's focus trap) %s", async (_case, target, lost) => {
    const project = agentProject();
    const message = reviewMessage(project, `assistant-trap-${target}`, `user-trap-${target}`);
    const card = message.card;
    if (!card) throw new Error("expected a card");
    const onFocusLost = vi.fn();
    renderWithEditorStore(
      <>
        <button type="button">Elsewhere</button>
        <RemovableReview message={message} card={card} onFocusLost={onFocusLost} />
      </>,
      { project },
    );
    expect(screen.getByRole("button", { name: "Apply" })).toHaveFocus();
    act(() => hideReview());
    // Radix's focus trap focuses the sheet itself when the focused element goes away.
    act(() => (target === "AI" ? screen.getByRole("dialog", { name: "AI" }) : screen.getByRole("button", { name: "Elsewhere" })).focus());
    await act(async () => void (await new Promise((resolve) => requestAnimationFrame(resolve))));
    expect(onFocusLost).toHaveBeenCalledTimes(lost ? 1 : 0);
  });

  it("does not take focus again when it remounts", () => {
    const project = agentProject();
    const message = reviewMessage(project, "assistant-r3", "user-r3");
    const first = renderReview(message, project);
    first.unmount();
    renderReview(message, project);
    expect(screen.getByRole("button", { name: "Apply" })).not.toHaveFocus();
  });
});

describe("FailureCard", () => {
  function renderFailure(message: AgentAssistantMessage, canRetry: boolean) {
    if (!message.failure) throw new Error("expected a failure");
    const rendered = renderWithEditorStore(<FailureCard message={message} failure={message.failure} canRetry={canRetry} />, { project: agentProject() });
    const retry = vi.fn(async () => true);
    act(() => rendered.store.setState({ retryAgentTurn: retry }));
    return { ...rendered, retry };
  }

  it("explains a blocked validation with the first issue and no Retry", () => {
    renderFailure(failedMessage("assistant-f1", "user-f1", "validationBlocked", "The caption track is locked.", false), true);
    expect(screen.getByRole("article", { name: "Couldn't validate the edit" })).toHaveTextContent(
      "The edit couldn't be validated: The caption track is locked. Try rephrasing or narrowing the request.",
    );
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
  });

  it("says nothing changed after a failed apply and offers Retry for the latest turn", () => {
    const { retry } = renderFailure(failedMessage("assistant-f2", "user-f2", "applyFailed", "The project folder is read-only.", true), true);
    expect(screen.getByRole("article", { name: "Nothing was changed" })).toHaveTextContent("Nothing was changed. The project folder is read-only.");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(retry).toHaveBeenCalledWith("assistant-f2");
  });

  it("hides Retry for an earlier turn", () => {
    renderFailure(failedMessage("assistant-f3", "user-f3", "turnFailed", "The edit timed out.", true), false);
    expect(screen.getByRole("article", { name: "The edit didn't finish" })).toHaveTextContent("The edit timed out.");
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
  });

  it("shows an unconfirmed edit without claiming nothing changed or offering Retry", () => {
    renderFailure(failedMessage("assistant-unknown", "user-unknown", "outcomeUnknown", "The edit may have completed. Refresh the project before editing again.", false), true);
    expect(screen.getByRole("article", { name: "Edit unconfirmed" })).toHaveTextContent("The edit may have completed.");
    expect(screen.queryByText(/Nothing was changed/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
  });
});

describe("VariationResultCard", () => {
  it("shows the variation set and commits Use this as a user edit", async () => {
    const project = agentProject();
    const root = fixtureGeneratedAsset(project);
    const output = root.outputs[0];
    if (!output) throw new Error("expected a sample output");
    const variation = { ...root, id: "generated-variation", name: "Storm Clouds", parentAssetId: root.id, createdAt: "2026-07-13T00:00:00Z", outputs: [{ ...output, mediaId: "variation-output", relativePath: "generated/storm.mp4" }] };
    const rootMedia = project.media.find((media) => media.id === output.mediaId);
    if (!rootMedia) throw new Error("expected the sample output media");
    const withVariation: VideoProject = {
      ...project,
      generatedAssets: [...project.generatedAssets, variation],
      media: [...project.media, { ...rootMedia, id: "variation-output", name: "storm.mp4", relativePath: "generated/storm.mp4" }],
    };
    const actions: ProjectAction[] = [{ type: "recordGeneratedAsset", asset: variation }];
    const { store } = renderWithEditorStore(<VariationResultCard actions={actions} />, { project: withVariation, projectDir: "/projects/variations" });
    const applied: ProjectAction[][] = [];
    act(() =>
      store.setState({
        applyActions: async (batch) => {
          applied.push([...batch]);
          return null;
        },
      }),
    );
    const card = screen.getByRole("article", { name: "Variations" });
    expect(within(card).getByText("Current")).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Use Storm Clouds" }));
    await waitFor(() => expect(applied).toEqual([[{ type: "replaceTimelineItemWithGeneratedOutput", replacement: { itemId: "sample-generated-clip", mediaId: "variation-output" } }]]));
  });

  it("renders nothing without a variation set", () => {
    const project = agentProject();
    const { container } = renderWithEditorStore(<VariationResultCard actions={[]} />, { project });
    expect(container).toBeEmptyDOMElement();
  });
});
