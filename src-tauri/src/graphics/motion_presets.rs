use super::ir::{AnimationKeyframe, Easing, NodeAnimation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPresetId {
    SlideFadeUpV1,
    SnapPopV1,
    UnderlineWipeV1,
    MetricCountPopV1,
    VerticalRevealV1,
    TrackingDrawV1,
    SpringPopV2,
    SlideRotateSettleV2,
    MaskWipeV2,
    LineDrawV2,
    WordPopStaggerV2,
    SoftDepthCardV2,
    PulseEmphasisV2,
    ExitSnapV2,
}

pub const SUPPORTED_MOTION_PRESETS: &[&str] = &[
    "slide-fade-up-v1",
    "snap-pop-v1",
    "underline-wipe-v1",
    "metric-count-pop-v1",
    "vertical-reveal-v1",
    "tracking-draw-v1",
    "spring-pop-v2",
    "slide-rotate-settle-v2",
    "mask-wipe-v2",
    "line-draw-v2",
    "word-pop-stagger-v2",
    "soft-depth-card-v2",
    "pulse-emphasis-v2",
    "exit-snap-v2",
];

pub fn parse_motion_preset_id(value: &str) -> Option<MotionPresetId> {
    match value.trim() {
        "slide-fade-up-v1" => Some(MotionPresetId::SlideFadeUpV1),
        "snap-pop-v1" => Some(MotionPresetId::SnapPopV1),
        "underline-wipe-v1" => Some(MotionPresetId::UnderlineWipeV1),
        "metric-count-pop-v1" => Some(MotionPresetId::MetricCountPopV1),
        "vertical-reveal-v1" => Some(MotionPresetId::VerticalRevealV1),
        "tracking-draw-v1" => Some(MotionPresetId::TrackingDrawV1),
        "spring-pop-v2" => Some(MotionPresetId::SpringPopV2),
        "slide-rotate-settle-v2" => Some(MotionPresetId::SlideRotateSettleV2),
        "mask-wipe-v2" => Some(MotionPresetId::MaskWipeV2),
        "line-draw-v2" => Some(MotionPresetId::LineDrawV2),
        "word-pop-stagger-v2" => Some(MotionPresetId::WordPopStaggerV2),
        "soft-depth-card-v2" => Some(MotionPresetId::SoftDepthCardV2),
        "pulse-emphasis-v2" => Some(MotionPresetId::PulseEmphasisV2),
        "exit-snap-v2" => Some(MotionPresetId::ExitSnapV2),
        _ => None,
    }
}

pub fn motion_preset_animation(preset: MotionPresetId, node_id: &str) -> NodeAnimation {
    match preset {
        MotionPresetId::SlideFadeUpV1 => slide_fade_up(node_id),
        MotionPresetId::SnapPopV1 => snap_pop(node_id),
        MotionPresetId::UnderlineWipeV1 => underline_wipe(node_id),
        MotionPresetId::MetricCountPopV1 => metric_count_pop(node_id),
        MotionPresetId::VerticalRevealV1 => vertical_reveal(node_id),
        MotionPresetId::TrackingDrawV1 => tracking_draw(node_id),
        MotionPresetId::SpringPopV2 => spring_pop_v2(node_id),
        MotionPresetId::SlideRotateSettleV2 => slide_rotate_settle_v2(node_id),
        MotionPresetId::MaskWipeV2 => mask_wipe_v2(node_id),
        MotionPresetId::LineDrawV2 => line_draw_v2(node_id),
        MotionPresetId::WordPopStaggerV2 => word_pop_stagger_v2(node_id),
        MotionPresetId::SoftDepthCardV2 => soft_depth_card_v2(node_id),
        MotionPresetId::PulseEmphasisV2 => pulse_emphasis_v2(node_id),
        MotionPresetId::ExitSnapV2 => exit_snap_v2(node_id),
    }
}

fn slide_fade_up(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("subline") {
        0.08
    } else {
        0.0
    };
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            keyframe(0.0, Some(-34.0), Some(26.0), Some(0.98), Some(0.0)),
            keyframe(0.16, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.82, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(-10.0), Some(-10.0), Some(1.0), Some(0.0)),
        ],
    }
}

