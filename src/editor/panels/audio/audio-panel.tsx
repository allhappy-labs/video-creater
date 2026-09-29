import { Download, Sparkles } from "lucide-react";
import { useState } from "react";
import { useMediaService } from "../../services/media-service";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { GenerateView } from "../generate/generate-view";
import { AudioList } from "./audio-list";
import { CleanupCards } from "./cleanup-cards";

const actionButtonClass =
  "flex h-8 items-center justify-center gap-2 rounded-control text-[13px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none";

/** The Audio tab: Import and Generate, speech cleanup for a target, and the project audio list. */
export function AudioPanel() {
  const store = useEditorStoreApi();
  const service = useMediaService();
  const generateView = useEditorStore((state) => state.generateView);
  const [importing, setImporting] = useState(false);
  const [skippedNotice, setSkippedNotice] = useState<string | null>(null);

  async function importAudio() {
    setSkippedNotice(null);
    setImporting(true);
    const outcome = await service.importMediaFiles(undefined, { audioOnly: true });
    setImporting(false);
    if (outcome.status === "imported") setSkippedNotice(outcome.notice);
  }

  if (generateView?.open && generateView.mode === "audio") {
    return <GenerateView tab="audio" targetFolderId={null} onClose={() => store.getState().setGenerateView(null)} />;
  }

  return (
    <div className="flex flex-col gap-4 p-3">
      <div className="grid grid-cols-2 gap-2">
        <button
          type="button"
          aria-label="Import audio"
          disabled={importing}
          onClick={() => void importAudio()}
          className={`${actionButtonClass} bg-primary text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-40`}
        >
          <Download className="h-4 w-4" aria-hidden />
          {importing ? "Importing…" : "Import"}
        </button>
        <button
          type="button"
          aria-label="Generate audio"
          onClick={() => store.getState().setGenerateView({ open: true, mode: "audio" })}
          className={`${actionButtonClass} bg-raised text-foreground hover:bg-hover`}
        >
          <Sparkles className="h-4 w-4" aria-hidden />
          Generate
        </button>
      </div>
      {skippedNotice && (
        <p role="status" className="text-[12px] text-warning">
          Skipped {skippedNotice}
        </p>
      )}
      <CleanupCards />
      <AudioList />
    </div>
  );
}
