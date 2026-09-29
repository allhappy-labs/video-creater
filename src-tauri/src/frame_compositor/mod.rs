pub mod backend;
pub mod blend;
pub mod effects;
#[cfg(feature = "gpu-render")]
pub mod gpu;
pub mod lut;
pub mod program;
pub mod transition;

pub use backend::{BlendBackendReport, FrameBlendCompositor};
pub use blend::{composite_rgba8_srgb, BlendMode};
pub use effects::{EffectStackError, PreparedEffect, PreparedEffectStack, SUPPORTED_CPU_EFFECTS};
#[cfg(feature = "gpu-render")]
pub use gpu::{GpuFrameCompositor, GpuFrameCompositorError};
pub use lut::{CubeLut, LutError};
pub use program::{
    transform_rgba8_srgb, CanvasTransform, FrameProgram, FrameProgramError, KeyframeEasing,
    NumericCurve, NumericKeyframe, SampledFrameProgram,
};
pub use transition::{
    apply_wipe_mask_rgba8, dissolve_rgba8_srgb, transition_solid_rgba8, wipe_column_visible,
    TransitionFrameState, TransitionRole, TransitionWindow,
};
