import { beforeEach, describe, expect, it, vi } from "vitest";
import { importTranscriptionModel } from "./transcription-models";

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: vi.fn(),
}));

describe("transcription model command adapters", () => {
  beforeEach(async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockReset();
  });

  it("calls the Rust model import command with model id and source path", async () => {
    const { backendRequest: invoke } = await import("@/lib/runtime/backend-client");
    vi.mocked(invoke).mockResolvedValue({
      modelId: "nvidia/parakeet-tdt-0.6b-v3",
      displayName: "Parakeet TDT 0.6B v3",
      isActive: true,
      installStatus: "ready",
      localPath: "/tmp/models/nvidia__parakeet-tdt-0.6b-v3",
      approximateSizeBytes: 485_000_000,
      downloadedFiles: 18,
      totalFiles: 18,
    });

    await importTranscriptionModel(
      "nvidia/parakeet-tdt-0.6b-v3",
      "/tmp/source/parakeet",
    );

    expect(invoke).toHaveBeenCalledWith("import_transcription_model", {
      modelId: "nvidia/parakeet-tdt-0.6b-v3",
      sourcePath: "/tmp/source/parakeet",
    });
  });
});
