import { useEffect, useState } from "react";
import { CheckCircle2, Loader2, RefreshCw, ShieldCheck, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useRuntimeMode } from "@/editor/services/use-runtime-mode";
import {
  getRemoteAccessStatus,
  setRemoteAccessRunning,
  type RemoteAccessStatus,
} from "@/lib/remote-access";
import {
  discoverRemoteSession,
  listRemoteDeviceSessions,
  revokeRemoteDeviceSession,
  type RemoteDeviceSession,
} from "@/lib/runtime/remote-session";

export function RemoteAccessSettings() {
  const runtimeMode = useRuntimeMode();
  const [status, setStatus] = useState<RemoteAccessStatus | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [devices, setDevices] = useState<readonly RemoteDeviceSession[]>([]);
  const [deviceCsrf, setDeviceCsrf] = useState<string | null>(null);
  const [deviceError, setDeviceError] = useState<string | null>(null);

  async function refreshDevices() {
    if (runtimeMode !== "browser") return;
    try {
      const session = await discoverRemoteSession();
      if (session.kind !== "connected") throw new Error("This browser is not paired.");
      setDeviceCsrf(session.csrfToken);
      setDevices(await listRemoteDeviceSessions());
      setDeviceError(null);
    } catch (cause) {
      setDeviceError(cause instanceof Error ? cause.message : "Paired devices could not be loaded.");
    }
  }

  async function revokeDevice(sessionId: string) {
    if (!deviceCsrf) return;
    setPending(true);
    try {
      await revokeRemoteDeviceSession(sessionId, deviceCsrf);
      if (devices.some((device) => device.sessionId === sessionId && device.current)) {
        window.location.reload();
        return;
      }
      await refreshDevices();
    } catch (cause) {
      setDeviceError(cause instanceof Error ? cause.message : "The paired device could not be revoked.");
    } finally {
      setPending(false);
    }
  }

  async function refresh() {
    setPending(true);
    setError(null);
    try {
      setStatus(await getRemoteAccessStatus());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Remote access status could not be loaded.");
    } finally {
      setPending(false);
    }
  }

  async function setRunning(running: boolean) {
    setPending(true);
    setError(null);
    try {
      setStatus(await setRemoteAccessRunning(running));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Remote access could not be changed.");
    } finally {
      setPending(false);
    }
  }

  useEffect(() => {
    void refresh();
    void refreshDevices();
  }, [runtimeMode]);

  const ready = status?.tailscale.state === "ready" && status.tailscale.healthVerified;
  const running = status?.serviceState === "running";

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-5 p-4 md:p-6">
      <div>
        <h2 className="text-lg font-semibold">Remote access</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Open this editor from another device on your tailnet. The host stays loopback-only and Tailscale provides HTTPS.
        </p>
      </div>

      <section className="rounded-xl border bg-card p-4" aria-label="Remote host service">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h3 className="text-sm font-medium">Host service</h3>
            <p className="mt-1 text-sm text-muted-foreground">{status?.detail ?? "Checking the host service…"}</p>
          </div>
          <span className="rounded-full bg-muted px-2.5 py-1 text-xs capitalize text-muted-foreground">
            {status?.serviceState ?? "checking"}
          </span>
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button
            type="button"
            disabled={pending || !status?.managementAvailable || running}
            onClick={() => void setRunning(true)}
          >
            {pending && <Loader2 className="mr-2 h-4 w-4 animate-spin" aria-hidden="true" />}
            Start
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={pending || !status?.managementAvailable || !running}
            onClick={() => void setRunning(false)}
          >
            Stop
          </Button>
          <Button type="button" variant="ghost" disabled={pending} onClick={() => void refresh()}>
            <RefreshCw className="mr-2 h-4 w-4" aria-hidden="true" />
            Refresh
          </Button>
        </div>
      </section>

      <section className="rounded-xl border bg-card p-4" aria-label="Tailnet connection">
        <div className="flex gap-3">
          {ready
            ? <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" aria-hidden="true" />
            : <WifiOff className="mt-0.5 h-5 w-5 shrink-0 text-amber-400" aria-hidden="true" />}
          <div className="min-w-0">
            <h3 className="text-sm font-medium">{ready ? "Verified tailnet URL" : "Tailnet route not ready"}</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              {status?.tailscale.detail ?? "Checking Tailscale Serve…"}
            </p>
            {ready && status?.tailscale.url && (
              <a
                className="mt-3 block break-all text-sm text-primary underline-offset-4 hover:underline"
                href={status.tailscale.url}
                rel="noreferrer"
                target="_blank"
              >
                {status.tailscale.url}
              </a>
            )}
          </div>
        </div>
      </section>

      {runtimeMode === "browser" && (
        <section className="rounded-xl border bg-card p-4" aria-label="Paired devices">
          <div className="flex items-start justify-between gap-3">
            <div>
              <h3 className="text-sm font-medium">Paired devices</h3>
              <p className="mt-1 text-sm text-muted-foreground">
                Revoke any browser that should no longer reach this host.
              </p>
            </div>
            <Button type="button" variant="ghost" disabled={pending} onClick={() => void refreshDevices()}>
              <RefreshCw className="mr-2 h-4 w-4" aria-hidden="true" />
              Refresh
            </Button>
          </div>
          <ul className="mt-3 divide-y divide-border">
            {devices.map((device) => (
              <li key={device.sessionId} className="flex items-center justify-between gap-3 py-3">
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">
                    {device.displayName}{device.current ? " (this device)" : ""}
                  </p>
                  <p className="truncate text-xs text-muted-foreground">
                    {device.identity ?? "Tailnet identity unavailable"} · {device.revoked ? "Revoked" : "Active"}
                  </p>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  disabled={pending || device.revoked}
                  onClick={() => void revokeDevice(device.sessionId)}
                >
                  Revoke
                </Button>
              </li>
            ))}
          </ul>
          {devices.length === 0 && !deviceError && (
            <p className="mt-3 text-sm text-muted-foreground">No paired devices were returned.</p>
          )}
          {deviceError && <p role="alert" className="mt-3 text-sm text-destructive">{deviceError}</p>}
        </section>
      )}

      <div className="flex gap-2 rounded-lg border border-white/10 bg-muted/20 p-3 text-xs text-muted-foreground">
        <ShieldCheck className="h-4 w-4 shrink-0" aria-hidden="true" />
        Pairing codes are shown only on the host. This screen never displays credentials, session tokens, identity headers, or private paths.
      </div>
      {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
    </div>
  );
}
