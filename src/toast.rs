fn escape(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            other => other.to_string(),
        })
        .collect()
}

/// Builds the XML for a two-line Windows toast (a bold title and one line of text).
pub fn toast_xml(title: &str, body: &str) -> String {
    format!(
        "<toast><visual><binding template=\"ToastGeneric\">\
         <text id=\"1\">{}</text><text id=\"2\">{}</text>\
         </binding></visual></toast>",
        escape(title),
        escape(body)
    )
}
