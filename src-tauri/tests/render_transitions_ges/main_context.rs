//! GES timelines must build without the process-wide default GLib main context.
//!
//! In the desktop app GTK and WebKitGTK own that context on the main thread. A GES asset request
//! that runs a nested loop on it either waits for the main thread (a render on a worker thread) or
//! dispatches webview IPC re-entrantly (a render on the main thread).

use super::fixtures::*;
use super::structure::summary;
use gstreamer::glib;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

/// The tests below take the default context; running them one at a time keeps the ownership each
/// one sets up.
static DEFAULT_CONTEXT: Mutex<()> = Mutex::new(());

const BUILD_TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn ges_timeline_builds_while_another_thread_owns_the_default_main_context() {
    let _serial = DEFAULT_CONTEXT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = project_dir();
    let plan = plan(dir.path(), &two_clip_project(None), "default-context-owned");

    // Like the GTK main thread blocked in a command: it owns the context but does not iterate it.
    let (owned_tx, owned_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let owner = std::thread::spawn(move || {
        let context = glib::MainContext::default();
        let _acquired = context.acquire().expect("own the default main context");
        owned_tx.send(()).expect("report ownership");
        let _ = release_rx.recv();
    });
    owned_rx.recv().expect("default main context is owned");

    let project_dir = dir.path().to_path_buf();
    let (built_tx, built_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = built_tx.send(summary(&project_dir, &plan));
    });
    let built = built_rx.recv_timeout(BUILD_TIMEOUT);
    release_tx
        .send(())
        .expect("release the default main context");
    owner.join().expect("default main context owner");

    let summary = built.expect("GES loads assets without iterating the default main context");
    assert!(
        summary.contains("GESUriClip"),
        "unexpected summary: {summary}"
    );
}

#[test]
fn ges_timeline_build_does_not_dispatch_the_default_main_context() {
    let _serial = DEFAULT_CONTEXT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = project_dir();
    let plan = plan(dir.path(), &two_clip_project(None), "default-context-idle");

    // Like the GTK main thread running a synchronous command: it owns the context, and pending
    // webview IPC waits on it.
    let context = glib::MainContext::default();
    let _acquired = context.acquire().expect("own the default main context");
    let dispatched = Arc::new(AtomicBool::new(false));
    let pending = glib::idle_add_once({
        let dispatched = Arc::clone(&dispatched);
        move || dispatched.store(true, Ordering::SeqCst)
    });

    let summary = summary(dir.path(), &plan);

    assert!(
        !dispatched.load(Ordering::SeqCst),
        "building a GES timeline dispatched a default main context source"
    );
    pending.remove();
    assert!(
        summary.contains("GESUriClip"),
        "unexpected summary: {summary}"
    );
}
