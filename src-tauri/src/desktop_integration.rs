//! Freedesktop desktop services used by the Linux build: revealing files in
//! the user's file manager and detecting a desktop notification service.
//! macOS uses Finder and UserNotifications directly from `main.rs`.

/// Host operating system reported to the frontend so copy and shortcut
/// labels can match the platform. Values are stable wire strings.
pub fn host_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "other"
    }
}

/// Percent-encodes an absolute path into a `file://` URI as required by
/// `org.freedesktop.FileManager1.ShowItems`.
pub fn file_uri_for_path(path: &std::path::Path) -> Result<String, String> {
    use std::os::unix::ffi::OsStrExt;

    if !path.is_absolute() {
        return Err(format!("path is not absolute: {}", path.display()));
    }
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(uri)
}

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "linux")]
mod linux {
    use super::file_uri_for_path;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    pub const FILE_MANAGER_BUS_NAME: &str = "org.freedesktop.FileManager1";
    pub const NOTIFICATIONS_BUS_NAME: &str = "org.freedesktop.Notifications";
    const DBUS_METHOD_TIMEOUT: Duration = Duration::from_secs(5);
    const OPENER_WAIT: Duration = Duration::from_secs(5);

    /// Which session bus to use. Production uses the login session bus;
    /// tests can point at a private bus address.
    #[derive(Clone, Debug)]
    pub enum SessionBus {
        Login,
        Address(String),
    }

    fn connect(bus: &SessionBus) -> Result<zbus::blocking::Connection, String> {
        let builder = match bus {
            SessionBus::Login => zbus::blocking::connection::Builder::session(),
            SessionBus::Address(address) => {
                zbus::blocking::connection::Builder::address(address.as_str())
            }
        };
        builder
            .and_then(|builder| builder.method_timeout(DBUS_METHOD_TIMEOUT).build())
            .map_err(|error| format!("the D-Bus session bus is unavailable: {error}"))
    }

    /// Returns whether `name` currently has an owner or can be D-Bus activated.
    pub fn session_bus_name_available(bus: &SessionBus, name: &str) -> Result<bool, String> {
        name_available(&connect(bus)?, name)
    }

    fn name_available(connection: &zbus::blocking::Connection, name: &str) -> Result<bool, String> {
        let proxy = zbus::blocking::fdo::DBusProxy::new(connection)
            .map_err(|error| format!("the D-Bus daemon could not be queried: {error}"))?;
        let bus_name = zbus::names::BusName::try_from(name)
            .map_err(|error| format!("invalid D-Bus name {name}: {error}"))?;
        if proxy
            .name_has_owner(bus_name)
            .map_err(|error| format!("the D-Bus daemon could not be queried: {error}"))?
        {
            return Ok(true);
        }
        let activatable = proxy
            .list_activatable_names()
            .map_err(|error| format!("the D-Bus daemon could not be queried: {error}"))?;
        Ok(activatable
            .iter()
            .any(|candidate| candidate.as_str() == name))
    }

    /// Detects the freedesktop notification service used for desktop alerts.
    pub fn notification_service_available() -> Result<bool, String> {
        session_bus_name_available(&SessionBus::Login, NOTIFICATIONS_BUS_NAME)
    }

    /// Asks the desktop file manager to select `path`, falling back to opening
    /// its parent directory with `xdg-open` when no FileManager1 service exists.
    pub fn reveal_path_in_file_manager(path: &Path) -> Result<(), String> {
        reveal_path_with(path, &SessionBus::Login, Path::new("xdg-open"))
    }

    pub fn reveal_path_with(path: &Path, bus: &SessionBus, opener: &Path) -> Result<(), String> {
        let file_manager_error = match show_items(path, bus) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        open_parent_directory(path, opener).map_err(|opener_error| {
            format!("{file_manager_error}; fallback failed: {opener_error}")
        })
    }

    fn show_items(path: &Path, bus: &SessionBus) -> Result<(), String> {
        let uri = file_uri_for_path(path)?;
        let connection = connect(bus)?;
        if !name_available(&connection, FILE_MANAGER_BUS_NAME)? {
            return Err("no org.freedesktop.FileManager1 service is available".to_string());
        }
        connection
            .call_method(
                Some(FILE_MANAGER_BUS_NAME),
                "/org/freedesktop/FileManager1",
                Some(FILE_MANAGER_BUS_NAME),
                "ShowItems",
                &(vec![uri.as_str()], ""),
            )
            .map(|_| ())
            .map_err(|error| format!("the file manager could not show the item: {error}"))
    }

