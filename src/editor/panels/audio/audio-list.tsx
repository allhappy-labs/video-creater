import { Pause, Play, Plus } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { audioFilterOptions, audioMediaCategory, filterAudioMedia, type AudioFilter } from "@/lib/media/media-filters";
import { mediaDisplayName } from "@/lib/media/names";
import { previewUrlForMedia } from "@/lib/media/preview-source";
import type { MediaAsset, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { waveformPeaks } from "@/lib/timeline-ops/automation";
import { timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";
import { cn } from "@/lib/utils";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { writeAssetDragData } from "../../timeline/drag-data";
import { useTimelineCommands } from "../../timeline/timeline-commands";
import { FilterChips } from "../effects/filter-chips";

const miniWaveformBars = 24;

/** "1:52" for list rows. */
function formatClock(seconds: number): string {
  const total = Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds) : 0;
  return `${Math.floor(total / 60).toString()}:${(total % 60).toString().padStart(2, "0")}`;
}

function audioKindLabel(project: VideoProject, media: MediaAsset, generated: boolean): string {
  const category = audioMediaCategory(project, media);
  const label = category === "sfx" ? "SFX" : category === "music" ? "Music" : category === "voice" ? "Voice" : "Audio";
  if (!generated) return label;
  return `Generated ${category === "sfx" ? label : label.toLowerCase()}`;
}

/** Peaks from a timeline clip of the media, or the deterministic placeholder shape for unused media. */
function mediaPeaks(project: VideoProject, media: MediaAsset): readonly number[] {
  const item = project.timeline.tracks.flatMap((track) => track.items).find((candidate) => timelineItemSourceMediaId(candidate) === media.id);
  const stand: TimelineItem = item ?? {
    id: media.id,
    kind: "audio_clip",
    startSeconds: 0,
    durationSeconds: media.durationSeconds,
    source: { type: "media", mediaId: media.id },
    label: mediaDisplayName(media),
    properties: {},
  };
  const peaks = waveformPeaks(stand);
  return Array.from({ length: miniWaveformBars }, (_, index) => peaks[Math.floor((index * peaks.length) / miniWaveformBars)] ?? 0);
}

function MiniWaveform({ peaks }: { readonly peaks: readonly number[] }) {
  return (
    <svg aria-hidden viewBox={`0 0 ${peaks.length.toString()} 100`} preserveAspectRatio="none" className="h-5 w-16 shrink-0 fill-current text-dim">
      {peaks.map((peak, index) => {
        const height = Math.max(8, Math.round(peak * 100));
        return <rect key={index} x={index + 0.25} width={0.5} y={(100 - height) / 2} height={height} />;
      })}
    </svg>
  );
}

interface AudioRowProps {
  readonly media: MediaAsset;
  readonly name: string;
  readonly details: string;
  readonly peaks: readonly number[];
  readonly playing: boolean;
  onTogglePlay(): void;
  onInsert(): void;
}

function AudioRow({ media, name, details, peaks, playing, onTogglePlay, onInsert }: AudioRowProps) {
  return (
    <li
      draggable
      onDragStart={(event) => writeAssetDragData(event.dataTransfer, { kind: "media", id: media.id })}
      className="group flex cursor-grab items-center gap-2.5 rounded-control px-1.5 py-1 transition-colors hover:bg-raised active:cursor-grabbing motion-reduce:transition-none"
    >
      <button
        type="button"
        aria-label={`${playing ? "Pause" : "Play"} ${name}`}
        aria-pressed={playing}
        onClick={onTogglePlay}
        className={cn(
          "grid h-8 w-8 shrink-0 place-items-center rounded-full transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none",
          playing ? "bg-primary text-primary-foreground" : "bg-raised text-muted-foreground hover:bg-hover hover:text-foreground",
        )}
      >
        {playing ? <Pause className="h-3.5 w-3.5" aria-hidden /> : <Play className="h-3.5 w-3.5" aria-hidden />}
      </button>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13px] text-foreground" title={name}>
          {name}
        </p>
        <p className="truncate text-[11px] text-dim">{details}</p>
      </div>
      <MiniWaveform peaks={peaks} />
      <button
        type="button"
        aria-label={`Add ${name} to the timeline`}
        onClick={onInsert}
        className="grid h-7 w-7 shrink-0 place-items-center rounded-full bg-primary text-primary-foreground opacity-0 transition-opacity focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring group-hover:opacity-100 motion-reduce:transition-none [@media(hover:none)]:opacity-100"
      >
        <Plus className="h-4 w-4" aria-hidden />
      </button>
    </li>
  );
}

/** Project audio with filter chips; rows preview through one shared `<audio>`, so one plays at a time. */
export function AudioList() {
  const store = useEditorStoreApi();
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const commands = useTimelineCommands();
  const [filter, setFilter] = useState<AudioFilter>("all");
  const [playingId, setPlayingId] = useState<string | null>(null);
  const audioRef = useRef<HTMLAudioElement>(null);
  const media = useMemo(() => filterAudioMedia(project, filter), [project, filter]);
  const generatedIds = useMemo(() => new Set(filterAudioMedia(project, "generated").map((asset) => asset.id)), [project]);

  useEffect(() => {
    const audio = audioRef.current;
    return () => {
      if (audio && !audio.paused) audio.pause();
    };
  }, []);

  const togglePlay = (asset: MediaAsset) => {
    const audio = audioRef.current;
    if (!audio) return;
    if (playingId === asset.id) {
      audio.pause();
      setPlayingId(null);
      return;
    }
    const url = previewUrlForMedia(projectDir, asset.relativePath);
    if (!url) {
      store.getState().setLastError("This audio can't be previewed.");
      return;
    }
    audio.pause();
    audio.src = url;
    setPlayingId(asset.id);
    Promise.resolve(audio.play()).catch(() => {
      setPlayingId((current) => (current === asset.id ? null : current));
    });
  };

  const filterLabel = audioFilterOptions.find((option) => option.value === filter)?.label ?? "";

  return (
    <section aria-label="Audio library" className="flex flex-col gap-2">
      <FilterChips<AudioFilter> label="Audio filter" value={filter} options={audioFilterOptions} onChange={setFilter} />
      {media.length === 0 ? (
        <p className="px-1.5 py-3 text-[12px] text-dim">{filter === "all" ? "No audio in this project" : `No ${filterLabel === "SFX" ? "SFX" : filterLabel.toLowerCase()} audio`}</p>
      ) : (
        <ul aria-label="Project audio" className="flex flex-col gap-0.5">
          {media.map((asset) => (
            <AudioRow
              key={asset.id}
              media={asset}
              name={mediaDisplayName(asset)}
              details={`${formatClock(asset.durationSeconds)} · ${audioKindLabel(project, asset, generatedIds.has(asset.id))}`}
              peaks={mediaPeaks(project, asset)}
              playing={playingId === asset.id}
              onTogglePlay={() => togglePlay(asset)}
              onInsert={() => void commands.insertAssetAtPlayhead({ kind: "media", id: asset.id })}
            />
          ))}
        </ul>
      )}
      {/* One element for every row: starting a row replaces the source, so only one row plays. */}
      <audio ref={audioRef} preload="none" onEnded={() => setPlayingId(null)} className="hidden" />
    </section>
  );
}
