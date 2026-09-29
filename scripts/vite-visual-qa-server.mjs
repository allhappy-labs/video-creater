#!/usr/bin/env node
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { createServer } from "vite";

const [host, portValue, readyPath] = process.argv.slice(2);
if (!host || !portValue || !readyPath) {
  throw new Error("Usage: vite-visual-qa-server <host> <port> <ready-path>");
}

const server = await createServer({
  clearScreen: false,
  server: { host, port: Number(portValue), strictPort: true },
});

function removeReadyFile() {
  if (existsSync(readyPath)) unlinkSync(readyPath);
}

async function close(signal) {
  removeReadyFile();
  await server.close();
  process.exit(signal === "SIGINT" ? 130 : 143);
}

process.once("exit", removeReadyFile);
process.once("SIGINT", () => void close("SIGINT"));
process.once("SIGTERM", () => void close("SIGTERM"));

try {
  await server.listen();
  writeFileSync(readyPath, `${process.pid}\n`, { mode: 0o600 });
} catch (error) {
  await server.close();
  throw error;
}

await new Promise(() => {});
