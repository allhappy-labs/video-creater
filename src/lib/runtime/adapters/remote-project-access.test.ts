import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  clearRemoteProjectAccessForTests,
  remoteEditorLeaseToken,
  remoteProjectAccess,
  setRemoteProjectAccess,
  setRemoteTakeoverHandler,
  takeOverRemoteProject,
} from "./remote-project-access";

describe("remote project access", () => {
  beforeEach(clearRemoteProjectAccessForTests);

  it("keeps lease tokens private while exposing edit ownership state", () => {
    setRemoteProjectAccess("project-a", { mode: "editing", token: "secret-token", expiresAt: 200 });
    expect(remoteEditorLeaseToken("project-a")).toBe("secret-token");
    expect(remoteProjectAccess("project-a")).toEqual({ mode: "editing", expiresAt: 200 });

    setRemoteProjectAccess("project-a", { mode: "readOnly", editorDisplayName: "Office laptop" });
    expect(remoteEditorLeaseToken("project-a")).toBeUndefined();
    expect(remoteProjectAccess("project-a")).toEqual({ mode: "readOnly", editorDisplayName: "Office laptop" });
  });

  it("requires an installed remote transport for explicit takeover", async () => {
    await expect(takeOverRemoteProject("project-a")).rejects.toThrow("not connected");
    const takeover = vi.fn(async () => undefined);
    setRemoteTakeoverHandler(takeover);
    await takeOverRemoteProject("project-a");
    expect(takeover).toHaveBeenCalledWith("project-a");
  });
});
