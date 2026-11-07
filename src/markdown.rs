//! Markdown parsing: top-level blocks with line ranges, plus the bits checks need.

use comrak::nodes::{AstNode, ListType, NodeValue};
use comrak::{Arena, Options, format_html, parse_document};

pub struct ParsedBlock {
    pub start: usize,
    pub end: usize,
    pub html: String,
    /// Source text, used to detect changes between reads.
    pub source: String,
}

pub struct CodeSpan {
    pub line: usize,
    pub text: String,
}

pub struct Parsed {
    pub title: Option<String>,
    pub blocks: Vec<ParsedBlock>,
    pub code_spans: Vec<CodeSpan>,
    /// `@path` imports outside code, with their line.
    pub imports: Vec<(usize, String)>,
    pub links: Vec<(usize, String)>,
}

fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.tasklist = true;
    o.extension.autolink = true;
    o.extension.wikilinks_title_after_pipe = true;
    o.extension.front_matter_delimiter = Some("---".into());
    // Raw HTML stays escaped: documents come from anywhere.
    o.render.r#unsafe = false;
    o
}

fn text_of<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    for d in node.descendants() {
        match &d.data().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            _ => {}
        }
    }
    out
}

/// `@path` tokens in plain text. Emails and handles like `@user` without a dot or slash are skipped.
pub fn find_imports(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices('@') {
        if i > 0 && !text[..i].ends_with(|c: char| c.is_whitespace() || c == '(') {
            continue;
        }
        let rest = &text[i + 1..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')' || c == ',')
            .unwrap_or(rest.len());
        let path = rest[..end].trim_end_matches(['.', ':', ';']);
        if !path.is_empty() && (path.contains('/') || path.contains('.')) {
            out.push(path.to_string());
        }
    }
    out
}

pub fn parse(src: &str) -> Parsed {
    let arena = Arena::new();
    let opts = options();
    let root = parse_document(&arena, src, &opts);
    let lines: Vec<&str> = src.lines().collect();

    let mut parsed = Parsed {
        title: None,
        blocks: Vec::new(),
        code_spans: Vec::new(),
        imports: Vec::new(),
        links: Vec::new(),
    };

    // Top-level blocks, with list items split out so changes are tracked per item.
    let mut units: Vec<(&AstNode, Option<String>)> = Vec::new();
    for node in root.children() {
        match &node.data().value {
            NodeValue::List(list) => {
                for (i, item) in node.children().enumerate() {
                    let open = match list.list_type {
                        ListType::Bullet => "<ul>\n".to_string(),
                        ListType::Ordered if list.start + i == 1 => "<ol>\n".to_string(),
                        ListType::Ordered => format!("<ol start=\"{}\">\n", list.start + i),
                    };
                    units.push((item, Some(open)));
                }
            }
            _ => units.push((node, None)),
        }
    }

    for (node, list_open) in units {
        let (start, end) = {
            let pos = node.data().sourcepos;
            (pos.start.line, pos.end.line.max(pos.start.line))
        };
        if let NodeValue::FrontMatter(_) = node.data().value {
            continue;
        }
        if parsed.title.is_none()
            && let NodeValue::Heading(h) = &node.data().value
            && h.level == 1
        {
            parsed.title = Some(text_of(node));
        }
        let mut html = String::new();
        if format_html(node, &opts, &mut html).is_err() {
            continue;
        }
        if let Some(open) = list_open {
            let close = if open.starts_with("<ul") {
                "</ul>\n"
            } else {
                "</ol>\n"
            };
            html = format!("{open}{html}{close}");
        }
        let source = lines
            .get(start.saturating_sub(1)..end.min(lines.len()))
            .map(|l| l.join("\n"))
            .unwrap_or_default();
        parsed.blocks.push(ParsedBlock {
            start,
            end,
            html,
            source,
        });
    }

    for d in root.descendants() {
        let line = d.data().sourcepos.start.line;
        match &d.data().value {
            NodeValue::Code(c) => parsed.code_spans.push(CodeSpan {
                line,
                text: c.literal.clone(),
            }),
            NodeValue::Link(l) => parsed.links.push((line, l.url.clone())),
            NodeValue::Text(t) => {
                for p in find_imports(t) {
                    parsed.imports.push((line, p));
                }
            }
            _ => {}
        }
    }
    parsed
}

/// Content an agent actually receives: block HTML comments are stripped by Claude Code.
pub fn strip_html_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_skip_code_and_emails() {
        let p = parse(
            "See @AGENTS.md and @docs/style.md.\n\nMail me@example.com\n\n`@README.md` stays literal\n\n```\n@ignored.md\n```\n",
        );
        let paths: Vec<_> = p.imports.iter().map(|(_, p)| p.as_str()).collect();
        assert_eq!(paths, ["AGENTS.md", "docs/style.md"]);
    }

    #[test]
    fn blocks_have_lines_and_title() {
        let p = parse("---\ntitle: x\n---\n# Hello\n\nText with `scripts/a.sh`.\n\n- one\n- two\n");
        assert_eq!(p.title.as_deref(), Some("Hello"));
        assert_eq!(p.blocks.len(), 4);
        assert_eq!(p.blocks[1].start, 6);
        assert_eq!(p.code_spans[0].text, "scripts/a.sh");
        assert_eq!(p.blocks[3].html, "<ul>\n<li>two</li>\n</ul>\n");
        assert_eq!(p.blocks[3].start, 9);
    }

    #[test]
    fn ordered_items_keep_their_numbers() {
        let p = parse(
            "3. c
4. d
",
        );
        assert!(p.blocks[1].html.starts_with("<ol start=\"4\">"));
    }

    #[test]
    fn wiki_links_render() {
        let p = parse("See [[TESTING|tests]].\n");
        assert!(p.blocks[0].html.contains("data-wikilink"));
    }

    #[test]
    fn strips_comments() {
        assert_eq!(strip_html_comments("a<!-- b -->c<!-- d"), "ac");
    }
}
