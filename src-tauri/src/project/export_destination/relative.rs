//! Filesystem operations anchored to retained directory descriptors.
//!
//! No operation below follows a destination-parent pathname. Private directories
//! and advisory locks exclude cooperating workers; same-UID hostile writers are
//! outside that exclusion. Quarantine closes the entry-swap window before
//! capture; writers able to mutate the private quarantine can still race unlink.

use super::prepared::same_file;
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

fn c_name(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

fn result(value: libc::c_int) -> io::Result<()> {
    if value == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(super) fn open_directory(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(path)
}

pub(super) fn open_at(directory: &File, name: &OsStr, flags: libc::c_int) -> io::Result<File> {
    let name = c_name(name)?;
    // SAFETY: valid live directory descriptor and terminated name; ownership of
    // the newly returned descriptor transfers exactly once to File.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub(super) fn metadata_at(directory: &File, name: &OsStr) -> io::Result<Metadata> {
    open_at(directory, name, libc::O_RDONLY)?.metadata()
}

pub(super) fn mkdir_at(directory: &File, name: &OsStr) -> io::Result<File> {
    let c = c_name(name)?;
    result(unsafe { libc::mkdirat(directory.as_raw_fd(), c.as_ptr(), 0o700) })?;
    open_at(directory, name, libc::O_RDONLY | libc::O_DIRECTORY)
}

pub(super) fn try_lock(file: &File) -> io::Result<bool> {
    let value = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if value == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::WouldBlock {
        Ok(false)
    } else {
        Err(error)
    }
}

pub(super) fn rename_noclobber(
    from: &File,
    source: &OsStr,
    to: &File,
    target: &OsStr,
) -> io::Result<()> {
    let source = c_name(source)?;
    let target = c_name(target)?;
    #[cfg(target_os = "linux")]
    return result(unsafe {
        libc::renameat2(
            from.as_raw_fd(),
            source.as_ptr(),
            to.as_raw_fd(),
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    });
    #[cfg(target_os = "macos")]
    return result(unsafe {
        libc::renameatx_np(
            from.as_raw_fd(),
            source.as_ptr(),
            to.as_raw_fd(),
            target.as_ptr(),
            libc::RENAME_EXCL,
        )
    });
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative no-clobber rename is unavailable",
    ))
}

pub(super) fn link_open(
    file: &File,
    source_directory: &File,
    source_name: &OsStr,
    to: &File,
    target: &OsStr,
) -> io::Result<()> {
    let target = c_name(target)?;
    #[cfg(target_os = "linux")]
    {
        let _ = (source_directory, source_name);
        // /proc/self/fd plus AT_SYMLINK_FOLLOW links the held inode, rather than
        // looking up a stage name that another process could replace. AT_EMPTY_PATH
        // would require CAP_DAC_READ_SEARCH for ordinary files on some kernels.
        let source =
            CString::new(format!("/proc/self/fd/{}", file.as_raw_fd())).expect("descriptor path");
        result(unsafe {
            libc::linkat(
                libc::AT_FDCWD,
                source.as_ptr(),
                to.as_raw_fd(),
                target.as_ptr(),
                libc::AT_SYMLINK_FOLLOW,
            )
        })
    }
    #[cfg(target_os = "macos")]
    {
        let _ = file;
        let source = c_name(source_name)?;
        result(unsafe {
            libc::linkat(
                source_directory.as_raw_fd(),
                source.as_ptr(),
                to.as_raw_fd(),
                target.as_ptr(),
                0,
            )
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative publication is unavailable",
    ))
}

fn unlink_at(directory: &File, name: &OsStr, is_directory: bool) -> io::Result<()> {
    let name = c_name(name)?;
    result(unsafe {
        libc::unlinkat(
            directory.as_raw_fd(),
            name.as_ptr(),
            if is_directory { libc::AT_REMOVEDIR } else { 0 },
        )
    })
}

/// Capture the entry atomically before deciding whether to delete it. A changed
/// entry is restored without clobbering; if restoration loses a race it remains
/// in a private forensic directory. Never recursively remove arbitrary contents.
pub(super) fn remove_owned(directory: &File, name: &OsStr, owner: &Metadata) -> io::Result<bool> {
    remove_owned_with_hook(directory, name, owner, &mut || {})
}

fn remove_owned_with_hook(
    directory: &File,
    name: &OsStr,
    owner: &Metadata,
    after_validation: &mut dyn FnMut(),
) -> io::Result<bool> {
    if !metadata_at(directory, name).is_ok_and(|current| same_file(owner, &current)) {
        return Ok(false);
    }
    let quarantine_name = OsString::from(format!(
        ".video-creater-quarantine-{}.partial",
        uuid::Uuid::new_v4()
    ));
    let quarantine = mkdir_at(directory, &quarantine_name)?;
    let quarantine_owner = quarantine.metadata()?;
    let item = OsStr::new("item");
    after_validation();
    let moved = rename_noclobber(directory, name, &quarantine, item);
    let outcome = match moved {
        Err(error) => Err(error),
        Ok(())
            if metadata_at(&quarantine, item).is_ok_and(|current| same_file(owner, &current)) =>
        {
            match unlink_at(&quarantine, item, owner.is_dir()) {
                Ok(()) => Ok(true),
                Err(error) => {
                    let _ = rename_noclobber(&quarantine, item, directory, name);
                    Err(error)
                }
            }
        }
        Ok(()) => {
            // Preserve both entries if somebody populated the original name after
            // capture. An inability to restore is not permission to delete.
            let _ = rename_noclobber(&quarantine, item, directory, name);
            Ok(false)
        }
    };
    if metadata_at(directory, &quarantine_name)
        .is_ok_and(|current| same_file(&quarantine_owner, &current))
    {
        let _ = unlink_at(directory, &quarantine_name, true);
    }
    outcome
}

/// Bounded discovery through the original directory even after it was renamed.
pub(super) fn stage_names(directory: &File) -> io::Result<Vec<OsString>> {
    let scan = open_at(
        directory,
        OsStr::new("."),
        libc::O_RDONLY | libc::O_DIRECTORY,
    )?;
    let raw = scan.into_raw_fd();
    let stream = unsafe { libc::fdopendir(raw) };
    if stream.is_null() {
        unsafe { libc::close(raw) };
        return Err(io::Error::last_os_error());
    }
    let mut names = Vec::new();
    for _ in 0..4096 {
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            break;
        }
        let bytes = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if bytes.starts_with(b".video-creater-export-") && bytes.ends_with(b".partial") {
            names.push(OsString::from_vec(bytes.to_vec()));
            if names.len() == 64 {
                break;
            }
        }
    }
    unsafe { libc::closedir(stream) };
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rollback_boundary_swap_restores_the_replacement_without_deleting_it() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Name.mp4");
        fs::write(&path, b"owned render").unwrap();
        let directory = open_directory(root.path()).unwrap();
        let owner = fs::metadata(&path).unwrap();
        let removed =
            remove_owned_with_hook(&directory, OsStr::new("Name.mp4"), &owner, &mut || {
                let replacement = root.path().join("replacement");
                fs::write(&replacement, b"keep user replacement").unwrap();
                fs::rename(replacement, &path).unwrap();
            })
            .unwrap();
        assert!(!removed);
        assert_eq!(fs::read(path).unwrap(), b"keep user replacement");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
