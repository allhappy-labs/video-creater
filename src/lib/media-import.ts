import { openMediaFiles } from "./runtime/adapters/tauri-dialog";

const supportedMediaFileExtensions = [
  "mp4",
  "mov",
  "m4v",
  "webm",
  "wav",
  "mp3",
  "m4a",
  "aac",
  "aiff",
  "aifc",
  "flac",
  "png",
  "jpg",
  "jpeg",
  "webp",
  "tiff",
  "tif",
  "heic",
  "heif",
  "lottie",
  "json",
] as const;

/** Opens the native media chooser; a cancelled chooser yields no paths. */
export async function openMediaFilePaths(): Promise<string[]> {
  return (
    (await openMediaFiles({
      title: "Import media",
      filters: [
        {
          name: "Video, image, audio, and Lottie",
          extensions: supportedMediaFileExtensions,
        },
      ],
    })) ?? []
  );
}

/** True when an HTML5 drag carries external files (not internal media or clip payloads). */
export function dataTransferHasFiles(dataTransfer: DataTransfer | null): boolean {
  return Array.from(dataTransfer?.types ?? []).includes("Files");
}

export interface ExternalFilePathOptions {
  /**
   * Fall back to bare `File.name` values when the drop exposes no filesystem paths. Only the
   * fixture runtime enables this, because its fixture backend resolves media by file name.
   */
  readonly fileNameFallback?: boolean;
}

export function externalFilePathsFromDataTransfer(
  dataTransfer: DataTransfer,
  options: ExternalFilePathOptions = {},
): string[] {
  const files = Array.from(dataTransfer.files ?? []);
  const filePaths = files
    .map((file) => (file as File & { path?: string }).path?.trim() ?? "")
    .filter(Boolean);
  if (filePaths.length > 0) return filePaths;

  const uriPaths = (
    typeof dataTransfer.getData === "function" ? dataTransfer.getData("text/uri-list") : ""
  )
    .split(/\r?\n/)
    .map((uri) => uri.trim())
    .filter((uri) => uri.startsWith("file://"))
    .map((uri) => decodeURIComponent(uri.replace(/^file:\/\//, "")));
  if (uriPaths.length > 0 || !options.fileNameFallback) return uriPaths;

  return files.map((file) => file.name.trim()).filter(Boolean);
}

export function formatSkippedMediaImports(
  skipped: readonly { sourcePath: string; reason: string }[],
): string | null {
  if (skipped.length === 0) return null;
  return skipped
    .map(({ sourcePath, reason }) => {
      const filename = sourcePath.split(/[\\/]/).pop() || sourcePath;
      return `${filename}: ${reason}`;
    })
    .join(" · ");
}
