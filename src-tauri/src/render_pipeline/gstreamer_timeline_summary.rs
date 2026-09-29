//! Deterministic text summaries of committed GES timelines.
//!
//! Structural tests compare these summaries to check what the GES backend
//! builds (layers, clips, effects, child properties and control envelopes)
//! without rendering.

use ges::prelude::*;
use gst::glib;
use gstreamer as gst;
use gstreamer_editing_services as ges;
use std::fmt::Write;

/// Child properties reported for every track element that exposes them.
const REPORTED_CHILD_PROPERTIES: &[&str] = &[
    "alpha", "posx", "posy", "width", "height", "volume", "angle", "top", "right", "bottom", "left",
];

#[link(name = "gstcontroller-1.0")]
unsafe extern "C" {
    fn gst_timed_value_control_source_get_all(
        source: *mut glib::gobject_ffi::GObject,
    ) -> *mut glib::ffi::GList;
}

#[repr(C)]
struct GstTimedValue {
    timestamp: u64,
    value: f64,
}

/// Summarizes `timeline`, replacing `strip_prefix` in clip URIs with `<dir>`.
pub(super) fn summarize_ges_timeline(timeline: &ges::Timeline, strip_prefix: &str) -> String {
    let mut summary = String::new();
    let mut layers = timeline.layers();
    layers.sort_by_key(|layer| layer.priority());
    for layer in layers {
        let _ = writeln!(summary, "layer priority={}", layer.priority());
        let mut clips = layer.clips();
        clips.sort_by_key(|clip| (clip.start(), clip.name()));
        for clip in clips {
            summarize_clip(&mut summary, &clip, strip_prefix);
        }
    }
    summary
}

fn summarize_clip(summary: &mut String, clip: &ges::Clip, strip_prefix: &str) {
    let _ = write!(
        summary,
        "  {} start={} inpoint={} duration={}",
        clip.type_().name(),
        seconds(clip.start()),
        seconds(clip.inpoint()),
        seconds(clip.duration()),
    );
    if let Some(uri_clip) = clip.downcast_ref::<ges::UriClip>() {
        let uri = uri_clip.uri().to_string();
        let file_uri = format!("file://{strip_prefix}");
        let _ = write!(summary, " uri={}", uri.replace(&file_uri, "<dir>"));
    }
    if let Some(test_clip) = clip.downcast_ref::<ges::TestClip>() {
        let _ = write!(summary, " vpattern={:?}", test_clip.vpattern());
    }
    let _ = writeln!(summary);
    let effects = clip.top_effects();
    let mut children = clip
        .children(false)
        .into_iter()
        .filter_map(|child| child.downcast::<ges::TrackElement>().ok())
        .filter(|child| {
            !effects
                .iter()
                .any(|effect| effect.upcast_ref::<ges::TrackElement>() == child)
        })
        .collect::<Vec<_>>();
    children.sort_by_key(|child| child.type_().name());
    for child in children {
        summarize_track_element(summary, &child, "core");
    }
    for effect in effects {
        summarize_track_element(summary, effect.upcast_ref(), "effect");
    }
}

fn summarize_track_element(summary: &mut String, element: &ges::TrackElement, role: &str) {
    let _ = write!(
        summary,
        "    {role} {} track={:?}",
        element.type_().name(),
        element.track_type()
    );
    if let Some(effect) = element.downcast_ref::<ges::Effect>() {
        let _ = write!(summary, " bin={:?}", effect.bin_description());
    }
    let _ = writeln!(summary);
    for property in REPORTED_CHILD_PROPERTIES {
        if TimelineElementExt::lookup_child(element, property).is_none() {
            continue;
        }
        let value = TimelineElementExt::child_property(element, property)
            .map(|value| format_value(&value))
            .unwrap_or_else(|| "?".to_string());
        let _ = write!(summary, "      {property}={value}");
        if let Some(binding) = element.control_binding(property) {
            let _ = write!(summary, " binding={}", format_binding(&binding));
        }
        let _ = writeln!(summary);
    }
}

fn format_binding(binding: &gst::ControlBinding) -> String {
    let absolute = binding
        .find_property("absolute")
        .map(|_| binding.property::<bool>("absolute"))
        .unwrap_or(false);
    let Some(source) = binding
        .find_property("control-source")
        .and_then(|_| binding.property::<Option<gst::ControlSource>>("control-source"))
    else {
        return "none".to_string();
    };
    let mode = source
        .find_property("mode")
        .map(|_| format_value(&source.property_value("mode")))
        .unwrap_or_else(|| "?".to_string());
    let points = control_points(&source)
        .into_iter()
        .map(|(timestamp, value)| format!("{}:{value:.4}", seconds_u64(timestamp)))
        .collect::<Vec<_>>();
    format!(
        "{}/{mode}[{}]",
        if absolute { "absolute" } else { "direct" },
        points.join(", ")
    )
}

/// Control points of a timed-value control source, in timestamp order.
pub(super) fn control_points(source: &gst::ControlSource) -> Vec<(u64, f64)> {
    let mut points = Vec::new();
    unsafe {
        let list = gst_timed_value_control_source_get_all(source.as_ptr() as *mut _);
        let mut node = list;
        while !node.is_null() {
            let value = (*node).data as *const GstTimedValue;
            if !value.is_null() {
                points.push(((*value).timestamp, (*value).value));
            }
            node = (*node).next;
        }
        glib::ffi::g_list_free(list);
    }
    points
}

fn format_value(value: &glib::Value) -> String {
    if let Some((_, enum_value)) = glib::EnumValue::from_value(value) {
        return enum_value.nick().to_string();
    }
    if let Ok(number) = value.get::<f64>() {
        return format!("{number:.4}");
    }
    if let Ok(number) = value.get::<i32>() {
        return number.to_string();
    }
    format!("{value:?}")
}

fn seconds(time: gst::ClockTime) -> String {
    seconds_u64(time.nseconds())
}

fn seconds_u64(nanoseconds: u64) -> String {
    format!("{:.6}", nanoseconds as f64 / 1_000_000_000.0)
}
