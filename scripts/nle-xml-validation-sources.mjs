// Pure helpers for the NLE XML validation driver (scripts/nle-xml-validation.mjs).
// The FCPXML and XMEML DTDs are Apple-copyrighted: they are downloaded into the
// tools folder, verified against these pins, and never committed.
import { createHash } from "node:crypto";

const XMEML_V5_HEADER = "Copyright 2009 Apple Inc.";

export const PINNED_DTDS = Object.freeze({
  fcpxml: Object.freeze({
    fileName: "FCPXMLv1_10.dtd",
    url: "https://raw.githubusercontent.com/CommandPost/CommandPost/e0698cbd6ac0bc13cd8279e115ff05dec296f5a9/src/extensions/cp/apple/fcpxml/dtd/FCPXMLv1_10.dtd",
    sha256: "32cbad28022f9a2033acdc25d0583b16d2e12745dc5efe4fa8f16e27aa59ff53",
  }),
  xmeml: Object.freeze({
    fileName: "xmeml-v5.dtd",
    url: "https://developer.apple.com/library/archive/documentation/AppleApplications/Reference/FinalCutPro_XML/DTD/DTD.html",
    sha256: "7be946ee484917dc1fd641ac3e0988a285a2ed6a3013395f9c9fbaa895169a2b",
  }),
});

function unescapeHtml(text) {
  return text
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&quot;", '"')
    .replaceAll("&#39;", "'")
    .replaceAll("&amp;", "&");
}

/** Returns the XMEML v5 DTD from Apple's DTD appendix page: every `<pre>` block from the v5 copyright header on. */
export function extractXmemlV5Dtd(html) {
  const blocks = [...html.matchAll(/<pre[^>]*>([\s\S]*?)<\/pre>/g)].map((match) => unescapeHtml(match[1].replace(/<[^>]*>/g, "")));
  const start = blocks.findIndex((block) => block.includes(XMEML_V5_HEADER));
  if (start === -1) {
    throw new Error("XMEML v5 DTD not found");
  }
  const dtd = blocks.slice(start).join("\n");
  for (const required of ["<!ELEMENT xmeml", "<!ELEMENT transitionitem"]) {
    if (!dtd.includes(required)) {
      throw new Error(`XMEML v5 DTD is missing ${required}`);
    }
  }
  return dtd;
}

export function sha256Hex(bufferOrString) {
  return createHash("sha256").update(bufferOrString).digest("hex");
}

/** Counts xmllint results (`{ file, format, exitCode, stderr }`) into passes and failures. */
export function summarizeValidationResults(results) {
  const failures = results.filter((result) => result.exitCode !== 0).map(({ file, stderr }) => ({ file, stderr }));
  return { passed: results.length - failures.length, failed: failures.length, failures };
}
