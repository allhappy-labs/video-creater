import type { MediaFolder } from "@/lib/project";

export function mediaFolderPathLabel(folders: MediaFolder[], folderId: string | null | undefined) {
  if (!folderId) {
    return null;
  }

  const foldersById = new Map(folders.map((folder) => [folder.id, folder]));
  const folder = foldersById.get(folderId);
  if (!folder) {
    return null;
  }

  const path = [folder.name];
  const seenFolderIds = new Set([folder.id]);
  let currentParentId = folder.parentId;
  while (currentParentId) {
    if (seenFolderIds.has(currentParentId)) {
      break;
    }
    const parent = foldersById.get(currentParentId);
    if (!parent) {
      break;
    }
    seenFolderIds.add(parent.id);
    path.unshift(parent.name);
    currentParentId = parent.parentId;
  }

  return path.join(" / ");
}

export interface MediaFolderNode {
  folder: MediaFolder;
  label: string;
  depth: number;
  children: MediaFolderNode[];
}

export function mediaFolderTree(folders: MediaFolder[]) {
  const childrenByParent = new Map<string | null, MediaFolder[]>();
  const foldersById = new Map(folders.map((folder) => [folder.id, folder]));

  function safeParentId(folder: MediaFolder) {
    if (!folder.parentId || !foldersById.has(folder.parentId)) {
      return null;
    }

    const seen = new Set([folder.id]);
    let currentParentId: string | null = folder.parentId;
    while (currentParentId) {
      if (seen.has(currentParentId)) {
        return null;
      }
      seen.add(currentParentId);
      currentParentId = foldersById.get(currentParentId)?.parentId ?? null;
    }

    return folder.parentId;
  }

  for (const folder of folders) {
    const parentId = safeParentId(folder);
    childrenByParent.set(parentId, [...(childrenByParent.get(parentId) ?? []), folder]);
  }

  function walk(folder: MediaFolder, depth: number, parentLabel: string | null): MediaFolderNode {
    const label = parentLabel ? `${parentLabel} / ${folder.name}` : folder.name;
    return {
      folder,
      label,
      depth,
      children: (childrenByParent.get(folder.id) ?? []).map((child) =>
        walk(child, depth + 1, label),
      ),
    };
  }

  return (childrenByParent.get(null) ?? []).map((folder) => walk(folder, 0, null));
}

export function flattenMediaFolderNodes(nodes: MediaFolderNode[]) {
  return nodes.flatMap((node): MediaFolderNode[] => [
    node,
    ...flattenMediaFolderNodes(node.children),
  ]);
}

export function mediaFolderSlug(name: string) {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return slug || "folder";
}

export function mediaFolderIdForName(name: string, folders: MediaFolder[]) {
  const baseId = `folder-${mediaFolderSlug(name)}`;
  const existingIds = new Set(folders.map((folder) => folder.id));
  if (!existingIds.has(baseId)) {
    return baseId;
  }

  let suffix = 2;
  while (existingIds.has(`${baseId}-${suffix}`)) {
    suffix += 1;
  }

  return `${baseId}-${suffix}`;
}
