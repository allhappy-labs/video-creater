import { Eye, EyeOff, Lock, LockOpen, Volume2, VolumeX } from "lucide-react";
import { IconButton } from "@/components/ui/icon-button";
import type { TimelineTrack } from "@/lib/timeline";
import { trackEnabled } from "@/lib/timeline-ops/navigation";
import { cn } from "@/lib/utils";
import { useEditorStore } from "../store/editor-store-context";
import { trackKindIcon } from "./clip-kind";

interface TrackHeaderProps {
  readonly track: TimelineTrack;
  /** Visible name from `trackDisplayNames`, for example "Video 1". */
  readonly name: string;
  readonly height: number;
}

const toggleClass = "h-[18px] w-[18px] shrink-0 rounded text-dim";

/** Track header: kind icon, short name, visibility (or mute for audio) and lock toggles. */
export function TrackHeader({ track, name, height }: TrackHeaderProps) {
  const applyActions = useEditorStore((state) => state.applyActions);
  const KindIcon = trackKindIcon[track.kind];
  const enabled = trackEnabled(track);
  const audio = track.kind === "audio";
  const enabledLabel = audio ? `${enabled ? "Mute" : "Unmute"} ${name}` : `${enabled ? "Hide" : "Show"} ${name}`;
  const EnabledIcon = audio ? (enabled ? Volume2 : VolumeX) : enabled ? Eye : EyeOff;
  const LockIcon = track.locked ? Lock : LockOpen;

  return (
    <div role="group" aria-label={`${name} track`} style={{ height }} className="flex items-center pl-1 text-dim">
      <KindIcon className="h-[13px] w-[13px] shrink-0" aria-hidden />
      <span className={cn("ml-1 min-w-0 flex-1 truncate text-[11px] tracking-tight", enabled ? "text-muted-foreground" : "text-dim")} title={name}>
        {name}
      </span>
      <IconButton
        size="sm"
        label={enabledLabel}
        className={cn(toggleClass, !enabled && "text-foreground")}
        onClick={() => void applyActions([{ type: "setTrackEnabled", trackId: track.id, enabled: !enabled }])}
      >
        <EnabledIcon className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
      <IconButton
        size="sm"
        label={`${track.locked ? "Unlock" : "Lock"} ${name}`}
        className={cn(toggleClass, track.locked && "text-foreground")}
        onClick={() => void applyActions([{ type: "setTrackLocked", trackId: track.id, locked: !track.locked }])}
      >
        <LockIcon className="h-3.5 w-3.5" aria-hidden />
      </IconButton>
    </div>
  );
}
