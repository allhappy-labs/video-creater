import { describe, expect, it } from "vitest";
import { buildEditJobRequest, editLanguageOptions, editPresetOptions } from "./edit";

describe("one-click edit request", () => {
  it("includes preset and prompt in the backend payload", () => {
    expect(
      buildEditJobRequest({
        mediaId: "media-1",
        preset: "trailer_cut",
        prompt: "Make it like an action movie",
      }),
    ).toMatchObject({
      mediaId: "media-1",
      preset: "trailer_cut",
      prompt: "Make it like an action movie",
      languageMode: "en",
      captionStyle: "bold",
    });
  });

  it("defaults v1 transcription language to English", () => {
    expect(
      buildEditJobRequest({
        mediaId: "media-1",
        preset: "trailer_cut",
        prompt: "Make it like an action movie",
      }),
    ).toMatchObject({
      languageMode: "en",
    });
  });

  it("allows Ukrainian and auto language modes", () => {
    expect(editLanguageOptions.map((option) => option.value)).toEqual(["en", "uk", "auto"]);
    expect(
      buildEditJobRequest({
        mediaId: "media-1",
        preset: "trailer_cut",
        prompt: "Зроби короткий ролик",
        languageMode: "uk",
      }),
    ).toMatchObject({
      languageMode: "uk",
    });
  });

  it("defines the three MVP presets", () => {
    expect(editPresetOptions.map((option) => option.value)).toEqual([
      "trailer_cut",
      "highlight_reel",
      "story_cut",
    ]);
  });
});
