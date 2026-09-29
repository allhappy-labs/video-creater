import { useMemo, useState } from "react";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { SelectField } from "../../properties/controls/select-field";
import { useEditorStore } from "../../store/editor-store-context";
import { CaptionStylesView } from "./caption-styles-view";
import {
  captionSourceOptions,
  defaultCaptionBuildSettings,
  defaultCaptionSourceId,
  transcriptForMedia,
  type CaptionBuildSettings,
} from "./captions-model";
import { GenerateCaptionsCard } from "./generate-captions-card";
import { TranscriptView } from "./transcript-view";
import { useCaptionGeneration } from "./use-caption-generation";

type CaptionsView = "transcript" | "styles";

/** Transcript view for the chosen source: the Generate captions card until a transcript exists. */
function CaptionsTranscript() {
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const options = useMemo(() => captionSourceOptions(project), [project]);
  // A choice holds until it leaves the project; otherwise the source follows the selection.
  const [chosenSourceId, setChosenSourceId] = useState<string | null>(null);
  const [settings, setSettings] = useState<CaptionBuildSettings>(defaultCaptionBuildSettings);
  const sourceId = options.some((option) => option.value === chosenSourceId) ? chosenSourceId : defaultCaptionSourceId(project, selectedItemIds);
  const generation = useCaptionGeneration(sourceId);
  const transcript = sourceId ? transcriptForMedia(project, sourceId) : null;
  const locked = generation.state === "transcribing" || generation.busy;

  const sourceField = (
    <SelectField
      label="Source"
      value={sourceId}
      options={options}
      placeholder="No speech source"
      disabled={options.length === 0 || (!transcript && locked)}
      onChange={setChosenSourceId}
    />
  );

  if (sourceId && transcript) {
    return (
      <TranscriptView
        mediaId={sourceId}
        transcript={transcript}
        header={options.length > 1 ? sourceField : null}
        settings={settings}
        generation={generation}
        onSettingsChange={setSettings}
      />
    );
  }
  return (
    <div className="p-3">
      <GenerateCaptionsCard
        sourceField={sourceField}
        hasSource={sourceId !== null}
        settings={settings}
        state={generation.state}
        busy={generation.busy}
        error={generation.error}
        onSettingsChange={setSettings}
        onGenerate={() => void generation.generate(settings)}
      />
    </div>
  );
}

/** The Captions tab: "Transcript | Styles". */
export function CaptionsPanel() {
  const [view, setView] = useState<CaptionsView>("transcript");
  return (
    <div className="flex min-h-full flex-col">
      <div className="px-3 pt-3">
        <ToggleGroup
          type="single"
          aria-label="Captions view"
          value={view}
          onValueChange={(next) => {
            if (next === "transcript" || next === "styles") setView(next);
          }}
          className="w-full"
        >
          <ToggleGroupItem value="transcript">Transcript</ToggleGroupItem>
          <ToggleGroupItem value="styles">Styles</ToggleGroupItem>
        </ToggleGroup>
      </div>
      {view === "transcript" ? <CaptionsTranscript /> : <CaptionStylesView />}
    </div>
  );
}
