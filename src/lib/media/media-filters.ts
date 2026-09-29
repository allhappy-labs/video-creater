import { mediaDisplayName } from "@/lib/media/names";
import type { MediaAsset, VideoProject } from "@/lib/project";

export type MediaFilter = "all" | "video" | "images" | "generated";
export type AudioFilter = "all" | "voice" | "music" | "sfx" | "generated";
export type MediaSort = "dateAdded" | "name" | "duration";
export type MediaContentKind = "video" | "image" | "audio" | "lottie";
export type AudioCategory = "voice" | "music" | "sfx";

interface Option<Value extends string> {
  readonly value: Value;
  readonly label: string;
}

export const mediaFilterOptions: readonly Option<MediaFilter>[] = [
  { value: "all", label: "All" },
  { value: "video", label: "Video" },
  { value: "images", label: "Images" },
  { value: "generated", label: "Generated" },
];

export const audioFilterOptions: readonly Option<AudioFilter>[] = [
  { value: "all", label: "All" },
  { value: "voice", label: "Voice" },
  { value: "music", label: "Music" },
  { value: "sfx", label: "SFX" },
  { value: "generated", label: "Generated" },
];

export const mediaSortOptions: readonly Option<MediaSort>[] = [
  { value: "dateAdded", label: "Date added" },
  { value: "name", label: "Name" },
  { value: "duration", label: "Duration" },
];

const imageExtensions = new Set(["png", "jpg", "jpeg", "webp", "gif", "tiff", "tif", "heic", "heif"]);
const audioExtensions = new Set(["wav", "mp3", "m4a", "aac", "aiff", "aifc", "flac", "ogg", "opus"]);

function fileExtension(path: string) {
  const match = /\.([^./\\]+)$/.exec(path);
  return match?.[1]?.toLowerCase() ?? "";
}

/**
 * What the media actually contains. Generated outputs are stored with kind `generated`, so their
 * content is resolved from the output file extension; anything else generated is treated as video,
 * matching preview and reference handling.
 */
export function mediaContentKind(media: MediaAsset): MediaContentKind {
  if (media.kind !== "generated") return media.kind;
  const extension = fileExtension(media.relativePath);
  if (imageExtensions.has(extension)) return "image";
  if (audioExtensions.has(extension)) return "audio";
  return "video";
}

interface GenerationFacts {
  readonly generatedMediaIds: ReadonlySet<string>;
  readonly categoryByMediaId: ReadonlyMap<string, string>;
}

function generationFacts(project: VideoProject): GenerationFacts {
  const generatedMediaIds = new Set<string>();
  const categoryByMediaId = new Map<string, string>();
  for (const asset of project.generatedAssets) {
    const category = asset.settings.category?.trim().toLowerCase();
    for (const output of asset.outputs) {
      generatedMediaIds.add(output.mediaId);
      if (category) categoryByMediaId.set(output.mediaId, category);
    }
  }
  for (const media of project.media) {
    if (media.kind === "generated") generatedMediaIds.add(media.id);
  }
  return { generatedMediaIds, categoryByMediaId };
}

function transcribedMediaIds(project: VideoProject): ReadonlySet<string> {
  return new Set(
    project.transcripts
      .filter((transcript) => transcript.words.length > 0)
      .map((transcript) => transcript.mediaId),
  );
}

function categoryFor(
  media: MediaAsset,
  facts: GenerationFacts,
  transcribed: ReadonlySet<string>,
): AudioCategory | null {
  const category = facts.categoryByMediaId.get(media.id);
  if (category === "music") return "music";
  if (category === "sfx") return "sfx";
  if (category === "tts" || category === "voice" || category === "speech") return "voice";
  return transcribed.has(media.id) ? "voice" : null;
}

/**
 * Voice, music or SFX for an audio item. Generated audio uses its generation category (`tts` is
 * voice). Other audio counts as voice when it has a transcript with words; otherwise it is
 * uncategorized and appears only under All (and Generated, when generated).
 */
export function audioMediaCategory(project: VideoProject, media: MediaAsset): AudioCategory | null {
  return categoryFor(media, generationFacts(project), transcribedMediaIds(project));
}

/** Media grid chips. All keeps every item, including audio, so folders show their full contents. */
export function filterVisualMedia(project: VideoProject, filter: MediaFilter): MediaAsset[] {
  if (filter === "all") return [...project.media];
  if (filter === "generated") {
    const { generatedMediaIds } = generationFacts(project);
    return project.media.filter((media) => generatedMediaIds.has(media.id));
  }
  return project.media.filter((media) => {
    const kind = mediaContentKind(media);
    return filter === "images" ? kind === "image" : kind === "video" || kind === "lottie";
  });
}

/** Audio list chips over the project's audio content, in project order. */
export function filterAudioMedia(project: VideoProject, filter: AudioFilter): MediaAsset[] {
  const audio = project.media.filter((media) => mediaContentKind(media) === "audio");
  if (filter === "all") return audio;
  const facts = generationFacts(project);
  if (filter === "generated") return audio.filter((media) => facts.generatedMediaIds.has(media.id));
  const transcribed = transcribedMediaIds(project);
  return audio.filter((media) => categoryFor(media, facts, transcribed) === filter);
}

const nameCollator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

/**
 * Sorted copy. Date added is project media order (media carry no import timestamp); name compares
 * display names case-insensitively with numeric runs; duration is longest first. Ties keep project
 * order.
 */
export function sortMedia(media: readonly MediaAsset[], sort: MediaSort): MediaAsset[] {
  const indexed = media.map((item, index) => ({ item, index }));
  if (sort === "name") {
    indexed.sort(
      (left, right) =>
        nameCollator.compare(mediaDisplayName(left.item), mediaDisplayName(right.item)) ||
        left.index - right.index,
    );
  } else if (sort === "duration") {
    indexed.sort(
      (left, right) =>
        (right.item.durationSeconds || 0) - (left.item.durationSeconds || 0) || left.index - right.index,
    );
  }
  return indexed.map(({ item }) => item);
}