    fn open_parent_directory(path: &Path, opener: &Path) -> Result<(), String> {
        let directory: PathBuf = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| format!("{} has no parent directory", path.display()))?
        };
        // Spawned directly (no shell) with detached stdio; wait briefly so a
        // missing handler reports an error without blocking on long-lived openers.
        let mut command = Command::new(opener);
        // The file manager and anything it launches must use the desktop's own GStreamer
        // configuration, not the app's filtered plugin directory and registry.
        for variable in crate::render_runtime::APP_GSTREAMER_ENVIRONMENT_VARIABLES {
            command.env_remove(variable);
        }
        let mut child = command
            .arg(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("{} could not start: {error}", opener.display()))?;
        let deadline = Instant::now() + OPENER_WAIT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(status)) => {
                    return Err(format!("{} exited with status {status}", opener.display()))
                }
                Ok(None) if Instant::now() >= deadline => {
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    return Ok(());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                Err(error) => {
                    return Err(format!(
                        "{} could not be observed: {error}",
                        opener.display()
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn host_platform_matches_compile_target() {
        let expected = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "other"
        };
        assert_eq!(host_platform(), expected);
    }

    #[test]
    fn file_uri_percent_encodes_reserved_and_non_ascii_bytes() {
        assert_eq!(
            file_uri_for_path(Path::new("/home/me/Video Creater/clip #1 (é).mp4")).unwrap(),
            "file:///home/me/Video%20Creater/clip%20%231%20%28%C3%A9%29.mp4"
        );
        assert!(file_uri_for_path(Path::new("relative/path")).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reveal_falls_back_to_opener_with_parent_directory_when_bus_is_missing() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("temp dir");
        let record = temp.path().join("opened.txt");
        let opener = temp.path().join("fake-xdg-open");
        std::fs::write(
            &opener,
            format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", record.display()),
        )
        .expect("write fake opener");
        std::fs::set_permissions(&opener, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake opener");
        let item = temp.path().join("model.bin");
        std::fs::write(&item, b"x").expect("write item");

        let bus = SessionBus::Address("unix:path=/nonexistent/video-creater-test-bus".to_string());
        reveal_path_with(&item, &bus, &opener).expect("fallback opener succeeds");
        assert_eq!(
            std::fs::read_to_string(&record).expect("opener record"),
            temp.path().display().to_string()
        );

        let failing = temp.path().join("failing-opener");
        std::fs::write(&failing, "#!/bin/sh\nexit 4\n").expect("write failing opener");
        std::fs::set_permissions(&failing, std::fs::Permissions::from_mode(0o755))
            .expect("chmod failing opener");
        let error = reveal_path_with(&item, &bus, &failing).expect_err("failing opener");
        assert!(
            error.contains("D-Bus session bus is unavailable"),
            "{error}"
        );
        assert!(error.contains("exit status: 4"), "{error}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn missing_session_bus_reports_an_error_not_an_absent_service() {
        let bus = SessionBus::Address("unix:path=/nonexistent/video-creater-test-bus".to_string());
        assert!(session_bus_name_available(&bus, NOTIFICATIONS_BUS_NAME).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires a private session bus (run under dbus-run-session)"]
    fn file_manager_show_items_receives_the_file_uri() {
        use std::sync::{Arc, Mutex};

        type ShowItemsCalls = Arc<Mutex<Vec<(Vec<String>, String)>>>;

        struct FakeFileManager(ShowItemsCalls);

        #[zbus::interface(name = "org.freedesktop.FileManager1")]
        impl FakeFileManager {
            #[zbus(name = "ShowItems")]
            fn show_items(&self, uris: Vec<String>, startup_id: String) {
                self.0.lock().unwrap().push((uris, startup_id));
            }
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let _server = zbus::blocking::connection::Builder::session()
            .expect("session bus")
            .name(FILE_MANAGER_BUS_NAME)
            .expect("bus name")
            .serve_at(
                "/org/freedesktop/FileManager1",
                FakeFileManager(Arc::clone(&calls)),
            )
            .expect("serve file manager")
            .build()
            .expect("own FileManager1");

        let temp = tempfile::tempdir().expect("temp dir");
        let item = temp.path().join("render output.mp4");
        std::fs::write(&item, b"x").expect("write item");
        reveal_path_with(&item, &SessionBus::Login, Path::new("/nonexistent-opener"))
            .expect("ShowItems succeeds");
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            &[(vec![file_uri_for_path(&item).unwrap()], String::new())]
        );
        assert!(session_bus_name_available(&SessionBus::Login, FILE_MANAGER_BUS_NAME).unwrap());
        assert!(!notification_service_available().unwrap());
        let _notifications = zbus::blocking::connection::Builder::session()
            .expect("session bus")
            .name(NOTIFICATIONS_BUS_NAME)
            .expect("bus name")
            .build()
            .expect("own Notifications");
        assert!(notification_service_available().unwrap());
    }
}
