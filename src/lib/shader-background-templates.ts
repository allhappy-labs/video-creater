import { backendRequest } from "@/lib/runtime/backend-client";
import type { TimelineItem, TrackKind } from "./timeline";

type ShaderBackgroundTemplateSourceKind = "built_in" | "user";
type ShaderBackgroundTemplateCategory =
  | "procedural"
  | "volumetric"
  | "geometry"
  | "tunnel"
  | "fractal"
  | "clouds"
  | "user";

export interface ShaderBackgroundTemplateDefinition {
  id: string;
  version: number;
  name: string;
  sourceKind: ShaderBackgroundTemplateSourceKind;
  category: ShaderBackgroundTemplateCategory;
  durationSeconds: number;
  shaderProfileId: string;
  configPath: string;
  shaderPath: string;
  utilityRefs: string[];
  sourceUrl: string | null;
  license: string;
  placement: {
    trackKind: TrackKind;
    defaultStartSeconds: number;
  };
  renderContract: {
    dimensions: "project";
    fps: "project";
    alpha: boolean;
  };
  preview: {
    thumbnailKind: "css";
    accentColor: string;
    secondaryColor: string;
    description: string;
  };
  visualTreatment: string;
  motion: string;
  safeZone: string;
  avoid: string;
  agentSummary: string;
}

export interface CreateShaderBackgroundTemplateItemInput {
  templateId: string;
  itemId: string;
  startSeconds: number;
  templates?: ShaderBackgroundTemplateDefinition[];
}

