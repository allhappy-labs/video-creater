import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState, type RefObject } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { agentSetupSnippet, projectFolderLabel, type AgentSetupClient } from "@/lib/agent/selection-context";
import { formatShortcut, type ShortcutPlatform } from "@/lib/keymap";
import { OverlayDialog } from "./overlay-dialog";

const clientLabels: Readonly<Record<AgentSetupClient, string>> = {
  codex: "Codex",
  claudeCode: "Claude Code",
  claudeDesktop: "Claude Desktop",
  cursor: "Cursor",
};

const clients: readonly AgentSetupClient[] = ["codex", "claudeCode", "claudeDesktop", "cursor"];

function isAgentSetupClient(value: string): value is AgentSetupClient {
  return (clients as readonly string[]).includes(value);
}

type CopyStatus = { readonly kind: "copied" | "selected"; readonly client: AgentSetupClient } | null;

/** How long "Copied" stays on the button before it reads "Copy" again. */
const copiedResetMs = 2_000;

interface ConnectAgentsDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly projectDir: string;
  readonly platform: ShortcutPlatform;
  readonly returnFocusRef?: RefObject<HTMLElement | null>;
}

/** MCP setup snippets for external agent clients, one tab per client, each with a Copy button. */
export function ConnectAgentsDialog({ open, onOpenChange, projectDir, platform, returnFocusRef }: ConnectAgentsDialogProps) {
  const [client, setClient] = useState<AgentSetupClient>("codex");
  const [status, setStatus] = useState<CopyStatus>(null);
  const snippetRefs = useRef(new Map<AgentSetupClient, HTMLPreElement>());

  useEffect(() => {
    if (status?.kind !== "copied") return;
    const timer = window.setTimeout(() => setStatus(null), copiedResetMs);
    return () => window.clearTimeout(timer);
  }, [status]);

  function selectSnippet(target: AgentSetupClient) {
    const element = snippetRefs.current.get(target);
    const selection = window.getSelection();
    if (!element || !selection) return;
    element.focus();
    selection.selectAllChildren(element);
    setStatus({ kind: "selected", client: target });
  }

  async function copy(target: AgentSetupClient) {
    const snippet = agentSetupSnippet(target, projectDir);
    if (!navigator.clipboard?.writeText) {
      selectSnippet(target);
      return;
    }
    try {
      await navigator.clipboard.writeText(snippet);
      setStatus({ kind: "copied", client: target });
    } catch {
      // Denied or unfocused clipboard writes leave the snippet selected for a manual copy.
      selectSnippet(target);
    }
  }

  function changeOpen(next: boolean) {
    if (!next) setStatus(null);
    onOpenChange(next);
  }

  const statusText =
    status === null
      ? ""
      : status.kind === "copied"
        ? `Copied ${clientLabels[status.client]} setup`
        : `Press ${formatShortcut("Mod+C", platform)} to copy the selected setup`;

  return (
    <OverlayDialog
      open={open}
      onOpenChange={changeOpen}
      title="Connect external agents"
      description="Paste the setup into an agent so it can read this project and propose edits the editor validates."
      size="lg"
      {...(returnFocusRef ? { returnFocusRef } : {})}
    >
      <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
        <div className="mt-3 flex min-w-0 items-baseline gap-2 text-[12px]">
          <span className="shrink-0 text-dim">Project folder</span>
          <span className="min-w-0 truncate font-mono text-foreground" title={projectFolderLabel(projectDir)}>
            {projectFolderLabel(projectDir)}
          </span>
        </div>
        <Tabs
          value={client}
          onValueChange={(value) => {
            if (isAgentSetupClient(value)) setClient(value);
            setStatus(null);
          }}
          className="mt-3 flex flex-col"
        >
          <TabsList aria-label="Agent client" className="flex-wrap gap-1">
            {clients.map((id) => (
              <TabsTrigger
                key={id}
                value={id}
                className="h-8 shrink-0 rounded-control px-3 text-[13px] text-muted-foreground transition-colors hover:text-foreground data-[state=active]:bg-raised motion-reduce:transition-none"
              >
                {clientLabels[id]}
              </TabsTrigger>
            ))}
          </TabsList>
          {clients.map((id) => {
            const copied = status?.kind === "copied" && status.client === id;
            return (
              <TabsContent key={id} value={id} className="mt-2">
                <div className="relative rounded-control border border-line bg-background">
                  <pre
                    ref={(element) => {
                      if (element) snippetRefs.current.set(id, element);
                      else snippetRefs.current.delete(id);
                    }}
                    role="region"
                    aria-label={`${clientLabels[id]} setup`}
                    tabIndex={0}
                    className="max-h-[40dvh] overflow-auto whitespace-pre-wrap break-words p-3 pr-24 font-mono text-[12px] leading-relaxed text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  >
                    <code>{agentSetupSnippet(id, projectDir)}</code>
                  </pre>
                  <button
                    type="button"
                    onClick={() => void copy(id)}
                    className="absolute right-2 top-2 flex h-7 items-center gap-1.5 rounded-control bg-raised px-2.5 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none"
                  >
                    {copied ? <Check className="h-3.5 w-3.5 text-success" aria-hidden /> : <Copy className="h-3.5 w-3.5" aria-hidden />}
                    {copied ? "Copied" : "Copy"}
                  </button>
                </div>
              </TabsContent>
            );
          })}
        </Tabs>
        <p role="status" className="mt-2 min-h-[18px] text-[12px] text-muted-foreground">
          {statusText}
        </p>
      </div>
    </OverlayDialog>
  );
}
