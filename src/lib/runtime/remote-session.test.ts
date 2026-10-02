import { describe, expect, it } from "vitest";

import {
  discoverRemoteSession,
  listRemoteDeviceSessions,
  pairRemoteDevice,
  revokeRemoteDeviceSession,
} from "./remote-session";

describe("remote session", () => {
  it("negotiates durable outcome recovery only when the host advertises protocol one", async () => {
    const state = await discoverRemoteSession(async () => new Response(JSON.stringify({ authenticated: true, sessionId: "s", csrfToken: "c", protocolVersion: 1, outcomeProtocol: 1 })));
    expect(state).toMatchObject({ kind: "connected", outcomeProtocol: 1 });
  });
  it("discovers authenticated compatible sessions from the same origin", async () => {
    const state = await discoverRemoteSession(async () => new Response(JSON.stringify({
      authenticated: true,
      sessionId: "session-1",
      displayName: "Phone",
      csrfToken: "csrf-1",
      hostLabel: "Studio host",
      protocolVersion: 1,
    }), { status: 200 }));
    expect(state).toEqual({ kind: "connected", sessionId: "session-1", displayName: "Phone", hostLabel: "Studio host", csrfToken: "csrf-1" });
  });

  it("blocks incompatible protocol versions and pairs without accepting a host URL", async () => {
    const incompatible = await discoverRemoteSession(async () => new Response(JSON.stringify({
      authenticated: true, sessionId: "s", displayName: "Phone", csrfToken: "c", protocolVersion: 2,
    }), { status: 200 }));
    expect(incompatible).toEqual({ kind: "incompatible", hostProtocolVersion: 2 });
    const older = await discoverRemoteSession(async () => new Response(JSON.stringify({
      authenticated: true, sessionId: "s", displayName: "Phone", csrfToken: "c", protocolVersion: 0,
    }), { status: 200 }));
    expect(older).toEqual({ kind: "incompatible", hostProtocolVersion: 0 });

    let target = "";
    await pairRemoteDevice("123456", "Phone", async (input) => {
      target = String(input);
      return new Response(JSON.stringify({ authenticated: true, sessionId: "s", csrfToken: "c", protocolVersion: 1 }));
    });
    expect(target).toBe("/api/v1/pair");
  });

  it("lists paired devices without credentials and revokes by opaque session ID", async () => {
    const devices = await listRemoteDeviceSessions(async () => new Response(JSON.stringify({
      currentSessionId: "session-1",
      sessions: [{
        sessionId: "session-1",
        displayName: "Phone",
        identity: "person@example.test",
        createdAt: 100,
        expiresAt: 200,
        revoked: false,
      }],
    })));
    expect(devices).toEqual([expect.objectContaining({ sessionId: "session-1", current: true })]);

    let request: { url?: string; init?: RequestInit | undefined } = {};
    await revokeRemoteDeviceSession("session-2", "csrf-1", async (input, init) => {
      request = { url: String(input), init };
      return new Response(JSON.stringify({ revoked: true }));
    });
    expect(request.url).toBe("/api/v1/admin/sessions/session-2/revoke");
    expect(request.init?.headers).toEqual(expect.objectContaining({ "x-csrf-token": "csrf-1" }));
  });
});
