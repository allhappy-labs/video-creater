import type { VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { stringProperty } from "@/lib/timeline-ops/item-properties";

/**
 * The audio clip carrying a video clip's sound. Timeline video layers are muted and export mixes
 * audio clips only, so a video clip's volume, fades and denoise live on the audio clip that
 * shares its `linkGroupId`.
 */
export function linkedAudioItem(project: VideoProject, item: TimelineItem): TimelineItem | null {
  if (item.kind === "audio_clip") return item;
  const groupId = stringProperty(item, "linkGroupId");
  if (!groupId) return null;
  for (const track of project.timeline.tracks) {
    const match = track.items.find(
      (candidate) => candidate.kind === "audio_clip" && stringProperty(candidate, "linkGroupId") === groupId,
    );
    if (match) return match;
  }
  return null;
}