fn snap_pop(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("accent") {
        0.06
    } else {
        0.0
    };
    NodeAnimation {
        ease: Some(Easing::OutBack),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            keyframe(0.0, Some(0.0), Some(18.0), Some(0.72), Some(0.0)),
            keyframe(0.12, Some(0.0), Some(0.0), Some(1.06), Some(1.0)),
            keyframe(0.22, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.84, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(0.0), Some(-8.0), Some(0.98), Some(0.0)),
        ],
    }
}

fn underline_wipe(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: Some(if node_id.contains("accent") {
            0.08
        } else {
            0.0
        }),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            keyframe(0.0, Some(-80.0), Some(0.0), Some(0.12), Some(0.0)),
            keyframe(0.18, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.86, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(36.0), Some(0.0), Some(0.92), Some(0.0)),
        ],
    }
}

fn metric_count_pop(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("subline") {
        0.10
    } else {
        0.0
    };
    NodeAnimation {
        ease: Some(Easing::OutBack),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            keyframe(0.0, Some(42.0), Some(-18.0), Some(0.86), Some(0.0)),
            keyframe(0.18, Some(0.0), Some(0.0), Some(1.04), Some(1.0)),
            keyframe(0.30, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.84, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(28.0), Some(8.0), Some(0.98), Some(0.0)),
        ],
    }
}

fn vertical_reveal(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("subline") {
        0.12
    } else {
        0.0
    };
    NodeAnimation {
        ease: Some(Easing::InOutCubic),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            keyframe(0.0, Some(-28.0), Some(18.0), Some(0.96), Some(0.0)),
            keyframe(0.22, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.82, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(0.0), Some(-20.0), Some(1.0), Some(0.0)),
        ],
    }
}

fn tracking_draw(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("label") || node_id.contains("headline") {
        0.10
    } else {
        0.0
    };
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            keyframe(0.0, Some(-16.0), Some(10.0), Some(0.88), Some(0.0)),
            keyframe(0.20, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.78, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(8.0), Some(-8.0), Some(0.98), Some(0.0)),
        ],
    }
}

fn spring_pop_v2(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::OutBack),
        delay_seconds: node_delay(node_id),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            v2_keyframe(0.0).scale(0.86).opacity(0.0).blur_radius(2.5),
            v2_keyframe(0.16).scale(1.05).opacity(1.0).blur_radius(0.0),
            v2_keyframe(0.28).scale(1.0).opacity(1.0),
            v2_keyframe(0.86).scale(1.0).opacity(1.0),
            v2_keyframe(1.0).scale(0.98).opacity(0.0).blur_radius(1.0),
        ],
    }
}

fn slide_rotate_settle_v2(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: node_delay(node_id),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            v2_keyframe(0.0)
                .x(-42.0)
                .y(14.0)
                .rotation_degrees(-4.0)
                .opacity(0.0)
                .blur_radius(3.0),
            v2_keyframe(0.22)
                .x(0.0)
                .y(0.0)
                .rotation_degrees(1.2)
                .opacity(1.0)
                .blur_radius(0.0),
            v2_keyframe(0.34).rotation_degrees(0.0).opacity(1.0),
            v2_keyframe(0.86).rotation_degrees(0.0).opacity(1.0),
            v2_keyframe(1.0)
                .x(12.0)
                .y(-8.0)
                .rotation_degrees(2.0)
                .opacity(0.0),
        ],
    }
}

fn mask_wipe_v2(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: node_delay(node_id),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            v2_keyframe(0.0).clip_progress(0.0).opacity(0.0),
            v2_keyframe(0.24).clip_progress(1.0).opacity(1.0),
            v2_keyframe(0.86).clip_progress(1.0).opacity(1.0),
            v2_keyframe(1.0).clip_progress(1.0).opacity(0.0),
        ],
    }
}

fn line_draw_v2(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: node_delay(node_id),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            v2_keyframe(0.0)
                .path_progress(0.0)
                .opacity(0.0)
                .glow_opacity(0.0),
            v2_keyframe(0.22)
                .path_progress(1.0)
                .opacity(1.0)
                .glow_opacity(0.34),
            v2_keyframe(0.84)
                .path_progress(1.0)
                .opacity(1.0)
                .glow_opacity(0.18),
            v2_keyframe(1.0)
                .path_progress(1.0)
                .opacity(0.0)
                .glow_opacity(0.0),
        ],
    }
}

