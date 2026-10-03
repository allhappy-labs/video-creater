use super::NleXmlExport;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

pub(super) fn write(project: &Path, export: &NleXmlExport, overwrite: bool) -> io::Result<PathBuf> {
    let name = Path::new(&export.filename);
    if !matches!(name.components().next(), Some(Component::Normal(_)))
        || name.components().count() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid export filename",
        ));
    }
    write_checked(project, export, overwrite)
}

#[cfg(unix)]
fn write_checked(project: &Path, export: &NleXmlExport, overwrite: bool) -> io::Result<PathBuf> {
    use std::ffi::CString;
    use std::fs::{File, OpenOptions};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::OpenOptionsExt;
    let project_dir = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(project)?;
    let exports = c"exports";
    // All destination operations remain anchored to retained directory descriptors.
    if unsafe { libc::mkdirat(project_dir.as_raw_fd(), exports.as_ptr(), 0o700) } == -1 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(error);
        }
    }
    let directory_fd = unsafe {
        libc::openat(
            project_dir.as_raw_fd(),
            exports.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if directory_fd == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ownership of this freshly opened descriptor transfers exactly once.
    let directory = unsafe { File::from_raw_fd(directory_fd) };
    let stage = CString::new(format!(
        ".video-creater-nle-{}.partial",
        uuid::Uuid::new_v4()
    ))?;
    let target = CString::new(export.filename.as_bytes())?;
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            stage.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let result = (|| {
        file.write_all(export.xml.as_bytes())?;
        file.sync_all()?;
        let result = unsafe {
            if overwrite {
                libc::renameat(
                    directory.as_raw_fd(),
                    stage.as_ptr(),
                    directory.as_raw_fd(),
                    target.as_ptr(),
                )
            } else {
                libc::linkat(
                    directory.as_raw_fd(),
                    stage.as_ptr(),
                    directory.as_raw_fd(),
                    target.as_ptr(),
                    0,
                )
            }
        };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        directory.sync_all()?;
        Ok(project.join("exports").join(&export.filename))
    })();
    // Cleanup never follows a substituted directory pathname.
    unsafe {
        libc::unlinkat(directory.as_raw_fd(), stage.as_ptr(), 0);
    }
    result
}

#[cfg(not(unix))]
fn write_checked(project: &Path, export: &NleXmlExport, overwrite: bool) -> io::Result<PathBuf> {
    let root = project.canonicalize()?;
    let directory = root.join("exports");
    std::fs::create_dir_all(&directory)?;
    if directory.canonicalize()? != directory {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "exports directory escapes project",
        ));
    }
    let output = directory.join(&export.filename);
    let mut stage = tempfile::NamedTempFile::new_in(&directory)?;
    stage.write_all(export.xml.as_bytes())?;
    stage.as_file().sync_all()?;
    if overwrite {
        stage.persist(&output)
    } else {
        stage.persist_noclobber(&output)
    }
    .map_err(|error| error.error)?;
    Ok(output)
}
