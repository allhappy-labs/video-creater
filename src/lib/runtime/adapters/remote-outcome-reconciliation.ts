import { projectRevisionFromResult, stringFieldFromUnknown } from "./remote-transport-fields";
import type { ServerRequestOutcome } from "./remote-server-outcomes";
import type { RemoteOutcomeMarker } from "./remote-outcome-storage";

export function validateCanonicalReconciliation(snapshot: unknown, receipt: ServerRequestOutcome | undefined, marker: RemoteOutcomeMarker | undefined, expectedProjectId: string | undefined): void {
  const revision = projectRevisionFromResult(snapshot);
  if (revision === undefined || !Number.isSafeInteger(revision) || revision < 0) {
    throw new Error("The host did not return a valid canonical project revision.");
  }
  const canonicalProjectId = receipt?.canonicalProjectId ?? marker?.canonicalProjectId ?? expectedProjectId;
  if (canonicalProjectId !== undefined && stringFieldFromUnknown(snapshot, "id") !== canonicalProjectId || expectedProjectId !== undefined && stringFieldFromUnknown(snapshot, "id") !== expectedProjectId) {
    throw new Error("The host returned a different project. The edit is still unconfirmed.");
  }
  if (receipt?.contentRevision !== undefined && revision < receipt.contentRevision) throw new Error("The canonical project is older than the retained operation.");
}
