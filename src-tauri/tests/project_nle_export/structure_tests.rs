//! Unit tests for the structure checkers: each rule has a passing base
//! document and a one-edit variant that breaks it.

use super::{check_fcpxml_structure, check_xmeml_structure};

const FCPXML: &str = r#"<fcpxml version="1.10">
  <resources>
    <format id="r1" frameDuration="1/24s"/>
    <effect id="fx" name="Cross Dissolve" uid="x"/>
    <effect id="basic-title" name="Basic Title" uid="y"/>
    <asset id="v" start="0s" duration="240/24s" hasVideo="1" format="r1"/>
    <asset id="a" start="0s" duration="240/24s" hasAudio="1"/>
  </resources>
  <library><event name="e"><project name="p"><sequence format="r1" duration="192/24s">
    <spine>
      <asset-clip ref="v" offset="0/24s" duration="96/24s" start="48/24s">
        <title ref="basic-title" lane="2" offset="60/24s" duration="24/24s"><text><text-style ref="s1">Hi</text-style></text><text-style-def id="s1"><text-style font="x"/></text-style-def></title>
      </asset-clip>
      <transition offset="84/24s" duration="24/24s"><filter-video ref="fx"/></transition>
      <asset-clip ref="v" offset="96/24s" duration="96/24s" start="144/24s">
        <spine lane="-1" offset="144/24s">
          <asset-clip ref="a" offset="0s" duration="48/24s" start="24/24s"/>
          <transition offset="36/24s" duration="24/24s"><filter-audio ref="fx"/></transition>
          <asset-clip ref="a" offset="48/24s" duration="48/24s" start="96/24s"/>
        </spine>
      </asset-clip>
    </spine>
  </sequence></project></event></library>
</fcpxml>"#;

const XMEML: &str = r#"<xmeml version="5"><sequence><media>
  <video><track>
    <clipitem id="c1"><start>0</start><end>-1</end><in>48</in><out>156</out><file><duration>288</duration></file><link><linkclipref>c2</linkclipref></link></clipitem>
    <transitionitem><start>84</start><end>108</end><effect><effecttype>transition</effecttype><mediatype>video</mediatype></effect></transitionitem>
    <clipitem id="c2"><start>-1</start><end>192</end><in>132</in><out>240</out><file><duration>288</duration></file></clipitem>
  </track></video>
  <audio><track><clipitem id="c3"><start>0</start><end>72</end><in>0</in><out>72</out><file><duration>288</duration></file></clipitem></track></audio>
</media></sequence></xmeml>"#;

/// The base document passes, and replacing `from` with `to` breaks exactly the rule named by `expected`.
fn assert_rule(check: fn(&str) -> Vec<String>, base: &str, from: &str, to: &str, expected: &str) {
    assert_eq!(check(base), Vec::<String>::new());
    assert!(base.contains(from), "{from}");
    let problems = check(&base.replacen(from, to, 1));
    assert!(
        problems.iter().any(|problem| problem.contains(expected)),
        "expected {expected:?} in {problems:?}"
    );
}

#[test]
fn fcpxml_root_is_version_1_10() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "version=\"1.10\"",
        "version=\"1.9\"",
        "root is not",
    );
}

#[test]
fn fcpxml_refs_resolve() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "ref=\"fx\"",
        "ref=\"missing\"",
        "ref=\"missing\"> does not resolve",
    );
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "ref=\"s1\"",
        "ref=\"s2\"",
        "ref=\"s2\"> does not resolve",
    );
}

#[test]
fn fcpxml_transitions_sit_between_story_elements() {
    let first = "<spine>\n      <asset-clip";
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        first,
        "<spine>\n      <transition offset=\"0s\" duration=\"1s\"/><asset-clip",
        "first or last",
    );
}

#[test]
fn fcpxml_primary_spine_is_in_time_order() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "<spine>\n      <asset-clip",
        "<spine>\n      <gap offset=\"192/24s\" duration=\"24/24s\"/>\n      <asset-clip",
        "comes after a later <gap>",
    );
}

#[test]
fn fcpxml_primary_story_elements_do_not_overlap() {
    // A lane-less title after the clips is a primary element that overlaps them.
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "\n    </spine>\n  </sequence>",
        "\n      <title ref=\"basic-title\" offset=\"120/24s\" duration=\"24/24s\"/>\n    </spine>\n  </sequence>",
        "primary <title> at offset 120/24s overlaps the <asset-clip> before it",
    );
}

