#!/usr/bin/env node
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { deflateSync, inflateSync } from "node:zlib";

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function usage() {
  return [
    "Usage: node scripts/compare-preview-render-frames.mjs --out preview-comparison.json --diff-dir diffs --frame seconds:preview.png:rendered.png [--frame ...]",
    "",
    "Options:",
    "  --threshold <ratio>          Per-frame mismatch ratio allowed before failure. Default: 0",
    "  --channel-threshold <0-255>  Per-channel absolute difference ignored for pixel mismatch. Default: 0",
    "  --fail-on-mismatch           Exit with status 1 after writing evidence when any frame fails.",
    "  --out <path>                 JSON output path for the previewComparison object.",
    "  --diff-dir <path>            Directory for failed-frame diff PNGs.",
    "  --frame <spec>               Frame spec in timelineSeconds:previewPath:renderedPath form.",
  ].join("\n");
}

function parseArgs(argv) {
  const args = argv.filter((arg) => arg !== "--");
  const options = {
    threshold: 0,
    channelThreshold: 0,
    out: null,
    diffDir: null,
    failOnMismatch: false,
    frames: [],
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--threshold") {
      options.threshold = parseNumberArg(arg, args[++index]);
    } else if (arg === "--channel-threshold") {
      options.channelThreshold = parseNumberArg(arg, args[++index]);
    } else if (arg === "--out") {
      options.out = requireValue(arg, args[++index]);
    } else if (arg === "--diff-dir") {
      options.diffDir = requireValue(arg, args[++index]);
    } else if (arg === "--fail-on-mismatch") {
      options.failOnMismatch = true;
    } else if (arg === "--frame") {
      options.frames.push(parseFrameSpec(requireValue(arg, args[++index])));
    } else if (arg === "--help" || arg === "-h") {
      console.log(usage());
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}\n\n${usage()}`);
    }
  }

  if (!options.out) {
    throw new Error(`Missing --out.\n\n${usage()}`);
  }
  if (!options.diffDir) {
    throw new Error(`Missing --diff-dir.\n\n${usage()}`);
  }
  if (options.frames.length === 0) {
    throw new Error(`At least one --frame is required.\n\n${usage()}`);
  }
  if (options.channelThreshold < 0 || options.channelThreshold > 255) {
    throw new Error("--channel-threshold must be between 0 and 255.");
  }
  if (options.threshold < 0 || options.threshold > 1) {
    throw new Error("--threshold must be between 0 and 1.");
  }

  return options;
}

function requireValue(flag, value) {
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}.`);
  }
  return value;
}

function parseNumberArg(flag, value) {
  const parsed = Number(requireValue(flag, value));
  if (!Number.isFinite(parsed)) {
    throw new Error(`${flag} must be a finite number.`);
  }
  return parsed;
}

function parseFrameSpec(spec) {
  const firstSeparator = spec.indexOf(":");
  const lastSeparator = spec.lastIndexOf(":");
  if (firstSeparator <= 0 || lastSeparator <= firstSeparator) {
    throw new Error(`Invalid --frame value "${spec}". Expected timelineSeconds:previewPath:renderedPath.`);
  }

  const timelineSeconds = Number(spec.slice(0, firstSeparator));
  if (!Number.isFinite(timelineSeconds)) {
    throw new Error(`Invalid frame timeline seconds in "${spec}".`);
  }

  return {
    timelineSeconds,
    previewFrame: spec.slice(firstSeparator + 1, lastSeparator),
    renderedFrame: spec.slice(lastSeparator + 1),
  };
}

function readPng(filePath) {
  const png = readFileSync(filePath);
  if (png.length < PNG_SIGNATURE.length || !png.subarray(0, PNG_SIGNATURE.length).equals(PNG_SIGNATURE)) {
    throw new Error(`${filePath} is not a PNG file.`);
  }

  let offset = PNG_SIGNATURE.length;
  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  const idatChunks = [];

  while (offset < png.length) {
    if (offset + 12 > png.length) {
      throw new Error(`${filePath} has a truncated PNG chunk.`);
    }
    const length = png.readUInt32BE(offset);
    const type = png.subarray(offset + 4, offset + 8).toString("ascii");
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    if (dataEnd + 4 > png.length) {
      throw new Error(`${filePath} has a truncated ${type} chunk.`);
    }
    const data = png.subarray(dataStart, dataEnd);

    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      const compression = data[10];
      const filter = data[11];
      const interlace = data[12];
      if (bitDepth !== 8 || (colorType !== 2 && colorType !== 6) || compression !== 0 || filter !== 0 || interlace !== 0) {
        throw new Error(`${filePath} must be an 8-bit non-interlaced RGB or RGBA PNG.`);
      }
    } else if (type === "IDAT") {
      idatChunks.push(Buffer.from(data));
    } else if (type === "IEND") {
      break;
    }

    offset = dataEnd + 4;
  }

  if (!width || !height || idatChunks.length === 0) {
    throw new Error(`${filePath} is missing PNG image data.`);
  }

  const sourceChannels = colorType === 6 ? 4 : 3;
  const sourceStride = width * sourceChannels;
  const inflated = inflateSync(Buffer.concat(idatChunks));
  const expectedBytes = (sourceStride + 1) * height;
  if (inflated.length < expectedBytes) {
    throw new Error(`${filePath} has incomplete PNG scanlines.`);
  }

  const rows = unfilterScanlines(inflated, width, height, sourceChannels);
  const rgba = Buffer.alloc(width * height * 4);
  for (let pixel = 0; pixel < width * height; pixel += 1) {
    const source = pixel * sourceChannels;
    const target = pixel * 4;
    rgba[target] = rows[source];
    rgba[target + 1] = rows[source + 1];
    rgba[target + 2] = rows[source + 2];
    rgba[target + 3] = colorType === 6 ? rows[source + 3] : 255;
  }

  return { width, height, rgba };
}

