/** Set once the v1 editor keys have been removed from this browser profile. */
export const legacyStorageCleanedKey = "video-creater.editor.v2.legacy-cleaned";

/** Exact v1 editor keys that no v2 code reads. */
const legacyKeys: ReadonlySet<string> = new Set([
  "video-creater.workspace-layout.v1",
  "video-creater:responsive-rail-state",
  "video-creater.editorTour.dismissed.v1",
]);

/** v1 editor key families: the (optionally scoped) chat transcripts and per-project timeline views. */
const legacyKeyPrefixes: readonly string[] = [
  "video-creater.codexChatTranscript.v1",
  "video-creater.timeline-view-state:",
];

function isLegacyKey(key: string): boolean {
  return legacyKeys.has(key) || legacyKeyPrefixes.some((prefix) => key.startsWith(prefix));
}

function localStorageOrNull(): Storage | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

/**
 * Removes the v1 editor keys once per profile, then records the flag so later mounts skip the scan.
 * Best-effort: unavailable or failing storage leaves everything as it was.
 */
export function cleanLegacyEditorStorage(storage: Storage | null = localStorageOrNull()): void {
  if (!storage) return;
  try {
    if (storage.getItem(legacyStorageCleanedKey) !== null) return;
    const stale: string[] = [];
    for (let index = 0; index < storage.length; index += 1) {
      const key = storage.key(index);
      if (key !== null && isLegacyKey(key)) stale.push(key);
    }
    for (const key of stale) storage.removeItem(key);
    storage.setItem(legacyStorageCleanedKey, "true");
  } catch {
    // Storage blocked or full: try again on the next mount.
  }
}
