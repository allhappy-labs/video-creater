//! Byte-for-byte golden exports for feature projects (captions, text overlays
//! and effect keyframes), so refactors of the writers can't change output.

use video_creater_lib::project::model::VideoProject;
use video_creater_lib::project::nle_export::{export_project_timeline_to_nle_xml, NleXmlFormat};

fn export(project: &VideoProject, format: NleXmlFormat) -> String {
    export_project_timeline_to_nle_xml(project, format)
        .expect("export")
        .xml
}

#[test]
fn feature_projects_export_byte_identical_xml() {
    let cases = [
        (
            "caption",
            super::sample_project_with_caption(),
            include_str!("../fixtures/nle_export/caption.xml"),
            include_str!("../fixtures/nle_export/caption.fcpxml"),
        ),
        (
            "text-overlay",
            super::sample_project_with_text_overlay(),
            include_str!("../fixtures/nle_export/text-overlay.xml"),
            include_str!("../fixtures/nle_export/text-overlay.fcpxml"),
        ),
        (
            "effect-keyframes",
            super::sample_project_with_effect_keyframes(),
            include_str!("../fixtures/nle_export/effect-keyframes.xml"),
            include_str!("../fixtures/nle_export/effect-keyframes.fcpxml"),
        ),
    ];
    for (name, project, xmeml, fcpxml) in cases {
        assert_eq!(
            export(&project, NleXmlFormat::PremiereXmeml),
            xmeml,
            "{name}.xml"
        );
        assert_eq!(
            export(&project, NleXmlFormat::DavinciFcpxml),
            fcpxml,
            "{name}.fcpxml"
        );
    }
}
