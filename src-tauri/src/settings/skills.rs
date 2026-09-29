use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub relative_path: &'static str,
    pub bundled_content: &'static str,
}

pub const MANDATORY_SKILLS: [SkillDefinition; 3] = [
    SkillDefinition {
        id: "video-creater-video-pipeline",
        label: "Edit planning and render pipeline",
        relative_path: ".agents/skills/video-creater-video-pipeline/SKILL.md",
        bundled_content: include_str!(
            "../../../.agents/skills/video-creater-video-pipeline/SKILL.md"
        ),
    },
    SkillDefinition {
        id: "video-creater-graphics",
        label: "Video graphics and overlays",
        relative_path: ".agents/skills/video-creater-graphics/SKILL.md",
        bundled_content: include_str!("../../../.agents/skills/video-creater-graphics/SKILL.md"),
    },
    SkillDefinition {
        id: "video-creater-visuals",
        label: "Editor interface and visual QA",
        relative_path: ".agents/skills/video-creater-visuals/SKILL.md",
        bundled_content: include_str!("../../../.agents/skills/video-creater-visuals/SKILL.md"),
    },
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillLoadState {
    Missing,
    MatchesBundled,
    Differs,
    Unreadable,
}

impl SkillLoadState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::MatchesBundled => "matchesBundled",
            Self::Differs => "differs",
            Self::Unreadable => "unreadable",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillVerification {
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    pub checksum: Option<String>,
    pub bundled_checksum: String,
    pub load_state: SkillLoadState,
    pub prompt_included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillRepairConfirmation {
    pub affected_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillRepairPreview {
    pub skill_ids: Vec<String>,
    pub affected_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillRepairReport {
    pub repaired_paths: Vec<PathBuf>,
    pub backup_paths: Vec<PathBuf>,
    pub verification: Vec<SkillVerification>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "result", rename_all = "camelCase")]
pub enum SkillRepairOutcome {
    ConfirmationRequired(SkillRepairPreview),
    Repaired(SkillRepairReport),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillRepairError {
    #[error("unknown mandatory skill id: {0}")]
    UnknownSkillId(String),
    #[error("failed to access {path}: {kind}")]
    Io {
        path: String,
        kind: std::io::ErrorKind,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillRootError {
    #[error("a project repository root was not provided")]
    Missing,
    #[error("project repository root must be absolute: {0}")]
    NotAbsolute(String),
    #[error("project repository root cannot contain parent traversal: {0}")]
    ParentTraversal(String),
    #[error("failed to canonicalize project repository root {path}: {kind}")]
    Canonicalize {
        path: String,
        kind: std::io::ErrorKind,
    },
    #[error("project repository root does not contain AGENTS.md: {path}")]
    NotRepository { path: String },
}

pub fn validate_skill_root(configured_root: Option<&Path>) -> Result<PathBuf, SkillRootError> {
    let root = configured_root.ok_or(SkillRootError::Missing)?;
    if !root.is_absolute() {
        return Err(SkillRootError::NotAbsolute(root.display().to_string()));
    }
    if root
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(SkillRootError::ParentTraversal(root.display().to_string()));
    }
    let canonical = root
        .canonicalize()
        .map_err(|error| SkillRootError::Canonicalize {
            path: root.display().to_string(),
            kind: error.kind(),
        })?;
    if !canonical.join("AGENTS.md").is_file() {
        return Err(SkillRootError::NotRepository {
            path: canonical.display().to_string(),
        });
    }
    Ok(canonical)
}

pub fn mandatory_skill_definition(id: &str) -> Option<&'static SkillDefinition> {
    MANDATORY_SKILLS
        .iter()
        .find(|definition| definition.id == id)
}

pub fn mandatory_skill_prompt_bundle() -> String {
    MANDATORY_SKILLS
        .iter()
        .map(|definition| {
            format!(
                "### {id}\n{content}",
                id = definition.id,
                content = definition.bundled_content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn sha256_checksum(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub fn verify_bundled_skills(
    project_root: impl AsRef<Path>,
    prompt_bundle: &str,
) -> Vec<SkillVerification> {
    let project_root = project_root.as_ref();
    MANDATORY_SKILLS
        .iter()
        .map(|definition| {
            let path = project_root.join(definition.relative_path);
            let bundled_checksum = sha256_checksum(definition.bundled_content);
            let (checksum, load_state) = match fs::read(&path) {
                Ok(content) => {
                    let checksum = sha256_checksum(content);
                    let load_state = if checksum == bundled_checksum {
                        SkillLoadState::MatchesBundled
                    } else {
                        SkillLoadState::Differs
                    };
                    (Some(checksum), load_state)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    (None, SkillLoadState::Missing)
                }
                Err(_) => (None, SkillLoadState::Unreadable),
            };
            let heading = format!("### {}", definition.id);
            let prompt_included = prompt_bundle.contains(&heading)
                && prompt_bundle.contains(definition.bundled_content);

            SkillVerification {
                id: definition.id.to_string(),
                label: definition.label.to_string(),
                path,
                checksum,
                bundled_checksum,
                load_state,
                prompt_included,
            }
        })
        .collect()
}

pub fn repair_bundled_skills(
    project_root: impl AsRef<Path>,
    skill_ids: &[&str],
    confirmation: Option<&SkillRepairConfirmation>,
) -> Result<SkillRepairOutcome, SkillRepairError> {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    repair_bundled_skills_at(project_root, skill_ids, confirmation, &timestamp)
}

pub fn preview_bundled_skill_repair(
    project_root: impl AsRef<Path>,
    skill_ids: &[&str],
) -> Result<SkillRepairPreview, SkillRepairError> {
    let project_root = project_root.as_ref();
    let requested_ids = skill_ids
        .iter()
        .map(|id| {
            mandatory_skill_definition(id)
                .map(|definition| definition.id)
                .ok_or_else(|| SkillRepairError::UnknownSkillId((*id).to_string()))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let definitions = MANDATORY_SKILLS
        .iter()
        .filter(|definition| requested_ids.contains(definition.id))
        .collect::<Vec<_>>();
    let verification = verify_bundled_skills(project_root, &mandatory_skill_prompt_bundle());
    let affected_paths = definitions
        .iter()
        .filter_map(|definition| {
            verification
                .iter()
                .find(|item| item.id == definition.id)
                .filter(|item| item.load_state != SkillLoadState::MatchesBundled)
                .map(|item| item.path.clone())
        })
        .collect::<Vec<_>>();

    Ok(SkillRepairPreview {
        skill_ids: definitions
            .iter()
            .map(|definition| definition.id.to_string())
            .collect(),
        affected_paths,
    })
}

fn repair_bundled_skills_at(
    project_root: impl AsRef<Path>,
    skill_ids: &[&str],
    confirmation: Option<&SkillRepairConfirmation>,
    backup_timestamp: &str,
) -> Result<SkillRepairOutcome, SkillRepairError> {
    let project_root = project_root.as_ref();
    let preview = preview_bundled_skill_repair(project_root, skill_ids)?;
    let requested_ids = preview
        .skill_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let definitions = MANDATORY_SKILLS
        .iter()
        .filter(|definition| requested_ids.contains(definition.id))
        .collect::<Vec<_>>();
    let verification = verify_bundled_skills(project_root, &mandatory_skill_prompt_bundle());
    let affected_paths = preview.affected_paths;

    if affected_paths.is_empty() {
        return Ok(SkillRepairOutcome::Repaired(SkillRepairReport {
            repaired_paths: Vec::new(),
            backup_paths: Vec::new(),
            verification,
        }));
    }

    if confirmation.map(|value| &value.affected_paths) != Some(&affected_paths) {
        return Ok(SkillRepairOutcome::ConfirmationRequired(
            SkillRepairPreview {
                skill_ids: definitions
                    .iter()
                    .map(|definition| definition.id.to_string())
                    .collect(),
                affected_paths,
            },
        ));
    }

    let mut repaired_paths = Vec::with_capacity(affected_paths.len());
    let mut backup_paths = Vec::new();
    for definition in definitions {
        let path = project_root.join(definition.relative_path);
        if !affected_paths.contains(&path) {
            continue;
        }
        let prior_state = verification
            .iter()
            .find(|item| item.id == definition.id)
            .map(|item| item.load_state)
            .expect("fixed catalog verification");
        if prior_state == SkillLoadState::Differs {
            let backup_path = backup_path(&path, backup_timestamp);
            copy_without_overwrite(&path, &backup_path)?;
            backup_paths.push(backup_path);
        }
        atomic_write(
            &path,
            definition.bundled_content.as_bytes(),
            backup_timestamp,
        )?;
        repaired_paths.push(path);
    }

    let verification = verify_bundled_skills(project_root, &mandatory_skill_prompt_bundle());
    Ok(SkillRepairOutcome::Repaired(SkillRepairReport {
        repaired_paths,
        backup_paths,
        verification,
    }))
}

fn backup_path(path: &Path, timestamp: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("SKILL.md");
    path.with_file_name(format!("{file_name}.backup-{timestamp}"))
}

fn copy_without_overwrite(source: &Path, destination: &Path) -> Result<(), SkillRepairError> {
    let mut source_file = fs::File::open(source).map_err(|error| io_error(source, error))?;
    let mut destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| io_error(destination, error))?;
    let mut content = Vec::new();
    source_file
        .read_to_end(&mut content)
        .map_err(|error| io_error(source, error))?;
    destination_file
        .write_all(&content)
        .map_err(|error| io_error(destination, error))?;
    destination_file
        .sync_all()
        .map_err(|error| io_error(destination, error))
}

fn atomic_write(path: &Path, content: &[u8], timestamp: &str) -> Result<(), SkillRepairError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("SKILL.md");
    let temp_path = parent.join(format!(
        ".{file_name}.repair-{}-{timestamp}.tmp",
        std::process::id()
    ));
    let result = (|| {
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| io_error(&temp_path, error))?;
        temp_file
            .write_all(content)
            .map_err(|error| io_error(&temp_path, error))?;
        temp_file
            .sync_all()
            .map_err(|error| io_error(&temp_path, error))?;
        fs::rename(&temp_path, path).map_err(|error| io_error(path, error))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn io_error(path: &Path, error: std::io::Error) -> SkillRepairError {
    SkillRepairError::Io {
        path: path.display().to_string(),
        kind: error.kind(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const TEST_TIMESTAMP: &str = "20260716T120000Z";

    fn write_skill(root: &std::path::Path, definition: &SkillDefinition, content: &str) {
        let path = root.join(definition.relative_path);
        fs::create_dir_all(path.parent().expect("skill parent")).expect("create skill parent");
        fs::write(path, content).expect("write skill");
    }

    #[test]
    fn verification_reports_a_missing_mandatory_skill() {
        let root = tempfile::tempdir().expect("temp root");
        let prompt = mandatory_skill_prompt_bundle();

        let item = verify_bundled_skills(root.path(), &prompt)
            .into_iter()
            .find(|item| item.id == "video-creater-video-pipeline")
            .expect("pipeline verification");

        assert_eq!(item.load_state, SkillLoadState::Missing);
        assert_eq!(item.checksum, None);
        assert!(item.prompt_included);
        assert_eq!(
            item.path,
            root.path()
                .join(".agents/skills/video-creater-video-pipeline/SKILL.md")
        );
    }

    #[test]
    fn verification_reports_matching_checksums() {
        let root = tempfile::tempdir().expect("temp root");
        let definition =
            mandatory_skill_definition("video-creater-graphics").expect("graphics definition");
        write_skill(root.path(), definition, definition.bundled_content);

        let item = verify_bundled_skills(root.path(), &mandatory_skill_prompt_bundle())
            .into_iter()
            .find(|item| item.id == definition.id)
            .expect("graphics verification");

        assert_eq!(item.load_state, SkillLoadState::MatchesBundled);
        assert_eq!(
            item.checksum.as_deref(),
            Some(item.bundled_checksum.as_str())
        );
        assert!(item.prompt_included);
    }

    #[test]
    fn verification_reports_differing_content() {
        let root = tempfile::tempdir().expect("temp root");
        let definition =
            mandatory_skill_definition("video-creater-visuals").expect("visuals definition");
        write_skill(root.path(), definition, "customized mandatory skill\n");

        let item = verify_bundled_skills(root.path(), &mandatory_skill_prompt_bundle())
            .into_iter()
            .find(|item| item.id == definition.id)
            .expect("visuals verification");

        assert_eq!(item.load_state, SkillLoadState::Differs);
        assert_ne!(
            item.checksum.as_deref(),
            Some(item.bundled_checksum.as_str())
        );
        assert!(item.prompt_included);
    }

    #[test]
    fn compiled_prompt_contains_every_mandatory_skill_heading() {
        let prompt = mandatory_skill_prompt_bundle();

        for definition in MANDATORY_SKILLS {
            assert!(
                prompt.contains(&format!("### {}", definition.id)),
                "missing heading for {}",
                definition.id
            );
            assert!(prompt.contains(definition.bundled_content));
        }
    }

    #[test]
    fn catalog_uses_the_specified_friendly_labels() {
        assert_eq!(
            MANDATORY_SKILLS.map(|definition| definition.label),
            [
                "Edit planning and render pipeline",
                "Video graphics and overlays",
                "Editor interface and visual QA",
            ]
        );
    }

    #[test]
    fn missing_configured_root_never_falls_back_to_the_process_cwd() {
        assert_eq!(
            validate_skill_root(None).expect_err("root must be explicit"),
            SkillRootError::Missing
        );
    }

    #[test]
    fn configured_root_must_be_a_video_creater_repository() {
        let non_project = tempfile::tempdir().expect("non-project root");

        let error = validate_skill_root(Some(non_project.path()))
            .expect_err("non-project root must be rejected");

        assert!(matches!(error, SkillRootError::NotRepository { .. }));
    }

    #[test]
    fn configured_repository_root_is_canonicalized() {
        let root = tempfile::tempdir().expect("repository root");
        fs::write(root.path().join("AGENTS.md"), "project policy\n").expect("write AGENTS.md");

        assert_eq!(
            validate_skill_root(Some(root.path())).expect("valid repository root"),
            root.path().canonicalize().expect("canonical root")
        );
    }

    #[test]
    fn repair_requires_confirmed_paths_and_backs_up_differing_content() {
        let root = tempfile::tempdir().expect("temp root");
        let agents_path = root.path().join("AGENTS.md");
        let agents_content = "# Disposable Video Creater project\n";
        fs::write(&agents_path, agents_content).expect("write AGENTS.md");
        for bundled in MANDATORY_SKILLS {
            write_skill(root.path(), &bundled, bundled.bundled_content);
        }
        let definition = mandatory_skill_definition("video-creater-video-pipeline")
            .expect("pipeline definition");
        let original = "locally modified pipeline skill\n";
        write_skill(root.path(), definition, original);

        let preview =
            match repair_bundled_skills_at(root.path(), &[definition.id], None, TEST_TIMESTAMP)
                .expect("repair preview")
            {
                SkillRepairOutcome::ConfirmationRequired(preview) => preview,
                SkillRepairOutcome::Repaired(_) => panic!("repair must require confirmation"),
            };
        assert_eq!(
            preview.affected_paths,
            vec![root.path().join(definition.relative_path)]
        );
        assert_eq!(
            fs::read_to_string(root.path().join(definition.relative_path)).expect("original skill"),
            original
        );

        let confirmation = SkillRepairConfirmation {
            affected_paths: preview.affected_paths,
        };
        let report = match repair_bundled_skills_at(
            root.path(),
            &[definition.id],
            Some(&confirmation),
            TEST_TIMESTAMP,
        )
        .expect("confirmed repair")
        {
            SkillRepairOutcome::Repaired(report) => report,
            SkillRepairOutcome::ConfirmationRequired(_) => panic!("confirmation should match"),
        };

        let skill_path = root.path().join(definition.relative_path);
        let backup_path = skill_path.with_file_name(format!("SKILL.md.backup-{TEST_TIMESTAMP}"));
        assert_eq!(report.repaired_paths, vec![skill_path.clone()]);
        assert_eq!(report.backup_paths, vec![backup_path.clone()]);
        assert_eq!(fs::read_to_string(backup_path).expect("backup"), original);
        assert_eq!(
            fs::read_to_string(skill_path).expect("repaired skill"),
            definition.bundled_content
        );
        assert_eq!(
            fs::read_to_string(agents_path).expect("AGENTS.md"),
            agents_content
        );
        for untouched in MANDATORY_SKILLS
            .iter()
            .filter(|candidate| candidate.id != definition.id)
        {
            assert_eq!(
                fs::read_to_string(root.path().join(untouched.relative_path))
                    .expect("untouched mandatory skill"),
                untouched.bundled_content,
                "repair must remain scoped away from {}",
                untouched.id
            );
        }
        assert!(report.verification.iter().all(|item| {
            item.id != definition.id || item.load_state == SkillLoadState::MatchesBundled
        }));
    }

    #[test]
    fn repair_never_touches_agents_md_or_custom_skills() {
        let root = tempfile::tempdir().expect("temp root");
        let agents_path = root.path().join("AGENTS.md");
        let custom_path = root.path().join(".agents/skills/custom/SKILL.md");
        fs::create_dir_all(custom_path.parent().expect("custom parent"))
            .expect("create custom parent");
        fs::write(&agents_path, "custom agents instructions\n").expect("write AGENTS.md");
        fs::write(&custom_path, "custom skill\n").expect("write custom skill");

        let definition =
            mandatory_skill_definition("video-creater-graphics").expect("graphics definition");
        let expected_path = root.path().join(definition.relative_path);
        let preview =
            match repair_bundled_skills_at(root.path(), &[definition.id], None, TEST_TIMESTAMP)
                .expect("repair preview")
            {
                SkillRepairOutcome::ConfirmationRequired(preview) => preview,
                SkillRepairOutcome::Repaired(_) => panic!("missing file must need confirmation"),
            };
        assert_eq!(preview.affected_paths, vec![expected_path]);

        let confirmation = SkillRepairConfirmation {
            affected_paths: preview.affected_paths,
        };
        repair_bundled_skills_at(
            root.path(),
            &[definition.id],
            Some(&confirmation),
            TEST_TIMESTAMP,
        )
        .expect("repair custom-safe");

        assert_eq!(
            fs::read_to_string(agents_path).expect("AGENTS.md"),
            "custom agents instructions\n"
        );
        assert_eq!(
            fs::read_to_string(custom_path).expect("custom skill"),
            "custom skill\n"
        );

        let error = repair_bundled_skills_at(root.path(), &["custom"], None, TEST_TIMESTAMP)
            .expect_err("custom ids are outside the fixed catalog");
        assert_eq!(
            error,
            SkillRepairError::UnknownSkillId("custom".to_string())
        );
    }
}
