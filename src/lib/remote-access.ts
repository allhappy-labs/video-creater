import { backendRequest } from "./runtime/backend-client";

type TailscaleRemoteState =
  | "missing"
  | "daemon_stopped"
  | "signed_out"
  | "unauthorized"
  | "serve_not_configured"
  | "health_unreachable"
  | "ready";

export interface RemoteAccessStatus {
  readonly serviceState: "running" | "stopped" | "failed" | "unavailable";
  readonly managementAvailable: boolean;
  readonly detail: string;
  readonly tailscale: {
    readonly state: TailscaleRemoteState;
    readonly dnsName: string | null;
    readonly tailnetIp: string | null;
    readonly url: string | null;
    readonly healthVerified: boolean;
    readonly detail: string;
  };
}

export function getRemoteAccessStatus(): Promise<RemoteAccessStatus> {
  return backendRequest("get_remote_access_status");
}

export function setRemoteAccessRunning(running: boolean): Promise<RemoteAccessStatus> {
  return backendRequest("set_remote_access_running", { running });
}
