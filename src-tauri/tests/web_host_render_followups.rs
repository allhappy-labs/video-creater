//! Real Linux authenticated host/dispatcher/native worker integration.
//! Long encoding is proven by advancing GStreamer progress and output bytes.
#![cfg(all(target_os = "linux", feature = "web-host", feature = "ges-render"))]

#[path = "web_host_render_followups/helpers.rs"]
mod helpers;
#[path = "web_host_render_followups/legacy.rs"]
mod legacy;
#[path = "web_host_render_followups/process_tests.rs"]
mod process_tests;
