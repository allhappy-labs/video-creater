import { describe, expect, it } from "vitest";
import {
  generationReferenceLimitMessage,
  generationReferenceMediaForModel,
  generationReferencePromptTags,
  isFrameReferenceMediaAsset,
  isSourceVideoMediaAsset,
  isVisualMediaAsset,
  promptWithInsertedReferenceTag,
  trailingReferenceTagQuery,
  typedGenerationReferenceMediaRefs,
} from "@/lib/generation/references";
import { generationModelOptions } from "@/lib/generation/provider-rules";
import type { GenerationModelOption, MediaGenerationMode } from "@/lib/generation/types";
import type { MediaAsset, MediaKind } from "@/lib/project";

function mediaAsset(id: string, kind: MediaKind, durationSeconds: number): MediaAsset {
  return {
    id,
    relativePath: `media/${id}`,
    kind,
    durationSeconds,
    width: kind === "audio" ? null : 1920,
    height: kind === "audio" ? null : 1080,
    fps: kind === "video" || kind === "generated" ? 30 : null,
  };
}

const images = Array.from({ length: 12 }, (_, index) =>
  mediaAsset(`image-${index + 1}`, "image", 0),
);
const video30 = mediaAsset("video-30", "video", 30);
const audio400 = mediaAsset("audio-400", "audio", 400);
const generated8 = mediaAsset("generated-8", "generated", 8);

const mediaLists: Record<string, readonly MediaAsset[]> = {
  none: [],
  oneImage: images.slice(0, 1),
  twelveImages: images,
  oneVideo: [video30],
  oneAudio: [audio400],
  mixed: [images[0]!, video30, audio400, images[1]!, generated8],
};

function referenceRow(mode: MediaGenerationMode, option: GenerationModelOption) {
  return {
    model: `${option.provider}:${option.id}`,
    lists: Object.fromEntries(
      Object.entries(mediaLists).map(([name, media]) => {
        const forModel = generationReferenceMediaForModel(mode, option, media);
        const typed = typedGenerationReferenceMediaRefs(
          mode,
          option,
          media.map((asset) => asset.id),
          media,
        );
        return [
          name,
          {
            forModel: forModel.map((asset) => asset.id).join(" "),
            typed: `images=[${typed.referenceImageMediaRefs.join(" ")}] videos=[${typed.referenceVideoMediaRefs.join(" ")}] audios=[${typed.referenceAudioMediaRefs.join(" ")}]`,
            message: generationReferenceLimitMessage(mode, option, typed, media),
          },
        ];
      }),
    ),
  };
}

function matrix(
  mode: MediaGenerationMode,
  providers?: readonly string[],
  excludeProviders?: readonly string[],
) {
  return generationModelOptions[mode]
    .filter((option) => !providers || providers.includes(option.provider))
    .filter((option) => !excludeProviders || !excludeProviders.includes(option.provider))
    .map((option) => referenceRow(mode, option));
}