export const shaderBackgroundTemplateCatalog = [
  {
    "id": "shadertoy-octagrams-v1",
    "version": 1,
    "name": "Octagrams",
    "sourceKind": "built_in",
    "category": "procedural",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-octagrams-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-octagrams-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-octagrams-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/tlVGDt",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#38bdf8",
      "secondaryColor": "#10b981",
      "description": "recursive octagram box tunnel with blue-green volumetric glow and centered depth"
    },
    "visualTreatment": "recursive octagram box tunnel with blue-green volumetric glow and centered depth",
    "motion": "forward raymarch drift with slow rotation and breathing repeated geometry",
    "safeZone": "keep the brightest octagram forms inside the central 80% and preserve 10% edge margins",
    "avoid": "strobing, opaque caption slabs, flat static boxes, and unsafe high-frequency shimmer",
    "agentSummary": "shadertoy-octagrams-v1: Octagrams. kind shader_background. recursive octagram box tunnel with blue-green volumetric glow and centered depth"
  },
  {
    "id": "shadertoy-base-warp-fbm-v1",
    "version": 1,
    "name": "Base warp fBM",
    "sourceKind": "built_in",
    "category": "procedural",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-base-warp-fbm-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-base-warp-fbm-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-base-warp-fbm-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/tdG3Rd",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#f472b6",
      "secondaryColor": "#38bdf8",
      "description": "pink and indigo domain-warped fBM field with soft smoke-like structure"
    },
    "visualTreatment": "pink and indigo domain-warped fBM field with soft smoke-like structure",
    "motion": "slow nested noise drift with palette breathing and gentle vertical flow",
    "safeZone": "keep center contrast soft enough for titles and maintain 10% margins",
    "avoid": "strobing, harsh posterization, noisy pixel crawl, and full-screen white flashes",
    "agentSummary": "shadertoy-base-warp-fbm-v1: Base warp fBM. kind shader_background. pink and indigo domain-warped fBM field with soft smoke-like structure"
  },
  {
    "id": "shadertoy-phantom-star-v1",
    "version": 1,
    "name": "Phantom Star",
    "sourceKind": "built_in",
    "category": "geometry",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-phantom-star-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-phantom-star-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-phantom-star-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/ttKGDt",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#67e8f9",
      "secondaryColor": "#64748b",
      "description": "ghostly fivefold star lattice with pale cyan volumetric trails"
    },
    "visualTreatment": "ghostly fivefold star lattice with pale cyan volumetric trails",
    "motion": "camera flies through repeating mirrored star boxes with periodic light pulses",
    "safeZone": "keep star convergence near center and leave 10% margins free of bright spikes",
    "avoid": "strobing, hard white flashes, static text cards, and dense unreadable noise",
    "agentSummary": "shadertoy-phantom-star-v1: Phantom Star. kind shader_background. ghostly fivefold star lattice with pale cyan volumetric trails"
  },
  {
    "id": "shadertoy-volumetric-clouds-v1",
    "version": 1,
    "name": "Clouds",
    "sourceKind": "built_in",
    "category": "clouds",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-volumetric-clouds-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-volumetric-clouds-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-volumetric-clouds-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/XslGRr",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#fbbf24",
      "secondaryColor": "#93c5fd",
      "description": "procedural volumetric cloud slab with warm sky lighting and soft depth"
    },
    "visualTreatment": "procedural volumetric cloud slab with warm sky lighting and soft depth",
    "motion": "slow cloud drift with subtle camera roll and non-looping atmospheric parallax",
    "safeZone": "keep the middle 80% low contrast for overlay readability and preserve 10% margins",
    "avoid": "strobing, black slabs, blown-out white sky, and high-frequency texture crawl",
    "agentSummary": "shadertoy-volumetric-clouds-v1: Clouds. kind shader_background. procedural volumetric cloud slab with warm sky lighting and soft depth"
  },
  {
    "id": "shadertoy-cube-lines-v1",
    "version": 1,
    "name": "Cube lines",
    "sourceKind": "built_in",
    "category": "geometry",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-cube-lines-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-cube-lines-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-cube-lines-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/NslGRN",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#22d3ee",
      "secondaryColor": "#fb923c",
      "description": "transparent glass cube impression filled with animated internal line lattices"
    },
    "visualTreatment": "transparent glass cube impression filled with animated internal line lattices",
    "motion": "slow cube rotation with internal contour lines sliding and refractive edge glow",
    "safeZone": "keep the cube inside the central 80% and avoid critical text within 10% margins",
    "avoid": "strobing, full opaque boxes, plain wireframe-only holds, and texture-dependent noise",
    "agentSummary": "shadertoy-cube-lines-v1: Cube lines. kind shader_background. transparent glass cube impression filled with animated internal line lattices"
  },
  {
    "id": "shadertoy-crumpled-wave-v1",
    "version": 1,
    "name": "crumpledWave",
    "sourceKind": "built_in",
    "category": "procedural",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-crumpled-wave-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-crumpled-wave-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-crumpled-wave-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/3ttSzr",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#38bdf8",
      "secondaryColor": "#1d4ed8",
      "description": "crumpled cyan wave sheet with smooth layered interference bands"
    },
    "visualTreatment": "crumpled cyan wave sheet with smooth layered interference bands",
    "motion": "overlapping sine folds travel diagonally with subtle noise displacement",
    "safeZone": "keep the wave gradient readable under titles and preserve 10% margins",
    "avoid": "strobing, clipped saturated channels, static flat gradients, and tiny pattern shimmer",
    "agentSummary": "shadertoy-crumpled-wave-v1: crumpledWave. kind shader_background. crumpled cyan wave sheet with smooth layered interference bands"
  },
  {
    "id": "shadertoy-glowing-marbling-black-v1",
    "version": 1,
    "name": "glowingMarblingBlack",
    "sourceKind": "built_in",
    "category": "procedural",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-glowing-marbling-black-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-glowing-marbling-black-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-glowing-marbling-black-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/WtdXR8",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#f8fafc",
      "secondaryColor": "#475569",
      "description": "black marbling field with thin luminous veins and restrained monochrome glow"
    },
    "visualTreatment": "black marbling field with thin luminous veins and restrained monochrome glow",
    "motion": "slow fluid advection with breathing vein brightness and no hard flashes",
    "safeZone": "keep central veins below caption dominance and preserve 10% margins",
    "avoid": "strobing, blown-out white vein slabs, muddy low-contrast fields, and static holds",
    "agentSummary": "shadertoy-glowing-marbling-black-v1: glowingMarblingBlack. kind shader_background. black marbling field with thin luminous veins and restrained monochrome glow"
  },
  {
    "id": "shadertoy-geodesic-tiling-v1",
    "version": 1,
    "name": "Geodesic tiling",
    "sourceKind": "built_in",
    "category": "geometry",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-geodesic-tiling-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-geodesic-tiling-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-geodesic-tiling-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/llVXRd",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#a78bfa",
      "secondaryColor": "#60a5fa",
      "description": "lit spherical geodesic tile shell with spectral seams and dark space background"
    },
    "visualTreatment": "lit spherical geodesic tile shell with spectral seams and dark space background",
    "motion": "slow orbital rotation with tile height breathing and animated seam highlights",
    "safeZone": "keep sphere and bright seams inside the central 80% and leave 10% margins clear",
    "avoid": "strobing, jagged aliasing, full-screen white flashes, and flat static sphere renders",
    "agentSummary": "shadertoy-geodesic-tiling-v1: Geodesic tiling. kind shader_background. lit spherical geodesic tile shell with spectral seams and dark space background"
  },
  {
    "id": "shadertoy-kaleidoscope-tunnel-v1",
    "version": 1,
    "name": "Kaleidoscope Tunnel",
    "sourceKind": "built_in",
    "category": "tunnel",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-kaleidoscope-tunnel-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-kaleidoscope-tunnel-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-kaleidoscope-tunnel-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/WsSGWG",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#8b5cf6",
      "secondaryColor": "#38bdf8",
      "description": "radial kaleidoscope tunnel with layered dust, rectangular motifs, and violet-blue depth"
    },
    "visualTreatment": "radial kaleidoscope tunnel with layered dust, rectangular motifs, and violet-blue depth",
    "motion": "forward tunnel pulses with folded rotational symmetry and staggered light layers",
    "safeZone": "keep the vanishing point near center and reserve 10% margins from hard flashes",
    "avoid": "strobing, nausea-inducing rapid cuts, hard white centers, and unreadable high-frequency grids",
    "agentSummary": "shadertoy-kaleidoscope-tunnel-v1: Kaleidoscope Tunnel. kind shader_background. radial kaleidoscope tunnel with layered dust, rectangular motifs, and violet-blue depth"
  },
  {
    "id": "shadertoy-digital-brain-v1",
    "version": 1,
    "name": "Digital Brain",
    "sourceKind": "built_in",
    "category": "procedural",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-digital-brain-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-digital-brain-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-digital-brain-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/4sl3Dr",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#60a5fa",
      "secondaryColor": "#f97316",
      "description": "blue-orange Voronoi circuit field with moving electron glints and dark vignette"
    },
    "visualTreatment": "blue-orange Voronoi circuit field with moving electron glints and dark vignette",
    "motion": "slow rotational drift with flickering node paths and layered cell movement",
    "safeZone": "keep bright circuit clusters away from lower captions and preserve 10% margins",
    "avoid": "strobing, unreadable tiny circuit crawl, flat noise maps, and texture dependencies",
    "agentSummary": "shadertoy-digital-brain-v1: Digital Brain. kind shader_background. blue-orange Voronoi circuit field with moving electron glints and dark vignette"
  },
  {
    "id": "shadertoy-starry-planes-v1",
    "version": 1,
    "name": "Starry planes",
    "sourceKind": "built_in",
    "category": "tunnel",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-starry-planes-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-starry-planes-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-starry-planes-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/MfjyWK",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#f0abfc",
      "secondaryColor": "#60a5fa",
      "description": "stacked translucent star planes flying through a dark curved path"
    },
    "visualTreatment": "stacked translucent star planes flying through a dark curved path",
    "motion": "forward plane march with star bursts fading in depth and gentle camera drift",
    "safeZone": "keep brightest stars inside central 80% while preserving 10% edge margins",
    "avoid": "strobing, dense star clutter over captions, hard aliasing, and static star wallpaper",
    "agentSummary": "shadertoy-starry-planes-v1: Starry planes. kind shader_background. stacked translucent star planes flying through a dark curved path"
  },
  {
    "id": "shadertoy-mandelbulb-interior-v1",
    "version": 1,
    "name": "Inside the mandelbulb",
    "sourceKind": "built_in",
    "category": "fractal",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-mandelbulb-interior-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-mandelbulb-interior-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-mandelbulb-interior-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/mtScRc",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#fb923c",
      "secondaryColor": "#2563eb",
      "description": "refractive mandelbulb interior with amber highlights and deep blue atmosphere"
    },
    "visualTreatment": "refractive mandelbulb interior with amber highlights and deep blue atmosphere",
    "motion": "slow fractal rotation with camera hovering inside reflective crystalline folds",
    "safeZone": "keep fractal detail inside the central 80% and reserve 10% margins for overlays",
    "avoid": "strobing, excessively sharp aliasing, black unreadable interiors, and texture feedback passes",
    "agentSummary": "shadertoy-mandelbulb-interior-v1: Inside the mandelbulb. kind shader_background. refractive mandelbulb interior with amber highlights and deep blue atmosphere"
  },
  {
    "id": "shadertoy-tiny-clouds-v1",
    "version": 1,
    "name": "Tiny Clouds",
    "sourceKind": "built_in",
    "category": "clouds",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-tiny-clouds-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-tiny-clouds-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-tiny-clouds-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/lsBfDz",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#93c5fd",
      "secondaryColor": "#fbbf24",
      "description": "compact stylized cloud volume with powder blue gradients and soft marching haze"
    },
    "visualTreatment": "compact stylized cloud volume with powder blue gradients and soft marching haze",
    "motion": "short forward cloud drift with procedural billows and subtle luminance shimmer",
    "safeZone": "keep center readable for text and preserve 10% margins around dense cloud ridges",
    "avoid": "strobing, overbright fog walls, texture-only noise, and flat static cloud slabs",
    "agentSummary": "shadertoy-tiny-clouds-v1: Tiny Clouds. kind shader_background. compact stylized cloud volume with powder blue gradients and soft marching haze"
  },
  {
    "id": "shadertoy-neon-lit-hexagons-v1",
    "version": 1,
    "name": "Neon Lit Hexagons",
    "sourceKind": "built_in",
    "category": "geometry",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-neon-lit-hexagons-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-neon-lit-hexagons-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-neon-lit-hexagons-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/MsVfz1",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#f97316",
      "secondaryColor": "#38bdf8",
      "description": "raymarched hex pylon floor with intermittent neon light bands and glossy dark material"
    },
    "visualTreatment": "raymarched hex pylon floor with intermittent neon light bands and glossy dark material",
    "motion": "low camera glide through a tunnel of pulsing hex columns and volumetric glow",
    "safeZone": "keep glow bands below lower captions and leave 10% margins clear of hard highlights",
    "avoid": "strobing, overly saturated blobs, texture-dependent surfaces, and static grid wallpaper",
    "agentSummary": "shadertoy-neon-lit-hexagons-v1: Neon Lit Hexagons. kind shader_background. raymarched hex pylon floor with intermittent neon light bands and glossy dark material"
  },
  {
    "id": "shadertoy-another-cube-v1",
    "version": 1,
    "name": "another cube",
    "sourceKind": "built_in",
    "category": "geometry",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-another-cube-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-another-cube-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-another-cube-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/3XdXRr",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#60a5fa",
      "secondaryColor": "#0f172a",
      "description": "rounded reflective cube with electric blue rim glow and procedural floor reflection"
    },
    "visualTreatment": "rounded reflective cube with electric blue rim glow and procedural floor reflection",
    "motion": "slow tumbling cube rotation with shell glow and moving reflected floor bands",
    "safeZone": "keep the cube inside the central 80% and preserve 10% edge margins",
    "avoid": "strobing, plain filled cube holds, blown highlights, and texture-dependent reflections",
    "agentSummary": "shadertoy-another-cube-v1: another cube. kind shader_background. rounded reflective cube with electric blue rim glow and procedural floor reflection"
  },
  {
    "id": "shadertoy-object-mesh-v1",
    "version": 1,
    "name": "Object mesh",
    "sourceKind": "built_in",
    "category": "geometry",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-object-mesh-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-object-mesh-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-object-mesh-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/4slGzn",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#cbd5e1",
      "secondaryColor": "#f59e0b",
      "description": "procedural grunge-metal mesh object with wire seams and shaded surface ripples"
    },
    "visualTreatment": "procedural grunge-metal mesh object with wire seams and shaded surface ripples",
    "motion": "slow mesh rotation with animated surface waves and sparse wire glints",
    "safeZone": "keep mesh silhouette inside central 80% and preserve 10% margins from bright wires",
    "avoid": "strobing, copied texture assets, flat gray meshes, and unreadable high-density wire noise",
    "agentSummary": "shadertoy-object-mesh-v1: Object mesh. kind shader_background. procedural grunge-metal mesh object with wire seams and shaded surface ripples"
  },
  {
    "id": "shadertoy-looping-clouds-v1",
    "version": 1,
    "name": "looping clouds",
    "sourceKind": "built_in",
    "category": "clouds",
    "durationSeconds": 4.0,
    "shaderProfileId": "shadertoy-looping-clouds-v1",
    "configPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-looping-clouds-v1/template.json",
    "shaderPath": "src-tauri/assets/shader-background-templates/builtin/shadertoy-looping-clouds-v1/shader.frag",
    "utilityRefs": [
      "shared/utils.glsl"
    ],
    "sourceUrl": "https://www.shadertoy.com/view/33cGDj",
    "license": "Video Creater original procedural variant; ShaderToy URL retained as visual reference",
    "placement": {
      "trackKind": "hyperframe_scene",
      "defaultStartSeconds": 0.0
    },
    "renderContract": {
      "dimensions": "project",
      "fps": "project",
      "alpha": false
    },
    "preview": {
      "thumbnailKind": "css",
      "accentColor": "#a78bfa",
      "secondaryColor": "#22d3ee",
      "description": "looping abstract cloud tunnel with colorful repeated line and plane forms"
    },
    "visualTreatment": "looping abstract cloud tunnel with colorful repeated line and plane forms",
    "motion": "seamless forward march through repeated soft geometry with palette cycling",
    "safeZone": "keep brightest cloud lines inside central 80% and reserve 10% margins for overlays",
    "avoid": "strobing, hard white flashes, excessive particle noise, and static flat cloud cards",
    "agentSummary": "shadertoy-looping-clouds-v1: looping clouds. kind shader_background. looping abstract cloud tunnel with colorful repeated line and plane forms"
  }
] satisfies ShaderBackgroundTemplateDefinition[];

