use wdys::toast::toast_xml;

#[test]
fn toast_contains_title_and_body_as_text_lines() {
    let xml = toast_xml("Select your key", "Press the key you want to use.");
    assert!(xml.starts_with("<toast>") && xml.ends_with("</toast>"));
    assert!(xml.contains("<text id=\"1\">Select your key</text>"));
    assert!(xml.contains("<text id=\"2\">Press the key you want to use.</text>"));
}

#[test]
fn toast_text_is_xml_escaped() {
    let xml = toast_xml("A & B <c>", "say \"hi\" & 'bye'");
    assert!(xml.contains("A &amp; B &lt;c&gt;"));
    assert!(xml.contains("say &quot;hi&quot; &amp; &apos;bye&apos;"));
    assert!(!xml.contains("<c>"));
}
