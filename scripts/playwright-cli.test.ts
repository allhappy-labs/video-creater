import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

const repoRoot = process.cwd();
const cliPath = join(repoRoot, "scripts/playwright-cli.mjs");

test("repository Playwright adapter exposes the visual-QA command contract", () => {
  const result = spawnSync(process.execPath, [cliPath, "--help"], {
    cwd: repoRoot,
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /open <url>/);
  assert.match(result.stdout, /resize <width> <height>/);
  assert.match(result.stdout, /run-code <async-function>/);
  assert.match(result.stdout, /screenshot --filename <path>/);
  assert.match(result.stdout, /close/);
});

test("browser smoke configuration retains evidence on failure", () => {
  const config = readFileSync(join(repoRoot, "playwright.config.ts"), "utf8");
  const smoke = readFileSync(join(repoRoot, "e2e/app-smoke.spec.ts"), "utf8");

  assert.match(config, /trace:\s*["']retain-on-failure["']/);
  assert.match(config, /screenshot:\s*["']only-on-failure["']/);
  assert.match(config, /reuseExistingServer:\s*false/);
  assert.match(config, /testIgnore:\s*["']remote-host\*\.spec\.ts["']/);
  assert.match(smoke, /pageerror/);
  assert.match(smoke, /console/);
  assert.match(smoke, /Project home/);
  assert.match(smoke, /Settings categories/);
  assert.match(smoke, /Video editor workspace/);
  assert.match(smoke, /desktop/);
  assert.match(smoke, /narrow/);
});

test("repository Playwright adapter keeps one page across commands and closes it", () => {
  const session = `adapter-test-${process.pid}-${Date.now()}`;
  const screenshot = join(mkdtempSync(join(tmpdir(), "video-creater-pwcli-")), "page.png");
  const stateRoot = join(tmpdir(), "video-creater-playwright-sessions");
  const statesBefore = existsSync(stateRoot) ? new Set(readdirSync(stateRoot)) : new Set<string>();
  const run = (...args: string[]) => spawnSync(
    process.execPath,
    [cliPath, "--session", session, ...args],
    { cwd: repoRoot, encoding: "utf8" },
  );

  const opened = run("open", "data:text/html,<main>Fixture ready</main>");
  assert.equal(opened.status, 0, opened.stderr);
  const evaluated = run(
    "run-code",
    "async (page) => { await page.getByText('Fixture ready').waitFor(); await page.evaluate(() => document.body.dataset.checked = 'yes'); }",
  );
  assert.equal(evaluated.status, 0, evaluated.stderr);
  const persisted = run(
    "run-code",
    "async (page) => { const checked = await page.evaluate(() => document.body.dataset.checked); if (checked !== 'yes') throw new Error('page state was not preserved'); }",
  );
  assert.equal(persisted.status, 0, persisted.stderr);
  const captured = run("screenshot", "--filename", screenshot);
  assert.equal(captured.status, 0, captured.stderr);
  assert.equal(existsSync(screenshot), true);
  assert.ok(statSync(screenshot).size > 0);
  const closed = run("close");
  assert.equal(closed.status, 0, closed.stderr);
  const statesAfter = existsSync(stateRoot) ? readdirSync(stateRoot) : [];
  assert.deepEqual(statesAfter.filter((state) => !statesBefore.has(state)), []);
});