describe("generation reference matrix", () => {
  it("records image fal.ai options", () => {
    expect(matrix("image", ["fal.ai"])).toMatchInlineSnapshot(`
      [
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 image-2",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/nano-banana-pro/edit",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 image-2",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/flux/schnell",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/krea-2/turbo",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/recraft/v3/text-to-image",
        },
      ]
    `);
  });

  it("records non-fal image options", () => {
    expect(matrix("image", undefined, ["fal.ai"])).toMatchInlineSnapshot(`
      [
        {
          "lists": {
            "mixed": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "openai:gpt-image-2",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "replicate:black-forest-labs/flux-schnell",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "replicate:black-forest-labs/flux-dev",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "replicate:black-forest-labs/flux-1.1-pro",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "replicate:black-forest-labs/flux-1.1-pro-ultra",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 image-2",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "openai:gpt-image-1.5",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 image-2",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "xai:grok-imagine-image-quality",
        },
      ]
    `);
  });

  it("records fal.ai video options", () => {
    expect(matrix("video", ["fal.ai"])).toMatchInlineSnapshot(`
      [
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 audio-400 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[audio-400]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "audio-400",
              "message": null,
              "typed": "images=[] videos=[] audios=[audio-400]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/wan-25-preview/text-to-video",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 audio-400 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[audio-400]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "audio-400",
              "message": null,
              "typed": "images=[] videos=[] audios=[audio-400]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/wan/v2.7/image-to-video",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": "fal-ai/wan/v2.7/reference-to-video requires an image reference",
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": "fal-ai/wan/v2.7/reference-to-video requires an image reference",
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[image-1] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": "fal-ai/wan/v2.7/reference-to-video requires an image reference",
              "typed": "images=[] videos=[video-30] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": "fal-ai/wan/v2.7/reference-to-video accepts at most 4 image references",
              "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/wan/v2.7/reference-to-video",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": "fal-ai/wan/v2.2-a14b/video-to-video accepts at most 1 image reference",
              "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[image-1] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": "fal-ai/wan/v2.2-a14b/video-to-video does not accept video references",
              "typed": "images=[] videos=[video-30] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": "fal-ai/wan/v2.2-a14b/video-to-video accepts at most 1 image reference",
              "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/wan/v2.2-a14b/video-to-video",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/kling-video/v3/standard/text-to-video",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/kling-video/v3/pro/image-to-video",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": "fal-ai/kling-video/v3/pro/motion-control accepts at most 1 image reference",
              "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": "fal-ai/kling-video/v3/pro/motion-control requires an image reference",
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": "fal-ai/kling-video/v3/pro/motion-control requires an image reference",
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[image-1] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": "fal-ai/kling-video/v3/pro/motion-control requires an image reference",
              "typed": "images=[] videos=[video-30] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": "fal-ai/kling-video/v3/pro/motion-control accepts at most 1 image reference",
              "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:fal-ai/kling-video/v3/pro/motion-control",
        },
      ]
    `);
  });

  it("records non-fal video options", () => {
    expect(matrix("video", undefined, ["fal.ai"])).toMatchInlineSnapshot(`
      [
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 audio-400 image-2 generated-8",
              "message": "Combined video reference duration exceeds 15s",
              "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[audio-400]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "audio-400",
              "message": "Combined audio reference duration exceeds 15s",
              "typed": "images=[] videos=[] audios=[audio-400]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[image-1] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": "Combined video reference duration exceeds 15s",
              "typed": "images=[] videos=[video-30] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": "replicate/bytedance/seedance-2.0-fast accepts at most 4 image references",
              "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
            },
          },
          "model": "replicate:bytedance/seedance-2.0-fast",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 audio-400 image-2 generated-8",
              "message": "Combined video reference duration exceeds 15s",
              "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[audio-400]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "audio-400",
              "message": "Combined audio reference duration exceeds 15s",
              "typed": "images=[] videos=[] audios=[audio-400]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[image-1] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": "Combined video reference duration exceeds 15s",
              "typed": "images=[] videos=[video-30] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": "replicate/bytedance/seedance-2.0 accepts at most 4 image references",
              "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
            },
          },
          "model": "replicate:bytedance/seedance-2.0",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": "xai/grok-imagine-video does not accept video references",
              "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[image-1] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": "xai/grok-imagine-video does not accept video references",
              "typed": "images=[] videos=[video-30] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": "xai/grok-imagine-video accepts at most 7 image references",
              "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
            },
          },
          "model": "xai:grok-imagine-video",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": "google/veo3.1-fast does not accept video references",
              "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[image-1] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": "google/veo3.1-fast does not accept video references",
              "typed": "images=[] videos=[video-30] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": "google/veo3.1-fast accepts at most 3 image references",
              "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
            },
          },
          "model": "google:veo3.1-fast",
        },
      ]
    `);
  });

  it("records audio options", () => {
    expect(matrix("audio")).toMatchInlineSnapshot(`
      [
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:bytedance/seed-audio-1.0",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:sonilo/v1.1/text-to-music",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:sonilo/v1.1/video-to-music",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "fal.ai:mirelo-ai/sfx-v1.5/video-to-audio",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "openai:gpt-4o-mini-tts",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "elevenlabs:elevenlabs-tts-v3",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "elevenlabs:elevenlabs-music",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "minimax:minimax-music-v2.6",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "google:gemini-3.1-flash-tts-preview",
        },
        {
          "lists": {
            "mixed": {
              "forModel": "image-1 video-30 image-2 generated-8",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "none": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneAudio": {
              "forModel": "",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneImage": {
              "forModel": "image-1",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "oneVideo": {
              "forModel": "video-30",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
            "twelveImages": {
              "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
              "message": null,
              "typed": "images=[] videos=[] audios=[]",
            },
          },
          "model": "google:lyria3-pro",
        },
      ]
    `);
  });

  it("records catalog limit messages", () => {
    const catalogOptions: GenerationModelOption[] = [
      {
        provider: "catalog",
        id: "requires-image",
        supportsReferences: true,
        requiresReferenceImage: true,
        maxReferenceImages: 1,
      },
      {
        provider: "catalog",
        id: "no-images",
        supportsReferences: true,
        maxReferenceImages: 0,
        maxReferenceVideos: 2,
      },
      {
        provider: "catalog",
        id: "one-of-each",
        supportsReferences: true,
        maxReferenceImages: 1,
        maxReferenceVideos: 1,
        maxReferenceAudios: 1,
        maxTotalReferences: 2,
      },
      {
        provider: "catalog",
        id: "duration-limits",
        supportsReferences: true,
        maxReferenceImages: 5,
        maxReferenceVideos: 5,
        maxReferenceAudios: 5,
        maxCombinedVideoRefSeconds: 30,
        maxCombinedAudioRefSeconds: 399,
      },
    ];
    const duplicateVideoMedia = [video30, generated8];
    expect({
      rows: catalogOptions.map((option) => referenceRow("video", option)),
      unknownMediaIds: typedGenerationReferenceMediaRefs(
        "video",
        catalogOptions[3]!,
        ["missing", "video-30", "audio-400"],
        [video30, audio400],
      ),
      combinedVideoDuration: generationReferenceLimitMessage(
        "video",
        catalogOptions[3]!,
        {
          referenceImageMediaRefs: [],
          referenceVideoMediaRefs: ["video-30", "generated-8", "missing"],
          referenceAudioMediaRefs: [],
        },
        duplicateVideoMedia,
      ),
      totalReferences: generationReferenceLimitMessage(
        "video",
        catalogOptions[2]!,
        typedGenerationReferenceMediaRefs(
          "video",
          catalogOptions[2]!,
          ["image-1", "video-30", "audio-400"],
          mediaLists.mixed ?? [],
        ),
        mediaLists.mixed ?? [],
      ),
      imageModeMessage: generationReferenceLimitMessage(
        "image",
        catalogOptions[0]!,
        { referenceImageMediaRefs: [], referenceVideoMediaRefs: [], referenceAudioMediaRefs: [] },
        [],
      ),
    }).toMatchInlineSnapshot(`
      {
        "combinedVideoDuration": "Combined video reference duration exceeds 30s",
        "imageModeMessage": null,
        "rows": [
          {
            "lists": {
              "mixed": {
                "forModel": "image-1 video-30 image-2 generated-8",
                "message": "catalog/requires-image accepts at most 1 image reference",
                "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[]",
              },
              "none": {
                "forModel": "",
                "message": "catalog/requires-image requires an image reference",
                "typed": "images=[] videos=[] audios=[]",
              },
              "oneAudio": {
                "forModel": "",
                "message": "catalog/requires-image requires an image reference",
                "typed": "images=[] videos=[] audios=[]",
              },
              "oneImage": {
                "forModel": "image-1",
                "message": null,
                "typed": "images=[image-1] videos=[] audios=[]",
              },
              "oneVideo": {
                "forModel": "video-30",
                "message": "catalog/requires-image requires an image reference",
                "typed": "images=[] videos=[video-30] audios=[]",
              },
              "twelveImages": {
                "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
                "message": "catalog/requires-image accepts at most 1 image reference",
                "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
              },
            },
            "model": "catalog:requires-image",
          },
          {
            "lists": {
              "mixed": {
                "forModel": "image-1 video-30 image-2 generated-8",
                "message": "catalog/no-images does not accept image references",
                "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[]",
              },
              "none": {
                "forModel": "",
                "message": null,
                "typed": "images=[] videos=[] audios=[]",
              },
              "oneAudio": {
                "forModel": "",
                "message": null,
                "typed": "images=[] videos=[] audios=[]",
              },
              "oneImage": {
                "forModel": "image-1",
                "message": "catalog/no-images does not accept image references",
                "typed": "images=[image-1] videos=[] audios=[]",
              },
              "oneVideo": {
                "forModel": "video-30",
                "message": null,
                "typed": "images=[] videos=[video-30] audios=[]",
              },
              "twelveImages": {
                "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
                "message": "catalog/no-images does not accept image references",
                "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
              },
            },
            "model": "catalog:no-images",
          },
          {
            "lists": {
              "mixed": {
                "forModel": "image-1 video-30 audio-400 image-2 generated-8",
                "message": "catalog/one-of-each accepts at most 1 image reference",
                "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[audio-400]",
              },
              "none": {
                "forModel": "",
                "message": null,
                "typed": "images=[] videos=[] audios=[]",
              },
              "oneAudio": {
                "forModel": "audio-400",
                "message": null,
                "typed": "images=[] videos=[] audios=[audio-400]",
              },
              "oneImage": {
                "forModel": "image-1",
                "message": null,
                "typed": "images=[image-1] videos=[] audios=[]",
              },
              "oneVideo": {
                "forModel": "video-30",
                "message": null,
                "typed": "images=[] videos=[video-30] audios=[]",
              },
              "twelveImages": {
                "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
                "message": "catalog/one-of-each accepts at most 1 image reference",
                "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
              },
            },
            "model": "catalog:one-of-each",
          },
          {
            "lists": {
              "mixed": {
                "forModel": "image-1 video-30 audio-400 image-2 generated-8",
                "message": "Combined video reference duration exceeds 30s",
                "typed": "images=[image-1 image-2] videos=[video-30 generated-8] audios=[audio-400]",
              },
              "none": {
                "forModel": "",
                "message": null,
                "typed": "images=[] videos=[] audios=[]",
              },
              "oneAudio": {
                "forModel": "audio-400",
                "message": "Combined audio reference duration exceeds 399s",
                "typed": "images=[] videos=[] audios=[audio-400]",
              },
              "oneImage": {
                "forModel": "image-1",
                "message": null,
                "typed": "images=[image-1] videos=[] audios=[]",
              },
              "oneVideo": {
                "forModel": "video-30",
                "message": null,
                "typed": "images=[] videos=[video-30] audios=[]",
              },
              "twelveImages": {
                "forModel": "image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12",
                "message": "catalog/duration-limits accepts at most 5 image references",
                "typed": "images=[image-1 image-2 image-3 image-4 image-5 image-6 image-7 image-8 image-9 image-10 image-11 image-12] videos=[] audios=[]",
              },
            },
            "model": "catalog:duration-limits",
          },
        ],
        "totalReferences": "catalog/one-of-each accepts at most 2 references total",
        "unknownMediaIds": {
          "referenceAudioMediaRefs": [
            "audio-400",
          ],
          "referenceImageMediaRefs": [],
          "referenceVideoMediaRefs": [
            "video-30",
          ],
        },
      }
    `);
  });
});

