import assert from "node:assert/strict";
import test from "node:test";

import { PINNED_DTDS, extractXmemlV5Dtd, sha256Hex, summarizeValidationResults } from "./nle-xml-validation-sources.mjs";

const v4Header = "<pre>&lt;!-- Copyright 2007 Apple Inc. --><span></span></pre>\n<pre>&lt;!-- Interchange Format v4.0 --></pre>";
const v4Body = "<pre>&lt;!ELEMENT xmeml (sequence)*&gt;</pre>\n<pre>&lt;!ELEMENT transitionitem (name)*&gt;</pre>";
const v5Header = "<pre>&lt;!-- Copyright 2009 Apple Inc. --><span></span></pre>\n<p>prose between blocks</p>\n<pre>&lt;!-- Interchange Format v5.0 --></pre>";
const v5Body = '<pre>&lt;!ELEMENT xmeml (sequence | bin)*&gt;</pre>\n<pre>&lt;!ELEMENT transitionitem (name | effect)*&gt;</pre>\n<pre>&lt;!ATTLIST xmeml version CDATA &quot;5&quot; note CDATA &#39;a &amp; b&#39;&gt;</pre>';

test("extracts only the XMEML v5 DTD, unescaped and joined by newlines", () => {
  const html = `<html><body>${v4Header}\n${v4Body}\n<h2>Version 5</h2>\n${v5Header}\n${v5Body}</body></html>`;

  assert.equal(
    extractXmemlV5Dtd(html),
    [
      "<!-- Copyright 2009 Apple Inc. -->",
      "<!-- Interchange Format v5.0 -->",
      "<!ELEMENT xmeml (sequence | bin)*>",
      "<!ELEMENT transitionitem (name | effect)*>",
      "<!ATTLIST xmeml version CDATA \"5\" note CDATA 'a & b'>",
    ].join("\n"),
  );
});

test("fails when the XMEML v5 header is missing", () => {
  assert.throws(() => extractXmemlV5Dtd(`<html>${v4Header}\n${v4Body}</html>`), /XMEML v5 DTD not found/);
});

test("fails when the extracted DTD lacks the xmeml or transitionitem declarations", () => {
  assert.throws(() => extractXmemlV5Dtd(`<html>${v5Header}\n<pre>&lt;!ELEMENT transitionitem (name)*&gt;</pre></html>`), /<!ELEMENT xmeml/);
  assert.throws(() => extractXmemlV5Dtd(`<html>${v5Header}\n<pre>&lt;!ELEMENT xmeml (sequence)*&gt;</pre></html>`), /<!ELEMENT transitionitem/);
});

test("hashes strings and buffers as lowercase SHA-256 hex", () => {
  assert.equal(sha256Hex("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
  assert.equal(sha256Hex(Buffer.from("abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
});

test("summarizes xmllint results into pass and failure counts", () => {
  assert.deepEqual(summarizeValidationResults([]), { passed: 0, failed: 0, failures: [] });
  assert.deepEqual(
    summarizeValidationResults([
      { file: "corpus/a.xml", format: "xmeml", exitCode: 0, stderr: "" },
      { file: "corpus/a.fcpxml", format: "fcpxml", exitCode: 3, stderr: "validity error" },
      { file: "corpus/b.fcpxml", format: "fcpxml", exitCode: 0, stderr: "" },
    ]),
    { passed: 2, failed: 1, failures: [{ file: "corpus/a.fcpxml", stderr: "validity error" }] },
  );
});

test("pins the FCPXML 1.10 and XMEML v5 DTD sources", () => {
  assert.equal(
    PINNED_DTDS.fcpxml.url,
    "https://raw.githubusercontent.com/CommandPost/CommandPost/e0698cbd6ac0bc13cd8279e115ff05dec296f5a9/src/extensions/cp/apple/fcpxml/dtd/FCPXMLv1_10.dtd",
  );
  assert.equal(PINNED_DTDS.fcpxml.sha256, "32cbad28022f9a2033acdc25d0583b16d2e12745dc5efe4fa8f16e27aa59ff53");
  assert.equal(PINNED_DTDS.fcpxml.fileName, "FCPXMLv1_10.dtd");
  assert.equal(
    PINNED_DTDS.xmeml.url,
    "https://developer.apple.com/library/archive/documentation/AppleApplications/Reference/FinalCutPro_XML/DTD/DTD.html",
  );
  assert.match(PINNED_DTDS.xmeml.sha256, /^[0-9a-f]{64}$/);
  assert.equal(PINNED_DTDS.xmeml.sha256, "7be946ee484917dc1fd641ac3e0988a285a2ed6a3013395f9c9fbaa895169a2b");
  assert.equal(PINNED_DTDS.xmeml.fileName, "xmeml-v5.dtd");
});
