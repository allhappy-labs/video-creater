function normalizeProjectDirectory(projectDir: string): string {
  const scheme = projectDir.match(/^([a-z][a-z0-9+.-]*:\/\/)(.*)$/i);
  const schemePrefix = scheme?.[1];
  const schemePath = scheme?.[2];
  const normalized = schemePrefix !== undefined && schemePath !== undefined
    ? `${schemePrefix}${schemePath.replace(/\/{2,}/g, "/")}`
    : projectDir.replace(/\/{2,}/g, "/");
  return normalized.length > 1 ? normalized.replace(/\/+$/, "") : normalized;
}

export function projectSessionKey(
  projectDir: string,
  projectId: string,
): string {
  return JSON.stringify([
    normalizeProjectDirectory(projectDir),
    projectId,
  ]);
}
