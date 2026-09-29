import type { ProjectSpeakerIdentity } from "@/lib/project";
import { PropertySection } from "./controls/property-section";
import { useSpeakerRegistry } from "./use-speaker-registry";

interface SpeakerNameListProps {
  readonly speakers: readonly ProjectSpeakerIdentity[];
  rename(speakerId: string, name: string): Promise<void>;
}

/** Speaker rows with inline rename that commits on blur (or Enter); Escape or a blank name restores it. */
export function SpeakerNameList({ speakers, rename }: SpeakerNameListProps) {
  return (
    <ul aria-label="Speakers" className="flex flex-col gap-1.5">
      {speakers.map((speaker) => (
        <li key={speaker.id} className="flex items-center gap-2">
          {/* The dot shows the speaker's project color, not a UI color. */}
          <span aria-hidden className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ backgroundColor: speaker.color }} />
          <input
            key={speaker.name}
            type="text"
            aria-label={`Name for ${speaker.name}`}
            defaultValue={speaker.name}
            autoComplete="off"
            spellCheck={false}
            onBlur={(event) => {
              const input = event.currentTarget;
              if (!input.value.trim()) input.value = speaker.name;
              void rename(speaker.id, input.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
              if (event.key === "Escape") {
                event.currentTarget.value = speaker.name;
                event.currentTarget.blur();
              }
            }}
            className="h-7 min-w-0 flex-1 rounded-md bg-raised px-2 text-[12px] text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
        </li>
      ))}
    </ul>
  );
}

/** Project speakers with inline rename that commits on blur (or Enter) with a trimmed, non-empty name. */
export function SpeakersSection() {
  const { available, speakers, rename } = useSpeakerRegistry();
  return (
    <PropertySection title="Speakers">
      {speakers.length === 0 ? (
        <p className="text-[12px] text-dim">
          {available ? "No identified speakers yet." : "Speakers are available once the project is saved."}
        </p>
      ) : (
        <SpeakerNameList speakers={speakers} rename={rename} />
      )}
    </PropertySection>
  );
}
