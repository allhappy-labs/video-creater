import type { VideoProject } from "@/lib/project";

const historyByteBudget = 128 * 1024 * 1024;
const sizes = new WeakMap<VideoProject, number>();

/** Conservative serialized UTF-16 representation budget, not an exact JS heap estimate. */
function bytes(project: VideoProject): number {
  const cached = sizes.get(project);
  if (cached !== undefined) return cached;
  const size = JSON.stringify(project).length * 2;
  sizes.set(project, size);
  return size;
}

export function boundHistory(past: readonly VideoProject[], future: readonly VideoProject[], budget = historyByteBudget) {
  let retained = past.reduce((total, snapshot) => total + bytes(snapshot), 0)
    + future.reduce((total, snapshot) => total + bytes(snapshot), 0);
  let droppedPast = Math.min(past.length, Math.max(0, past.length + future.length - 100));
  for (let index = 0; index < droppedPast; index += 1) retained -= bytes(past[index]!);
  let futureEnd = Math.min(future.length, 100 - (past.length - droppedPast));
  for (let index = futureEnd; index < future.length; index += 1) retained -= bytes(future[index]!);
  // Discard the oldest undo state before the nearest undo/redo states. A single
  // oversize snapshot stays undoable; it is the explicit exception to the byte cap.
  while (retained > budget && past.length - droppedPast + futureEnd > 1) {
    if (past.length - droppedPast > 1) retained -= bytes(past[droppedPast++]!);
    else if (futureEnd > 0) retained -= bytes(future[--futureEnd]!);
    else break;
  }
  return { past: past.slice(droppedPast), future: future.slice(0, futureEnd), droppedPast };
}
