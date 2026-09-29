import { useState, type FormEvent, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import {
  pairRemoteDevice,
  type RemoteSessionState,
} from "@/lib/runtime/remote-session";

interface DesktopHostConnectionProps {
  readonly initialState?: RemoteSessionState | undefined;
  readonly onPaired?: () => void;
}

export function DesktopHostConnection({
  initialState = { kind: "unpaired" },
  onPaired = () => window.location.reload(),
}: DesktopHostConnectionProps) {
  const [state, setState] = useState(initialState);
  const [code, setCode] = useState("");
  const [deviceName, setDeviceName] = useState(defaultDeviceName);
  const [pairing, setPairing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submitPairing = async (event: FormEvent) => {
    event.preventDefault();
    setPairing(true);
    setError(null);
    try {
      const next = await pairRemoteDevice(code, deviceName.trim());
      setState(next);
      if (next.kind === "connected") onPaired();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "This device could not be paired.");
    } finally {
      setPairing(false);
    }
  };

  const content = state.kind === "incompatible" ? (
    <StatusMessage
      title="Update required"
      detail={`This browser supports protocol 1, but the host protocol ${state.hostProtocolVersion} is running. Update Video Creater on this machine, then reload.`}
    />
  ) : state.kind === "unavailable" ? (
    <StatusMessage
      title="Host unavailable"
      detail="The browser could not reach the Video Creater service. Check that the host is still running, then reload."
      action={<Button onClick={() => window.location.reload()}>Try again</Button>}
    />
  ) : state.kind === "revoked" ? (
    <StatusMessage
      title="Access revoked"
      detail="This browser no longer has access to the host. Reload to pair it again with a new code."
      action={<Button onClick={() => window.location.reload()}>Pair again</Button>}
    />
  ) : state.kind === "reconnecting" ? (
    <StatusMessage
      title="Reconnecting"
      detail="Your edits stay on the host. This browser is reconnecting to the event stream."
    />
  ) : state.kind === "readOnly" ? (
    <StatusMessage
      title="View only"
      detail={`${state.editorDisplayName} is currently editing this project. You can review it here or request a deliberate takeover.`}
    />
  ) : state.kind === "takeover" ? (
    <StatusMessage
      title="Editor access moved here"
      detail={`${state.previousEditorDisplayName} is now view only. This device owns the editing lease.`}
    />
  ) : (
    <>
      <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
        Secure browser connection
      </p>
      <h1 className="mt-2 text-xl font-semibold text-foreground">Pair this device</h1>
      <p className="mt-3 text-sm leading-6 text-muted-foreground">
        Enter the six-digit code shown by the running Video Creater host. The browser connects only to this page&apos;s host.
      </p>
      <form className="mt-6 space-y-4" onSubmit={submitPairing}>
        <label className="block text-sm font-medium text-foreground">
          Pairing code
          <input
            autoComplete="one-time-code"
            autoFocus
            className="mt-2 h-11 w-full rounded-md border border-input bg-background px-3 font-mono text-lg tracking-[0.28em] text-foreground outline-none transition focus-visible:ring-1 focus-visible:ring-ring"
            inputMode="numeric"
            maxLength={6}
            pattern="[0-9]{6}"
            placeholder="000000"
            value={code}
            onChange={(event) => setCode(event.target.value.replace(/\D/g, "").slice(0, 6))}
          />
        </label>
        <label className="block text-sm font-medium text-foreground">
          Device name
          <input
            autoComplete="off"
            className="mt-2 h-10 w-full rounded-md border border-input bg-background px-3 text-sm text-foreground outline-none transition focus-visible:ring-1 focus-visible:ring-ring"
            maxLength={100}
            value={deviceName}
            onChange={(event) => setDeviceName(event.target.value)}
          />
        </label>
        {error ? <p role="alert" className="text-sm text-destructive">{error}</p> : null}
        <Button className="w-full" disabled={pairing || code.length !== 6 || !deviceName.trim()} type="submit">
          {pairing ? "Pairing…" : "Pair device"}
        </Button>
      </form>
    </>
  );

  return (
    <main
      aria-label="Desktop host connection"
      className="grid min-h-screen place-items-center bg-background p-5 text-foreground sm:p-8"
    >
      <section className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-2xl shadow-black/20 sm:p-8">
        {content}
      </section>
    </main>
  );
}

function StatusMessage({
  title,
  detail,
  action,
}: {
  readonly title: string;
  readonly detail: string;
  readonly action?: ReactNode;
}) {
  return (
    <>
      <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">Browser connection</p>
      <h1 className="mt-2 text-xl font-semibold text-foreground">{title}</h1>
      <p className="mt-3 text-sm leading-6 text-muted-foreground">{detail}</p>
      {action ? <div className="mt-6">{action}</div> : null}
    </>
  );
}

const defaultDeviceName = typeof navigator === "undefined"
  ? "Browser device"
  : /iPad|iPhone|Android/i.test(navigator.userAgent)
    ? "Mobile browser"
    : "Web browser";
