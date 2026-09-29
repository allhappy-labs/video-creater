import { useEffect, useRef, useState } from "react";
import { Copy, Loader2, RadioTower } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  getMcpClientConfiguration,
  type McpClientConfigurationState,
} from "@/lib/settings/agent";

export interface AgentMcpSettingsProps {
  projectRoot: string | null;
}

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  return typeof error === "string" ? error : String(error);
}

export function AgentMcpSettings({ projectRoot }: AgentMcpSettingsProps) {
  const [result, setResult] = useState<{
    root: string;
    configuration: McpClientConfigurationState;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copiedRoot, setCopiedRoot] = useState<string | null>(null);
  const rootRef = useRef(projectRoot);
  const generationRef = useRef(0);
  if (rootRef.current !== projectRoot) {
    rootRef.current = projectRoot;
    generationRef.current += 1;
  }

  useEffect(() => {
    setResult(null);
    setError(null);
    setCopiedRoot(null);
    if (!projectRoot) return;
    let cancelled = false;
    const requestRoot = projectRoot;
    const generation = generationRef.current;
    void getMcpClientConfiguration(requestRoot)
      .then((configuration) => {
        if (
          !cancelled &&
          rootRef.current === requestRoot &&
          generationRef.current === generation
        ) {
          setResult({ root: requestRoot, configuration });
        }
      })
      .catch((requestError) => {
        if (
          !cancelled &&
          rootRef.current === requestRoot &&
          generationRef.current === generation
        ) {
          setError(errorMessage(requestError));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectRoot]);

  async function copyConfiguration() {
    if (!projectRoot || result?.root !== projectRoot) return;
    const configuration = result.configuration.configuration;
    if (!configuration) return;
    const requestRoot = projectRoot;
    const generation = generationRef.current;
    setError(null);
    try {
      await navigator.clipboard.writeText(configuration);
      if (
        rootRef.current === requestRoot &&
        generationRef.current === generation
      ) {
        setCopiedRoot(requestRoot);
      }
    } catch (copyError) {
      if (
        rootRef.current === requestRoot &&
        generationRef.current === generation
      ) {
        setError(errorMessage(copyError));
      }
    }
  }

  const configuration =
    result?.root === projectRoot ? result.configuration : null;

  return (
    <section
      data-settings-target="advanced:mcp"
      tabIndex={-1}
      aria-labelledby="mcp-client-configuration-heading"
      className="grid gap-3 border-t py-3 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)]"
    >
      <div className="flex min-w-0 items-start gap-2">
        <RadioTower className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div>
          <h2 id="mcp-client-configuration-heading" className="text-xs font-semibold">
            MCP client configuration
          </h2>
          <p className="text-[11px] leading-5 text-muted-foreground">
            Generate a client entry pinned to the currently open project.
          </p>
        </div>
      </div>

      <div className="grid min-w-0 gap-2 md:justify-items-end">
        {!projectRoot ? (
          <p className="text-[11px] text-muted-foreground">
            Open a project to copy MCP configuration.
          </p>
        ) : !configuration && !error ? (
          <p role="status" className="flex items-center gap-2 text-[11px] text-muted-foreground">
            <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
            Generating project configuration
          </p>
        ) : null}
        {configuration?.configuration ? (
          <>
            <code className="block max-w-full truncate font-mono text-[11px] text-muted-foreground" title={configuration.projectDir ?? undefined}>
              {configuration.projectDir}
            </code>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 px-2"
              onClick={() => void copyConfiguration()}
              aria-label="Copy client configuration"
            >
              <Copy className="h-3.5 w-3.5" aria-hidden="true" />
              {copiedRoot === projectRoot ? "Copied" : "Copy configuration"}
            </Button>
          </>
        ) : null}
        {error ? (
          <p role="alert" className="border-l-2 border-red-500 pl-2 text-red-700">
            MCP configuration could not be generated: {error}
          </p>
        ) : null}
      </div>
    </section>
  );
}
