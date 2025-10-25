//! Recursive descent reStructuredText parser that targets a lightweight AST.
//!
//! The crate exposes helpers to parse raw docstrings into [`Block`] nodes via [`parse`],
//! and render them as HTML with [`html_of`].
//!
//! When the `markdown` feature is enabled, you can also normalize docstrings into
//! Markdown using [`markdown_of`].
//!
//! The internal parser is intentionally small and resilient enough to handle the
//! eclectic docstring styles used in the Python ecosystem.

mod ast;
pub mod error;
pub use ast::{Block, Field, Inline, ListKind};
pub use error::ParseError;

#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    _num: usize,
    raw: &'a str,
}

#[derive(Debug)]
struct Lines<'a> {
    all: Vec<Line<'a>>,
    i: usize,
}

impl<'a> Lines<'a> {
    fn new(input: &'a str) -> Self {
        let all = input
            .lines()
            .enumerate()
            .map(|(i, raw)| Line { _num: i + 1, raw })
            .collect();
        Self { all, i: 0 }
    }

    fn peek(&self) -> Option<&Line<'a>> {
        self.all.get(self.i)
    }

    fn peek_next(&self) -> Option<&Line<'a>> {
        self.all.get(self.i + 1)
    }

    fn next(&mut self) -> Option<Line<'a>> {
        let l = self.all.get(self.i).cloned();
        self.i += (l.is_some()) as usize;
        l
    }

    fn backtrack(&mut self) {
        if self.i > 0 {
            self.i -= 1;
        }
    }

    fn is_eof(&self) -> bool {
        self.i >= self.all.len()
    }
}

fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

fn leading_indent(s: &str) -> usize {
    s.chars().take_while(|c| c.is_whitespace()).count()
}

fn colon_heading_text(current: &Line<'_>, next: Option<&Line<'_>>) -> Option<String> {
    let trimmed = current.raw.trim();
    if trimmed.starts_with("..") {
        return None;
    }
    if !trimmed.ends_with(':') || trimmed.ends_with("::") {
        return None;
    }
    let without_colon = trimmed.trim_end_matches(':').trim();
    if without_colon.is_empty() {
        return None;
    }
    if !without_colon
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '_' || c == '-')
    {
        return None;
    }
    match next {
        Some(next_line) if is_blank(next_line.raw) => Some(without_colon.to_string()),
        Some(next_line) => {
            if leading_indent(next_line.raw) > leading_indent(current.raw) {
                Some(without_colon.to_string())
            } else {
                None
            }
        }
        None => Some(without_colon.to_string()),
    }
}

fn strip_indent_preserve(s: &str, indent: usize) -> &str {
    if indent == 0 {
        return s;
    }
    let mut consumed = 0;
    for (idx, ch) in s.char_indices() {
        if consumed >= indent {
            return &s[idx..];
        }
        match ch {
            ' ' => consumed += 1,
            '\t' => consumed += 4,
            _ => return &s[idx..],
        }
    }
    ""
}

