export interface RecoveryCopy {
  title: string;
  message: string;
}

export type ProjectFolderRecoveryKind = "open" | "create";

export function projectFolderRecoveryCopy(
  error: unknown,
  kind: ProjectFolderRecoveryKind,
): RecoveryCopy {
  const detail = errorMessage(error);
  const lowerDetail = detail.toLowerCase();

  if (
    lowerDetail.includes("video-creater.project.json") ||
    lowerDetail.includes("manifest")
  ) {
    return {
      title: "Project folder is missing its manifest",
      message:
        "Choose a split project folder that contains video-creater.project.json, or create a new project in this folder. " +
        detail,
    };
  }

  if (
    lowerDetail.includes("permission") ||
    lowerDetail.includes("writable") ||
    lowerDetail.includes("read-only") ||
    lowerDetail.includes("denied")
  ) {
    return {
      title:
        kind === "create"
          ? "Project folder is not writable"
          : "Project folder cannot be read",
      message:
        "Check folder permissions or choose a different local folder, then try again. " +
        detail,
    };
  }

  return {
    title:
      kind === "create"
        ? "Project folder could not be created"
        : "Project folder could not be opened",
    message:
      "Check that the folder exists, is accessible, and belongs to a Video Creater split project. " +
      detail,
  };
}

function errorMessage(error: unknown) {
  if (error instanceof Error && error.message.trim()) {
    return error.message.trim();
  }
  if (typeof error === "string" && error.trim()) {
    return error.trim();
  }
  return "No detailed error was returned.";
}
