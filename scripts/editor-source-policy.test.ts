import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { relative, resolve, sep } from "node:path";
import test from "node:test";
import ts from "typescript";

const repoRoot = resolve(import.meta.dirname, "..");
const sourceRoot = resolve(repoRoot, "src");
const editorRoot = resolve(sourceRoot, "editor");
const uiPrimitiveRoot = resolve(sourceRoot, "components", "ui");

const maximumEditorFileLines = 600;
const arbitraryHexColorPattern = /\[#[0-9a-fA-F]{3,8}\]/g;
// Opacity fragments such as `border-white/15` or `bg-black/40`; solid `bg-black` stays allowed.
const rawOpacityColorPattern = /\b(?:white|black)\/[\w.[\]]+/g;
const tauriImportPattern = /["']@tauri-apps\/[^"']*["']/g;
const nativeDialogPattern = /\bwindow\.(?:prompt|confirm)\(/g;
const internalProductNamePattern = /[A-Za-z0-9_]*(?:Codex|HyperFrames)[A-Za-z0-9_]*/g;

/** JSX attributes whose string values are read or announced to the user. */
const userVisibleAttributes = new Set([
  "aria-label",
  "aria-description",
  "aria-placeholder",
  "aria-roledescription",
  "aria-valuetext",
  "alt",
  "description",
  "label",
  "placeholder",
  "title",
]);

/**
 * Code identifiers that may appear inside user-visible literals (for example a command name shown
 * in diagnostics). Entries must be identifiers, never the bare product names.
 */
const codeIdentifierAllowlist: ReadonlySet<string> = new Set<string>([]);

function sourceFiles(directory: string, extensions: RegExp): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...sourceFiles(path, extensions));
    } else if (entry.isFile() && extensions.test(entry.name)) {
      files.push(path);
    }
  }
  return files.sort();
}

function isTestFile(path: string): boolean {
  return /\.(?:test|spec)\.(?:ts|tsx)$/.test(path);
}

function productionFiles(directory: string, extensions = /\.(?:ts|tsx)$/): string[] {
  return sourceFiles(directory, extensions).filter((path) => !isTestFile(path));
}

function repositoryPath(path: string): string {
  return relative(repoRoot, path).split(sep).join("/");
}

function lineOf(text: string, index: number): number {
  return text.slice(0, index).split("\n").length;
}

function patternViolations(paths: readonly string[], pattern: RegExp): string[] {
  return paths.flatMap((path) => {
    const text = readFileSync(path, "utf8");
    return [...text.matchAll(pattern)].map(
      (match) => `${repositoryPath(path)}:${lineOf(text, match.index ?? 0)} ${match[0]}`,
    );
  });
}

function literalText(node: ts.Node): string | null {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
  if (ts.isTemplateExpression(node)) {
    return [node.head.text, ...node.templateSpans.map((span) => span.literal.text)].join(" ");
  }
  if (ts.isParenthesizedExpression(node)) return literalText(node.expression);
  if (ts.isConditionalExpression(node)) {
    return [literalText(node.whenTrue), literalText(node.whenFalse)].filter(Boolean).join(" ") || null;
  }
  if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.PlusToken) {
    return [literalText(node.left), literalText(node.right)].filter(Boolean).join(" ") || null;
  }
  return null;
}

/** JSX text, literal JSX children, and literal values of user-visible JSX attributes. */
function userVisibleLiterals(path: string, text: string): { line: number; text: string }[] {
  const sourceFile = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const literals: { line: number; text: string }[] = [];
  const record = (node: ts.Node, value: string | null) => {
    if (!value) return;
    literals.push({ line: sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1, text: value });
  };
  const visit = (node: ts.Node) => {
    if (ts.isJsxText(node)) {
      record(node, node.text);
    } else if (ts.isJsxExpression(node) && node.expression && !ts.isJsxAttribute(node.parent)) {
      record(node, literalText(node.expression));
    } else if (ts.isJsxAttribute(node) && userVisibleAttributes.has(node.name.getText(sourceFile)) && node.initializer) {
      const initializer = node.initializer;
      if (ts.isStringLiteral(initializer)) record(initializer, initializer.text);
      else if (ts.isJsxExpression(initializer) && initializer.expression) {
        record(initializer, literalText(initializer.expression));
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return literals;
}

test(`editor source files stay under ${maximumEditorFileLines} lines`, () => {
  const violations = productionFiles(editorRoot)
    .map((path) => ({ path, lines: readFileSync(path, "utf8").split("\n").length }))
    .filter(({ lines }) => lines > maximumEditorFileLines)
    .map(({ path, lines }) => `${repositoryPath(path)} (${lines} lines)`);

  assert.deepEqual(violations, [], `Split these editor files:\n${violations.join("\n")}`);
});

test("editor and UI primitives use theme tokens instead of raw colours", () => {
  const files = [editorRoot, uiPrimitiveRoot].flatMap((root) => productionFiles(root, /\.(?:ts|tsx|css)$/));
  const violations = [
    ...patternViolations(files, arbitraryHexColorPattern),
    ...patternViolations(files, rawOpacityColorPattern),
  ];

  assert.deepEqual(violations, [], `Replace raw colours with theme tokens:\n${violations.join("\n")}`);
});

test("editor code reaches native APIs only through runtime adapters", () => {
  const violations = patternViolations(productionFiles(editorRoot), tauriImportPattern);

  assert.deepEqual(violations, [], `Editor files imported Tauri packages:\n${violations.join("\n")}`);
});

test("the app never opens native prompt or confirm dialogs", () => {
  const violations = patternViolations(sourceFiles(sourceRoot, /\.(?:ts|tsx)$/), nativeDialogPattern);

  assert.deepEqual(violations, [], `Use in-app dialogs instead:\n${violations.join("\n")}`);
});

test("the identifier allowlist holds code identifiers only", () => {
  for (const entry of codeIdentifierAllowlist) {
    assert.match(entry, /^[A-Za-z_][A-Za-z0-9_]*$/, `${entry} is not an identifier`);
    assert.notEqual(entry, "Codex");
    assert.notEqual(entry, "HyperFrames");
  }
});

test("editor copy uses product vocabulary instead of internal engine names", () => {
  const violations = productionFiles(editorRoot).flatMap((path) =>
    userVisibleLiterals(path, readFileSync(path, "utf8")).flatMap(({ line, text }) =>
      [...text.matchAll(internalProductNamePattern)]
        .map((match) => match[0])
        .filter((word) => !codeIdentifierAllowlist.has(word))
        .map((word) => `${repositoryPath(path)}:${line} ${word}: ${text.trim()}`),
    ),
  );

  assert.deepEqual(
    violations,
    [],
    `Say "AI" or "agent" instead of Codex and "Graphics" instead of HyperFrames:\n${violations.join("\n")}`,
  );
});

test("the copy scan sees JSX text, literal children, and user-visible attributes only", () => {
  const literals = userVisibleLiterals(
    "probe.tsx",
    'const a = <p title="Codex title">Codex text {"HyperFrames child"}<i className="Codex" data-id={codexId} /></p>;',
  );

  assert.deepEqual(
    literals.map(({ text }) => text.trim()),
    ["Codex title", "Codex text", "HyperFrames child"],
  );
});
