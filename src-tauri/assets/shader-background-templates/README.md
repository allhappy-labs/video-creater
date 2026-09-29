# Shader Background Templates

This folder is the file-backed catalog for GPU shader background templates.

- `builtin/<template-id>/template.json` contains agent- and UI-browsable metadata.
- `builtin/<template-id>/shader.frag` contains the template-specific Video Creater fragment entry.
- `builtin/shared/utils.glsl` contains reusable GLSL helpers shared by built-in ports.
- User templates can follow the same shape in a project-local template root and use `sourceKind: "user"`.

Built-in fragments expose `vec4 video_creater_fragment(vec2 uv, float time, float progress)` and call original Video Creater procedural variants in `shared/utils.glsl`. External visual references may remain in template metadata for review, but built-in GLSL should not be a direct source copy. Fragments must not require includes, custom bindings, declared external sampler uniforms, storage buffers, network resources, or canonical project mutation.
