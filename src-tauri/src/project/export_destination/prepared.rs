//! Durable owned staging before mutation ownership; descriptor-relative publication.

use super::relative;
use super::*;
use std::ffi::{OsStr, OsString};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::sync::{Arc, OnceLock};

const PAYLOAD: &str = "output";
const ANCHOR: &str = "anchor";
const OWNER: &str = "owner.json";

pub struct PreparedExportOutput {
    destination: ExportDestination,
    requested_directory: PathBuf,
    directory: File,
    file: File,
    staging: OwnedStaging,
    published_name: Arc<OnceLock<OsString>>,
}

/// Kept by completion while canonical bookkeeping is committed. This capability
/// is deliberately not persisted or serialized; it anchors rollback to the
/// actual publication directory, including after that directory was renamed.
pub struct ExportPublicationOwner {
    directory: File,
    directory_path: PathBuf,
    owner: Metadata,
    file: File,
    published_name: Arc<OnceLock<OsString>>,
}

impl ExportPublicationOwner {
    pub fn rollback(&self, absolute_path: &Path) -> Result<(), ExportDestinationError> {
        if absolute_path.parent() != Some(self.directory_path.as_path()) {
            return Err(ExportDestinationError::Io(
                "the rollback folder does not match publication ownership".into(),
            ));
        }
        let name = absolute_path
            .file_name()
            .ok_or_else(|| ExportDestinationError::Io("the rollback name is invalid".into()))?;
        if Path::new(name).components().count() != 1
            || self.published_name.get().map(OsString::as_os_str) != Some(name)
        {
            return Err(ExportDestinationError::Io(
                "the rollback name is invalid".into(),
            ));
        }
        relative::remove_owned(&self.directory, name, &self.file.metadata()?)?;
        self.directory.sync_all()?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageOwner {
    version: u32,
    name: String,
    parent_device: u64,
    parent_inode: u64,
    directory_device: u64,
    directory_inode: u64,
    payload_device: u64,
    payload_inode: u64,
    marker_device: u64,
    marker_inode: u64,
    parent_birth: Option<u64>,
    directory_birth: Option<u64>,
    payload_birth: Option<u64>,
    marker_birth: Option<u64>,
}

/// A private staging directory, locked for the full prepare/publish lifetime.
/// Recovery requires this lock plus the durable directory/payload identities.
struct OwnedStaging {
    parent: File,
    name: OsString,
    directory: File,
    directory_owner: Metadata,
    payload_owner: Metadata,
    payload: File,
    marker: Option<File>,
}

impl OwnedStaging {
    fn create(
        parent: &File,
        source: &File,
        source_path: &Path,
        policy: LinkPolicy,
    ) -> Result<(Self, File), ExportDestinationError> {
        let name = OsString::from(format!(
            ".video-creater-export-{}.partial",
            uuid::Uuid::new_v4()
        ));
        let directory = relative::mkdir_at(parent, &name)?;
        if !relative::try_lock(&directory)? {
            return Err(ExportDestinationError::Io(
                "the new export stage is unexpectedly in use".into(),
            ));
        }
        let linked = if policy == LinkPolicy::Auto {
            let source_parent = source_path
                .parent()
                .and_then(|path| relative::open_directory(path).ok());
            source_parent.as_ref().is_some_and(|source_parent| {
                source_path.file_name().is_some_and(|source_name| {
                    relative::link_open(
                        source,
                        source_parent,
                        source_name,
                        &directory,
                        OsStr::new(PAYLOAD),
                    )
                    .is_ok()
                })
            })
        } else {
            false
        };
        let file = relative::open_at(
            &directory,
            OsStr::new(PAYLOAD),
            if linked {
                libc::O_RDONLY
            } else {
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL
            },
        )?;
        let staging = Self {
            parent: parent.try_clone()?,
            name,
            directory_owner: directory.metadata()?,
            payload_owner: if linked {
                source.metadata()?
            } else {
                file.metadata()?
            },
            directory,
            payload: if linked {
                source.try_clone()?
            } else {
                file.try_clone()?
            },
            marker: None,
        };
        if linked && !same_file(&source.metadata()?, &file.metadata()?) {
            // Preserve a substituted entry, but release descriptors and the lock.
            return Err(ExportDestinationError::Io(
                "the rendered file changed during staging".into(),
            ));
        }
        // A held descriptor cannot relink a regular inode after its last name
        // was removed. Keep a private second name for Linux descriptor linking;
        // macOS also publishes through this private anchor instead of payload.
        relative::link_open(
            &file,
            &staging.directory,
            OsStr::new(PAYLOAD),
            &staging.directory,
            OsStr::new(ANCHOR),
        )?;
        Ok((staging, file))
    }

    fn persist_owner(&mut self) -> Result<(), ExportDestinationError> {
        let parent = self.parent.metadata()?;
        let mut marker = relative::open_at(
            &self.directory,
            OsStr::new(OWNER),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        let marker_metadata = marker.metadata()?;
        self.marker = Some(marker.try_clone()?);
        let record = StageOwner {
            version: 1,
            name: self.name.to_string_lossy().into_owned(),
            parent_device: parent.dev(),
            parent_inode: parent.ino(),
            directory_device: self.directory_owner.dev(),
            directory_inode: self.directory_owner.ino(),
            payload_device: self.payload_owner.dev(),
            payload_inode: self.payload_owner.ino(),
            marker_device: marker_metadata.dev(),
            marker_inode: marker_metadata.ino(),
            parent_birth: birth(&parent),
            directory_birth: birth(&self.directory_owner),
            payload_birth: birth(&self.payload_owner),
            marker_birth: birth(&marker_metadata),
        };
        marker.write_all(
            &serde_json::to_vec(&record)
                .map_err(|error| ExportDestinationError::Io(error.to_string()))?,
        )?;
        marker.sync_all()?;
        self.directory.sync_all()?;
        self.parent.sync_all()?;
        Ok(())
    }

    fn validate(&self, file: &File) -> Result<(), ExportDestinationError> {
        let file_owner = file.metadata()?;
        if !relative::metadata_at(&self.parent, &self.name)
            .is_ok_and(|current| same_file(&self.directory_owner, &current))
            || !relative::metadata_at(&self.directory, OsStr::new(PAYLOAD))
                .is_ok_and(|current| same_file(&file_owner, &current))
            || !relative::metadata_at(&self.directory, OsStr::new(ANCHOR))
                .is_ok_and(|current| same_file(&file_owner, &current))
        {
            return Err(ExportDestinationError::Io(
                "the export staging file changed".into(),
            ));
        }
        Ok(())
    }
}

impl Drop for OwnedStaging {
    fn drop(&mut self) {
        // Work through the actual locked directory, never a possibly replaced
        // absolute parent path. Unexpected entries prevent rmdir and survive.
        let _ = self
            .payload
            .metadata()
            .and_then(|owner| relative::remove_owned(&self.directory, OsStr::new(PAYLOAD), &owner));
        let _ = self
            .payload
            .metadata()
            .and_then(|owner| relative::remove_owned(&self.directory, OsStr::new(ANCHOR), &owner));
        if let Some(marker) = &self.marker {
            let _ = marker.metadata().and_then(|owner| {
                relative::remove_owned(&self.directory, OsStr::new(OWNER), &owner)
            });
        }
        let _ = relative::remove_owned(&self.parent, &self.name, &self.directory_owner);
        let _ = self.parent.sync_all();
    }
}

/// Recover at most 64 candidate stages from at most 4096 entries per prepare.
/// Missing/malformed ownership, replacement payloads, other-user directories,
/// and locked live stages are retained; filenames or age alone are never proof.
fn recover_interrupted_stages(parent: &File) {
    let Ok(parent_owner) = parent.metadata() else {
        return;
    };
    let Ok(names) = relative::stage_names(parent) else {
        return;
    };
    for name in names {
        let Ok(directory) = relative::open_at(parent, &name, libc::O_RDONLY | libc::O_DIRECTORY)
        else {
            continue;
        };
        let Ok(directory_owner) = directory.metadata() else {
            continue;
        };
        if directory_owner.uid() != unsafe { libc::geteuid() }
            || directory_owner.permissions().mode() & 0o777 != 0o700
            || !relative::try_lock(&directory).unwrap_or(false)
        {
            continue;
        }
        let Ok(marker) = relative::open_at(&directory, OsStr::new(OWNER), libc::O_RDONLY) else {
            continue;
        };
        let Ok(marker_owner) = marker.metadata() else {
            continue;
        };
        if !marker_owner.is_file() || marker_owner.len() > 4096 {
            continue;
        }
        let mut bytes = Vec::new();
        if (&marker).take(4097).read_to_end(&mut bytes).is_err() || bytes.len() > 4096 {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<StageOwner>(&bytes) else {
            continue;
        };
        let Ok(payload) = relative::open_at(&directory, OsStr::new(PAYLOAD), libc::O_RDONLY) else {
            continue;
        };
        let Ok(payload_owner) = payload.metadata() else {
            continue;
        };
        if record.version != 1
            || name != OsStr::new(&record.name)
            || record.parent_device != parent_owner.dev()
            || record.parent_inode != parent_owner.ino()
            || record.directory_device != directory_owner.dev()
            || record.directory_inode != directory_owner.ino()
            || record.payload_device != payload_owner.dev()
            || record.payload_inode != payload_owner.ino()
            || record.marker_device != marker_owner.dev()
            || record.marker_inode != marker_owner.ino()
            || !birth_matches(record.parent_birth, &parent_owner)
            || !birth_matches(record.directory_birth, &directory_owner)
            || !birth_matches(record.payload_birth, &payload_owner)
            || !birth_matches(record.marker_birth, &marker_owner)
            || !payload_owner.is_file()
            || !relative::metadata_at(&directory, OsStr::new(ANCHOR))
                .is_ok_and(|current| same_file(&payload_owner, &current))
        {
            continue;
        }
        let staging = OwnedStaging {
            parent: match parent.try_clone() {
                Ok(parent) => parent,
                Err(_) => continue,
            },
            name,
            directory,
            directory_owner,
            payload_owner,
            payload,
            marker: Some(marker),
        };
        drop(staging);
    }
}

pub fn prepare_export_output(
    request: ExportMaterialization<'_>,
) -> Result<PreparedExportOutput, ExportDestinationError> {
    prepare_export_output_cancellable(request, &mut || true)
}

pub(crate) fn prepare_export_output_cancellable(
    request: ExportMaterialization<'_>,
    keep_working: &mut dyn FnMut() -> bool,
) -> Result<PreparedExportOutput, ExportDestinationError> {
    if !keep_working() {
        return Err(ExportDestinationError::Cancelled);
    }
    let ExportMaterialization {
        destination,
        source,
        link_policy,
        ..
    } = request;
    let mut source_file = open_regular_file(source)?;
    let source_metadata = source_file.metadata()?;
    if source_metadata.len() == 0 {
        return Err(ExportDestinationError::EmptySource);
    }
    if destination.inside_project_exports {
        fs::create_dir_all(&destination.directory)?;
    }
    let requested_directory = destination.directory.clone();
    let mut destination = destination.clone();
    destination.directory = fs::canonicalize(&requested_directory)?;
    if destination.inside_project_exports {
        let depth = Path::new(&destination.recorded_directory)
            .components()
            .count();
        let project = requested_directory
            .ancestors()
            .nth(depth)
            .ok_or(ExportDestinationError::DirectoryInsideProject)?;
        if destination.directory != fs::canonicalize(project)?.join(&destination.recorded_directory)
        {
            return Err(ExportDestinationError::DirectoryInsideProject);
        }
    }
    let directory = relative::open_directory(&destination.directory)?;
    recover_interrupted_stages(&directory);
    let (mut staging, mut file) =
        OwnedStaging::create(&directory, &source_file, source, link_policy)?;
    // Persist identities before copying any bytes; restart can recover an
    // interrupted copy, but never an unmarked or merely similarly named file.
    staging.persist_owner()?;
    if !same_file(&source_metadata, &file.metadata()?) {
        let mut buffer = [0; 128 * 1024];
        let mut copied = 0;
        while copied < source_metadata.len() {
            if !keep_working() {
                return Err(ExportDestinationError::Cancelled);
            }
            let limit = (source_metadata.len() - copied).min(buffer.len() as u64) as usize;
            let read = source_file.read(&mut buffer[..limit])?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])?;
            copied += read as u64;
        }
        if copied != source_metadata.len() || source_file.metadata()?.len() != copied {
            return Err(ExportDestinationError::Io(
                "the copied export doesn't match the rendered file".into(),
            ));
        }
        file.set_permissions(source_metadata.permissions())?;
    }
    file.sync_all()?;
    if !keep_working() {
        return Err(ExportDestinationError::Cancelled);
    }
    let prepared = PreparedExportOutput {
        destination,
        requested_directory,
        directory,
        file,
        staging,
        published_name: Arc::new(OnceLock::new()),
    };
    prepared.validate()?;
    Ok(prepared)
}

impl PreparedExportOutput {
    pub fn publish(self) -> Result<MaterializedExport, ExportDestinationError> {
        self.publish_with_hook(&mut |_| {})
    }

    pub fn publication_owner(&self) -> Result<ExportPublicationOwner, ExportDestinationError> {
        Ok(ExportPublicationOwner {
            directory: self.directory.try_clone()?,
            directory_path: self.destination.directory.clone(),
            owner: self.file.metadata()?,
            file: self.file.try_clone()?,
            published_name: self.published_name.clone(),
        })
    }

    fn validate_directory(&self) -> Result<(), ExportDestinationError> {
        if fs::canonicalize(&self.requested_directory)? != self.destination.directory
            || !same_file(
                &self.directory.metadata()?,
                &fs::metadata(&self.destination.directory)?,
            )
        {
            return Err(ExportDestinationError::Io(
                "the export folder changed".into(),
            ));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ExportDestinationError> {
        self.validate_directory()?;
        self.staging.validate(&self.file)
    }

    pub(super) fn publish_with_hook(
        self,
        before_publish: &mut dyn FnMut(&Path),
    ) -> Result<MaterializedExport, ExportDestinationError> {
        self.publish_with_hooks(before_publish, &mut |_| {})
    }

    #[cfg(test)]
    pub(super) fn publish_with_boundary_hook(
        self,
        after_validation: &mut dyn FnMut(&Path),
    ) -> Result<MaterializedExport, ExportDestinationError> {
        self.publish_with_hooks(&mut |_| {}, after_validation)
    }

    fn publish_with_hooks(
        self,
        before_publish: &mut dyn FnMut(&Path),
        after_validation: &mut dyn FnMut(&Path),
    ) -> Result<MaterializedExport, ExportDestinationError> {
        self.validate()?;
        let owner = self.publication_owner()?;
        for index in 1..=MAX_COLLISION_SUFFIX {
            let name = self.destination.candidate_file_name(index);
            let path = self.destination.directory.join(&name);
            before_publish(&path);
            self.validate()?;
            after_validation(&path);
            match relative::link_open(
                &self.file,
                &self.staging.directory,
                OsStr::new(ANCHOR),
                &self.directory,
                OsStr::new(&name),
            ) {
                Ok(()) => {
                    let _ = self.published_name.set(OsString::from(&name));
                    // Parent exchanges cannot redirect the syscall. Recheck the
                    // recorded path and linked inode before returning success.
                    if let Err(error) = self.validate_directory().and_then(|()| {
                        if relative::metadata_at(&self.directory, OsStr::new(&name))
                            .is_ok_and(|current| same_file(&owner.owner, &current))
                        {
                            Ok(())
                        } else {
                            Err(ExportDestinationError::Io(
                                "the published export changed".into(),
                            ))
                        }
                    }) {
                        let _ = owner.rollback(&path);
                        return Err(error);
                    }
                    if let Err(error) = self.directory.sync_all() {
                        let _ = owner.rollback(&path);
                        return Err(error.into());
                    }
                    return Ok(materialized(&self.destination, &name, path));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(ExportDestinationError::NoFreeName)
    }
}

fn open_regular_file(path: &Path) -> Result<File, ExportDestinationError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(ExportDestinationError::Io(
            "the rendered output is not a regular file".into(),
        ));
    }
    Ok(file)
}

pub(super) fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.file_type() == right.file_type()
        && birth(left) == birth(right)
}

fn birth(metadata: &Metadata) -> Option<u64> {
    metadata
        .created()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .try_into()
        .ok()
}

fn birth_matches(expected: Option<u64>, metadata: &Metadata) -> bool {
    expected.is_some() && expected == birth(metadata)
}

/// Compatibility caller without a retained publication capability. Opening the
/// parent anchors subsequent operations, and mismatching file identity is kept.
pub fn remove_export_output_if_owned(path: &Path, owner: &Metadata) {
    if birth(owner).is_none() {
        return;
    }
    let Some(parent) = path.parent() else {
        return;
    };
    let Some(name) = path.file_name() else {
        return;
    };
    let Ok(directory) = relative::open_directory(parent) else {
        return;
    };
    let _ = relative::remove_owned(&directory, name, owner);
}
