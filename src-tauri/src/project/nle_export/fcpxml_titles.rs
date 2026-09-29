//! DaVinci FCPXML titles for captions and text overlays.
//!
//! The FCPXML DTD requires every `<title>` to reference an effect, so titles
//! reference FCP's Basic Title (uid verified from Final Cut Pro exports, see
//! `docs/research/2026-09-16-nle-transition-interchange-references.md`, R4/R5).

use super::{format_fcpxml_number, xml_escape, NleCaption, NleTextOverlay};
use crate::project::model::TimelineItem;

const FCPXML_BASIC_TITLE_ID: &str = "vc-title-basic";

/// Declares the Basic Title effect the titles reference.
pub(super) fn push_fcpxml_basic_title_effect(xml: &mut String) {
    xml.push_str(&format!(
        "    <effect id=\"{FCPXML_BASIC_TITLE_ID}\" name=\"Basic Title\" uid=\".../Titles.localized/Bumper:Opener.localized/Basic Title.localized/Basic Title.moti\"/>\n"
    ));
}

pub(super) fn push_fcpxml_text_overlay(xml: &mut String, overlay: &NleTextOverlay<'_>, fps: i64) {
    push_fcpxml_title(
        xml,
        (overlay.item, overlay.text),
        1,
        (overlay.timeline_start_frames, overlay.duration_frames),
        fps,
        12,
    );
}

/// Writes a caption as a connected title on `lane` at `offset_frames` in its
/// parent's time: an anchor host's local time, or the sequence spine's.
pub(super) fn push_fcpxml_caption(
    xml: &mut String,
    caption: &NleCaption<'_>,
    lane: i64,
    offset_frames: i64,
    fps: i64,
    indent: usize,
) {
    push_fcpxml_title(
        xml,
        (caption.item, caption.text),
        lane,
        (offset_frames, caption.duration_frames),
        fps,
        indent,
    );
}

fn push_fcpxml_title(
    xml: &mut String,
    (item, text): (&TimelineItem, &str),
    lane: i64,
    (offset_frames, duration_frames): (i64, i64),
    fps: i64,
    indent: usize,
) {
    let pad = " ".repeat(indent);
    xml.push_str(&format!(
        "{pad}<title name=\"{}\" ref=\"{FCPXML_BASIC_TITLE_ID}\" lane=\"{lane}\" offset=\"{offset_frames}/{fps}s\" duration=\"{duration_frames}/{fps}s\">\n",
        xml_escape(&item.label),
    ));
    xml.push_str(&format!("{pad}  <text>\n"));
    xml.push_str(&format!(
        "{pad}    <text-style ref=\"{}-style\">{}</text-style>\n",
        xml_escape(&item.id),
        xml_escape(text)
    ));
    xml.push_str(&format!("{pad}  </text>\n"));
    xml.push_str(&format!(
        "{pad}  <text-style-def id=\"{}-style\">\n",
        xml_escape(&item.id)
    ));
    xml.push_str(&format!(
        "{pad}    <text-style {}/>\n",
        fcpxml_text_style_attributes(item)
    ));
    xml.push_str(&format!("{pad}  </text-style-def>\n"));
    xml.push_str(&format!("{pad}</title>\n"));
}

pub(super) fn fcpxml_text_style_attributes(item: &TimelineItem) -> String {
    let font_name = item
        .properties
        .get("fontName")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Helvetica");
    let font_size = item
        .properties
        .get("fontSize")
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(48.0);
    let color = item
        .properties
        .get("color")
        .and_then(serde_json::Value::as_str)
        .and_then(fcpxml_color_string)
        .unwrap_or_else(|| "1 1 1 1".to_string());
    let alignment = item
        .properties
        .get("alignment")
        .and_then(serde_json::Value::as_str)
        .filter(|value| matches!(*value, "left" | "center" | "right"))
        .unwrap_or("center");
    format!(
        "font=\"{}\" fontFace=\"{}\" fontSize=\"{}\" fontColor=\"{}\" alignment=\"{}\"",
        xml_escape(&fcpxml_font_family(font_name)),
        xml_escape(&fcpxml_font_face(font_name)),
        format_fcpxml_number(font_size),
        xml_escape(&color),
        xml_escape(alignment)
    )
}

pub(super) fn fcpxml_font_family(font_name: &str) -> String {
    font_name
        .split_once('-')
        .map(|(family, _)| family)
        .unwrap_or(font_name)
        .trim()
        .to_string()
}

pub(super) fn fcpxml_font_face(font_name: &str) -> String {
    let face = font_name
        .split_once('-')
        .map(|(_, face)| face.trim())
        .filter(|face| !face.is_empty());
    match face {
        Some("BoldItalic") | Some("Bold-Italic") | Some("Bold Italic") => "Bold Italic".to_string(),
        Some("Bold") => "Bold".to_string(),
        Some("Italic") => "Italic".to_string(),
        Some(other) => other.replace('-', " "),
        None => "Regular".to_string(),
    }
}

pub(super) fn fcpxml_color_string(value: &str) -> Option<String> {
    let hex = value.trim().strip_prefix('#')?;
    let (red, green, blue, alpha) = match hex.len() {
        6 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            255,
        ),
        8 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some(format!(
        "{} {} {} {}",
        format_fcpxml_number(f64::from(red) / 255.0),
        format_fcpxml_number(f64::from(green) / 255.0),
        format_fcpxml_number(f64::from(blue) / 255.0),
        format_fcpxml_number(f64::from(alpha) / 255.0)
    ))
}
