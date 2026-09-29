import type { RefObject } from "react";
import { pluralize } from "@/lib/format";
import { mediaDisplayName } from "@/lib/media/names";
import type { MediaAsset, MediaFolder } from "@/lib/project";
import { mediaInUseCopy, mediaUsedByGeneration, type MediaService } from "../../services/media-service";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { ConfirmDeleteDialog, NameDialog } from "./folder-dialogs";
import { folderItemCount, mediaClips } from "./media-browser";
import { MatteDialog } from "./matte-dialog";

export type MediaPanelDialog =
  | { readonly kind: "newFolder" }
  | { readonly kind: "renameFolder"; readonly folder: MediaFolder }
  | { readonly kind: "deleteFolder"; readonly folder: MediaFolder }
  | { readonly kind: "renameMedia"; readonly media: MediaAsset }
  | { readonly kind: "deleteMedia"; readonly media: MediaAsset }
  | { readonly kind: "matte" };

interface MediaPanelDialogsProps {
  readonly dialog: MediaPanelDialog | null;
  readonly service: MediaService;
  /** The open folder; new mattes are created in it. */
  readonly folderId: string | null;
  readonly moreButtonRef: RefObject<HTMLButtonElement | null>;
  onClose(): void;
}

function deleteMediaDescription(clipCount: number): string {
  if (clipCount === 0) return "This removes it from the project. It isn't used on the timeline.";
  const clips = clipCount === 1 ? "the clip that uses" : `the ${clipCount} clips that use`;
  return `This removes it from the project and deletes ${clips} it on the timeline.`;
}

/** The Media tab's folder, rename, delete and matte dialogs; one is open at a time. */
export function MediaPanelDialogs({ dialog, service, folderId, moreButtonRef, onClose }: MediaPanelDialogsProps) {
  const store = useEditorStoreApi();
  const project = useEditorStore((state) => state.project);
  if (!dialog) return null;
  switch (dialog.kind) {
    case "newFolder":
      return (
        <NameDialog
          title="New folder"
          label="Folder name"
          initialName=""
          submitLabel="Create folder"
          returnFocusRef={moreButtonRef}
          onSubmit={async (name) => {
            // Folders are created at the top level, so show the project root where the new tile is.
            const created = (await service.createFolder(name)) !== null;
            if (created) store.getState().setMediaFolderId(null);
            return created;
          }}
          onClose={onClose}
        />
      );
    case "renameFolder":
      return (
        <NameDialog
          title={`Rename folder "${dialog.folder.name}"`}
          label="Folder name"
          initialName={dialog.folder.name}
          submitLabel="Rename"
          onSubmit={(name) => service.renameFolder(dialog.folder.id, name)}
          onClose={onClose}
        />
      );
    case "deleteFolder": {
      const count = folderItemCount(project, dialog.folder.id);
      return (
        <ConfirmDeleteDialog
          title={`Delete folder "${dialog.folder.name}"?`}
          description={
            count > 0
              ? `It holds ${pluralize(count, "item")}. They stay in the project; only the folder is removed.`
              : "The folder is empty. Only the folder is removed."
          }
          onConfirm={() => service.deleteFolder(dialog.folder.id)}
          onClose={onClose}
        />
      );
    }
    case "renameMedia":
      return (
        <NameDialog
          title={`Rename "${mediaDisplayName(dialog.media)}"`}
          label="Media name"
          initialName={mediaDisplayName(dialog.media)}
          submitLabel="Rename"
          onSubmit={(name) => service.renameMedia(dialog.media.id, name)}
          onClose={onClose}
        />
      );
    case "deleteMedia":
      return (
        <ConfirmDeleteDialog
          title={`Delete "${mediaDisplayName(dialog.media)}"?`}
          description={deleteMediaDescription(mediaClips(project, dialog.media.id).length)}
          blockedReason={mediaUsedByGeneration(project, dialog.media.id) ? mediaInUseCopy : null}
          onConfirm={() => service.deleteMedia(dialog.media.id)}
          onClose={onClose}
        />
      );
    case "matte":
      return (
        <MatteDialog
          timelineWidth={project.renderSettings.width}
          timelineHeight={project.renderSettings.height}
          returnFocusRef={moreButtonRef}
          onCreate={(input) => service.createMatte(input, folderId)}
          onClose={onClose}
        />
      );
  }
}