function unfilterScanlines(inflated, width, height, channels) {
  const rowBytes = width * channels;
  const output = Buffer.alloc(rowBytes * height);

  for (let y = 0; y < height; y += 1) {
    const sourceRow = y * (rowBytes + 1);
    const filter = inflated[sourceRow];
    const targetRow = y * rowBytes;
    for (let x = 0; x < rowBytes; x += 1) {
      const raw = inflated[sourceRow + 1 + x];
      const left = x >= channels ? output[targetRow + x - channels] : 0;
      const up = y > 0 ? output[targetRow + x - rowBytes] : 0;
      const upLeft = y > 0 && x >= channels ? output[targetRow + x - rowBytes - channels] : 0;

      let value;
      if (filter === 0) {
        value = raw;
      } else if (filter === 1) {
        value = raw + left;
      } else if (filter === 2) {
        value = raw + up;
      } else if (filter === 3) {
        value = raw + Math.floor((left + up) / 2);
      } else if (filter === 4) {
        value = raw + paeth(left, up, upLeft);
      } else {
        throw new Error(`Unsupported PNG filter type ${filter}.`);
      }
      output[targetRow + x] = value & 0xff;
    }
  }

  return output;
}

function paeth(left, up, upLeft) {
  const estimate = left + up - upLeft;
  const leftDistance = Math.abs(estimate - left);
  const upDistance = Math.abs(estimate - up);
  const upLeftDistance = Math.abs(estimate - upLeft);
  if (leftDistance <= upDistance && leftDistance <= upLeftDistance) {
    return left;
  }
  return upDistance <= upLeftDistance ? up : upLeft;
}

function compareImages(preview, rendered, channelThreshold) {
  if (preview.width !== rendered.width || preview.height !== rendered.height) {
    throw new Error(
      `Preview/render frame dimensions differ: ${preview.width}x${preview.height} vs ${rendered.width}x${rendered.height}.`,
    );
  }

  let mismatchedPixels = 0;
  const diff = Buffer.alloc(preview.width * preview.height * 4);
  for (let pixel = 0; pixel < preview.width * preview.height; pixel += 1) {
    const offset = pixel * 4;
    const mismatched =
      Math.abs(preview.rgba[offset] - rendered.rgba[offset]) > channelThreshold ||
      Math.abs(preview.rgba[offset + 1] - rendered.rgba[offset + 1]) > channelThreshold ||
      Math.abs(preview.rgba[offset + 2] - rendered.rgba[offset + 2]) > channelThreshold ||
      Math.abs(preview.rgba[offset + 3] - rendered.rgba[offset + 3]) > channelThreshold;

    if (mismatched) {
      mismatchedPixels += 1;
      diff[offset] = 255;
      diff[offset + 1] = 0;
      diff[offset + 2] = 64;
      diff[offset + 3] = 255;
    } else {
      diff[offset] = preview.rgba[offset];
      diff[offset + 1] = preview.rgba[offset + 1];
      diff[offset + 2] = preview.rgba[offset + 2];
      diff[offset + 3] = 64;
    }
  }

  return {
    mismatchRatio: mismatchedPixels / (preview.width * preview.height),
    diff,
  };
}

function writeRgbaPng(filePath, width, height, rgba) {
  const rowBytes = width * 4;
  const raw = Buffer.alloc((rowBytes + 1) * height);
  for (let y = 0; y < height; y += 1) {
    raw[y * (rowBytes + 1)] = 0;
    rgba.copy(raw, y * (rowBytes + 1) + 1, y * rowBytes, y * rowBytes + rowBytes);
  }

  writeFileSync(
    filePath,
    Buffer.concat([
      PNG_SIGNATURE,
      pngChunk("IHDR", ihdr(width, height)),
      pngChunk("IDAT", deflateSync(raw)),
      pngChunk("IEND", Buffer.alloc(0)),
    ]),
  );
}

function ihdr(width, height) {
  const data = Buffer.alloc(13);
  data.writeUInt32BE(width, 0);
  data.writeUInt32BE(height, 4);
  data[8] = 8;
  data[9] = 6;
  return data;
}

function pngChunk(type, data) {
  const typeBuffer = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function compareFrames(options) {
  mkdirSync(options.diffDir, { recursive: true });
  const comparedFrames = options.frames.map((frame, index) => {
    const preview = readPng(frame.previewFrame);
    const rendered = readPng(frame.renderedFrame);
    const comparison = compareImages(preview, rendered, options.channelThreshold);
    const passed = comparison.mismatchRatio <= options.threshold;
    const diffFrame = passed ? null : path.join(options.diffDir, `diff-${String(index + 1).padStart(4, "0")}.png`);
    if (diffFrame) {
      writeRgbaPng(diffFrame, preview.width, preview.height, comparison.diff);
    }

    return {
      timelineSeconds: frame.timelineSeconds,
      previewFrame: frame.previewFrame,
      renderedFrame: frame.renderedFrame,
      diffFrame,
      mismatchRatio: comparison.mismatchRatio,
      passed,
    };
  });

  return {
    status: comparedFrames.every((frame) => frame.passed) ? "passed" : "failed",
    comparedFrames,
  };
}

try {
  const options = parseArgs(process.argv.slice(2));
  const comparison = compareFrames(options);
  mkdirSync(path.dirname(options.out), { recursive: true });
  const json = `${JSON.stringify(comparison, null, 2)}\n`;
  writeFileSync(options.out, json);
  process.stdout.write(json);
  if (options.failOnMismatch && comparison.status !== "passed") {
    process.exitCode = 1;
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 2;
}
