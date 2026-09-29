import { useEffect, useState } from "react";
import {
  loadShaderBackgroundTemplates,
  shaderBackgroundTemplateCatalog,
  type ShaderBackgroundTemplateDefinition,
} from "@/lib/shader-background-templates";
import { useEditorStore } from "../../store/editor-store-context";
import { useTimelineCommands } from "../../timeline/timeline-commands";
import { BackgroundTile } from "./background-tile";

const builtInTemplates: readonly ShaderBackgroundTemplateDefinition[] = shaderBackgroundTemplateCatalog;
const insertableIds: ReadonlySet<string> = new Set(builtInTemplates.map((template) => template.id));

/**
 * The project's shader backgrounds, starting from the built-in catalog. The backend list is kept
 * to templates the timeline can place (the built-in catalog); without a backend the built-ins stay.
 */
function useShaderBackgrounds(projectDir: string): readonly ShaderBackgroundTemplateDefinition[] {
  const [templates, setTemplates] = useState(builtInTemplates);
  useEffect(() => {
    let cancelled = false;
    loadShaderBackgroundTemplates({ projectDir: projectDir.trim() || null }).then(
      (loaded: unknown) => {
        if (cancelled || !Array.isArray(loaded)) return;
        const insertable = (loaded as ShaderBackgroundTemplateDefinition[]).filter((template) => insertableIds.has(template.id));
        if (insertable.length > 0) setTemplates(insertable);
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [projectDir]);
  return templates;
}

/** Shader background tiles; `+` places one at the playhead on a graphics track, creating it when needed. */
export function BackgroundsView() {
  const projectDir = useEditorStore((state) => state.projectDir);
  const templates = useShaderBackgrounds(projectDir);
  const commands = useTimelineCommands();
  return (
    <ul aria-label="Backgrounds" className="grid grid-cols-2 gap-2">
      {templates.map((template) => (
        <BackgroundTile
          key={template.id}
          template={template}
          onInsert={(picked) => void commands.insertAssetAtPlayhead({ kind: "background", id: picked.id })}
        />
      ))}
    </ul>
  );
}
