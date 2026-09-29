import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  BackendInput,
  BackendTransport,
  BackendUnlisten,
} from "../backend-transport";

export class TauriTransport implements BackendTransport {
  readonly kind = "tauri" as const;

  request<Result>(
    operation: string,
    input?: BackendInput,
  ): Promise<Result> {
    return invoke<Result>(operation, input);
  }

  listen<Payload>(
    event: string,
    handler: (payload: Payload) => void,
  ): Promise<BackendUnlisten> {
    return listen<Payload>(event, ({ payload }) => handler(payload));
  }

  mediaUrl(path: string): string {
    const streamBase = mediaStreamBase();
    return streamBase ? `${streamBase}/${encodeURIComponent(path)}` : convertFileSrc(path);
  }
}

/**
 * Linux desktop hosts inject an authenticated loopback HTTP base URL because WebKitGTK does
 * not play media from custom URI schemes such as asset://.
 */
function mediaStreamBase(): string | null {
  const base = (globalThis as { __VIDEO_CREATER_MEDIA_STREAM_BASE__?: unknown })
    .__VIDEO_CREATER_MEDIA_STREAM_BASE__;
  return typeof base === "string" && /^http:\/\/127\.0\.0\.1:\d+\/media\/[0-9a-f]+$/.test(base)
    ? base
    : null;
}
