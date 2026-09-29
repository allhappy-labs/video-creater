export function parseMachORpaths(output) {
  const lines = output.split(/\r?\n/);
  const rpaths = [];
  let declaredCommands = 0;

  for (let index = 0; index < lines.length; index += 1) {
    if (!/^Load command \d+$/.test(lines[index].trim())) continue;
    const block = [];
    for (index += 1; index < lines.length; index += 1) {
      if (/^Load command \d+$/.test(lines[index].trim())) {
        index -= 1;
        break;
      }
      block.push(lines[index]);
    }
    if (!block.some((line) => /^\s*cmd\s+LC_RPATH\s*$/.test(line))) {
      continue;
    }
    declaredCommands += 1;
    const pathLines = block.filter((line) => /^\s*path\b/.test(line));
    const match = pathLines.length === 1
      ? pathLines[0].match(/^\s*path\s+(.+)\s+\(offset\s+\d+\)\s*$/)
      : null;
    const rpath = match?.[1]?.trim();
    if (!rpath || /[\0\r\n]/.test(rpath)) {
      throw new Error("malformed LC_RPATH command in Mach-O load commands");
    }
    rpaths.push(rpath);
  }

  const rawCommandCount = lines.filter((line) =>
    /^\s*cmd\s+LC_RPATH\s*$/.test(line)
  ).length;
  if (declaredCommands !== rawCommandCount) {
    throw new Error("malformed LC_RPATH command outside a load-command block");
  }
  return rpaths;
}

export function planPackagedRuntimeRpaths({
  existingRpaths,
  requiredRpaths,
}) {
  const required = [...new Set(requiredRpaths)];
  for (const rpath of required) {
    if (!isReviewedRuntimeRpath(rpath)) {
      throw new Error(`unsafe required packaged runtime rpath: ${rpath}`);
    }
  }

  const requiredSet = new Set(required);
  const retained = new Set();
  const deleteRpaths = [];
  for (const rpath of existingRpaths) {
    if (
      requiredSet.has(rpath) &&
      isReviewedRuntimeRpath(rpath) &&
      !retained.has(rpath)
    ) {
      retained.add(rpath);
    } else {
      deleteRpaths.push(rpath);
    }
  }
  const addRpaths = required.filter((rpath) => !retained.has(rpath));
  return { deleteRpaths, addRpaths };
}

export function reconcilePackagedRuntimeRpaths({
  path,
  requiredRpaths,
  run,
}) {
  const initialRpaths = readMachORpaths(path, run);
  const initialPlan = planPackagedRuntimeRpaths({
    existingRpaths: initialRpaths,
    requiredRpaths,
  });
  for (const rpath of initialPlan.deleteRpaths) {
    run("install_name_tool", ["-delete_rpath", rpath, path]);
  }
  for (const rpath of initialPlan.addRpaths) {
    run("install_name_tool", ["-add_rpath", rpath, path]);
  }

  const finalRpaths = readMachORpaths(path, run);
  const residualPlan = planPackagedRuntimeRpaths({
    existingRpaths: finalRpaths,
    requiredRpaths,
  });
  if (
    residualPlan.deleteRpaths.length > 0 ||
    residualPlan.addRpaths.length > 0
  ) {
    throw new Error(
      `packaged runtime rpath reconciliation did not converge for ${path}`,
    );
  }
  return { initialPlan, finalRpaths };
}

function readMachORpaths(path, run) {
  const result = run("otool", ["-l", path]);
  return parseMachORpaths(`${result.stdout ?? ""}${result.stderr ?? ""}`);
}

export function isReviewedRuntimeRpath(rpath) {
  return /^@(loader_path|executable_path|rpath)(?:\/[^\0\r\n]*)?$/.test(rpath);
}