describe("reference media predicates and prompt tags", () => {
  it("records media kind predicates", () => {
    expect(
      (["image", "video", "audio", "generated"] as const).map((kind) => {
        const asset = mediaAsset(`${kind}-asset`, kind, 1);
        return {
          kind,
          visual: isVisualMediaAsset(asset),
          frameReference: isFrameReferenceMediaAsset(asset),
          sourceVideo: isSourceVideoMediaAsset(asset),
        };
      }),
    ).toMatchInlineSnapshot(`
      [
        {
          "frameReference": true,
          "kind": "image",
          "sourceVideo": false,
          "visual": true,
        },
        {
          "frameReference": false,
          "kind": "video",
          "sourceVideo": true,
          "visual": true,
        },
        {
          "frameReference": false,
          "kind": "audio",
          "sourceVideo": false,
          "visual": false,
        },
        {
          "frameReference": false,
          "kind": "generated",
          "sourceVideo": false,
          "visual": true,
        },
      ]
    `);
  });

  it("records prompt tags for mixed media", () => {
    expect(
      generationReferencePromptTags(
        ["image-1", "video-30", "missing", "audio-400", "image-2", "generated-8", "image-1"],
        mediaLists.mixed ?? [],
      ),
    ).toMatchInlineSnapshot(`
      [
        {
          "kindLabel": "Image",
          "mediaId": "image-1",
          "tag": "@Image1",
        },
        {
          "kindLabel": "Video",
          "mediaId": "video-30",
          "tag": "@Video1",
        },
        {
          "kindLabel": "Audio",
          "mediaId": "audio-400",
          "tag": "@Audio1",
        },
        {
          "kindLabel": "Image",
          "mediaId": "image-2",
          "tag": "@Image2",
        },
        {
          "kindLabel": "Video",
          "mediaId": "generated-8",
          "tag": "@Video2",
        },
        {
          "kindLabel": "Image",
          "mediaId": "image-1",
          "tag": "@Image3",
        },
      ]
    `);
  });

  it("records trailing tag queries and inserted tags", () => {
    expect(
      ["make @in", "@", "email a@b.c", "", "ends with space ", "line\n@Vid"].map((prompt) => ({
        prompt,
        query: trailingReferenceTagQuery(prompt),
        inserted: promptWithInsertedReferenceTag(prompt, "@Image1"),
      })),
    ).toMatchInlineSnapshot(`
      [
        {
          "inserted": "make @Image1 ",
          "prompt": "make @in",
          "query": "in",
        },
        {
          "inserted": "@Image1 ",
          "prompt": "@",
          "query": "",
        },
        {
          "inserted": "email a@b.c @Image1 ",
          "prompt": "email a@b.c",
          "query": null,
        },
        {
          "inserted": "@Image1 ",
          "prompt": "",
          "query": null,
        },
        {
          "inserted": "ends with space @Image1 ",
          "prompt": "ends with space ",
          "query": null,
        },
        {
          "inserted": "line
      @Image1 ",
          "prompt": "line
      @Vid",
          "query": "Vid",
        },
      ]
    `);
  });
});
