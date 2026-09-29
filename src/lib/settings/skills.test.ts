import { beforeEach, describe, expect, it, vi } from "vitest";

import { getSkillsHealth, repairBundledSkills } from "./skills";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: invokeMock }));

describe("Skills settings Tauri adapters", () => {
  beforeEach(() => invokeMock.mockReset());

  it("passes an explicit null root without falling back to CWD", async () => {
    invokeMock.mockResolvedValue({ id: "skills", state: "unavailable", items: [] });
    await getSkillsHealth(null);
    expect(invokeMock).toHaveBeenCalledWith("get_skills_settings_health", {
      projectRoot: null,
    });
  });

  it("sends exact affected paths only on the confirmed repair call", async () => {
    const paths = ["/projects/current/.agents/skills/video-creater-graphics/SKILL.md"];
    invokeMock.mockResolvedValue({ preview: { skillIds: [], affectedPaths: paths } });
    await repairBundledSkills(
      "/projects/current",
      ["video-creater-graphics"],
      paths,
    );
    expect(invokeMock).toHaveBeenCalledWith("repair_bundled_skills", {
      projectRoot: "/projects/current",
      skillIds: ["video-creater-graphics"],
      confirmedPaths: paths,
    });
  });
});