fn word_pop_stagger_v2(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::OutBack),
        delay_seconds: node_delay(node_id),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            v2_keyframe(0.0).scale(0.92).opacity(0.0),
            v2_keyframe(0.18).scale(1.04).opacity(1.0),
            v2_keyframe(0.30).scale(1.0).opacity(1.0),
            v2_keyframe(0.86).scale(1.0).opacity(1.0),
            v2_keyframe(1.0).scale(0.98).opacity(0.0),
        ],
    }
}

fn soft_depth_card_v2(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: node_delay(node_id),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            v2_keyframe(0.0)
                .y(18.0)
                .rotation_degrees(-2.5)
                .opacity(0.0)
                .shadow_opacity(0.0)
                .glow_opacity(0.0)
                .blur_radius(2.0),
            v2_keyframe(0.24)
                .y(0.0)
                .rotation_degrees(0.0)
                .opacity(1.0)
                .shadow_opacity(0.55)
                .glow_opacity(0.18)
                .blur_radius(0.0),
            v2_keyframe(0.84)
                .rotation_degrees(0.0)
                .opacity(1.0)
                .shadow_opacity(0.55)
                .glow_opacity(0.14),
            v2_keyframe(1.0)
                .y(-8.0)
                .rotation_degrees(1.5)
                .opacity(0.0)
                .shadow_opacity(0.0)
                .glow_opacity(0.0),
        ],
    }
}

fn pulse_emphasis_v2(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::InOutQuad),
        delay_seconds: node_delay(node_id),
        repeat: Some(1),
        yoyo: Some(true),
        origin: None,
        keyframes: vec![
            v2_keyframe(0.0).scale(1.0).opacity(1.0).glow_opacity(0.12),
            v2_keyframe(1.0)
                .scale(1.035)
                .opacity(1.0)
                .glow_opacity(0.36),
        ],
    }
}

fn exit_snap_v2(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::InCubic),
        delay_seconds: node_delay(node_id),
        repeat: None,
        yoyo: None,
        origin: None,
        keyframes: vec![
            v2_keyframe(0.0).x(0.0).y(0.0).opacity(1.0).blur_radius(0.0),
            v2_keyframe(0.80)
                .x(0.0)
                .y(0.0)
                .opacity(1.0)
                .blur_radius(0.0),
            v2_keyframe(1.0)
                .x(18.0)
                .y(-12.0)
                .opacity(0.0)
                .blur_radius(2.0),
        ],
    }
}

fn node_delay(node_id: &str) -> Option<f64> {
    if node_id.contains("subline") || node_id.contains("label") {
        Some(0.08)
    } else if node_id.contains("accent") {
        Some(0.04)
    } else {
        None
    }
}

fn keyframe(
    at: f64,
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
    opacity: Option<f64>,
) -> AnimationKeyframe {
    AnimationKeyframe {
        at,
        x,
        y,
        scale,
        scale_x: None,
        scale_y: None,
        opacity,
        rotation_degrees: None,
        blur_radius: None,
        shadow_opacity: None,
        glow_opacity: None,
        clip_progress: None,
        path_progress: None,
    }
}

fn v2_keyframe(at: f64) -> AnimationKeyframe {
    AnimationKeyframe {
        at,
        ..AnimationKeyframe::default()
    }
}

impl AnimationKeyframe {
    fn x(mut self, value: f64) -> Self {
        self.x = Some(value);
        self
    }

    fn y(mut self, value: f64) -> Self {
        self.y = Some(value);
        self
    }

    fn scale(mut self, value: f64) -> Self {
        self.scale = Some(value);
        self
    }

    fn opacity(mut self, value: f64) -> Self {
        self.opacity = Some(value);
        self
    }

    fn rotation_degrees(mut self, value: f64) -> Self {
        self.rotation_degrees = Some(value);
        self
    }

    fn blur_radius(mut self, value: f64) -> Self {
        self.blur_radius = Some(value);
        self
    }

    fn shadow_opacity(mut self, value: f64) -> Self {
        self.shadow_opacity = Some(value);
        self
    }

    fn glow_opacity(mut self, value: f64) -> Self {
        self.glow_opacity = Some(value);
        self
    }

    fn clip_progress(mut self, value: f64) -> Self {
        self.clip_progress = Some(value);
        self
    }

    fn path_progress(mut self, value: f64) -> Self {
        self.path_progress = Some(value);
        self
    }
}
