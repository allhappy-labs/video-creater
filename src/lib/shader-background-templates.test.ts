import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  createShaderBackgroundTemplateItem,
  getShaderBackgroundTemplate,
  loadShaderBackgroundTemplates,
  shaderBackgroundTemplateCatalog,
  type ShaderBackgroundTemplateDefinition,
} from "./shader-background-templates";
import { requiredAt } from "../test-utils/required";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: invokeMock,
}));

describe("shader background template catalog", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("ships all collected ShaderToy backgrounds as built-in catalog entries", () => {
    expect(shaderBackgroundTemplateCatalog).toHaveLength(17);
    expect(shaderBackgroundTemplateCatalog.map((template) => template.id)).toEqual([
      "shadertoy-octagrams-v1",
      "shadertoy-base-warp-fbm-v1",
      "shadertoy-phantom-star-v1",
      "shadertoy-volumetric-clouds-v1",
      "shadertoy-cube-lines-v1",
      "shadertoy-crumpled-wave-v1",
      "shadertoy-glowing-marbling-black-v1",
      "shadertoy-geodesic-tiling-v1",
      "shadertoy-kaleidoscope-tunnel-v1",
      "shadertoy-digital-brain-v1",
      "shadertoy-starry-planes-v1",
      "shadertoy-mandelbulb-interior-v1",
      "shadertoy-tiny-clouds-v1",
      "shadertoy-neon-lit-hexagons-v1",
      "shadertoy-another-cube-v1",
      "shadertoy-object-mesh-v1",
      "shadertoy-looping-clouds-v1",
    ]);

    for (const template of shaderBackgroundTemplateCatalog) {
      expect(template.sourceKind).toBe("built_in");
      expect(template.placement.trackKind).toBe("hyperframe_scene");
      expect(template.renderContract).toEqual({
        dimensions: "project",
        fps: "project",
        alpha: false,
      });
      expect(template.configPath).toContain(`builtin/${template.id}/template.json`);
      expect(template.shaderPath).toContain(`builtin/${template.id}/shader.frag`);
      expect(template.utilityRefs).toContain("shared/utils.glsl");
      expect(template.agentSummary).toContain(template.id);
      expect(template.visualTreatment.length).toBeGreaterThan(20);
      expect(template.motion.length).toBeGreaterThan(20);
      expect(template.safeZone).toContain("10%");
      expect(template.avoid).toContain("strobing");
    }
  });

  it("looks up shader backgrounds by id", () => {
    expect(getShaderBackgroundTemplate("shadertoy-octagrams-v1")?.name).toBe("Octagrams");
    expect(getShaderBackgroundTemplate("missing-template")).toBeNull();
  });

  it("creates canonical HyperFrame timeline items for shader backgrounds", () => {
    const item = createShaderBackgroundTemplateItem({
      templateId: "shadertoy-octagrams-v1",
      itemId: "shader-bg-1",
      startSeconds: 1.5,
    });

    expect(item).toMatchObject({
      id: "shader-bg-1",
      kind: "hyperframe_scene",
      startSeconds: 1.5,
      durationSeconds: 4,
      source: {
        type: "generated",
        artifactId: "shader-background:shadertoy-octagrams-v1:shader-bg-1",
      },
      label: "Octagrams",
      properties: {
        shaderBackgroundTemplateId: "shadertoy-octagrams-v1",
        qualityProfile: "shadertoy-octagrams-v1",
      },
    });
  });

  it("loads the combined shader catalog from the Rust command", async () => {
    const userTemplate: ShaderBackgroundTemplateDefinition = {
      ...requiredAt(shaderBackgroundTemplateCatalog, 0, "built-in shader template"),
      id: "user-neon-v1",
      name: "User Neon",
      sourceKind: "user",
      category: "user",
      durationSeconds: 3,
      shaderProfileId: "user-neon-v1",
      configPath: "/tmp/project/shader-background-templates/user-neon-v1/template.json",
      shaderPath: "/tmp/project/shader-background-templates/user-neon-v1/shader.frag",
    };
    invokeMock.mockResolvedValue([userTemplate]);

    await expect(
      loadShaderBackgroundTemplates({ projectDir: "/tmp/project" }),
    ).resolves.toEqual([userTemplate]);

    expect(invokeMock).toHaveBeenCalledWith("list_shader_background_templates", {
      projectDir: "/tmp/project",
    });
  });

  it("creates HyperFrame items from a supplied user template catalog", () => {
    const userTemplate: ShaderBackgroundTemplateDefinition = {
      ...requiredAt(shaderBackgroundTemplateCatalog, 0, "built-in shader template"),
      id: "user-neon-v1",
      name: "User Neon",
      sourceKind: "user",
      category: "user",
      durationSeconds: 3,
      shaderProfileId: "user-neon-v1",
      configPath: "/tmp/project/shader-background-templates/user-neon-v1/template.json",
      shaderPath: "/tmp/project/shader-background-templates/user-neon-v1/shader.frag",
    };

    const item = createShaderBackgroundTemplateItem({
      templateId: "user-neon-v1",
      itemId: "shader-user-1",
      startSeconds: 0,
      templates: [userTemplate],
    });

    expect(item.label).toBe("User Neon");
    expect(item.durationSeconds).toBe(3);
    expect(item.properties).toMatchObject({
      shaderBackgroundTemplateId: "user-neon-v1",
      qualityProfile: "user-neon-v1",
      sourceKind: "user",
    });
  });
});
