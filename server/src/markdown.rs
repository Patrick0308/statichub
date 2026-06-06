use pulldown_cmark::{html, CowStr, Event, Options, Parser};

const DEFAULT_TITLE: &str = "StaticHub Markdown";

pub fn render_markdown_document(content: &[u8]) -> Result<Vec<u8>, String> {
    let markdown = std::str::from_utf8(content)
        .map_err(|_| "Markdown file must be valid UTF-8".to_string())?;

    let title = extract_title(markdown).unwrap_or(DEFAULT_TITLE);
    let body = render_markdown_body(markdown);
    let document = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{}</style>\n</head>\n<body>\n<main>\n{}\n</main>\n</body>\n</html>\n",
        escape_html(title),
        DOCUMENT_CSS,
        body
    );

    Ok(document.into_bytes())
}

fn render_markdown_body(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options).map(|event| match event {
        Event::Html(value) | Event::InlineHtml(value) => {
            Event::Text(CowStr::from(value.into_string()))
        }
        other => other,
    });

    let mut body = String::new();
    html::push_html(&mut body, parser);
    body
}

fn extract_title(markdown: &str) -> Option<&str> {
    markdown.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let title = trimmed.strip_prefix("# ")?;
        let title = title.trim();
        if title.is_empty() {
            None
        } else {
            Some(title)
        }
    })
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

const DOCUMENT_CSS: &str = r#"body{margin:0;background:#f7f7f4;color:#202124;font-family:ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;line-height:1.65}main{box-sizing:border-box;width:min(100%,760px);margin:0 auto;padding:48px 24px 72px}h1,h2,h3{line-height:1.2;color:#111;margin:1.6em 0 .55em}h1{font-size:2.25rem;margin-top:0}h2{font-size:1.55rem;border-bottom:1px solid #ddd;padding-bottom:.25rem}h3{font-size:1.2rem}p,ul,ol,blockquote,pre,table{margin:0 0 1.1rem}a{color:#0b63ce}blockquote{border-left:4px solid #c7d2da;padding-left:1rem;color:#4c5963}code{background:#ecefea;border-radius:4px;padding:.12em .32em;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.92em}pre{overflow:auto;background:#202124;color:#f4f4f0;border-radius:8px;padding:1rem}pre code{background:transparent;color:inherit;padding:0}table{border-collapse:collapse;width:100%;display:block;overflow:auto}th,td{border:1px solid #d8d8d2;padding:.45rem .6rem}th{background:#ecefea;text-align:left}img{max-width:100%;height:auto}"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_heading_and_paragraph() {
        let html =
            String::from_utf8(render_markdown_document(b"# Hello\n\nThis is **bold**.").unwrap())
                .unwrap();

        assert!(html.contains("<title>Hello</title>"));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.starts_with("<!doctype html>"));
    }

    #[test]
    fn rejects_non_utf8_markdown() {
        let err = render_markdown_document(&[0xff, 0xfe]).unwrap_err();

        assert!(err.contains("Markdown file must be valid UTF-8"));
    }

    #[test]
    fn escapes_raw_html_in_markdown_body() {
        let html = String::from_utf8(
            render_markdown_document(b"# Safe\n\n<script>alert('x')</script>").unwrap(),
        )
        .unwrap();

        assert!(html.contains("&lt;script&gt;alert('x')&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn escapes_html_in_title() {
        let html = String::from_utf8(render_markdown_document(b"# A < B & C").unwrap()).unwrap();

        assert!(html.contains("<title>A &lt; B &amp; C</title>"));
    }
}
