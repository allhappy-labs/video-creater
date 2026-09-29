import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAgentSession, ProjectAgentSessionManifest } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { agentProject } from "@/test-utils/agent-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureItem } from "@/test-utils/editor-fixtures";
import { useEditorStore } from "../../store/editor-store-context";
import { MediaPanel } from "../media/media-panel";
import { AiHeader } from "./ai-header";
import { Composer } from "./composer";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

/** The AI composer or the Media tab, whichever tab is active. */
function ActiveTab() {
  const tab = useEditorStore((state) => state.activeTab);
  return tab === "media" ? <MediaPanel /> : <Composer />;
}

function composer(): HTMLTextAreaElement {
  return screen.getByRole("textbox", { name: "Describe an edit" }) as HTMLTextAreaElement;
}

function type(value: string) {
  const element = composer();
  act(() => element.focus());
  fireEvent.change(element, { target: { value, selectionStart: value.length } });
  element.setSelectionRange(value.length, value.length);
  fireEvent.select(element);
}

beforeEach(() => {
  window.localStorage.clear();
  vi.mocked(backendRequest).mockReset();
  vi.mocked(backendRequest).mockImplementation(async () => {
    throw new BackendUnavailableError();
  });
});

describe("Composer", () => {
  it("opens the mention picker on @ and inserts a target with the keyboard", async () => {
    const { store } = renderWithEditorStore(<Composer />, { project: agentProject() });
    type("Tighten @Op");
    const picker = screen.getByRole("listbox", { name: "Mention media or a clip" });
    const options = within(picker).getAllByRole("option");
    expect(options[0]).toHaveTextContent("Opening clip");
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    expect(composer()).toHaveAttribute("aria-activedescendant", options[0]?.id);

    type("Tighten @");
    const all = within(screen.getByRole("listbox")).getAllByRole("option");
    expect(all.length).toBeGreaterThan(1);
    fireEvent.keyDown(composer(), { key: "ArrowDown" });
    const second = within(screen.getByRole("listbox")).getAllByRole("option")[1];
    expect(second).toHaveAttribute("aria-selected", "true");
    const name = second?.querySelector("span")?.textContent ?? "";
    fireEvent.keyDown(composer(), { key: "Enter" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    const expected = /\s/.test(name) ? `Tighten @"${name}" ` : `Tighten @${name} `;
    expect(store.getState().agentDraft).toBe(expected);
    await waitFor(() => expect(composer()).toHaveFocus());
  });

  it("rings the composer box while its borderless field has focus", () => {
    renderWithEditorStore(<Composer />, { project: agentProject() });
    expect(composer()).toHaveClass("focus-visible:ring-0");
    expect(composer().parentElement).toHaveClass("focus-within:ring-2", "focus-within:ring-ring");
  });

  it("closes the picker on Escape without sending, and Enter then sends", () => {
    const { store } = renderWithEditorStore(<Composer />, { project: agentProject() });
    const submit = vi.fn(async () => true);
    act(() => store.setState({ submitAgentPrompt: submit }));
    type("Look at @");
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    fireEvent.keyDown(composer(), { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(submit).not.toHaveBeenCalled();
    fireEvent.keyDown(composer(), { key: "Enter" });
    expect(submit).toHaveBeenCalledWith("Look at @");
  });

  it("the @ button starts a mention at the caret", () => {
    const { store } = renderWithEditorStore(<Composer />, { project: agentProject() });
    type("Trim");
    fireEvent.click(screen.getByRole("button", { name: "Mention media or a clip" }));
    expect(store.getState().agentDraft).toBe("Trim @");
  });

  it("shows the selection as one removable chip and removes it from the request context", () => {
    const project = agentProject();
    const video = fixtureItem(project, "video");
    const { store } = renderWithEditorStore(<Composer />, { project });
    act(() => store.getState().selectItems([video.id]));
    const remove = screen.getByRole("button", { name: `Remove ${video.label} from the request` });
    expect(remove.parentElement).toHaveTextContent(video.label);
    fireEvent.click(remove);
    expect(screen.queryByRole("button", { name: `Remove ${video.label} from the request` })).not.toBeInTheDocument();
    expect(store.getState().agentContextChip).toBeNull();
  });

  it("shows the unresolved-mention error inline", () => {
    const { store } = renderWithEditorStore(<Composer />, { project: agentProject() });
    act(() => store.setState({ agentComposerError: "Nothing in this project is named @ghost. Pick a name from the @ list." }));
    expect(screen.getByRole("alert")).toHaveTextContent("@ghost");
    expect(composer()).toHaveAccessibleDescription("Nothing in this project is named @ghost. Pick a name from the @ list.");
  });

  it("attach opens the Media tab in attach mode, and a pick mentions the media", () => {
    const project = agentProject();
    const { store } = renderWithEditorStore(<ActiveTab />, { project, projectDir: "" });
    act(() => store.getState().setAgentDraft("Use"));
    fireEvent.click(screen.getByRole("button", { name: "Attach media" }));
    expect(store.getState().activeTab).toBe("media");
    expect(store.getState().mediaAttachMode).toBe(true);

    expect(screen.getByRole("region", { name: "Attach mode" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Attach input.mp4" }));
    expect(store.getState().mediaAttachMode).toBe(false);
    expect(store.getState().activeTab).toBe("ai");
    expect(store.getState().agentDraft).toBe("Use @input.mp4 ");
    expect(composer()).toHaveValue("Use @input.mp4 ");
  });
});

function session(id: string, title: string): ProjectAgentSession {
  return { id, title, threadId: null, createdAt: "2026-09-15T10:00:00Z", updatedAt: "2026-09-15T10:00:00Z", turns: [], proposalStatus: "none", appliedActionIds: [] };
}

function manifest(): ProjectAgentSessionManifest {
  return { schemaVersion: 1, projectId: "project-sample", activeSessionId: "s1", sessions: [session("s1", "Intro cut"), session("s2", "Captions pass")], deletedSessions: [session("s0", "Old ideas")] };
}

describe("AiHeader", () => {
  it("switches, renames and deletes chats through the session actions, and restores from History", async () => {
    const { store } = renderWithEditorStore(<AiHeader />, { project: agentProject() });
    const actions = { selectAgentSession: vi.fn(async () => true), renameAgentSession: vi.fn(async () => true), deleteAgentSession: vi.fn(async () => true), restoreAgentSession: vi.fn(async () => true) };
    act(() => store.setState({ agentSessions: manifest(), ...actions }));

    const trigger = screen.getByRole("button", { name: "Chat: Intro cut" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Captions pass" }));
    expect(actions.selectAgentSession).toHaveBeenCalledWith("s2");

    fireEvent.keyDown(screen.getByRole("button", { name: "Chat: Intro cut" }), { key: "Enter" });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Rename chat…" }));
    const rename = await screen.findByRole("dialog", { name: "Rename chat" });
    fireEvent.change(within(rename).getByRole("textbox", { name: "Chat name" }), { target: { value: "Final cut" } });
    fireEvent.click(within(rename).getByRole("button", { name: "Rename" }));
    await waitFor(() => expect(actions.renameAgentSession).toHaveBeenCalledWith("s1", "Final cut"));

    fireEvent.keyDown(screen.getByRole("button", { name: "Chat: Intro cut" }), { key: "Enter" });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete chat…" }));
    const confirm = await screen.findByRole("dialog", { name: "Delete “Intro cut”?" });
    fireEvent.click(within(confirm).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(actions.deleteAgentSession).toHaveBeenCalledWith("s1"));

    const history = screen.getByRole("button", { name: "History" });
    fireEvent.click(history);
    expect(history).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(screen.getByRole("region", { name: "Deleted chats" })).getByRole("button", { name: "Restore Old ideas" }));
    expect(actions.restoreAgentSession).toHaveBeenCalledWith("s0");
  });

  it("persists the auto-apply switch", () => {
    const { store } = renderWithEditorStore(<AiHeader />, { project: agentProject() });
    fireEvent.click(screen.getByRole("switch", { name: "Auto-apply safe edits" }));
    expect(store.getState().autoApplySafe).toBe(false);
  });
});