#[test]
fn fcpxml_connected_titles_anchor_inside_their_parent() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "lane=\"2\" offset=\"60/24s\"",
        "lane=\"2\" offset=\"12/24s\"",
        "title lane 2 at offset 12/24s is outside its parent's local time",
    );
}

#[test]
fn fcpxml_storylines_have_a_lane_and_an_anchor() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "<spine lane=\"-1\" offset=\"144/24s\">",
        "<spine offset=\"144/24s\">",
        "has no lane",
    );
}

#[test]
fn fcpxml_storyline_children_carry_no_lane() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "<asset-clip ref=\"a\" offset=\"0s\"",
        "<asset-clip ref=\"a\" lane=\"1\" offset=\"0s\"",
        "carries a lane attribute",
    );
}

#[test]
fn fcpxml_transitions_cover_the_cut() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "<transition offset=\"36/24s\"",
        "<transition offset=\"60/24s\"",
        "does not cover the cut",
    );
}

#[test]
fn fcpxml_storylines_anchor_inside_their_parent() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "<spine lane=\"-1\" offset=\"144/24s\">",
        "<spine lane=\"-1\" offset=\"96/24s\">",
        "outside its parent's local time",
    );
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "<spine lane=\"-1\" offset=\"144/24s\">",
        "<spine lane=\"-1\" offset=\"240/24s\">",
        "outside its parent's local time",
    );
}

#[test]
fn fcpxml_storylines_start_at_zero() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "<asset-clip ref=\"a\" offset=\"0s\"",
        "<asset-clip ref=\"a\" offset=\"24/24s\"",
        "does not start at 0s",
    );
}

#[test]
fn fcpxml_transitions_have_media_handles() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "start=\"48/24s\"",
        "start=\"200/24s\"",
        "outgoing clip's media ends",
    );
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "start=\"144/24s\">",
        "start=\"6/24s\">",
        "incoming clip's media starts",
    );
}

#[test]
fn fcpxml_audio_only_transitions_carry_no_filter_video() {
    assert_rule(
        check_fcpxml_structure,
        FCPXML,
        "<filter-audio ref=\"fx\"/>",
        "<filter-video ref=\"fx\"/><filter-audio ref=\"fx\"/>",
        "carries filter-video",
    );
}

#[test]
fn xmeml_root_is_version_5() {
    assert_rule(
        check_xmeml_structure,
        XMEML,
        "version=\"5\"",
        "version=\"4\"",
        "root is not",
    );
}

#[test]
fn xmeml_transitions_sit_between_open_clip_edges() {
    assert_rule(
        check_xmeml_structure,
        XMEML,
        "<end>-1</end>",
        "<end>96</end>",
        "is not between a clip ending at -1",
    );
}

#[test]
fn xmeml_open_clip_edges_touch_a_transition() {
    assert_rule(
        check_xmeml_structure,
        XMEML,
        "<end>72</end>",
        "<end>-1</end>",
        "c3 ends at -1 without a transition",
    );
}

#[test]
fn xmeml_transition_effects_match_their_section() {
    assert_rule(
        check_xmeml_structure,
        XMEML,
        "<mediatype>video</mediatype>",
        "<mediatype>audio</mediatype>",
        "effect is not a video transition",
    );
}

#[test]
fn xmeml_transitions_start_before_they_end() {
    assert_rule(
        check_xmeml_structure,
        XMEML,
        "<start>84</start><end>108</end>",
        "<start>108</start><end>84</end>",
        "does not start before it ends",
    );
}

#[test]
fn xmeml_clip_ids_are_unique_and_links_resolve() {
    assert_rule(
        check_xmeml_structure,
        XMEML,
        "<clipitem id=\"c3\">",
        "<clipitem id=\"c2\">",
        "c2 is not unique",
    );
    assert_rule(
        check_xmeml_structure,
        XMEML,
        "<linkclipref>c2<",
        "<linkclipref>c9<",
        "linkclipref c9 does not resolve",
    );
}

#[test]
fn xmeml_source_ranges_fit_the_file() {
    assert_rule(
        check_xmeml_structure,
        XMEML,
        "<out>72</out>",
        "<out>300</out>",
        "is not within the file duration",
    );
}
