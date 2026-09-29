# MP4 Export Policy Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an unavailable-by-default Rust MP4 export policy report and use it to explain disabled MP4 export actions in the editor.

**Architecture:** Rust owns export profile policy state and exposes a deterministic, secret-free report through a Tauri command. TypeScript loads that report once in `EditorWorkspace` and renders MP4/H.264, MP4/H.265, and ProRes MOV disabled/enabled state from the report instead of hard-coded copy. This slice does not enable codecs or start MP4 exports.

**Tech Stack:** Rust/Tauri commands, Serde camelCase JSON, React TypeScript, Vitest, Cargo tests.

---

## File Structure

- Create: `src-tauri/src/project/export_profiles.rs`
  - Owns `ExportProfile`, `ExportPolicyStatus`, `ExportProfileAvailability`, and `mp4_export_profile_availability_report()`.
- Modify: `src-tauri/src/project/mod.rs`
  - Exposes the new `export_profiles` module.
- Modify: `src-tauri/src/main.rs`
  - Imports the policy report and registers `get_export_profile_availability_report`.
- Modify: `src/lib/project.ts`
  - Adds frontend export profile availability types and a Tauri wrapper.
- Modify: `src/components/workspace/editor-workspace.tsx`
  - Loads the report and renders MP4-family export menu actions from it.
- Modify: `src/components/workspace/editor-workspace.test.tsx`
  - Adds tests for Rust-provided disabled reasons.
- Test: `src-tauri/tests/export_profiles.rs`
  - Verifies policy-gated defaults and secret-free profile metadata.

## Task 1: Rust Policy Report

**Files:**
- Create: `src-tauri/tests/export_profiles.rs`
- Create: `src-tauri/src/project/export_profiles.rs`
- Modify: `src-tauri/src/project/mod.rs`

- [ ] **Step 1: Write the failing Rust tests**

Create `src-tauri/tests/export_profiles.rs`:

```rust
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportPolicyStatus, ExportProfile,
};

#[test]
fn mp4_export_profiles_are_policy_gated_by_default() {
    let report = mp4_export_profile_availability_report();

    assert_eq!(report.len(), 3);
    assert!(report.iter().all(|profile| !profile.available));
    assert!(report
        .iter()
        .all(|profile| profile.policy_status == ExportPolicyStatus::PolicyGated));
    assert!(report
        .iter()
        .all(|profile| profile.unavailable_reason.as_deref() == Some("Policy gated until encoder approval")));
}

#[test]
fn mp4_export_profiles_describe_container_codecs_and_runtime_without_secrets() {
    let report = mp4_export_profile_availability_report();

    let h264 = report
        .iter()
        .find(|profile| profile.profile == ExportProfile::Mp4H264)
        .expect("h264 profile");
    assert_eq!(h264.container, "mp4");
    assert_eq!(h264.video_codec, "h264");
    assert_eq!(h264.audio_codec.as_deref(), Some("aac"));
    assert_eq!(h264.extension, "mp4");
    assert_eq!(h264.mime_type, "video/mp4");

    let h265 = report
        .iter()
        .find(|profile| profile.profile == ExportProfile::Mp4H265)
        .expect("h265 profile");
    assert_eq!(h265.video_codec, "h265");

    let prores = report
        .iter()
        .find(|profile| profile.profile == ExportProfile::ProResMov)
        .expect("prores profile");
    assert_eq!(prores.container, "mov");
    assert_eq!(prores.video_codec, "prores");
    assert_eq!(prores.mime_type, "video/quicktime");

    let serialized = serde_json::to_string(&report).expect("serialized report");
    assert!(!serialized.contains("FAL"));
    assert!(!serialized.contains("KEY"));
    assert!(!serialized.contains("token"));
    assert!(!serialized.contains("secret"));
}
```

