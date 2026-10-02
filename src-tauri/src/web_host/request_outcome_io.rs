//! Bounded no-follow journal I/O anchored to a process-owned directory.
use std::ffi::{CStr, CString};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

pub(super) struct JournalDirectory {
    directory: File,
}

impl JournalDirectory {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { directory })
    }
    pub fn names(&self, limit: usize) -> std::io::Result<Vec<String>> {
        self.names_with_reader(limit, |directory| unsafe { libc::readdir(directory) })
    }

    fn names_with_reader(
        &self,
        limit: usize,
        mut read_entry: impl FnMut(*mut libc::DIR) -> *mut libc::dirent,
    ) -> std::io::Result<Vec<String>> {
        let fd = unsafe { libc::dup(self.directory.as_raw_fd()) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let directory = unsafe { libc::fdopendir(fd) };
        if directory.is_null() {
            unsafe {
                libc::close(fd);
            }
            return Err(std::io::Error::last_os_error());
        }
        let result = (|| {
            let mut names = Vec::new();
            loop {
                // NULL means either EOF or failure; unrelated successful calls may
                // leave errno populated, so clear it immediately before each read.
                clear_directory_errno()?;
                let entry = read_entry(directory);
                if entry.is_null() {
                    let error = std::io::Error::last_os_error();
                    if error.raw_os_error().is_some_and(|code| code != 0) {
                        return Err(error);
                    }
                    break;
                }
                let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                    .to_str()
                    .map_err(|_| std::io::Error::other("invalid journal filename"))?;
                if name == "." || name == ".." {
                    continue;
                }
                if names.len() >= limit {
                    return Err(std::io::Error::other("journal file limit exceeded"));
                }
                names.push(name.to_owned());
            }
            Ok(names)
        })();
        unsafe {
            libc::closedir(directory);
        }
        result
    }
    pub fn read(&self, name: &str, limit: usize) -> std::io::Result<Vec<u8>> {
        let name = CString::new(name)?;
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() || file.metadata()?.len() > limit as u64 {
            return Err(std::io::Error::other("journal record limit exceeded"));
        }
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(std::io::Error::other("journal record grew"));
        }
        Ok(bytes)
    }
    pub fn write(&self, name: &str, bytes: &[u8]) -> std::io::Result<()> {
        let temporary = CString::new(format!(".transaction-{}", uuid::Uuid::new_v4().simple()))?;
        let name = CString::new(name)?;
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            if unsafe {
                libc::renameat(
                    self.directory.as_raw_fd(),
                    temporary.as_ptr(),
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                )
            } != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            self.directory.sync_all()
        })();
        if result.is_err() {
            unsafe {
                libc::unlinkat(self.directory.as_raw_fd(), temporary.as_ptr(), 0);
            }
        }
        result
    }
    pub fn remove(&self, name: &str) -> std::io::Result<()> {
        let name = CString::new(name)?;
        if unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        self.directory.sync_all()
    }
}

fn clear_directory_errno() -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    unsafe {
        *libc::__errno_location() = 0;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    unsafe {
        *libc::__error() = 0;
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "journal enumeration requires verified platform errno access",
    ))
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
#[path = "request_outcome_io_tests.rs"]
mod tests;
