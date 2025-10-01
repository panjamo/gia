use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::Path;

const GITHUB_CSS: &str = r#"
body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Roboto", "Helvetica Neue", Arial, sans-serif;
    line-height: 1.6;
    color: #333;
    max-width: 800px;
    margin: 0 auto;
    padding: 20px;
    background-color: #fff;
}
h1, h2, h3, h4, h5, h6 {
    margin-top: 24px;
    margin-bottom: 16px;
    font-weight: 600;
    line-height: 1.25;
}
h1, h2 {
    padding-bottom: 0.3em;
    border-bottom: 1px solid #eaecef;
}
code {
    background-color: #f6f8fa;
    padding: 0.2em 0.4em;
    border-radius: 3px;
    font-size: 85%;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
}
pre {
    background-color: #f6f8fa;
    border-radius: 6px;
    padding: 16px;
    overflow: auto;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
}
pre code {
    background-color: transparent;
    padding: 0;
}
blockquote {
    border-left: 4px solid #dfe2e5;
    padding: 0 16px;
    color: #6a737d;
    margin: 16px 0;
}
ul, ol {
    padding-left: 30px;
    margin: 16px 0;
}
table {
    border-collapse: collapse;
    border-spacing: 0;
    width: 100%;
    margin: 16px 0;
}
table th, table td {
    border: 1px solid #d1d5da;
    padding: 8px 16px;
    text-align: left;
    vertical-align: top;
}
table th {
    background-color: #f6f8fa;
    font-weight: 600;
    border-bottom: 2px solid #d1d5da;
}
table tbody tr:nth-child(even) {
    background-color: #f8f9fa;
}
table tbody tr:hover {
    background-color: #f1f3f4;
}
p {
    margin: 16px 0;
}
img {
    max-width: 100%;
    height: auto;
}
hr {
    border: none;
    border-top: 1px solid #eaecef;
    margin: 24px 0;
}
.gia-footer {
    margin-top: 40px;
    padding: 16px;
    background-color: #f8f9fa;
    border-top: 1px solid #dee2e6;
    font-size: 0.7em;
    color: #6c757d;
    line-height: 1.6;
}
.gia-footer h4 {
    margin: 0 0 8px 0;
    font-size: 1.1em;
    color: #495057;
    border: none;
    padding: 0;
}
.gia-footer p {
    margin: 4px 0;
}
.gia-footer ul {
    margin: 4px 0;
    padding-left: 20px;
}
.gia-prompt {
    margin-bottom: 24px;
    padding: 16px;
    background-color: #f0f6ff;
    border-left: 4px solid #0969da;
    border-radius: 6px;
}
.gia-prompt h3 {
    margin: 0 0 8px 0;
    font-size: 1em;
    color: #0969da;
    border: none;
    padding: 0;
}
.gia-prompt p {
    margin: 0;
    color: #1f2328;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
    font-size: 0.9em;
}
"#;

pub struct FooterMetadata {
    pub model_name: String,
    pub provider_name: String,
    pub timestamp: DateTime<Utc>,
    pub image_files: Vec<String>,
    pub text_files: Vec<String>,
    pub has_clipboard: bool,
    pub has_audio: bool,
    pub has_stdin: bool,
    pub prompt: String,
}

pub fn open_markdown_preview(
    markdown_content: &str,
    md_file_path: &Path,
    metadata: Option<&FooterMetadata>,
) -> Result<()> {
    let html_content = create_markdown_html(markdown_content, metadata);

    // Create HTML file with same name as MD file but with .html extension
    let html_file_path = md_file_path.with_extension("html");

    // Write HTML content to file
    fs::write(&html_file_path, html_content)?;

    // Open the HTML file in browser
    webbrowser::open(html_file_path.to_str().unwrap())
        .map_err(|e| anyhow!("Failed to open browser: {e}"))?;

    Ok(())
}

fn build_footer_html(metadata: &FooterMetadata) -> String {
    let mut footer = String::from(r#"<div class="gia-footer">"#);
    footer.push_str(&format!(r#"<h4>🤖 Powered by <a href="https://github.com/panjamo/gia" target="_blank">GIA v{}</a></h4>"#, env!("GIA_VERSION")));

    // Timestamp
    footer.push_str(&format!(
        "<p><strong>Generated:</strong> {}</p>",
        metadata.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // Model
    footer.push_str(&format!(
        "<p><strong>Model:</strong> {}::{}</p>",
        metadata.provider_name, metadata.model_name
    ));

    // Inputs section
    if !metadata.image_files.is_empty()
        || !metadata.text_files.is_empty()
        || metadata.has_clipboard
        || metadata.has_audio
        || metadata.has_stdin
    {
        footer.push_str("<p><strong>Inputs:</strong></p><ul>");

        if !metadata.image_files.is_empty() {
            footer.push_str(&format!(
                "<li>Images: {}</li>",
                metadata.image_files.join(", ")
            ));
        }

        if !metadata.text_files.is_empty() {
            footer.push_str(&format!(
                "<li>Text files: {}</li>",
                metadata.text_files.join(", ")
            ));
        }

        if metadata.has_clipboard {
            footer.push_str("<li>Clipboard content</li>");
        }

        if metadata.has_audio {
            footer.push_str("<li>Audio recording</li>");
        }

        if metadata.has_stdin {
            footer.push_str("<li>Stdin content</li>");
        }

        footer.push_str("</ul>");
    }

    footer.push_str("</div>");
    footer
}

fn build_prompt_header(metadata: &FooterMetadata) -> String {
    // Don't show prompt header if prompt is empty
    if metadata.prompt.is_empty() {
        return String::new();
    }

    let prompt_escaped = html_escape::encode_text(&metadata.prompt);
    format!(
        r#"<div class="gia-prompt">
<h3>💬 Prompt</h3>
<p>{}</p>
</div>"#,
        prompt_escaped
    )
}

fn create_markdown_html(markdown_content: &str, metadata: Option<&FooterMetadata>) -> String {
    let mut options = comrak::ComrakOptions::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;

    let html_body = comrak::markdown_to_html(markdown_content, &options);

    let prompt_header = metadata.map(build_prompt_header).unwrap_or_default();
    let footer_html = metadata.map(build_footer_html).unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Markdown Preview</title>
    <style>{GITHUB_CSS}</style>
</head>
<body>
    {prompt_header}
    {html_body}
    {footer_html}
</body>
</html>"#
    )
}
