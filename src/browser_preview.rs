use anyhow::{anyhow, Result};
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
"#;

pub fn open_markdown_preview(markdown_content: &str, md_file_path: &Path) -> Result<()> {
    let html_content = create_markdown_html(markdown_content);

    // Create HTML file with same name as MD file but with .html extension
    let html_file_path = md_file_path.with_extension("html");

    // Write HTML content to file
    fs::write(&html_file_path, html_content)?;

    // Open the HTML file in browser
    webbrowser::open(html_file_path.to_str().unwrap())
        .map_err(|e| anyhow!("Failed to open browser: {}", e))?;

    Ok(())
}

fn create_markdown_html(markdown_content: &str) -> String {
    let mut options = comrak::ComrakOptions::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;

    let html_body = comrak::markdown_to_html(markdown_content, &options);

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Markdown Preview</title>
    <style>{}</style>
</head>
<body>
    {}
</body>
</html>"#,
        GITHUB_CSS, html_body
    )
}
