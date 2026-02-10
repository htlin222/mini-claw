fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn markdown_to_html(text: &str) -> String {
    // Use null character as delimiter for placeholders (same approach as TS version)
    const PH: char = '\x00';

    // Extract and preserve code blocks
    let mut code_blocks: Vec<String> = Vec::new();
    let mut processed = {
        let re = regex::Regex::new(r"```(\w*)\n?([\s\S]*?)```").unwrap();
        re.replace_all(text, |caps: &regex::Captures| {
            let idx = code_blocks.len();
            let code = caps.get(2).map_or("", |m| m.as_str()).trim();
            code_blocks.push(format!("<pre>{}</pre>", escape_html(code)));
            format!("{PH}CODE_BLOCK_{idx}{PH}")
        })
        .into_owned()
    };

    // Extract inline code
    let mut inline_codes: Vec<String> = Vec::new();
    processed = {
        let re = regex::Regex::new(r"`([^`]+)`").unwrap();
        re.replace_all(&processed, |caps: &regex::Captures| {
            let idx = inline_codes.len();
            let code = caps.get(1).map_or("", |m| m.as_str());
            inline_codes.push(format!("<code>{}</code>", escape_html(code)));
            format!("{PH}INLINE_CODE_{idx}{PH}")
        })
        .into_owned()
    };

    // Escape HTML in remaining text
    processed = escape_html(&processed);

    // Bold: **text** or __text__
    let re = regex::Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    processed = re.replace_all(&processed, "<b>$1</b>").into_owned();
    let re = regex::Regex::new(r"__([^_]+)__").unwrap();
    processed = re.replace_all(&processed, "<b>$1</b>").into_owned();

    // Italic: *text* or _text_ (bold ** already processed above)
    let re = regex::Regex::new(r"\*([^*]+)\*").unwrap();
    processed = re.replace_all(&processed, "<i>$1</i>").into_owned();
    let re = regex::Regex::new(r"\b_([^_]+)_\b").unwrap();
    processed = re.replace_all(&processed, "<i>$1</i>").into_owned();

    // Strikethrough: ~~text~~
    let re = regex::Regex::new(r"~~([^~]+)~~").unwrap();
    processed = re.replace_all(&processed, "<s>$1</s>").into_owned();

    // Links: [text](url)
    let re = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    processed = re
        .replace_all(&processed, r#"<a href="$2">$1</a>"#)
        .into_owned();

    // Restore code blocks
    for (i, block) in code_blocks.iter().enumerate() {
        processed = processed.replace(&format!("{PH}CODE_BLOCK_{i}{PH}"), block);
    }

    // Restore inline code
    for (i, code) in inline_codes.iter().enumerate() {
        processed = processed.replace(&format!("{PH}INLINE_CODE_{i}{PH}"), code);
    }

    processed
}

pub fn strip_markdown(text: &str) -> String {
    let mut result = text.to_string();

    // Remove code blocks (keep content)
    let re = regex::Regex::new(r"```\w*\n?([\s\S]*?)```").unwrap();
    result = re.replace_all(&result, "$1").into_owned();

    // Remove inline code backticks
    let re = regex::Regex::new(r"`([^`]+)`").unwrap();
    result = re.replace_all(&result, "$1").into_owned();

    // Remove bold
    let re = regex::Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    result = re.replace_all(&result, "$1").into_owned();
    let re = regex::Regex::new(r"__([^_]+)__").unwrap();
    result = re.replace_all(&result, "$1").into_owned();

    // Remove italic
    let re = regex::Regex::new(r"\*([^*]+)\*").unwrap();
    result = re.replace_all(&result, "$1").into_owned();
    let re = regex::Regex::new(r"_([^_]+)_").unwrap();
    result = re.replace_all(&result, "$1").into_owned();

    // Remove strikethrough
    let re = regex::Regex::new(r"~~([^~]+)~~").unwrap();
    result = re.replace_all(&result, "$1").into_owned();

    // Convert links to just text
    let re = regex::Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    result = re.replace_all(&result, "$1").into_owned();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bold() {
        assert_eq!(markdown_to_html("**bold**"), "<b>bold</b>");
        assert_eq!(markdown_to_html("__bold__"), "<b>bold</b>");
    }

    #[test]
    fn test_italic() {
        assert_eq!(markdown_to_html("*italic*"), "<i>italic</i>");
        assert_eq!(markdown_to_html("_italic_"), "<i>italic</i>");
    }

    #[test]
    fn test_code_block() {
        assert_eq!(
            markdown_to_html("```rust\nfn main() {}\n```"),
            "<pre>fn main() {}</pre>"
        );
    }

    #[test]
    fn test_inline_code() {
        assert_eq!(markdown_to_html("`code`"), "<code>code</code>");
    }

    #[test]
    fn test_html_escaping() {
        assert_eq!(markdown_to_html("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn test_html_in_code_block() {
        assert_eq!(
            markdown_to_html("```\n<div>test</div>\n```"),
            "<pre>&lt;div&gt;test&lt;/div&gt;</pre>"
        );
    }

    #[test]
    fn test_link() {
        assert_eq!(
            markdown_to_html("[click](https://example.com)"),
            r#"<a href="https://example.com">click</a>"#
        );
    }

    #[test]
    fn test_strikethrough() {
        assert_eq!(markdown_to_html("~~deleted~~"), "<s>deleted</s>");
    }

    #[test]
    fn test_code_blocks_not_processed() {
        let input = "```\n**not bold** _not italic_\n```";
        let output = markdown_to_html(input);
        assert!(output.contains("**not bold**"));
        assert!(!output.contains("<b>"));
    }

    #[test]
    fn test_strip_markdown_bold() {
        assert_eq!(strip_markdown("**bold**"), "bold");
    }

    #[test]
    fn test_strip_markdown_link() {
        assert_eq!(strip_markdown("[text](url)"), "text");
    }

    #[test]
    fn test_strip_markdown_code_block() {
        assert_eq!(strip_markdown("```\ncode\n```"), "code\n");
    }

    #[test]
    fn test_plain_text_unchanged() {
        assert_eq!(markdown_to_html("hello world"), "hello world");
    }
}
