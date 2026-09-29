import { ChevronDown, History, Pencil, Plus, Trash2 } from "lucide-react";
import { useId, useRef, useState } from "react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { IconButton } from "@/components/ui/icon-button";
import { Switch } from "@/components/ui/switch";
import { Tooltip } from "@/components/ui/tooltip";
import { isConversationBusy } from "@/lib/agent/conversation-state";
import type { ProjectAgentSession } from "@/lib/project";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { ConfirmDeleteDialog, NameDialog } from "../media/folder-dialogs";

const autoApplyTooltip = "Paid, generative, export, and broad changes always wait for review.";

type SessionDialog = { readonly kind: "rename" | "delete"; readonly session: ProjectAgentSession };

function sessionTitle(session: ProjectAgentSession | undefined): string {
  return session?.title.trim() || "New chat";
}

/** Archived (deleted) chats with Restore; History toggles it. */
function ArchivedSessions({ sessions, disabled }: { readonly sessions: readonly ProjectAgentSession[]; readonly disabled: boolean }) {
  const store = useEditorStoreApi();
  return (
    <section aria-label="Deleted chats" className="mx-3 mb-1.5 rounded-control bg-raised p-2">
      {sessions.length === 0 ? (
        <p className="px-1 text-[12px] text-dim">No deleted chats.</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {sessions.map((session) => (
            <li key={session.id} className="flex items-center gap-2 px-1 text-[12.5px]">
              <span className="min-w-0 flex-1 truncate text-foreground">{sessionTitle(session)}</span>
              <button
                type="button"
                disabled={disabled}
                aria-label={`Restore ${sessionTitle(session)}`}
                onClick={() => void store.getState().restoreAgentSession(session.id)}
                className="h-7 shrink-0 rounded-md px-2 text-[12px] font-medium text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
              >
                Restore
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** Session dropdown, auto-apply switch, History and New chat. */
export function AiHeader() {
  const store = useEditorStoreApi();
  const manifest = useEditorStore((state) => state.agentSessions);
  const sessionError = useEditorStore((state) => state.agentSessionError);
  const autoApplySafe = useEditorStore((state) => state.autoApplySafe);
  const busy = useEditorStore((state) => isConversationBusy(state.agentConversation));
  const [historyOpen, setHistoryOpen] = useState(false);
  const [dialog, setDialog] = useState<SessionDialog | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const autoApplyDescriptionId = useId();
  const sessions = manifest?.sessions ?? [];
  const active = sessions.find((session) => session.id === manifest?.activeSessionId);
  const title = sessionTitle(active);
  const chatsAvailable = manifest !== null;

  return (
    <>
      <div className="flex items-center gap-1.5 px-3 pb-1.5 pt-2.5">
        {chatsAvailable ? (
          <DropdownMenu>
            <DropdownMenuTrigger
              ref={triggerRef}
              aria-label={`Chat: ${title}`}
              className="flex min-w-0 items-center gap-1 rounded-control px-1 py-0.5 text-[13px] font-semibold text-foreground hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <span className="truncate">{title}</span>
              <ChevronDown className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="max-w-72">
              <DropdownMenuLabel>Chats</DropdownMenuLabel>
              <DropdownMenuRadioGroup value={active?.id ?? ""} onValueChange={(id) => void store.getState().selectAgentSession(id)}>
                {sessions.map((session) => (
                  <DropdownMenuRadioItem key={session.id} value={session.id} disabled={busy && session.id !== active?.id}>
                    <span className="truncate">{sessionTitle(session)}</span>
                  </DropdownMenuRadioItem>
                ))}
              </DropdownMenuRadioGroup>
              {active && (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onSelect={() => setDialog({ kind: "rename", session: active })}>
                    <Pencil className="h-3.5 w-3.5" aria-hidden />
                    Rename chat…
                  </DropdownMenuItem>
                  <DropdownMenuItem disabled={busy} onSelect={() => setDialog({ kind: "delete", session: active })}>
                    <Trash2 className="h-3.5 w-3.5" aria-hidden />
                    Delete chat…
                  </DropdownMenuItem>
                </>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        ) : (
          <h2 className="min-w-0 truncate px-1 text-[13px] font-semibold text-foreground">{title}</h2>
        )}
        <div className="flex-1" />
        {/* The tooltip wraps the label: as the switch's trigger it would overwrite its checked data-state. */}
        <Tooltip content={autoApplyTooltip}>
          <label className="flex shrink-0 items-center gap-1.5 rounded-control px-1 text-[11.5px] text-muted-foreground">
            Auto-apply
            <Switch
              aria-label="Auto-apply safe edits"
              aria-describedby={autoApplyDescriptionId}
              checked={autoApplySafe}
              onCheckedChange={(checked) => store.getState().setAutoApplySafe(checked)}
            />
            <span id={autoApplyDescriptionId} className="sr-only">
              {autoApplyTooltip}
            </span>
          </label>
        </Tooltip>
        {chatsAvailable && (
          <IconButton label="History" size="sm" active={historyOpen} aria-pressed={historyOpen} onClick={() => setHistoryOpen((open) => !open)}>
            <History className="h-4 w-4" aria-hidden />
          </IconButton>
        )}
        <IconButton label="New chat" size="sm" disabled={!chatsAvailable || busy} onClick={() => void store.getState().createAgentSession()}>
          <Plus className="h-4 w-4" aria-hidden />
        </IconButton>
      </div>
      {historyOpen && chatsAvailable && <ArchivedSessions sessions={manifest.deletedSessions} disabled={busy} />}
      {sessionError && (
        <p role="alert" className="px-4 pb-1 text-[12px] text-destructive">
          {sessionError}
        </p>
      )}
      {dialog?.kind === "rename" && (
        <NameDialog
          title="Rename chat"
          label="Chat name"
          initialName={sessionTitle(dialog.session)}
          submitLabel="Rename"
          returnFocusRef={triggerRef}
          onSubmit={(name) => store.getState().renameAgentSession(dialog.session.id, name)}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "delete" && (
        <ConfirmDeleteDialog
          title={`Delete “${sessionTitle(dialog.session)}”?`}
          description="The chat moves to History, where you can restore it."
          returnFocusRef={triggerRef}
          onConfirm={() => store.getState().deleteAgentSession(dialog.session.id)}
          onClose={() => setDialog(null)}
        />
      )}
    </>
  );
}