- [ ] **Step 2: Run the Rust test and verify RED**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test export_profiles
```

Expected: FAIL because `project::export_profiles` does not exist.

- [ ] **Step 3: Implement the minimal Rust module**

Create `src-tauri/src/project/export_profiles.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportProfile {
    Mp4H264,
    Mp4H265,
    ProResMov,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportPolicyStatus {
    Approved,
    MissingRuntime,
    PolicyGated,
    UnsupportedBuild,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfileAvailability {
    pub profile: ExportProfile,
    pub label: String,
    pub available: bool,
    pub container: String,
    pub extension: String,
    pub mime_type: String,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub required_runtime: Vec<String>,
    pub policy_status: ExportPolicyStatus,
    pub unavailable_reason: Option<String>,
}

pub fn mp4_export_profile_availability_report() -> Vec<ExportProfileAvailability> {
    vec![
        policy_gated_profile(
            ExportProfile::Mp4H264,
            "MP4 / H.264",
            "mp4",
            "mp4",
            "video/mp4",
            "h264",
            Some("aac"),
            &["approved-h264-encoder", "approved-aac-encoder"],
        ),
        policy_gated_profile(
            ExportProfile::Mp4H265,
            "MP4 / H.265",
            "mp4",
            "mp4",
            "video/mp4",
            "h265",
            Some("aac"),
            &["approved-h265-encoder", "approved-aac-encoder"],
        ),
        policy_gated_profile(
            ExportProfile::ProResMov,
            "ProRes MOV",
            "mov",
            "mov",
            "video/quicktime",
            "prores",
            Some("pcm_or_aac"),
            &["approved-prores-encoder", "approved-audio-encoder"],
        ),
    ]
}

fn policy_gated_profile(
    profile: ExportProfile,
    label: &str,
    container: &str,
    extension: &str,
    mime_type: &str,
    video_codec: &str,
    audio_codec: Option<&str>,
    required_runtime: &[&str],
) -> ExportProfileAvailability {
    ExportProfileAvailability {
        profile,
        label: label.to_string(),
        available: false,
        container: container.to_string(),
        extension: extension.to_string(),
        mime_type: mime_type.to_string(),
        video_codec: video_codec.to_string(),
        audio_codec: audio_codec.map(str::to_string),
        required_runtime: required_runtime.iter().map(|runtime| (*runtime).to_string()).collect(),
        policy_status: ExportPolicyStatus::PolicyGated,
        unavailable_reason: Some("Policy gated until encoder approval".to_string()),
    }
}
```

Modify `src-tauri/src/project/mod.rs`:

```rust
pub mod export_profiles;
```

- [ ] **Step 4: Run the Rust test and verify GREEN**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test export_profiles
```

Expected: PASS.

## Task 2: Tauri Command And Frontend Contract

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src/lib/project.ts`
- Modify: `src/lib/project.test.ts`

- [ ] **Step 1: Write failing frontend and Rust command tests**

In `src/lib/project.test.ts`, import `getExportProfileAvailabilityReport` and add:

```ts
it("calls the Rust export profile availability report command", async () => {
  const report = [
    {
      profile: "mp4H264",
      label: "MP4 / H.264",
      available: false,
      container: "mp4",
      extension: "mp4",
      mimeType: "video/mp4",
      videoCodec: "h264",
      audioCodec: "aac",
      requiredRuntime: ["approved-h264-encoder"],
      policyStatus: "policyGated",
      unavailableReason: "Policy gated until encoder approval",
    },
  ];
  vi.mocked(invoke).mockResolvedValue(report);

  const result = await getExportProfileAvailabilityReport();

  expect(invoke).toHaveBeenCalledWith("get_export_profile_availability_report");
  expect(result).toEqual(report);
});
```

In `src-tauri/src/main.rs` tests, add:

```rust
#[test]
fn export_profile_availability_report_command_keeps_mp4_policy_gated() {
    let report = get_export_profile_availability_report();

    assert_eq!(report.len(), 3);
    assert!(report.iter().all(|profile| !profile.available));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
rtk pnpm vitest run src/lib/project.test.ts
rtk cargo test --manifest-path src-tauri/Cargo.toml export_profile_availability_report_command_keeps_mp4_policy_gated
```

Expected: FAIL because the wrapper and command do not exist.

- [ ] **Step 3: Add command and TypeScript wrapper**

Modify `src-tauri/src/main.rs` imports:

```rust
use video_creater_lib::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfileAvailability,
};
```

Add command:

```rust
#[tauri::command]
fn get_export_profile_availability_report() -> Vec<ExportProfileAvailability> {
    mp4_export_profile_availability_report()
}
```

Register it in `tauri::generate_handler!`.

Modify `src/lib/project.ts`:

```ts
export type ExportProfile = "mp4H264" | "mp4H265" | "proResMov";
export type ExportPolicyStatus =
  | "approved"
  | "missingRuntime"
  | "policyGated"
  | "unsupportedBuild";

export interface ExportProfileAvailability {
  profile: ExportProfile;
  label: string;
  available: boolean;
  container: string;
  extension: string;
  mimeType: string;
  videoCodec: string;
  audioCodec?: string | null;
  requiredRuntime: string[];
  policyStatus: ExportPolicyStatus;
  unavailableReason?: string | null;
}

export function getExportProfileAvailabilityReport(): Promise<ExportProfileAvailability[]> {
  return invoke("get_export_profile_availability_report");
}
```

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```bash
rtk pnpm vitest run src/lib/project.test.ts
rtk cargo test --manifest-path src-tauri/Cargo.toml export_profile_availability_report_command_keeps_mp4_policy_gated
```

Expected: PASS.

## Task 3: Export Menu Uses Rust Policy Reasons

**Files:**
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`

- [ ] **Step 1: Write failing UI tests**

Update the existing export menu test to expect the Rust-provided reason:

```ts
expect(exportMenu).toHaveTextContent("Policy gated by Rust export profile report");
```

Override `invokeMock` in that test:

```ts
const defaultInvokeImplementation = invokeMock.getMockImplementation();
invokeMock.mockImplementation((command: string, input: unknown) => {
  if (command === "get_export_profile_availability_report") {
    return Promise.resolve([
      {
        profile: "mp4H264",
        label: "MP4 / H.264",
        available: false,
        container: "mp4",
        extension: "mp4",
        mimeType: "video/mp4",
        videoCodec: "h264",
        audioCodec: "aac",
        requiredRuntime: ["approved-h264-encoder"],
        policyStatus: "policyGated",
        unavailableReason: "Policy gated by Rust export profile report",
      },
      {
        profile: "mp4H265",
        label: "MP4 / H.265",
        available: false,
        container: "mp4",
        extension: "mp4",
        mimeType: "video/mp4",
        videoCodec: "h265",
        audioCodec: "aac",
        requiredRuntime: ["approved-h265-encoder"],
        policyStatus: "policyGated",
        unavailableReason: "Policy gated by Rust export profile report",
      },
      {
        profile: "proResMov",
        label: "ProRes MOV",
        available: false,
        container: "mov",
        extension: "mov",
        mimeType: "video/quicktime",
        videoCodec: "prores",
        audioCodec: "pcm_or_aac",
        requiredRuntime: ["approved-prores-encoder"],
        policyStatus: "policyGated",
        unavailableReason: "Policy gated by Rust export profile report",
      },
    ]);
  }
  return defaultInvokeImplementation?.(command, input);
});
```

- [ ] **Step 2: Run UI test and verify RED**

Run:

```bash
rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because the export menu still uses hard-coded MP4 copy.

- [ ] **Step 3: Implement frontend loading and rendering**

In `EditorWorkspace`, import and load `getExportProfileAvailabilityReport`, store the report in
state, and render the three MP4 buttons from the report:

```tsx
const [exportProfileAvailability, setExportProfileAvailability] = useState<
  ExportProfileAvailability[]
>([]);

useEffect(() => {
  let cancelled = false;
  getExportProfileAvailabilityReport()
    .then((report) => {
      if (!cancelled) {
        setExportProfileAvailability(Array.isArray(report) ? report : []);
      }
    })
    .catch(() => {
      if (!cancelled) {
        setExportProfileAvailability([]);
      }
    });
  return () => {
    cancelled = true;
  };
}, []);
```

Use a fallback array for missing reports so MP4 remains disabled.

- [ ] **Step 4: Run UI test and verify GREEN**

Run:

```bash
rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx
```

Expected: PASS.

## Task 4: Full Verification And Commit

**Files:**
- All changed files.

- [ ] **Step 1: Run frontend verification**

```bash
rtk pnpm test
rtk pnpm lint
rtk pnpm build
```

Expected: all commands exit 0. Existing React `act(...)` warnings may appear in workspace tests.

- [ ] **Step 2: Run Rust verification**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

Expected: exits 0.

- [ ] **Step 3: Review diff and scan for secrets**

```bash
rtk rg -n "FAL_KEY=.*[A-Za-z0-9:_-]{20,}|api[_ -]?key.*[A-Za-z0-9:_-]{20,}|secret.*[A-Za-z0-9:_-]{20,}" src src-tauri docs
rtk git diff --check
rtk git status --short
```

Expected: no secret value matches; diff check exits 0.

- [ ] **Step 4: Commit**

```bash
rtk git add src-tauri/src/project/export_profiles.rs src-tauri/src/project/mod.rs src-tauri/src/main.rs src-tauri/tests/export_profiles.rs src/lib/project.ts src/lib/project.test.ts src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx docs/superpowers/plans/2026-06-24-mp4-export-policy-report.md
rtk git commit -m "feat: surface mp4 export policy report"
```

Expected: commit succeeds on the current `codex/palmier-agent-editor` branch.

## Self-Review

- The plan implements Slice 1 from `2026-06-24-mp4-export-policy-temporal-design.md`.
- It keeps MP4-family exports disabled by default.
- It adds a Rust policy report before changing UI behavior.
- It does not add any encoder runtime, Temporal MP4 start path, or credential handling.
- It includes RED/GREEN test steps before production edits.
