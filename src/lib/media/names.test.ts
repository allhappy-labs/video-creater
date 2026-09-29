import { describe, expect, it } from "vitest";
import {
  aspectRatioLabel,
  filenameFromPath,
  formatAspectRatioOrNull,
  formatAspectRatioOrUnknown,
  formatDimensions,
  formatFrameRate,
  formatOptionalSeconds,
  greatestCommonDivisor,
  mediaAssetMeta,
  mediaDisplayName,
  mediaQualityLabel,
  truncatedGreatestCommonDivisor,
} from "@/lib/media/names";
import { fixtureMedia, fixtureProject } from "@/test-utils/editor-fixtures";

const paths = ["media/input.mp4", "/abs/path/clip.MOV", "noext", "dir/", "", "a\\b\\c.wav"];
const dims: Array<[number, number]> = [
  [1920, 1080],
  [1080, 1920],
  [640, 360],
  [1000, 1000],
  [3840, 2160],
  [2560, 1440],
  [1280, 720],
  [720, 480],
  [1366, 768],
  [1919.6, 1080.4],
  [-1920, 1080],
  [1000, 0],
  [0, 0],
  [Number.NaN, 1080],
];

describe("media names characterization", () => {
  it("derives filenames and display names identically", () => {
    const project = fixtureProject();
    const media = fixtureMedia(project, "video");
    const expected = paths.map((path) => filenameFromPath(path));
    expect(expected).toMatchInlineSnapshot(`
      [
        "input.mp4",
        "clip.MOV",
        "noext",
        "",
        "",
        "c.wav",
      ]
    `);

    const { name: _name, ...unnamedMedia } = media;
    const assets = [
      media,
      { ...media, name: "  Named clip  " },
      { ...media, name: "   " },
      { ...media, name: null },
      { ...unnamedMedia, relativePath: "media/folder/from-path.mov" },
    ];
    const expectedNames = assets.map((asset) => mediaDisplayName(asset));
    expect(expectedNames).toMatchInlineSnapshot(`
      [
        "input.mp4",
        "Named clip",
        "input.mp4",
        "input.mp4",
        "from-path.mov",
      ]
    `);

    expect([
      mediaAssetMeta(media),
      mediaAssetMeta({ ...media, fps: 29.97 }),
      mediaAssetMeta({ ...media, kind: "image", fps: null }),
      mediaAssetMeta({ ...media, kind: "audio", width: null, height: null, fps: null }),
    ]).toMatchInlineSnapshot(`
      [
        "video - 640x360 - 24 fps",
        "video - 640x360 - 30 fps",
        "image - 640x360",
        "audio",
      ]
    `);
  });

  it("computes greatest common divisors", () => {
    const expected = dims.map(([w, h]) => greatestCommonDivisor(w, h));
    expect(expected).toMatchInlineSnapshot(`
      [
        120,
        120,
        40,
        1000,
        240,
        160,
        80,
        240,
        2,
        120,
        120,
        1000,
        1,
        1080,
      ]
    `);
    expect(dims.map(([w, h]) => truncatedGreatestCommonDivisor(w, h))).toMatchInlineSnapshot(`
      [
        120,
        120,
        40,
        1000,
        240,
        160,
        80,
        240,
        2,
        1,
        120,
        1000,
        0,
        1080,
      ]
    `);
  });

  it("labels aspect ratios", () => {
    expect(
      dims.map(([w, h]) => [
        aspectRatioLabel(w, h),
        formatAspectRatioOrNull(w, h),
        formatAspectRatioOrUnknown(w, h),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "16:9",
          "16:9",
          "16:9",
        ],
        [
          "9:16",
          "9:16",
          "9:16",
        ],
        [
          "16:9",
          "16:9",
          "16:9",
        ],
        [
          "1:1",
          "1:1",
          "1:1",
        ],
        [
          "16:9",
          "16:9",
          "16:9",
        ],
        [
          "16:9",
          "16:9",
          "16:9",
        ],
        [
          "16:9",
          "16:9",
          "16:9",
        ],
        [
          "3:2",
          "3:2",
          "3:2",
        ],
        [
          "683:384",
          "683:384",
          "683:384",
        ],
        [
          "16:9",
          "16:9",
          "1919:1080",
        ],
        [
          "-16:9",
          "-16:9",
          "unknown",
        ],
        [
          "1:0",
          null,
          "unknown",
        ],
        [
          "0:0",
          null,
          "unknown",
        ],
        [
          "NaN:1",
          null,
          "unknown",
        ],
      ]
    `);
    expect([formatAspectRatioOrNull(null, 1080), formatAspectRatioOrNull(1920, undefined)]).toMatchInlineSnapshot(`
      [
        null,
        null,
      ]
    `);
  });

  it("labels media quality identically", () => {
    const qualityDims: Array<[number | null, number | null]> = [...dims, [null, 1080], [1920, null]];
    const expected = qualityDims.map(([w, h]) => mediaQualityLabel(w, h));
    expect(expected).toMatchInlineSnapshot(`
      [
        "FHD",
        "FHD",
        "SD",
        "HD",
        "4K",
        "QHD",
        "HD",
        "SD",
        "HD",
        "FHD",
        "SD",
        null,
        null,
        null,
        null,
        null,
      ]
    `);
  });

  it("formats source clip details", () => {
    const project = fixtureProject();
    const media = fixtureMedia(project, "video");
    expect([
      formatDimensions(media.width, media.height),
      formatDimensions(1920, null),
      formatDimensions(0, 1080),
      formatDimensions(undefined, undefined),
      formatFrameRate(media.fps),
      formatFrameRate(29.97),
      formatFrameRate(0),
      formatFrameRate(null),
      formatOptionalSeconds(4.25),
      formatOptionalSeconds(0),
      formatOptionalSeconds(Number.NaN),
      formatOptionalSeconds(null),
      formatOptionalSeconds(undefined),
    ]).toMatchInlineSnapshot(`
      [
        "640 x 360",
        "unknown",
        "unknown",
        "unknown",
        "24 fps",
        "29.97 fps",
        "still",
        "still",
        "4.25s",
        "0.00s",
        "unknown",
        "unknown",
        "unknown",
      ]
    `);
  });
});
