import { spawnSync } from "node:child_process";

// ThorVG defaults to clang++, and bindgen needs compiler builtin headers.
// A Linux host may supply libclang without clang's resource headers; use the
// platform C++ compiler and GCC headers consistently for every release build.
export function linuxNativeBuildEnvironment(env, { capture = spawnSync } = {}) {
  const next = { ...env };
  if (!next.CXX) next.CXX = "c++";
  if (!next.BINDGEN_EXTRA_CLANG_ARGS) {
    const gccInclude = capture("cc", ["-print-file-name=include"], { encoding: "utf8" });
    const includeDir = gccInclude.status === 0 ? gccInclude.stdout.trim() : "";
    if (includeDir.startsWith("/")) next.BINDGEN_EXTRA_CLANG_ARGS = `-isystem ${includeDir}`;
  }
  return next;
}
