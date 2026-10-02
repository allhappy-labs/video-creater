//! Unsupported descriptor/process-lock platforms refuse durable mutations safely.
use std::path::Path;
pub(super) struct JournalDirectory;
fn unsupported<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "durable host journal requires descriptor-relative locking",
    ))
}
impl JournalDirectory {
    pub fn open(_: &Path) -> std::io::Result<Self> {
        unsupported()
    }
    pub fn names(&self, _: usize) -> std::io::Result<Vec<String>> {
        unsupported()
    }
    pub fn read(&self, _: &str, _: usize) -> std::io::Result<Vec<u8>> {
        unsupported()
    }
    pub fn write(&self, _: &str, _: &[u8]) -> std::io::Result<()> {
        unsupported()
    }
    pub fn remove(&self, _: &str) -> std::io::Result<()> {
        unsupported()
    }
}