fn underline_level(s: &str) -> Option<u8> {
    let t = s.trim();
    if !t.is_empty() && t.chars().all(|c| c == '=') {
        Some(1)
    } else if !t.is_empty() && t.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

#[cfg(feature = "markdown")]
fn normalize_docstring(input: &str) -> String {
    let trimmed = input.trim_matches(|c| c == '\n' || c == '\r');
    if trimmed.is_empty() {
        return String::new();
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let mut min_indent = usize::MAX;

    for line in &lines {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.chars().take_while(|c| *c == ' ').count();
        if indent < min_indent {
            min_indent = indent;
        }
    }

    if min_indent == usize::MAX {
        return trimmed.to_string();
    }

    lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line.chars().skip(min_indent).collect::<String>()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn skip_blank_lines(ls: &mut Lines<'_>) {
    while let Some(l) = ls.peek() {
        if is_blank(l.raw) {
            ls.next();
        } else {
            break;
        }
    }
}

/// Try to parse a code fence block (```)
fn try_parse_code_fence(ls: &mut Lines<'_>) -> Option<Block> {
    let l = ls.peek()?;
    if l.raw.trim() != "```" {
        return None;
    }

    ls.next();
    let mut buf = String::new();
    while let Some(inner) = ls.next() {
        if inner.raw.trim() == "```" {
            break;
        }
        buf.push_str(inner.raw);
        buf.push('\n');
    }
    Some(Block::CodeBlock(buf))
}

/// Try to parse a quote block (>)
fn try_parse_quote(ls: &mut Lines<'_>) -> Result<Option<Block>, ParseError> {
    let l = ls.peek();
    if !l.map(|l| l.raw.trim_start().starts_with('>')).unwrap_or(false) {
        return Ok(None);
    }

    let mut quote = String::new();
    while let Some(q) = ls.peek() {
        let t = q.raw.trim_start();
        if t.starts_with('>') {
            ls.next();
            quote.push_str(t.trim_start_matches("> ").trim_start_matches('>'));
            quote.push('\n');
        } else {
            break;
        }
    }
    let inner = parse(&quote)?;
    Ok(Some(Block::Quote(inner)))
}

/// Try to parse a colon-style heading (Heading:)
fn try_parse_colon_heading(ls: &mut Lines<'_>) -> Option<Block> {
    let line = ls.peek()?;
    let title = colon_heading_text(line, ls.peek_next())?;
    ls.next();
    Some(Block::Heading { level: 2, inlines: ast::parse_inlines(&title) })
}

/// Try to parse a setext-style heading (underlined with = or -)
fn try_parse_setext_heading(ls: &mut Lines<'_>) -> Option<Block> {
    let title = ls.next()?;
    let ul = ls.peek()?;
    let level = underline_level(ul.raw)?;
    ls.next();
    let inlines = ast::parse_inlines(title.raw.trim());
    Some(Block::Heading { level, inlines })
}

/// Try to parse a literal block (::)
fn try_parse_literal_block(ls: &mut Lines<'_>) -> Option<Block> {
    let line = ls.peek()?;
    if line.raw.trim() != "::" {
        return None;
    }

    ls.next();

    let base_indent = if let Some(next_line) = ls.peek() {
        if is_blank(next_line.raw) {
            ls.next();
            if let Some(content_line) = ls.peek() {
                leading_indent(content_line.raw)
            } else {
                return Some(Block::LiteralBlock(String::new()));
            }
        } else {
            leading_indent(next_line.raw)
        }
    } else {
        return Some(Block::LiteralBlock(String::new()));
    };

    let mut buf = String::new();
    while let Some(l) = ls.peek() {
        if is_blank(l.raw) {
            if let Some(next) = ls.peek_next() {
                if !is_blank(next.raw) && leading_indent(next.raw) < base_indent {
                    break;
                }
            }
            buf.push('\n');
            ls.next();
        } else if leading_indent(l.raw) >= base_indent {
            let content = strip_indent_preserve(ls.next().unwrap().raw, base_indent);
            buf.push_str(content);
            buf.push('\n');
        } else {
            break;
        }
    }

    Some(Block::LiteralBlock(buf.trim_end().to_string()))
}

/// Try to parse a comment (.. without ::)
fn try_parse_comment(ls: &mut Lines<'_>) -> Result<Option<Block>, ParseError> {
    let line = ls.peek().ok_or(ParseError::Eof)?;
    let trimmed = line.raw.trim_start();

    if !trimmed.starts_with(".. ") {
        return Ok(None);
    }

    let after_dots = &trimmed[3..];

    if after_dots.contains("::") {
        return Ok(None);
    }

    let base_indent = leading_indent(line.raw);
    ls.next();

    let mut content_text = String::new();
    if !after_dots.trim().is_empty() {
        content_text.push_str(after_dots.trim());
    }

    if let Some(next) = ls.peek() {
        if is_blank(next.raw) {
            ls.next();
        }
    }

    let content_indent = base_indent + 1;

    while let Some(l) = ls.peek() {
        if is_blank(l.raw) {
            if let Some(next) = ls.peek_next() {
                if !is_blank(next.raw) && leading_indent(next.raw) <= base_indent {
                    break;
                }
            }
            if !content_text.is_empty() {
                content_text.push('\n');
            }
            ls.next();
        } else if leading_indent(l.raw) > base_indent {
            let stripped = strip_indent_preserve(ls.next().unwrap().raw, content_indent);
            if !content_text.is_empty() {
                content_text.push('\n');
            }
            content_text.push_str(stripped);
        } else {
            break;
        }
    }

    let content = if content_text.trim().is_empty() { Vec::new() } else { parse(&content_text)? };

    Ok(Some(Block::Comment(content)))
}

/// Try to parse a directive (.. name:: argument)
fn try_parse_directive(ls: &mut Lines<'_>) -> Result<Option<Block>, ParseError> {
    let line = ls.peek().ok_or(ParseError::Eof)?;
    let trimmed = line.raw.trim_start();

    if !trimmed.starts_with(".. ") {
        return Ok(None);
    }

    let after_dots = &trimmed[3..];

    let Some(double_colon_idx) = after_dots.find("::") else {
        return Ok(None);
    };

    let name = after_dots[..double_colon_idx].trim();
    if name.is_empty() {
        return Ok(None);
    }

    let argument = after_dots[double_colon_idx + 2..].trim().to_string();

    let base_indent = leading_indent(line.raw);
    ls.next();

    if let Some(next) = ls.peek() {
        if is_blank(next.raw) {
            ls.next();
        }
    }

    let mut content_text = String::new();
    let content_indent = base_indent + 4;

    while let Some(l) = ls.peek() {
        if is_blank(l.raw) {
            if let Some(next) = ls.peek_next() {
                if !is_blank(next.raw) && leading_indent(next.raw) < content_indent {
                    break;
                }
            }
            content_text.push('\n');
            ls.next();
        } else if leading_indent(l.raw) >= content_indent {
            let stripped = strip_indent_preserve(ls.next().unwrap().raw, content_indent);
            content_text.push_str(stripped);
            content_text.push('\n');
        } else {
            break;
        }
    }

    let content = if content_text.trim().is_empty() {
        Vec::new()
    } else if name == "code-block" || name == "code" {
        vec![Block::LiteralBlock(content_text.trim_end().to_string())]
    } else {
        parse(&content_text)?
    };

    Ok(Some(Block::Directive { name: name.to_string(), argument, content }))
}

/// Check if a line starts a new block (not a paragraph continuation)
fn starts_new_block(line: &str) -> bool {
    is_blank(line) || ast::list_kind(line).is_some() || line.trim() == "```" || line.trim_start().starts_with('>')
}

/// Parse remaining content as a paragraph
fn parse_paragraph(ls: &mut Lines<'_>) -> Option<Block> {
    let mut buf = String::new();
    while let Some(l) = ls.peek() {
        if starts_new_block(l.raw) {
            break;
        }
        buf.push_str(ls.next().unwrap().raw);
        buf.push('\n');
    }
    let text = buf.trim_end();
    if text.is_empty() { None } else { Some(Block::Paragraph(ast::parse_inlines(text))) }
}

/// Parse raw reStructuredText-like input into a vector of [`Block`] nodes.
///
/// The parser walks the input top-to-bottom, attempting the most specific block constructs first
/// (code fences, block quotes, lists, field lists, definition lists, headings) before falling back to paragraphs.
/// When the stream cannot be consumed because of malformed markup, a [`ParseError`] is returned to the caller.
pub fn parse(input: &str) -> Result<Vec<Block>, ParseError> {
    let mut ls = Lines::new(input);
    let mut blocks = Vec::new();

    while !ls.is_eof() {
        skip_blank_lines(&mut ls);
        if ls.is_eof() {
            break;
        }

        if let Some(block) = try_parse_code_fence(&mut ls) {
            blocks.push(block);
            continue;
        }

        if let Some(block) = try_parse_quote(&mut ls)? {
            blocks.push(block);
            continue;
        }

        if let Some(block) = ast::try_parse_list(&mut ls) {
            blocks.push(block);
            continue;
        }

        if let Some(block) = ast::try_parse_grid_table(&mut ls) {
            blocks.push(block);
            continue;
        }

        if let Some(block) = ast::try_parse_simple_table(&mut ls) {
            blocks.push(block);
            continue;
        }

        if let Some(block) = try_parse_comment(&mut ls)? {
            blocks.push(block);
            continue;
        }

        if let Some(block) = try_parse_directive(&mut ls)? {
            blocks.push(block);
            continue;
        }

        if let Some(field_block) = ast::parse_field_entries(&mut ls)? {
            blocks.push(field_block);
            continue;
        }

        if let Some(def_blocks) = ast::parse_definition_entries(&mut ls)? {
            blocks.extend(def_blocks);
            continue;
        }

        if let Some(block) = try_parse_colon_heading(&mut ls) {
            blocks.push(block);
            continue;
        }

        if let Some(block) = try_parse_setext_heading(&mut ls) {
            blocks.push(block);
            continue;
        } else {
            ls.backtrack();
        }

        if let Some(block) = try_parse_literal_block(&mut ls) {
            blocks.push(block);
            continue;
        }

        if let Some(block) = parse_paragraph(&mut ls) {
            blocks.push(block);
        }
    }

    Ok(blocks)
}

/// Render the provided docstring to HTML by parsing it and concatenating the
/// HTML representation of each [`Block`].
///
/// ## Panics
///
/// Panics if [`parse`] returns an error. Use [`parse`] directly when you need
/// to surface parsing failures to your caller.
pub fn html_of(input: &str) -> String {
    parse(input)
        .unwrap()
        .into_iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert docstrings that mix Google/Numpy/Sphinx conventions into Markdown.
///
/// The string is first normalized to a reStructuredText subset understood by this crate,
/// rendered to HTML via [`html_of`], and finally converted back to Markdown through [html2md].
///
/// This function is only available when the `markdown` feature is enabled.
#[cfg(feature = "markdown")]
pub fn markdown_of(input: &str) -> String {
    let normalized = normalize_docstring(input);
    let html = &html_of(&normalized);
    html2md::parse_html(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_basic_doc() {
        let doc = r#"
Title
=====

A paragraph with *emphasis*, **strong**, and `code`.
"#;

        let html = html_of(doc);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<em>emphasis</em>"));
        assert!(html.contains("<strong>strong</strong>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn parses_setext_headings() {
        let doc = "Heading 1\n=========\n\nHeading 2\n---------";
        let ast = parse(doc).unwrap();

        assert_eq!(ast.len(), 2);
        match &ast[0] {
            Block::Heading { level, inlines } => {
                assert_eq!(*level, 1);
                assert_eq!(inlines[0], Inline::Text("Heading 1".into()));
            }
            _ => panic!("expected heading"),
        }

        match &ast[1] {
            Block::Heading { level, inlines } => {
                assert_eq!(*level, 2);
                assert_eq!(inlines[0], Inline::Text("Heading 2".into()));
            }
            _ => panic!("expected heading"),
        }
    }

    #[test]
    fn parses_paragraphs() {
        let doc = "This is a paragraph.\n\nAnother paragraph.";
        let ast = parse(doc).unwrap();

        assert_eq!(ast.len(), 2);
        assert!(matches!(ast[0], Block::Paragraph(_)));
        assert!(matches!(ast[1], Block::Paragraph(_)));
    }

    #[test]
    fn parses_unordered_list() {
        let doc = "- One\n- Two\n- Three";
        let ast = parse(doc).unwrap();

        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::List { kind, items } => {
                assert_eq!(*kind, ListKind::Unordered);
                assert_eq!(items.len(), 3);
                assert_eq!(items[0][0], Inline::Text("One".into()));
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn parses_ordered_list() {
        let doc = "1. First\n2. Second";
        let ast = parse(doc).unwrap();

        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::List { kind, items } => {
                assert_eq!(*kind, ListKind::Ordered);
                assert_eq!(items.len(), 2);
                assert_eq!(items[0][0], Inline::Text("First".into()));
            }
            _ => panic!("expected ordered list"),
        }
    }

    #[test]
    fn parses_code_fence() {
        let doc = "```\nline1\nline2\n```";
        let ast = parse(doc).unwrap();

        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::CodeBlock(code) => {
                assert!(code.contains("line1"));
                assert!(code.contains("line2"));
            }
            _ => panic!("expected code block"),
        }
    }

    #[test]
    fn parses_quote_block() {
        let doc = "> quoted line\n> continues\n\nregular paragraph";
        let ast = parse(doc).unwrap();

        assert_eq!(ast.len(), 2);
        match &ast[0] {
            Block::Quote(inner) => {
                assert_eq!(inner.len(), 1);
                assert!(matches!(&inner[0], Block::Paragraph(_)));
            }
            _ => panic!("expected quote block"),
        }
    }

    #[test]
    fn parses_emphasis_and_strong() {
        let line = "A *word* and a **strong** one";
        let inl = ast::parse_inlines(line);
        let html = ast::join_inlines(&inl);
        assert!(html.contains("<em>word</em>"));
        assert!(html.contains("<strong>strong</strong>"));
    }

    #[test]
    fn parses_inline_code() {
        let line = "Inline `code` works";
        let html = ast::join_inlines(&ast::parse_inlines(line));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn parses_double_backtick_code() {
        let line = "Use ``inline`` literals";
        let html = ast::join_inlines(&ast::parse_inlines(line));
        assert!(html.contains("<code>inline</code>"));
    }

    #[test]
    fn parses_inline_link() {
        let line = "`example <https://example.com>`_";
        let html = ast::join_inlines(&ast::parse_inlines(line));
        assert!(html.contains("<a href=\"https://example.com\">example</a>"));
    }

    #[test]
    fn inline_link_requires_reference_suffix() {
        let line = "`example <https://example.com>`";
        let inl = ast::parse_inlines(line);
        assert_eq!(inl, vec![Inline::Code("example <https://example.com>".into())]);
    }

    #[test]
    fn inline_link_mixed_with_text() {
        let line = "Read `docs <https://example.com>`_ now.";
        let inl = ast::parse_inlines(line);
        assert_eq!(
            inl,
            vec![
                Inline::Text("Read ".into()),
                Inline::Link { text: vec![Inline::Text("docs".into())], url: "https://example.com".into() },
                Inline::Text(" now.".into())
            ]
        );
    }

    #[test]
    fn parses_mixed_inline_styles() {
        let line = "**bold** *em* `code` and `link <x>`_";
        let html = ast::join_inlines(&ast::parse_inlines(line));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>em</em>"));
        assert!(html.contains("<code>code</code>"));
        assert!(html.contains("<a href=\"x\">link</a>"));
    }

    #[test]
    fn unmatched_markup_falls_back_to_text() {
        let line = "An *unfinished emphasis";
        let inl = ast::parse_inlines(line);
        assert_eq!(inl, vec![Inline::Text("An *unfinished emphasis".into())]);
    }

    #[test]
    fn html_of_renders_expected_html() {
        let doc = "Heading\n=======\n\nBody text.";
        let rendered = html_of(doc);
        assert_eq!(rendered.trim(), "<h1>Heading</h1>\n<p>Body text.</p>");
    }

    #[test]
    #[cfg(feature = "markdown")]
    fn markdown_of_round_trips_to_markdown() {
        let doc = "Heading\n=======\n\n- Item 1\n- Item 2";
        let markdown = markdown_of(doc);
        let normalized = markdown.trim();
        assert_eq!(normalized, "Heading\n==========\n\n* Item 1\n* Item 2");
    }

    #[test]
    fn parses_sphinx_field_list() {
        let doc = ":param foo: Foo value\n:param bar: Bar value";
        let ast = parse(doc).unwrap();

        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::FieldList { fields } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "param");
                assert_eq!(fields[0].argument, "foo");
                assert_eq!(fields[1].name, "param");
                assert_eq!(fields[1].argument, "bar");
            }
            _ => panic!("expected FieldList"),
        }
    }

    #[test]
    fn parses_numpy_definition_list() {
        let doc = "Parameters\n----------\nfoo : int\n    Foo value";
        let ast = parse(doc).unwrap();

        assert!(matches!(ast[0], Block::Heading { .. }));
        match &ast[1] {
            Block::Paragraph(inlines) => {
                assert_eq!(inlines[0], Inline::Strong(vec![Inline::Text("foo".into())]));
                assert!(
                    inlines
                        .iter()
                        .any(|i| *i == Inline::Em(vec![Inline::Text("int".into())]))
                );
            }
            _ => panic!("expected paragraph for numpy definition"),
        }
    }

    #[test]
    fn parses_google_style_args() {
        let doc = "Args:\n    foo (int): Foo value\n    bar: Another";
        let ast = parse(doc).unwrap();

        match &ast[0] {
            Block::Heading { level, inlines } => {
                assert_eq!(*level, 2);
                assert_eq!(inlines[0], Inline::Text("Args".into()));
            }
            _ => panic!("expected heading"),
        }
        match &ast[1] {
            Block::Paragraph(inlines) => {
                assert_eq!(inlines[0], Inline::Strong(vec![Inline::Text("foo".into())]));
                assert!(
                    inlines
                        .iter()
                        .any(|i| *i == Inline::Em(vec![Inline::Text("int".into())]))
                );
            }
            _ => panic!("expected first definition paragraph"),
        }
        match &ast[2] {
            Block::Paragraph(inlines) => {
                assert_eq!(inlines[0], Inline::Strong(vec![Inline::Text("bar".into())]));
            }
            _ => panic!("expected second definition paragraph"),
        }
    }

    #[test]
    #[cfg(feature = "markdown")]
    fn markdown_of_converts_docstring_sections() {
        let doc = r#"
        Summary line.

        Parameters
        ----------
        foo : int
            Foo value.

        Returns
        -------
        int
            The result.
        "#;

        let markdown = markdown_of(doc);
        assert!(markdown.contains("Parameters"));
        assert!(markdown.contains("**foo** (*int*): Foo value."));
        assert!(markdown.contains("Returns\n----------"));
        assert!(markdown.contains("int The result."));
    }

    #[test]
    fn parses_multiple_blocks_correctly() {
        let doc = r#"
Title
=====

- One
- Two
- Three

> quote

````

code

```
"#;
        let html = html_of(doc);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<ul>"));
        assert!(html.contains("<blockquote>"));
        assert!(html.contains("<pre><code>"));
    }

    #[test]
    fn ignores_blank_lines() {
        let doc = "\n\nParagraph\n\n\nAnother\n";
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 2);
    }

    #[test]
    fn html_escape_works() {
        let code = "<script>alert('x')</script>";
        let esc = ast::html_escape(code);
        assert!(esc.contains("&lt;"));
        assert!(esc.contains("&gt;"));
        assert!(!esc.contains("<script>"));
    }

    #[test]
    fn parses_nested_strong_with_emphasis() {
        let line = "**bold *italic* bold**";
        let inl = ast::parse_inlines(line);
        assert_eq!(inl.len(), 1);
        match &inl[0] {
            Inline::Strong(children) => {
                assert_eq!(children.len(), 3);
                assert_eq!(children[0], Inline::Text("bold ".into()));
                match &children[1] {
                    Inline::Em(em_children) => {
                        assert_eq!(em_children.len(), 1);
                        assert_eq!(em_children[0], Inline::Text("italic".into()));
                    }
                    _ => panic!("expected Em"),
                }
                assert_eq!(children[2], Inline::Text(" bold".into()));
            }
            _ => panic!("expected Strong"),
        }
    }

    #[test]
    fn parses_nested_emphasis_with_strong() {
        let line = "*em **strong** em*";
        let inl = ast::parse_inlines(line);
        assert_eq!(inl.len(), 1);
        match &inl[0] {
            Inline::Em(children) => {
                assert_eq!(children.len(), 3);
                assert_eq!(children[0], Inline::Text("em ".into()));
                match &children[1] {
                    Inline::Strong(strong_children) => {
                        assert_eq!(strong_children.len(), 1);
                        assert_eq!(strong_children[0], Inline::Text("strong".into()));
                    }
                    _ => panic!("expected Strong"),
                }
                assert_eq!(children[2], Inline::Text(" em".into()));
            }
            _ => panic!("expected Em"),
        }
    }

    #[test]
    fn renders_nested_inline_markup_to_html() {
        let line = "**bold *italic* text**";
        let html = ast::join_inlines(&ast::parse_inlines(line));
        assert_eq!(html, "<strong>bold <em>italic</em> text</strong>");
    }

    #[test]
    fn parses_link_with_nested_markup() {
        let line = "`**bold** link <https://example.com>`_";
        let inl = ast::parse_inlines(line);
        assert_eq!(inl.len(), 1);
        match &inl[0] {
            Inline::Link { text, url } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(text.len(), 2);
                match &text[0] {
                    Inline::Strong(strong_children) => {
                        assert_eq!(strong_children.len(), 1);
                        assert_eq!(strong_children[0], Inline::Text("bold".into()));
                    }
                    _ => panic!("expected Strong in link text"),
                }
                assert_eq!(text[1], Inline::Text(" link".into()));
            }
            _ => panic!("expected Link"),
        }
    }

    #[test]
    fn nested_markup_does_not_break_code_blocks() {
        let line = "Use ``**not bold**`` for literals";
        let inl = ast::parse_inlines(line);
        assert!(matches!(&inl[1], Inline::Code(s) if s == "**not bold**"));
    }

    #[test]
    fn multiple_levels_of_nesting() {
        let line = "**strong with *emphasis* inside** and *emphasis with **strong** inside*";
        let html = ast::join_inlines(&ast::parse_inlines(line));
        assert!(html.contains("<strong>strong with <em>emphasis</em> inside</strong>"));
        assert!(html.contains("<em>emphasis with <strong>strong</strong> inside</em>"));
    }

    #[test]
    fn parses_standalone_literal_block() {
        let doc = r#"
::

    This is a literal block.
    It preserves    spacing.
    <html> is escaped.
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::LiteralBlock(code) => {
                assert!(code.contains("This is a literal block"));
                assert!(code.contains("preserves    spacing"));
                assert!(code.contains("<html>"));
            }
            _ => panic!("expected LiteralBlock"),
        }

        let html = html_of(doc);
        assert!(html.contains("<pre><code>"));
        assert!(html.contains("&lt;html&gt;"));
    }

    #[test]
    fn parses_directive_note() {
        let doc = r#"
.. note::

    This is a note directive.
    It can have multiple paragraphs.
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::Directive { name, argument, content } => {
                assert_eq!(name, "note");
                assert_eq!(argument, "");
                assert_eq!(content.len(), 1);
            }
            _ => panic!("expected Directive"),
        }

        let html = html_of(doc);
        assert!(html.contains("<div class=\"admonition note\">"));
        assert!(html.contains("<p class=\"admonition-title\">Note</p>"));
        assert!(html.contains("This is a note directive"));
    }

    #[test]
    fn parses_directive_warning() {
        let doc = ".. warning::\n\n    Be careful!";
        let ast = parse(doc).unwrap();

        match &ast[0] {
            Block::Directive { name, .. } => {
                assert_eq!(name, "warning");
            }
            _ => panic!("expected Directive"),
        }

        let html = html_of(doc);
        assert!(html.contains("<div class=\"admonition warning\">"));
        assert!(html.contains("<p class=\"admonition-title\">Warning</p>"));
    }

    #[test]
    fn parses_directive_code_block() {
        let doc = r#"
.. code-block:: python

    def hello():
        print("world")
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::Directive { name, argument, content } => {
                assert_eq!(name, "code-block");
                assert_eq!(argument, "python");
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::LiteralBlock(code) => {
                        assert!(code.contains("def hello()"));
                        assert!(code.contains("print(\"world\")"));
                    }
                    _ => panic!("expected LiteralBlock in code-block content"),
                }
            }
            _ => panic!("expected Directive"),
        }

        let html = html_of(doc);
        assert!(html.contains("<pre><code class=\"language-python\">"));
        assert!(html.contains("def hello()"));
    }

    #[test]
    fn parses_directive_image() {
        let doc = ".. image:: /path/to/image.png";
        let ast = parse(doc).unwrap();

        match &ast[0] {
            Block::Directive { name, argument, content } => {
                assert_eq!(name, "image");
                assert_eq!(argument, "/path/to/image.png");
                assert_eq!(content.len(), 0);
            }
            _ => panic!("expected Directive"),
        }

        let html = html_of(doc);
        assert!(html.contains("<img src=\"/path/to/image.png\""));
    }

    #[test]
    fn multiple_directives_in_sequence() {
        let doc = r#"
.. note::

    First note.

.. warning::

    A warning.

.. code-block:: rust

    fn main() {}
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 3);
        assert!(matches!(&ast[0], Block::Directive { name, .. } if name == "note"));
        assert!(matches!(&ast[1], Block::Directive { name, .. } if name == "warning"));
        assert!(matches!(&ast[2], Block::Directive { name, .. } if name == "code-block"));
    }

    #[test]
    fn literal_block_preserves_indentation() {
        let doc = "::

    Line 1
        Indented line 2
            More indented line 3";
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::LiteralBlock(code) => {
                assert!(code.contains("Line 1"));
                assert!(code.contains("    Indented line 2"));
                assert!(code.contains("        More indented line 3"));
            }
            _ => panic!("expected LiteralBlock"),
        }
    }

    #[test]
    fn directive_with_blank_lines_in_content() {
        let doc = r#"
.. note::

    First paragraph.

    Second paragraph after blank.
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::Directive { content, .. } => {
                assert_eq!(content.len(), 2);
                assert!(matches!(&content[0], Block::Paragraph(_)));
                assert!(matches!(&content[1], Block::Paragraph(_)));
            }
            _ => panic!("expected Directive"),
        }
    }

    #[test]
    fn parses_simple_table() {
        let doc = r#"
====  ====
Col1  Col2
====  ====
val1  val2
val3  val4
====  ====
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::Table { headers, rows } => {
                assert_eq!(headers.len(), 2);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 2);
                assert_eq!(rows[1].len(), 2);
            }
            _ => panic!("expected Table"),
        }

        let html = html_of(doc);
        assert!(html.contains("<table>"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<th>Col1</th>"));
        assert!(html.contains("<th>Col2</th>"));
        assert!(html.contains("<td>val1</td>"));
        assert!(html.contains("<td>val2</td>"));
    }

    #[test]
    fn parses_table_with_inline_markup() {
        let doc = r#"
=========  =========
**Name**   *Type*
=========  =========
foo        `int`
bar        `str`
=========  =========
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::Table { headers, rows } => {
                assert_eq!(headers.len(), 2);
                assert!(matches!(&headers[0][0], Inline::Strong(_)));
                assert!(matches!(&headers[1][0], Inline::Em(_)));
                assert!(matches!(&rows[0][1][0], Inline::Code(_)));
            }
            _ => panic!("expected Table"),
        }

        let html = html_of(doc);
        assert!(html.contains("<strong>Name</strong>"));
        assert!(html.contains("<em>Type</em>"));
        assert!(html.contains("<code>int</code>"));
    }

    #[test]
    fn parses_table_with_three_columns() {
        let doc = r#"
====  ====  ====
A     B     C
====  ====  ====
1     2     3
4     5     6
====  ====  ====
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::Table { headers, rows } => {
                assert_eq!(headers.len(), 3);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 3);
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn table_with_empty_cells() {
        let doc = r#"
====  ====
A     B
====  ====
x
      y
====  ====
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::Table { headers, rows } => {
                assert_eq!(headers.len(), 2);
                assert_eq!(rows.len(), 2);
                assert!(!rows[0][0].is_empty());
                assert!(!rows[1][1].is_empty());
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn table_in_mixed_content() {
        let doc = r#"
Paragraph before table.

====  ====
A     B
====  ====
1     2
====  ====

Paragraph after table.
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 3);
        assert!(matches!(&ast[0], Block::Paragraph(_)));
        assert!(matches!(&ast[1], Block::Table { .. }));
        assert!(matches!(&ast[2], Block::Paragraph(_)));
    }

    #[test]
    fn parses_basic_grid_table() {
        let doc = r#"
+-------+-------+
| Col1  | Col2  |
+=======+=======+
| val1  | val2  |
+-------+-------+
| val3  | val4  |
+-------+-------+
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::Table { headers, rows } => {
                assert_eq!(headers.len(), 2);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 2);
                assert_eq!(rows[1].len(), 2);
            }
            _ => panic!("expected Table"),
        }

        let html = html_of(doc);
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>Col1</th>"));
        assert!(html.contains("<th>Col2</th>"));
        assert!(html.contains("<td>val1</td>"));
        assert!(html.contains("<td>val2</td>"));
    }

    #[test]
    fn parses_grid_table_with_inline_markup() {
        let doc = r#"
+-----------+-----------+
| **Name**  | *Type*    |
+===========+===========+
| foo       | `int`     |
+-----------+-----------+
| bar       | `str`     |
+-----------+-----------+
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::Table { headers, rows } => {
                assert_eq!(headers.len(), 2);
                assert!(matches!(&headers[0][0], Inline::Strong(_)));
                assert!(matches!(&headers[1][0], Inline::Em(_)));
                assert_eq!(rows.len(), 2);
            }
            _ => panic!("expected Table"),
        }

        let html = html_of(doc);
        assert!(html.contains("<strong>Name</strong>"));
        assert!(html.contains("<em>Type</em>"));
        assert!(html.contains("<code>int</code>"));
    }

    #[test]
    fn parses_grid_table_with_multiline_cells() {
        let doc = r#"
+-------+-------+
| A     | B     |
| long  | long  |
+=======+=======+
| val1  | val2  |
+-------+-------+
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::Table { headers, rows } => {
                assert_eq!(headers.len(), 2);
                let header0_text = ast::join_inlines(&headers[0]);
                assert!(header0_text.contains("A"));
                assert!(header0_text.contains("long"));
                assert_eq!(rows.len(), 1);
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn parses_grid_table_three_columns() {
        let doc = r#"
+----+----+----+
| A  | B  | C  |
+====+====+====+
| 1  | 2  | 3  |
+----+----+----+
| 4  | 5  | 6  |
+----+----+----+
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::Table { headers, rows } => {
                assert_eq!(headers.len(), 3);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 3);
                assert_eq!(rows[1].len(), 3);
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn grid_table_in_mixed_content() {
        let doc = r#"
Before table.

+----+----+
| A  | B  |
+====+====+
| 1  | 2  |
+----+----+

After table.
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 3);
        assert!(matches!(&ast[0], Block::Paragraph(_)));
        assert!(matches!(&ast[1], Block::Table { .. }));
        assert!(matches!(&ast[2], Block::Paragraph(_)));
    }

    #[test]
    fn parses_simple_comment() {
        let doc = r#"
.. This is a comment
   It continues on the next line
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::Comment(content) => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph(inlines) => {
                        let text = ast::join_inlines(inlines);
                        assert!(text.contains("This is a comment"));
                        assert!(text.contains("continues"));
                    }
                    _ => panic!("expected Paragraph in comment"),
                }
            }
            _ => panic!("expected Comment"),
        }
    }

    #[test]
    fn comment_excluded_from_html() {
        let doc = r#"
Visible paragraph.

.. This is a hidden comment
   with multiple lines

Another visible paragraph.
"#;
        let html = html_of(doc);
        assert!(html.contains("Visible paragraph"));
        assert!(html.contains("Another visible paragraph"));
        assert!(!html.contains("hidden comment"));
        assert!(!html.contains("multiple lines"));
    }

    #[test]
    fn comment_with_nested_list() {
        let doc = r#"
.. This comment contains a list:

   - Item 1
   - Item 2
   - Item 3
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::Comment(content) => {
                assert!(content.len() >= 1);
                let has_list = content.iter().any(|b| matches!(b, Block::List { .. }));
                assert!(has_list, "Comment should contain a list");
            }
            _ => panic!("expected Comment"),
        }
    }

    #[test]
    fn comment_vs_directive_distinction() {
        let doc = ".. This is a comment\n\n.. note::\n\n   This is a directive with content.";
        let ast = parse(doc).unwrap();
        let has_comment = ast.iter().any(|b| matches!(b, Block::Comment(_)));
        let has_directive = ast.iter().any(|b| matches!(b, Block::Directive { .. }));
        assert!(has_comment, "Should have a Comment block");
        assert!(has_directive, "Should have a Directive block");
        assert!(matches!(&ast[0], Block::Comment(_)), "First block should be Comment");
    }

    #[test]
    fn multiple_comments() {
        let doc = r#"
.. First comment

Some text.

.. Second comment
   with continuation

More text.
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 4);
        assert!(matches!(&ast[0], Block::Comment(_)));
        assert!(matches!(&ast[1], Block::Paragraph(_)));
        assert!(matches!(&ast[2], Block::Comment(_)));
        assert!(matches!(&ast[3], Block::Paragraph(_)));
    }

    #[test]
    fn empty_comment() {
        let doc = ".. ";
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::Comment(content) => {
                assert!(content.is_empty());
            }
            _ => panic!("expected Comment"),
        }
    }

    #[test]
    fn field_list_basic() {
        let doc = r#"
:param x: The x parameter
:param y: The y parameter
"#;
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::FieldList { fields } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "param");
                assert_eq!(fields[0].argument, "x");
                assert_eq!(fields[1].name, "param");
                assert_eq!(fields[1].argument, "y");
            }
            _ => panic!("expected FieldList"),
        }
    }

    #[test]
    fn field_list_empty_body() {
        let doc = ":param x:";
        let ast = parse(doc).unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            Block::FieldList { fields } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "param");
                assert_eq!(fields[0].argument, "x");
                assert!(fields[0].body.is_empty());
            }
            _ => panic!("expected FieldList"),
        }
    }

    #[test]
    fn field_list_variable_indent() {
        let doc = r#"
:param x: First line
  Continuation with 2 spaces
   Continuation with 3 spaces
    Continuation with 4 spaces
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::FieldList { fields } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].body.len(), 1);
                match &fields[0].body[0] {
                    Block::Paragraph(inlines) => {
                        let text = ast::join_inlines(inlines);
                        assert!(text.contains("First line"));
                        assert!(text.contains("Continuation"));
                    }
                    _ => panic!("expected Paragraph"),
                }
            }
            _ => panic!("expected FieldList"),
        }
    }

    #[test]
    fn field_list_with_nested_list() {
        let doc = r#"
:param items: The items to process:

  - Item 1
  - Item 2
  - Item 3
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::FieldList { fields } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "param");
                assert_eq!(fields[0].argument, "items");
                assert!(fields[0].body.len() >= 1);
                eprintln!("Field body blocks: {:?}", fields[0].body);
                let has_list = fields[0].body.iter().any(|b| matches!(b, Block::List { .. }));
                assert!(has_list, "Expected list in field body");
            }
            _ => panic!("expected FieldList"),
        }
    }

    #[test]
    fn field_list_with_code_block() {
        let doc = r#"
:example: Here's an example:

  ```
  code here
  ```
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::FieldList { fields } => {
                assert_eq!(fields.len(), 1);
                assert!(fields[0].body.len() >= 1);
                let has_code = fields[0].body.iter().any(|b| matches!(b, Block::CodeBlock(_)));
                assert!(has_code);
            }
            _ => panic!("expected FieldList"),
        }
    }

    #[test]
    fn field_list_renders_as_dl() {
        let doc = r#"
:param x: The x value
:returns: The result
"#;
        let html = html_of(doc);
        assert!(html.contains("<dl>"));
        assert!(html.contains("<dt>param x</dt>"));
        assert!(html.contains("<dd>"));
        assert!(html.contains("The x value"));
        assert!(html.contains("</dl>"));
    }

    #[test]
    fn field_list_multiple_blocks_in_body() {
        let doc = r#"
:param x: First paragraph about x.

  Second paragraph about x.

  - A list item
  - Another item
"#;
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::FieldList { fields } => {
                assert_eq!(fields.len(), 1);
                assert!(fields[0].body.len() >= 2);
            }
            _ => panic!("expected FieldList"),
        }
    }

    #[test]
    fn field_list_no_argument() {
        let doc = ":returns: The return value";
        let ast = parse(doc).unwrap();
        match &ast[0] {
            Block::FieldList { fields } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "returns");
                assert_eq!(fields[0].argument, "");
            }
            _ => panic!("expected FieldList"),
        }
    }
}
