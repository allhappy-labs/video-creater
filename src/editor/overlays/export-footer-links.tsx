import { useId } from "react";
import { Tooltip } from "@/components/ui/tooltip";
import type { NleXmlExportFormat } from "@/lib/project";

type FooterExport = NleXmlExportFormat | "projectPackage";

const links: readonly { readonly id: FooterExport; readonly label: string }[] = [
  { id: "premiereXmeml", label: "Premiere XML" },
  { id: "davinciFcpxml", label: "DaVinci XML" },
  { id: "projectPackage", label: "Project package" },
];

interface ExportFooterLinksProps {
  /** Why none of these exports can start (e.g. an unsaved project); null when they can. */
  readonly disabledReason: string | null;
  onExport(target: FooterExport): void;
}

/** "Also export: Premiere XML · DaVinci XML · Project package". */
export function ExportFooterLinks({ disabledReason, onExport }: ExportFooterLinksProps) {
  const reasonId = useId();
  return (
    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[12px] text-muted-foreground">
      <span>Also export:</span>
      {links.map((link, index) => {
        const button = (
          <button
            type="button"
            aria-disabled={disabledReason !== null || undefined}
            aria-describedby={disabledReason ? reasonId : undefined}
            onClick={() => disabledReason === null && onExport(link.id)}
            className="rounded-sm text-foreground underline underline-offset-2 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring aria-disabled:cursor-not-allowed aria-disabled:opacity-50 aria-disabled:hover:text-foreground"
          >
            {link.label}
          </button>
        );
        return (
          <span key={link.id} className="flex items-center gap-2">
            {disabledReason === null ? button : <Tooltip content={disabledReason}>{button}</Tooltip>}
            {index < links.length - 1 && <span aria-hidden>·</span>}
          </span>
        );
      })}
      {disabledReason && (
        <span id={reasonId} className="sr-only">
          {disabledReason}
        </span>
      )}
    </div>
  );
}
