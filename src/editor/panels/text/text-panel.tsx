import { Type } from "lucide-react";
import { useId, useState } from "react";
import { Button } from "@/components/ui/button";
import { motionTemplateCatalog, type MotionTemplateDefinition } from "@/lib/motion-templates";
import { useTimelineCommands } from "../../timeline/timeline-commands";
import { textStyleTemplates, titleTemplateGroups } from "./text-catalog";
import { TemplateTile } from "./template-tile";
import { useMediaQuery } from "./use-media-query";

const textStyles = textStyleTemplates(motionTemplateCatalog);
const titleGroups = titleTemplateGroups(motionTemplateCatalog);

interface TemplateGridProps {
  readonly labelledBy: string;
  readonly templates: readonly MotionTemplateDefinition[];
  readonly canHover: boolean;
  readonly previewTemplateId: string | null;
  onTogglePreview(templateId: string): void;
  onInsert(templateId: string): void;
}

function TemplateGrid({ labelledBy, templates, previewTemplateId, ...tileProps }: TemplateGridProps) {
  return (
    <ul aria-labelledby={labelledBy} className="grid grid-cols-2 gap-2">
      {templates.map((template) => (
        <TemplateTile key={template.id} template={template} previewing={previewTemplateId === template.id} {...tileProps} />
      ))}
    </ul>
  );
}

const sectionHeadingClass = "text-[11px] font-medium uppercase tracking-wide text-dim";

/** Text tab: add a text overlay, text style templates, and titles & lower thirds by category. */
export function TextPanel() {
  const commands = useTimelineCommands();
  const canHover = !useMediaQuery("(hover: none)");
  const [previewTemplateId, setPreviewTemplateId] = useState<string | null>(null);
  const headingId = useId();
  const gridProps = {
    canHover,
    previewTemplateId,
    onTogglePreview: (templateId: string) => setPreviewTemplateId((current) => (current === templateId ? null : templateId)),
    onInsert: (templateId: string) => void commands.insertAssetAtPlayhead({ kind: "template", id: templateId }),
  };

  return (
    <div className="flex flex-col gap-4 p-3">
      <Button type="button" className="w-full" onClick={() => void commands.insertAssetAtPlayhead({ kind: "text", id: "text" })}>
        <Type className="h-4 w-4" aria-hidden />
        Add text
      </Button>

      <section aria-labelledby={`${headingId}-styles`} className="flex flex-col gap-2">
        <h2 id={`${headingId}-styles`} className={sectionHeadingClass}>
          Text styles
        </h2>
        <TemplateGrid labelledBy={`${headingId}-styles`} templates={textStyles} {...gridProps} />
      </section>

      <section aria-labelledby={`${headingId}-titles`} className="flex flex-col gap-3">
        <h2 id={`${headingId}-titles`} className={sectionHeadingClass}>
          Titles &amp; lower thirds
        </h2>
        {titleGroups.map((group) => (
          <div key={group.category} className="flex flex-col gap-1.5">
            <h3 id={`${headingId}-${group.category}`} className="text-[12px] text-muted-foreground">
              {group.label}
            </h3>
            <TemplateGrid labelledBy={`${headingId}-${group.category}`} templates={group.templates} {...gridProps} />
          </div>
        ))}
      </section>
    </div>
  );
}