export function getShaderBackgroundTemplate(
  templateId: string,
  templates: ShaderBackgroundTemplateDefinition[] = shaderBackgroundTemplateCatalog,
): ShaderBackgroundTemplateDefinition | null {
  return templates.find((template) => template.id === templateId) ?? null;
}

export async function loadShaderBackgroundTemplates(input: {
  projectDir?: string | null;
} = {}): Promise<ShaderBackgroundTemplateDefinition[]> {
  return backendRequest("list_shader_background_templates", {
    projectDir: input.projectDir ?? null,
  });
}

export function createShaderBackgroundTemplateItem(
  input: CreateShaderBackgroundTemplateItemInput,
): TimelineItem {
  const template = getShaderBackgroundTemplate(input.templateId, input.templates);
  if (!template) {
    throw new Error(`Unknown shader background template: ${input.templateId}`);
  }
  if (!Number.isFinite(input.startSeconds) || input.startSeconds < 0) {
    throw new Error("Shader background startSeconds must be a finite non-negative number");
  }

  return {
    id: input.itemId,
    kind: "hyperframe_scene",
    startSeconds: input.startSeconds,
    durationSeconds: template.durationSeconds,
    source: {
      type: "generated",
      artifactId: `shader-background:${template.id}:${input.itemId}`,
    },
    label: template.name,
    properties: {
      shaderBackgroundTemplateId: template.id,
      qualityProfile: template.shaderProfileId,
      sourceKind: template.sourceKind,
      configPath: template.configPath,
      shaderPath: template.shaderPath,
      utilityRefs: template.utilityRefs,
      visualTreatment: template.visualTreatment,
      motion: template.motion,
      safeZone: template.safeZone,
      avoid: template.avoid,
      renderContract: template.renderContract,
    },
  };
}
