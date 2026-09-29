//! GES rendering of clip transitions, audio crossfades and audio fades.
//!
//! These tests render real media and need the `ges-render` feature and a
//! resolvable render runtime. On Linux development machines point
//! `VIDEO_CREATER_RENDER_RUNTIME_ROOT` at a staged runtime root.
#![cfg(feature = "ges-render")]

#[path = "render_transitions_ges/audio_speed.rs"]
mod audio_speed;
#[path = "render_transitions_ges/detach_audio.rs"]
mod detach_audio;
#[path = "render_transitions_ges/fixtures.rs"]
mod fixtures;
#[path = "render_transitions_ges/flips.rs"]
mod flips;
#[path = "render_transitions_ges/lottie_handles.rs"]
mod lottie_handles;
#[path = "render_transitions_ges/main_context.rs"]
mod main_context;
#[path = "render_transitions_ges/media.rs"]
mod media;
#[path = "render_transitions_ges/parity.rs"]
mod parity;
#[path = "render_transitions_ges/precomposed.rs"]
mod precomposed;
#[path = "render_transitions_ges/renders.rs"]
mod renders;
#[path = "render_transitions_ges/result_frames.rs"]
mod result_frames;
#[path = "render_transitions_ges/reverse.rs"]
mod reverse;
#[path = "render_transitions_ges/reverse_audio.rs"]
mod reverse_audio;
#[path = "render_transitions_ges/speed.rs"]
mod speed;
#[path = "render_transitions_ges/structure.rs"]
mod structure;
