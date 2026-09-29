import { describe, expect, it } from "vitest";

import type { GeneratedAsset, MediaAsset, VideoProject } from "@/lib/project";
import { fixtureGeneratedAsset, fixtureProject } from "@/test-utils/editor-fixtures";

import {
  audioFilterOptions,
  audioMediaCategory,
  filterAudioMedia,
  filterVisualMedia,
  mediaContentKind,
  mediaFilterOptions,
  mediaSortOptions,
  sortMedia,
} from "./media-filters";

function media(id: string, kind: MediaAsset["kind"], relativePath: string, durationSeconds: number, name?: string): MediaAsset {
  return { id, ...(name === undefined ? {} : { name }), relativePath, kind, durationSeconds, width: null, height: null, fps: null };
}

function generatedAudio(project: VideoProject, id: string, mediaId: string, category: string | null): GeneratedAsset {
  const template = fixtureGeneratedAsset(project);
  return {
    ...template,
    id,
    settings: { ...template.settings, category },
    outputs: [{ mediaId, relativePath: `generated/${mediaId}.mp3`, width: 0, height: 0, durationSeconds: 3, fps: 0 }],
  };
}

/** The sample project (video, voiceover audio, generated video) plus images, Lottie, and generated audio. */
function libraryProject(): VideoProject {
  const project = fixtureProject();
  project.media.push(
    media("still", "image", "media/still.png", 0, "Still frame"),
    media("logo", "lottie", "media/logo.lottie", 2),
    media("music-output", "audio", "generated/music-output.mp3", 30),
    media("sfx-output", "generated", "generated/sfx-output.wav", 1.5),
    media("tts-output", "audio", "generated/tts-output.mp3", 6),
    media("generated-still", "generated", "generated/poster.webp", 0),
    media("room-tone", "audio", "media/room-tone.wav", 12),
  );
  project.generatedAssets.push(
    generatedAudio(project, "music-asset", "music-output", "music"),
    generatedAudio(project, "sfx-asset", "sfx-output", "sfx"),
    generatedAudio(project, "tts-asset", "tts-output", "tts"),
    { ...generatedAudio(project, "poster-asset", "generated-still", null), outputs: [] },
  );
  return project;
}

const ids = (items: readonly MediaAsset[]) => items.map((item) => item.id);

describe("media filters", () => {
  it("labels the Media and Audio chips and the sort menu", () => {
    expect(mediaFilterOptions.map((option) => option.label)).toEqual(["All", "Video", "Images", "Generated"]);
    expect(audioFilterOptions.map((option) => option.label)).toEqual(["All", "Voice", "Music", "SFX", "Generated"]);
    expect(mediaSortOptions.map((option) => option.label)).toEqual(["Date added", "Name", "Duration"]);
  });

  it("resolves generated media to its content kind from the file extension", () => {
    const project = libraryProject();
    const byId = (id: string) => project.media.find((item) => item.id === id) as MediaAsset;

    expect(mediaContentKind(byId("sample-generated-output"))).toBe("video");
    expect(mediaContentKind(byId("sfx-output"))).toBe("audio");
    expect(mediaContentKind(byId("generated-still"))).toBe("image");
    expect(mediaContentKind(byId("logo"))).toBe("lottie");
  });

  it("filters the Media grid by every chip", () => {
    const project = libraryProject();

    expect(ids(filterVisualMedia(project, "all"))).toEqual(ids(project.media));
    expect(ids(filterVisualMedia(project, "video"))).toEqual(["media-1", "sample-generated-output", "logo"]);
    expect(ids(filterVisualMedia(project, "images"))).toEqual(["still", "generated-still"]);
    expect(ids(filterVisualMedia(project, "generated"))).toEqual([
      "sample-generated-output",
      "music-output",
      "sfx-output",
      "tts-output",
      "generated-still",
    ]);
  });

  it("classifies audio by generation category and transcripts", () => {
    const project = libraryProject();
    project.transcripts.push({
      id: "transcript-voiceover",
      mediaId: "media-voiceover",
      repairs: [],
      segments: [],
      words: [{ text: "Hello", startSeconds: 0, endSeconds: 0.4 }],
    });
    const byId = (id: string) => project.media.find((item) => item.id === id) as MediaAsset;

    expect(audioMediaCategory(project, byId("music-output"))).toBe("music");
    expect(audioMediaCategory(project, byId("sfx-output"))).toBe("sfx");
    expect(audioMediaCategory(project, byId("tts-output"))).toBe("voice");
    expect(audioMediaCategory(project, byId("media-voiceover"))).toBe("voice");
    expect(audioMediaCategory(project, byId("room-tone"))).toBeNull();
  });

  it("filters the Audio list by every chip", () => {
    const project = libraryProject();
    project.transcripts.push({
      id: "transcript-voiceover",
      mediaId: "media-voiceover",
      repairs: [],
      segments: [],
      words: [{ text: "Hello", startSeconds: 0, endSeconds: 0.4 }],
    });

    expect(ids(filterAudioMedia(project, "all"))).toEqual([
      "media-voiceover",
      "music-output",
      "sfx-output",
      "tts-output",
      "room-tone",
    ]);
    expect(ids(filterAudioMedia(project, "voice"))).toEqual(["media-voiceover", "tts-output"]);
    expect(ids(filterAudioMedia(project, "music"))).toEqual(["music-output"]);
    expect(ids(filterAudioMedia(project, "sfx"))).toEqual(["sfx-output"]);
    expect(ids(filterAudioMedia(project, "generated"))).toEqual(["music-output", "sfx-output", "tts-output"]);
  });

  it("does not treat an empty transcript as voice", () => {
    const project = libraryProject();
    project.transcripts.push({ id: "empty", mediaId: "room-tone", repairs: [], segments: [], words: [] });

    expect(ids(filterAudioMedia(project, "voice"))).toEqual(["tts-output"]);
  });
});

describe("media sort", () => {
  const library = [
    media("b", "video", "media/b-roll 10.mp4", 4, "B-roll 10"),
    media("a", "video", "media/alpha.mp4", 12),
    media("c", "image", "media/b-roll 2.png", 0, "b-roll 2"),
    media("d", "audio", "media/delta.wav", 12),
  ];

  it("keeps project order for date added", () => {
    expect(ids(sortMedia(library, "dateAdded"))).toEqual(["b", "a", "c", "d"]);
  });

  it("sorts by display name, case-insensitively and with numeric runs", () => {
    expect(ids(sortMedia(library, "name"))).toEqual(["a", "c", "b", "d"]);
  });

  it("sorts by duration, longest first, keeping project order for ties", () => {
    expect(ids(sortMedia(library, "duration"))).toEqual(["a", "d", "b", "c"]);
  });

  it("does not mutate the input", () => {
    const copy = [...library];
    sortMedia(library, "name");
    expect(library).toEqual(copy);
  });
});
