import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { quickEdits } from "@/lib/agent/quick-edits";
import type { VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { agentProject, appliedMessage, failedMessage, reviewMessage, userMessage } from "@/test-utils/agent-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureItem } from "@/test-utils/editor-fixtures";
import { TooltipProvider } from "@/components/ui/tooltip";
import { EditorEnvironmentProvider } from "../../services/editor-environment";
import { EditorStoreProvider } from "../../store/editor-store-context";
import { createEditorStore, type EditorStore } from "../../store/editor-store";
import { AiPanel } from "./ai-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => `asset://${p}` }));

let dirSequence = 0;

function renderPanel(project: VideoProject = agentProject()) {
  dirSequence += 1;
  return renderWithEditorStore(<AiPanel />, { project, projectDir: `/projects/ai-panel-${dirSequence.toString()}` });
}

function composer(): HTMLTextAreaElement {
  return screen.getByRole("textbox", { name: "Describe an edit" }) as HTMLTextAreaElement;
}

function spyOn<Key extends "submitAgentPrompt" | "cancelAgentTurn" | "approveAgentProposal" | "rejectAgentProposal">(store: EditorStore, key: Key) {
  const spy = vi.fn(async () => true);
  act(() => store.setState({ [key]: spy } as never));
  return spy;
}

describe("AiPanel", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    vi.mocked(backendRequest).mockImplementation(async () => {
      throw new BackendUnavailableError();
    });
  });

  it("names the header, composer and quick edit controls", () => {
    renderPanel();
    const autoApply = screen.getByRole("switch", { name: "Auto-apply safe edits" });
    expect(autoApply).toBeChecked();
    expect(autoApply).toHaveAttribute("data-state", "checked");
    expect(autoApply).toHaveAccessibleDescription("Paid, generative, export, and broad changes always wait for review.");
    expect(screen.getByRole("button", { name: "New chat" })).toBeInTheDocument();
    expect(composer()).toHaveValue("");
    expect(screen.getByRole("button", { name: "Mention media or a clip" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Attach media" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    const suggestions = within(screen.getByRole("list", { name: "Quick edits" })).getAllByRole("button");
    expect(suggestions.map((button) => button.textContent)).toEqual(quickEdits.map((edit) => edit.label));
  });

  it("fills the draft from a quick edit without sending, and hides quick edits after the first message", async () => {
    const { store } = renderPanel();
    const submit = spyOn(store, "submitAgentPrompt");
    fireEvent.click(screen.getByRole("button", { name: "Remove dead air" }));
    expect(composer()).toHaveValue(quickEdits[1]?.prompt);
    await waitFor(() => expect(composer()).toHaveFocus());
    expect(submit).not.toHaveBeenCalled();

    act(() => store.setState({ agentMessages: [userMessage("user-quick-1", "Remove dead air")] }));
    expect(screen.queryByRole("list", { name: "Quick edits" })).not.toBeInTheDocument();
  });

  it("sends on Enter and adds a line on Shift+Enter", () => {
    const { store } = renderPanel();
    const submit = spyOn(store, "submitAgentPrompt");
    fireEvent.change(composer(), { target: { value: "Tighten the pacing" } });
    fireEvent.keyDown(composer(), { key: "Enter", shiftKey: true });
    expect(submit).not.toHaveBeenCalled();
    fireEvent.keyDown(composer(), { key: "Enter" });
    expect(submit).toHaveBeenCalledWith("Tighten the pacing");
  });

  it("turns Send into Stop while a turn runs, and shows the phase as text", () => {
    const { store } = renderPanel();
    const cancel = spyOn(store, "cancelAgentTurn");
    act(() =>
      store.setState({
        agentMessages: [userMessage("user-stop-1", "Tighten the pacing")],
        agentConversation: { status: "working", phase: "reviewing", turn: { messageId: "user-stop-1", draft: "Tighten the pacing" }, reviewApproved: false },
      }),
    );
    expect(screen.queryByRole("button", { name: "Send" })).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Reviewing the timeline");
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(cancel).toHaveBeenCalledTimes(1);

    // An atomic apply can't be interrupted.
    act(() => store.setState({ agentConversation: { status: "working", phase: "applying", turn: { messageId: "user-stop-1", draft: "x" }, reviewApproved: false } }));
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent("Applying changes");

    act(() => store.setState({ agentConversation: { status: "idle", note: "Stopped" } }));
    expect(screen.getByRole("status")).toHaveTextContent("Stopped");
    expect(screen.getByRole("button", { name: "Send" })).toBeInTheDocument();
  });

  it("consumes a pending request when one arrives, without sending", async () => {
    const project = agentProject();
    const video = fixtureItem(project, "video");
    const { store } = renderPanel(project);
    act(() => store.getState().setPendingAgentRequest({ itemIds: [video.id], prompt: "Organize the media" }));
    await waitFor(() => expect(composer()).toHaveValue("Organize the media"));
    expect(store.getState().pendingAgentRequest).toBeNull();
    expect(screen.getByRole("button", { name: `Remove ${video.label} from the request` })).toBeInTheDocument();

    act(() => store.getState().setPendingAgentRequest({ itemIds: [], prompt: "Ask about the captions" }));
    await waitFor(() => expect(composer()).toHaveValue("Ask about the captions"));
    expect(vi.mocked(backendRequest).mock.calls.some(([command]) => command === "start_codex_conversation_edit_for_project")).toBe(false);
  });

  it("asks before sending an Organize with AI request, and sends once confirmed", async () => {
    const { store } = renderPanel();
    const submit = spyOn(store, "submitAgentPrompt");
    act(() => store.getState().requestAgent({ itemIds: [], prompt: "Organize the current project media", confirmSend: true }));

    const confirmation = await screen.findByRole("group", { name: "Send this request?" });
    expect(composer()).toHaveValue("Organize the current project media");
    expect(within(confirmation).getByRole("button", { name: "Send" })).toHaveFocus();
    expect(screen.queryByRole("list", { name: "Quick edits" })).not.toBeInTheDocument();
    expect(submit).not.toHaveBeenCalled();

    // Cancel (or Escape) keeps the draft for editing and returns focus to the composer.
    fireEvent.keyDown(within(confirmation).getByRole("button", { name: "Send" }), { key: "Escape" });
    expect(screen.queryByRole("group", { name: "Send this request?" })).not.toBeInTheDocument();
    expect(composer()).toHaveValue("Organize the current project media");
    expect(composer()).toHaveFocus();
    expect(submit).not.toHaveBeenCalled();

    act(() => store.getState().requestAgent({ itemIds: [], prompt: "Organize the current project media", confirmSend: true }));
    fireEvent.click(within(await screen.findByRole("group", { name: "Send this request?" })).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("group", { name: "Send this request?" })).not.toBeInTheDocument();
    expect(submit).not.toHaveBeenCalled();

    act(() => store.getState().requestAgent({ itemIds: [], prompt: "Organize the current project media", confirmSend: true }));
    fireEvent.click(within(await screen.findByRole("group", { name: "Send this request?" })).getByRole("button", { name: "Send" }));
    expect(submit).toHaveBeenCalledTimes(1);
    expect(submit).toHaveBeenCalledWith("Organize the current project media");
  });

  it("sends a confirmed Organize with AI request through the agent turn", async () => {
    const { store } = renderPanel();
    act(() => store.getState().requestAgent({ itemIds: [], prompt: "Organize the current project media", confirmSend: true }));
    const confirmation = await screen.findByRole("group", { name: "Send this request?" });
    const turns = () => vi.mocked(backendRequest).mock.calls.filter(([command]) => command === "start_codex_conversation_edit_for_project");
    expect(turns()).toHaveLength(0);

    await act(async () => {
      fireEvent.click(within(confirmation).getByRole("button", { name: "Send" }));
      await Promise.resolve();
    });
    expect(turns()[0]?.[1]).toMatchObject({ request: { prompt: "Organize the current project media" } });
    expect(screen.queryByRole("group", { name: "Send this request?" })).not.toBeInTheDocument();
    expect(store.getState().agentMessages[0]).toMatchObject({ role: "user", text: "Organize the current project media" });
  });

  it("fills the chip and focuses the composer for Ask AI, without sending or asking", async () => {
    const project = agentProject();
    const video = fixtureItem(project, "video");
    const { store } = renderPanel(project);
    const submit = spyOn(store, "submitAgentPrompt");
    act(() => store.getState().requestAgent({ itemIds: [video.id] }));

    await waitFor(() => expect(composer()).toHaveFocus());
    expect(composer()).toHaveValue("");
    expect(screen.getByRole("button", { name: `Remove ${video.label} from the request` })).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Send this request?" })).not.toBeInTheDocument();
    expect(submit).not.toHaveBeenCalled();
  });

  it("consumes a request that was pending before the panel mounted", () => {
    const project = agentProject();
    const video = fixtureItem(project, "video");
    const store = createEditorStore({ projectDir: "/projects/premount", project });
    store.getState().setPendingAgentRequest({ itemIds: [video.id] });
    render(
      <TooltipProvider>
        <EditorStoreProvider store={store}>
          <AiPanel />
        </EditorStoreProvider>
      </TooltipProvider>,
    );
    expect(store.getState().pendingAgentRequest).toBeNull();
    expect(screen.getByText(video.label)).toBeInTheDocument();
  });

  it("never renders internal ids for a fixture conversation", async () => {
    const project = agentProject();
    const media = project.media.find((asset) => asset.kind === "video");
    const { store, container } = renderPanel(project);
    act(() =>
      store.setState({
        agentMessages: [
          userMessage("user-ids-1", `Tighten @${media?.name ?? "input.mp4"} please`),
          appliedMessage(project, "assistant-ids-1", "user-ids-1", { result: { historyEntryId: "agent-edit-ids" } }),
          userMessage("user-ids-2", "Make it darker"),
          reviewMessage(project, "assistant-ids-2", "user-ids-2"),
          userMessage("user-ids-3", "Fail please"),
          failedMessage("assistant-ids-3", "user-ids-3", "validationBlocked", "The clip is locked.", false),
        ],
      }),
    );
    const ids = [
      ...project.media.map((asset) => asset.id),
      ...project.timeline.tracks.flatMap((track) => [track.id, ...track.items.map((item) => item.id)]),
      ...project.generatedAssets.map((asset) => asset.id),
      "codex-action-",
      "agent-edit-ids",
      "user-ids-",
      "assistant-ids-",
    ];
    // Let the result frame capture settle (it fails without a backend).
    await act(async () => {
      await Promise.resolve();
    });
    const pattern = new RegExp(ids.map((id) => id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|"));
    const text = container.textContent ?? "";
    expect(text).toContain("Applied to Timeline 1");
    expect(text).toContain("Needs your review");
    expect(text).not.toMatch(pattern);
    expect(text).not.toMatch(/media-\d+/);
  });

  it("keeps focus in the composer when an edit applies, and focuses a review card's primary action", async () => {
    const project = agentProject();
    const { store } = renderPanel(project);
    act(() => composer().focus());
    act(() => store.setState({ agentMessages: [userMessage("user-focus-1", "Tighten"), appliedMessage(project, "assistant-focus-1", "user-focus-1", { result: { historyEntryId: "focus-applied" } })] }));
    expect(screen.getByRole("article", { name: "Applied to Timeline 1" })).toBeInTheDocument();
    expect(composer()).toHaveFocus();

    act(() => store.setState((state) => ({ agentMessages: [...state.agentMessages, userMessage("user-focus-2", "Darker"), reviewMessage(project, "assistant-focus-2", "user-focus-2")] })));
    const review = screen.getByRole("article", { name: "Needs your review" });
    expect(within(review).getByRole("button", { name: "Apply" })).toHaveFocus();

    // Dismissing removes the card; focus returns to the composer instead of the page.
    fireEvent.click(within(review).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("article", { name: "Needs your review" })).not.toBeInTheDocument();
    expect(screen.getByText("Dismissed")).toBeInTheDocument();
    await waitFor(() => expect(composer()).toHaveFocus());
  });

  it("prefills the composer with Revise", async () => {
    const project = agentProject();
    const { store } = renderPanel(project);
    act(() => store.setState({ agentMessages: [userMessage("user-revise-1", "Darker"), reviewMessage(project, "assistant-revise-1", "user-revise-1")] }));
    fireEvent.click(screen.getByRole("button", { name: "Revise" }));
    expect(composer()).toHaveValue("Revise: ");
    await waitFor(() => expect(composer()).toHaveFocus());
  });

  it("shows the missing-agent state with Open Agent settings", () => {
    const onOpenSettings = vi.fn();
    const project = agentProject();
    const rendered = renderWithEditorStore(
      <EditorEnvironmentProvider transcriptionModelReady onOpenModelSettings={() => undefined} onOpenSettings={onOpenSettings}>
        <AiPanel />
      </EditorEnvironmentProvider>,
      { project, projectDir: "/projects/missing-agent" },
    );
    act(() =>
      rendered.store.setState({
        agentMessages: [userMessage("user-missing-1", "Tighten"), failedMessage("assistant-missing-1", "user-missing-1", "agentUnavailable", "The AI agent isn't available right now.", false)],
      }),
    );
    const state = screen.getByRole("region", { name: "AI agent unavailable" });
    fireEvent.click(within(state).getByRole("button", { name: "Open Agent settings" }));
    expect(within(state).getByText(/^No AI agent is available\./)).toBeVisible();
    expect(onOpenSettings).toHaveBeenCalledWith({ category: "advanced", item: "agent" }, expect.any(HTMLElement));
  });
});
