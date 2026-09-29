#!/usr/bin/env node
import { spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const sourcePath = fileURLToPath(import.meta.url);
const stateRoot = resolve(tmpdir(), "video-creater-playwright-sessions");

function printHelp() {
  process.stdout.write(`Usage: playwright-cli --session <name> <command> [arguments]

Commands:
  open <url>
  resize <width> <height>
  run-code <async-function>
  screenshot --filename <path>
  close
`);
}

function statePathForSession(session) {
  const digest = createHash("sha256")
    .update(`${process.cwd()}\0${session}`)
    .digest("hex")
    .slice(0, 24);
  return resolve(stateRoot, `${digest}.json`);
}

function sleep(milliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function readState(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

async function serve(statePath) {
  mkdirSync(dirname(statePath), { recursive: true });
  const token = randomBytes(32).toString("hex");
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 960 } });
  const page = await context.newPage();
  let idleTimer;
  let closing = false;
  let server;
  const closeServer = async () => {
    if (closing) return;
    closing = true;
    clearTimeout(idleTimer);
    await browser.close();
    server.close();
  };
  const resetIdleTimer = () => {
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => void closeServer(), 30 * 60 * 1000);
  };
  server = createServer((request, response) => {
    resetIdleTimer();
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => {
      void (async () => {
        if (request.headers.authorization !== `Bearer ${token}`) {
          response.writeHead(401).end("Unauthorized");
          return;
        }
        const input = JSON.parse(Buffer.concat(chunks).toString("utf8"));
        await executeCommand(page, input.command, input.args ?? []);
        response.writeHead(204).end();
        if (input.command === "close") {
          await closeServer();
        }
      })().catch((error) => {
        response.writeHead(500).end(error instanceof Error ? error.stack ?? error.message : String(error));
      });
    });
  });
  await new Promise((resolveListening, rejectListening) => {
    server.once("error", rejectListening);
    server.listen(0, "127.0.0.1", resolveListening);
  });
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Playwright session server has no TCP port");
  writeFileSync(
    statePath,
    `${JSON.stringify({ port: address.port, token, pid: process.pid })}\n`,
    { mode: 0o600 },
  );
  const cleanup = () => {
    if (existsSync(statePath)) unlinkSync(statePath);
  };
  process.once("exit", cleanup);
  process.once("SIGINT", () => void closeServer());
  process.once("SIGTERM", () => void closeServer());
  resetIdleTimer();
  await new Promise((resolveClosed) => server.once("close", resolveClosed));
}

async function ensureSession(statePath) {
  if (existsSync(statePath)) {
    try {
      await requestSession(statePath, "ping", []);
      return;
    } catch {
      unlinkSync(statePath);
    }
  }

  mkdirSync(dirname(statePath), { recursive: true });
  const child = spawn(process.execPath, [sourcePath, "--serve", statePath], {
    detached: true,
    stdio: "ignore",
  });
  child.unref();
  const deadline = Date.now() + 30_000;
  while (!existsSync(statePath) && Date.now() < deadline) sleep(50);
  if (!existsSync(statePath)) {
    throw new Error("Timed out launching the repository Playwright session");
  }
}

async function executeCommand(page, command, args) {
  if (command === "ping" || command === "close") return;
  if (command === "open") {
    await page.goto(args[0] ?? "about:blank");
  } else if (command === "resize") {
    const width = Number(args[0]);
    const height = Number(args[1]);
    if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0) {
      throw new Error("resize requires positive integer width and height");
    }
    await page.setViewportSize({ width, height });
  } else if (command === "run-code") {
    const source = args[0];
    if (!source) throw new Error("run-code requires an async function");
    const execute = Function("page", `return (${source})(page);`);
    await execute(page);
  } else if (command === "screenshot") {
    const filenameIndex = args.indexOf("--filename");
    const filename = filenameIndex === -1 ? undefined : args[filenameIndex + 1];
    if (!filename) throw new Error("screenshot requires --filename <path>");
    await page.screenshot({ path: resolve(filename), type: "png" });
  } else {
    throw new Error(`Unknown Playwright command: ${command}`);
  }
}

async function requestSession(statePath, command, args) {
  const { port, token } = readState(statePath);
  const response = await fetch(`http://127.0.0.1:${port}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ command, args }),
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
}

async function runClient(session, command, args) {
  const statePath = statePathForSession(session);
  if (command === "open") await ensureSession(statePath);
  if (!existsSync(statePath)) {
    throw new Error(`Playwright session ${session} is not open`);
  }
  await requestSession(statePath, command, args);
  if (command === "close") {
    const deadline = Date.now() + 5_000;
    while (existsSync(statePath) && Date.now() < deadline) sleep(25);
    if (existsSync(statePath)) throw new Error("Playwright session did not close cleanly");
  }
}

async function main(argv) {
  if (argv.includes("--help") || argv.includes("-h")) {
    printHelp();
    return;
  }
  if (argv[0] === "--serve") {
    const statePath = argv[1];
    if (!statePath) throw new Error("--serve requires a state path");
    await serve(statePath);
    return;
  }
  const sessionIndex = argv.indexOf("--session");
  const session = sessionIndex === -1 ? undefined : argv[sessionIndex + 1];
  if (!session) throw new Error("--session requires a name");
  const commandIndex = sessionIndex + 2;
  const command = argv[commandIndex];
  if (!command) throw new Error("A Playwright command is required");
  await runClient(session, command, argv.slice(commandIndex + 1));
}

main(process.argv.slice(2)).then(
  () => process.exit(0),
  (error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exit(1);
  },
);
