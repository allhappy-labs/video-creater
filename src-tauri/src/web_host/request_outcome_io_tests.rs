use super::*;

fn set_errno(code: libc::c_int) {
    #[cfg(target_os = "linux")]
    unsafe {
        *libc::__errno_location() = code;
    }
    #[cfg(target_os = "macos")]
    unsafe {
        *libc::__error() = code;
    }
}

#[test]
fn directory_enumeration_error_rejects_partial_names() {
    let root = tempfile::tempdir().unwrap();
    let directory = JournalDirectory::open(root.path()).unwrap();
    directory.write("first.json", b"first receipt").unwrap();
    directory.write("second.json", b"second receipt").unwrap();
    let mut returned_receipt = false;
    let names = directory.names_with_reader(8, |stream| {
        if returned_receipt {
            set_errno(libc::EIO);
            return std::ptr::null_mut();
        }
        let entry = unsafe { libc::readdir(stream) };
        if !entry.is_null() {
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            returned_receipt = name.to_bytes().ends_with(b".json");
        }
        entry
    });
    assert!(
        returned_receipt,
        "the fault must follow a real receipt entry"
    );
    let error = names.expect_err("a failed journal scan must never admit partial names");
    assert_eq!(error.raw_os_error(), Some(libc::EIO));
}

#[test]
fn directory_enumeration_clears_stale_errno_before_clean_eof() {
    let root = tempfile::tempdir().unwrap();
    let directory = JournalDirectory::open(root.path()).unwrap();
    directory.write("receipt.json", b"receipt").unwrap();
    let names = directory.names_with_reader(8, |stream| {
        let entry = unsafe { libc::readdir(stream) };
        if !entry.is_null() {
            // POSIX permits successful operations to leave errno unchanged.
            set_errno(libc::EIO);
        }
        entry
    });
    assert_eq!(names.unwrap(), ["receipt.json"]);
}
