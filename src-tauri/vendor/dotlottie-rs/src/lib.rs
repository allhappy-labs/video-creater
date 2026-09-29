mod event_queue;
#[cfg(feature = "dotlottie")]
mod fms;
mod layout;
mod lottie_renderer;
mod player;
mod poll_events;
mod result;
pub mod string;
mod tween;

pub mod tools;
#[cfg(feature = "dotlottie")]
pub use fms::*;
pub use layout::*;
pub use lottie_renderer::*;
pub use player::*;
pub use poll_events::*;
pub use result::*;
pub use tween::TweenStatus;
#[cfg(any(
    feature = "audio",
    feature = "c_api",
    feature = "dev",
    feature = "state-machines",
    feature = "theming",
    feature = "tracking_allocator",
    feature = "tvg-gl",
    feature = "tvg-log",
    feature = "tvg-otf",
    feature = "tvg-simd",
    feature = "tvg-threads",
    feature = "tvg-ttf",
    feature = "tvg-wg",
    feature = "wasm-bindgen-api",
    feature = "webgl",
    feature = "webgpu"
))]
compile_error!("this vendored dotLottie runtime supports only the reviewed native CPU feature set");
