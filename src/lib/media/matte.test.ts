import { describe, expect, it } from "vitest";
import {
  evenSize,
  matteAspectOptions,
  mattePreviewSize,
  normalizedHex,
} from "@/lib/media/matte";

const timelineSizes: Array<[number, number]> = [
  [1920, 1080],
  [1080, 1920],
  [1001, 777],
  [0, 0],
  [Number.NaN, 1080],
];

describe("matte characterization", () => {
  it("lists aspect options", () => {
    expect(matteAspectOptions).toMatchInlineSnapshot(`
      [
        "Project",
        "16:9",
        "9:16",
        "1:1",
        "4:3",
        "9:14",
        "2.4:1",
      ]
    `);
  });

  it("rounds sizes down to even values", () => {
    expect(
      [0, 1, 2, 3, 4.9, 1081, -5, Number.NaN, Number.POSITIVE_INFINITY].map((value) => evenSize(value)),
    ).toMatchInlineSnapshot(`
      [
        2,
        2,
        2,
        2,
        4,
        1080,
        2,
        2,
        2,
      ]
    `);
  });

  it("sizes matte previews for every aspect option", () => {
    expect(
      matteAspectOptions.map((aspect) => [
        aspect,
        ...timelineSizes.map(([width, height]) => mattePreviewSize(aspect, width, height).join("x")),
      ]),
    ).toMatchInlineSnapshot(`
      [
        [
          "Project",
          "1920x1080",
          "1080x1920",
          "1000x776",
          "2x2",
          "2x1080",
        ],
        [
          "16:9",
          "1920x1080",
          "1920x1080",
          "1380x776",
          "4x2",
          "2x2",
        ],
        [
          "9:16",
          "1080x1920",
          "1080x1920",
          "776x1380",
          "2x4",
          "2x2",
        ],
        [
          "1:1",
          "1080x1080",
          "1080x1080",
          "776x776",
          "2x2",
          "2x2",
        ],
        [
          "4:3",
          "1440x1080",
          "1440x1080",
          "1036x776",
          "2x2",
          "2x2",
        ],
        [
          "9:14",
          "1080x1680",
          "1080x1680",
          "776x1208",
          "2x2",
          "2x2",
        ],
        [
          "2.4:1",
          "2592x1080",
          "2592x1080",
          "1864x776",
          "4x2",
          "2x2",
        ],
      ]
    `);
  });

  it("normalizes hex colors", () => {
    expect(
      ["#fff", "FFF", "#abcdef", " #abcdef ", "abcdef", "#ABCDEF0", "zzz", ""].map((value) =>
        normalizedHex(value),
      ),
    ).toMatchInlineSnapshot(`
      [
        null,
        null,
        "#ABCDEF",
        "#ABCDEF",
        null,
        null,
        null,
        null,
      ]
    `);
  });
});
