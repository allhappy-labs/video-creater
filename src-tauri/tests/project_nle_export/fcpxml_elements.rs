//! FCPXML element shapes required by the FCPXML 1.10 DTD: titles reference a
//! declared Basic Title effect, and an asset clip's note comes first.

use video_creater_lib::project::nle_export::{export_project_timeline_to_nle_xml, NleXmlFormat};

const BASIC_TITLE_EFFECT: &str = "<effect id=\"vc-title-basic\" name=\"Basic Title\" uid=\".../Titles.localized/Bumper:Opener.localized/Basic Title.localized/Basic Title.moti\"/>";

fn fcpxml(project: &video_creater_lib::project::model::VideoProject) -> String {
    export_project_timeline_to_nle_xml(project, NleXmlFormat::DavinciFcpxml)
        .expect("export fcpxml")
        .xml
}

#[test]
fn fcpxml_declares_the_basic_title_effect_once_when_titles_exist() {
    for project in [
        super::sample_project_with_caption(),
        super::sample_project_with_text_overlay(),
    ] {
        let xml = fcpxml(&project);
        assert_eq!(xml.matches(BASIC_TITLE_EFFECT).count(), 1, "{xml}");
    }

    let xml = fcpxml(&super::sample_project_with_effect_keyframes());
    assert!(!xml.contains("vc-title-basic"), "{xml}");
}

#[test]
fn fcpxml_asset_clip_notes_come_first() {
    let xml = fcpxml(&super::sample_project_with_effect_keyframes());

    let open = xml
        .find("<asset-clip name=\"Opening clip\"")
        .expect("asset clip");
    let children = &xml[open + xml[open..].find(">\n").expect("open tag end") + 2..];
    assert!(
        children.trim_start().starts_with("<note>"),
        "the note should directly follow the asset-clip open tag: {xml}"
    );
}
