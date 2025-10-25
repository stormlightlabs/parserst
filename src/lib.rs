//! Recursive descent reStructuredText parser that targets a lightweight AST.
//!
//! The crate exposes helpers to parse raw docstrings into [`Block`] nodes via [`parse`],
//! render them as HTML with [`html_of`], or normalize them into Markdown using [`markdown_of`].
//!
//! The internal parser is intentionally small and resilient enough to handle the
//! eclectic docstring styles used in the Python ecosystem.

mod ast;
pub mod error;
pub use ast::{Block, Inline, ListKind};
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

fn is_bullet(s: &str) -> bool {
    let t = s.trim_start();
    t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ")
}

fn is_ordered_bullet(s: &str) -> bool {
    let t = s.trim_start();
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i >= bytes.len() {
        return false;
    }
    bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' '
}

fn strip_bullet(s: &str) -> Option<&str> {
    let t = s.trim_start();
    for p in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(p) {
            return Some(rest);
        }
    }
    None
}

fn strip_ordered_bullet(s: &str) -> Option<&str> {
    let t = s.trim_start();
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i >= bytes.len() || bytes[i] != b'.' {
        return None;
    }
    if i + 1 >= bytes.len() || bytes[i + 1] != b' ' {
        return None;
    }
    Some(&t[i + 2..])
}

fn list_kind(s: &str) -> Option<ListKind> {
    if is_bullet(s) {
        Some(ListKind::Unordered)
    } else if is_ordered_bullet(s) {
        Some(ListKind::Ordered)
    } else {
        None
    }
}

fn strip_list_marker(s: &str, kind: ListKind) -> Option<&str> {
    match kind {
        ListKind::Unordered => strip_bullet(s),
        ListKind::Ordered => strip_ordered_bullet(s),
    }
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

fn is_definition_entry(s: &str) -> bool {
    let indent = leading_indent(s);
    let t = s.trim_start();
    if t.starts_with(':') || t.starts_with("..") {
        return false;
    }

    if indent == 0 {
        return match t.find(" : ") {
            Some(idx) => !t[..idx].trim().is_empty(),
            None => false,
        };
    }

    match t.find(':') {
        Some(idx) => !t[..idx].trim().is_empty(),
        None => false,
    }
}

fn build_definition_blocks(term: &str, classifier: Option<&str>, body_text: &str) -> Result<Vec<Block>, ParseError> {
    let mut blocks = if body_text.trim().is_empty() { Vec::new() } else { parse(body_text)? };

    let mut label = Vec::new();
    label.push(Inline::Strong(vec![Inline::Text(term.to_string())]));
    if let Some(classifier) = classifier {
        if !classifier.is_empty() {
            label.push(Inline::Text(" (".into()));
            label.push(Inline::Em(vec![Inline::Text(classifier.to_string())]));
            label.push(Inline::Text(")".into()));
        }
    }

    if blocks.is_empty() {
        blocks.push(Block::Paragraph(label));
        return Ok(blocks);
    }

    match &mut blocks[0] {
        Block::Paragraph(inlines) => {
            if !inlines.is_empty() {
                label.push(Inline::Text(": ".into()));
                label.append(inlines);
                *inlines = label;
            } else {
                *inlines = label;
            }
        }
        _ => {
            let mut para = label;
            para.push(Inline::Text(":".into()));
            blocks.insert(0, Block::Paragraph(para));
        }
    }

    Ok(blocks)
}

fn split_definition_line(input: &str) -> (String, Option<String>, String) {
    let idx = input.find(':').unwrap_or(input.len());
    let term_part = input[..idx].trim();
    let mut term = term_part.to_string();

    let after = if idx < input.len() { &input[idx + 1..] } else { "" };
    let prev_is_space = idx > 0 && input.as_bytes()[idx - 1] == b' ';
    let next_is_space = after.starts_with(' ');

    let mut classifier = None;
    if term.ends_with(')') {
        if let Some(open_idx) = term.rfind('(') {
            let inner = term[open_idx + 1..term.len() - 1].trim();
            if !inner.is_empty() {
                classifier = Some(inner.to_string());
                term = term[..open_idx].trim().to_string();
            }
        }
    }

    let after_trim = after.trim();
    let mut body_initial = String::new();

    if prev_is_space && next_is_space && !after_trim.is_empty() && classifier.is_none() {
        classifier = Some(after_trim.to_string());
    } else if !after_trim.is_empty() {
        body_initial = after_trim.to_string();
    }

    (term, classifier, body_initial)
}

fn parse_definition_entries(ls: &mut Lines<'_>) -> Result<Option<Vec<Block>>, ParseError> {
    let Some(line) = ls.peek() else {
        return Ok(None);
    };

    if !is_definition_entry(line.raw) {
        return Ok(None);
    }

    let mut blocks = Vec::new();

    while let Some(line) = ls.peek() {
        if !is_definition_entry(line.raw) {
            break;
        }

        let line = ls.next().unwrap();
        let trimmed = line.raw.trim_start();
        let (term, classifier_opt, body_initial) = split_definition_line(trimmed);
        let classifier_ref = classifier_opt.as_deref();
        let indent_base = leading_indent(line.raw);
        let mut body_text = String::new();

        if !body_initial.is_empty() {
            body_text.push_str(&body_initial);
        }

        while let Some(next) = ls.peek() {
            if is_blank(next.raw) {
                if let Some(after_blank) = ls.peek_next() {
                    if leading_indent(after_blank.raw) > indent_base {
                        ls.next();
                        if !body_text.is_empty() {
                            body_text.push('\n');
                            body_text.push('\n');
                        }
                        continue;
                    }
                }
                break;
            }

            if is_field_line(next.raw)
                || list_kind(next.raw).is_some()
                || is_definition_entry(next.raw)
                || next.raw.trim() == "```"
                || next.raw.trim_start().starts_with('>')
            {
                break;
            }

            let indent = leading_indent(next.raw);
            if indent <= indent_base {
                break;
            }

            let cont = ls.next().unwrap();
            let stripped = strip_indent_preserve(cont.raw, indent_base + 4).trim_end();
            if !body_text.is_empty() {
                body_text.push('\n');
            }
            body_text.push_str(stripped);
        }

        let mut entry_blocks = build_definition_blocks(&term, classifier_ref, &body_text)?;
        blocks.append(&mut entry_blocks);
    }

    Ok(Some(blocks))
}

fn field_label(kind: &str) -> String {
    let lower = kind.to_ascii_lowercase();
    match lower.as_str() {
        "param" | "parameter" | "arg" | "argument" => "Parameter".to_string(),
        "type" => "Type".to_string(),
        "return" | "returns" => "Returns".to_string(),
        "yield" | "yields" => "Yields".to_string(),
        "raise" | "raises" => "Raises".to_string(),
        "rtype" => "Return Type".to_string(),
        "ivar" => "Instance Variable".to_string(),
        "cvar" => "Class Variable".to_string(),
        "var" => "Variable".to_string(),
        "seealso" => "See Also".to_string(),
        "note" => "Note".to_string(),
        other => {
            if other.is_empty() {
                "Field".to_string()
            } else {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => other.to_string(),
                }
            }
        }
    }
}

fn build_field_label(kind: &str, arg: Option<&str>) -> Vec<Inline> {
    let mut inlines = Vec::new();
    inlines.push(Inline::Strong(vec![Inline::Text(field_label(kind))]));
    if let Some(arg) = arg {
        if !arg.is_empty() {
            inlines.push(Inline::Text(" ".into()));
            inlines.push(Inline::Code(arg.to_string()));
        }
    }
    inlines
}

fn is_field_line(s: &str) -> bool {
    let t = s.trim_start();
    if !t.starts_with(':') {
        return false;
    }
    let rest = &t[1..];
    if let Some(end) = rest.find(':') { !rest[..end].trim().is_empty() } else { false }
}

fn build_field_blocks(kind: &str, arg: Option<&str>, body_text: &str) -> Result<Vec<Block>, ParseError> {
    let mut blocks = if body_text.trim().is_empty() { Vec::new() } else { parse(body_text)? };
    let mut label_inlines = build_field_label(kind, arg);
    if blocks.is_empty() {
        blocks.push(Block::Paragraph(label_inlines));
        return Ok(blocks);
    }
    match &mut blocks[0] {
        Block::Paragraph(inlines) => {
            if !inlines.is_empty() {
                label_inlines.push(Inline::Text(": ".into()));
                label_inlines.append(inlines);
                *inlines = label_inlines;
            } else {
                *inlines = label_inlines;
            }
        }
        _ => {
            let mut para = label_inlines;
            para.push(Inline::Text(":".into()));
            blocks.insert(0, Block::Paragraph(para));
        }
    }
    Ok(blocks)
}

fn parse_field_entries(ls: &mut Lines<'_>) -> Result<Option<Vec<Block>>, ParseError> {
    let Some(line) = ls.peek() else {
        return Ok(None);
    };
    if !is_field_line(line.raw) {
        return Ok(None);
    }

    let mut blocks = Vec::new();

    while let Some(line) = ls.peek() {
        if !is_field_line(line.raw) {
            break;
        }

        let line = ls.next().unwrap();
        let trimmed = line.raw.trim_start();
        let rest = &trimmed[1..];
        let colon_idx = rest.find(':').unwrap();
        let heading = rest[..colon_idx].trim();
        let mut parts = heading.splitn(2, ' ');
        let kind = parts.next().unwrap();
        let arg = parts.next().map(|s| s.trim()).filter(|s| !s.is_empty());
        let body_initial = rest[colon_idx + 1..].trim_start();
        let mut body_text = String::new();
        if !body_initial.is_empty() {
            body_text.push_str(body_initial);
        }
        let indent_base = leading_indent(line.raw);

        while let Some(next) = ls.peek() {
            if is_blank(next.raw) {
                if let Some(after_blank) = ls.peek_next() {
                    if leading_indent(after_blank.raw) > indent_base {
                        ls.next();
                        if !body_text.is_empty() {
                            body_text.push('\n');
                            body_text.push('\n');
                        }
                        continue;
                    }
                }
                break;
            }

            if is_field_line(next.raw)
                || list_kind(next.raw).is_some()
                || next.raw.trim() == "```"
                || next.raw.trim_start().starts_with('>')
            {
                break;
            }

            let indent = leading_indent(next.raw);
            if indent <= indent_base {
                break;
            }

            let cont = ls.next().unwrap();
            let stripped = strip_indent_preserve(cont.raw, indent_base + 4).trim_end();
            if !body_text.is_empty() {
                body_text.push('\n');
            }
            body_text.push_str(stripped);
        }

        let mut entry_blocks = build_field_blocks(kind, arg, &body_text)?;
        blocks.append(&mut entry_blocks);
    }

    Ok(Some(blocks))
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

/// Find closing single asterisk that is not part of a double asterisk
fn find_single_asterisk_close(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    for i in 0..text.len() {
        if bytes[i] == b'*' {
            let before_is_asterisk = i > 0 && bytes[i - 1] == b'*';
            let after_is_asterisk = i + 1 < bytes.len() && bytes[i + 1] == b'*';
            if !before_is_asterisk && !after_is_asterisk {
                return Some(i);
            }
        }
    }
    None
}

/// Recursive descent parser for inline markup with nesting support.
/// Handles **strong**, *em*, `code`, and `text <url>`_ references.
fn parse_inlines(text: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    let flush_text = |buf: &mut String, out: &mut Vec<Inline>| {
        if !buf.is_empty() {
            out.push(Inline::Text(std::mem::take(buf)));
        }
    };

    while i < text.len() {
        if bytes[i] == b'`' && i + 1 < text.len() && bytes[i + 1] == b'`' {
            if let Some(end) = text[i + 2..].find("``") {
                let inner = &text[i + 2..i + 2 + end];
                flush_text(&mut buf, &mut out);
                out.push(Inline::Code(inner.to_string()));
                i += 2 + end + 2;
                continue;
            }
        }

        if bytes[i] == b'*' && i + 1 < text.len() && bytes[i + 1] == b'*' {
            if let Some(end) = text[i + 2..].find("**") {
                let inner = &text[i + 2..i + 2 + end];
                if !inner.is_empty() {
                    flush_text(&mut buf, &mut out);
                    let children = parse_inlines(inner);
                    out.push(Inline::Strong(children));
                    i += 2 + end + 2;
                    continue;
                }
            }
        }

        if bytes[i] == b'*' {
            if let Some(end) = find_single_asterisk_close(&text[i + 1..]) {
                let inner = &text[i + 1..i + 1 + end];
                if !inner.is_empty() {
                    flush_text(&mut buf, &mut out);
                    let children = parse_inlines(inner);
                    out.push(Inline::Em(children));
                    i += 1 + end + 1;
                    continue;
                }
            }
        }

        if bytes[i] == b'`' {
            if let Some(end) = text[i + 1..].find('`') {
                let closing_tick = i + 1 + end;
                let after_tick = closing_tick + 1;

                if after_tick < text.len() && bytes[after_tick] == b'_' {
                    let inner = &text[i + 1..closing_tick];
                    if let (Some(l), Some(r)) = (inner.find('<'), inner.rfind('>')) {
                        if r > l {
                            let label = inner[..l].trim();
                            let url = inner[l + 1..r].trim();
                            if !label.is_empty() && !url.is_empty() {
                                flush_text(&mut buf, &mut out);
                                let text_children = parse_inlines(label);
                                out.push(Inline::Link { text: text_children, url: url.to_string() });
                                i = after_tick + 1;
                                continue;
                            }
                        }
                    }
                }

                flush_text(&mut buf, &mut out);
                let inner = &text[i + 1..closing_tick];
                out.push(Inline::Code(inner.to_string()));
                i = closing_tick + 1;
                continue;
            }
        }

        let ch = text[i..].chars().next().unwrap();
        buf.push(ch);
        i += ch.len_utf8();
    }

    flush_text(&mut buf, &mut out);
    out
}

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

/// Skip blank lines in the input
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

/// Try to parse a list (ordered or unordered)
fn try_parse_list(ls: &mut Lines<'_>) -> Option<Block> {
    let l = ls.peek()?;
    let kind = list_kind(l.raw)?;

    let mut items: Vec<Vec<Inline>> = Vec::new();
    while let Some(it) = ls.peek() {
        match list_kind(it.raw) {
            Some(next_kind) if next_kind == kind => {
                let line = ls.next().unwrap();
                let content = strip_list_marker(line.raw, kind).unwrap().trim_end();
                items.push(parse_inlines(content));
            }
            _ => break,
        }
    }
    Some(Block::List { kind, items })
}

/// Try to parse a colon-style heading (Heading:)
fn try_parse_colon_heading(ls: &mut Lines<'_>) -> Option<Block> {
    let line = ls.peek()?;
    let title = colon_heading_text(line, ls.peek_next())?;
    ls.next();
    Some(Block::Heading { level: 2, inlines: parse_inlines(&title) })
}

/// Try to parse a setext-style heading (underlined with = or -)
fn try_parse_setext_heading(ls: &mut Lines<'_>) -> Option<Block> {
    let title = ls.next()?;
    let ul = ls.peek()?;
    let level = underline_level(ul.raw)?;
    ls.next();
    let inlines = parse_inlines(title.raw.trim());
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
    is_blank(line) || list_kind(line).is_some() || line.trim() == "```" || line.trim_start().starts_with('>')
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
    if text.is_empty() { None } else { Some(Block::Paragraph(parse_inlines(text))) }
}

/// Parse raw reStructuredText-like input into a vector of [`Block`] nodes.
///
/// The parser walks the input top-to-bottom, attempting the most specific block constructs
/// first (code fences, block quotes, lists, field lists, definition lists, headings) before
/// falling back to paragraphs. When the stream cannot be consumed because of malformed markup,
/// a [`ParseError`] is returned to the caller.
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

        if let Some(block) = try_parse_list(&mut ls) {
            blocks.push(block);
            continue;
        }

        if let Some(block) = try_parse_directive(&mut ls)? {
            blocks.push(block);
            continue;
        }

        if let Some(field_blocks) = parse_field_entries(&mut ls)? {
            blocks.extend(field_blocks);
            continue;
        }

        if let Some(def_blocks) = parse_definition_entries(&mut ls)? {
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
        let inl = parse_inlines(line);
        let html = ast::join_inlines(&inl);
        assert!(html.contains("<em>word</em>"));
        assert!(html.contains("<strong>strong</strong>"));
    }

    #[test]
    fn parses_inline_code() {
        let line = "Inline `code` works";
        let html = ast::join_inlines(&parse_inlines(line));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn parses_double_backtick_code() {
        let line = "Use ``inline`` literals";
        let html = ast::join_inlines(&parse_inlines(line));
        assert!(html.contains("<code>inline</code>"));
    }

    #[test]
    fn parses_inline_link() {
        let line = "`example <https://example.com>`_";
        let html = ast::join_inlines(&parse_inlines(line));
        assert!(html.contains("<a href=\"https://example.com\">example</a>"));
    }

    #[test]
    fn inline_link_requires_reference_suffix() {
        let line = "`example <https://example.com>`";
        let inl = parse_inlines(line);
        assert_eq!(inl, vec![Inline::Code("example <https://example.com>".into())]);
    }

    #[test]
    fn inline_link_mixed_with_text() {
        let line = "Read `docs <https://example.com>`_ now.";
        let inl = parse_inlines(line);
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
        let html = ast::join_inlines(&parse_inlines(line));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>em</em>"));
        assert!(html.contains("<code>code</code>"));
        assert!(html.contains("<a href=\"x\">link</a>"));
    }

    #[test]
    fn unmatched_markup_falls_back_to_text() {
        let line = "An *unfinished emphasis";
        let inl = parse_inlines(line);
        assert_eq!(inl, vec![Inline::Text("An *unfinished emphasis".into())]);
    }

    #[test]
    fn html_of_renders_expected_html() {
        let doc = "Heading\n=======\n\nBody text.";
        let rendered = html_of(doc);
        assert_eq!(rendered.trim(), "<h1>Heading</h1>\n<p>Body text.</p>");
    }

    #[test]
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

        assert!(matches!(ast.len(), 2));
        match &ast[0] {
            Block::Paragraph(inlines) => {
                assert_eq!(inlines[0], Inline::Strong(vec![Inline::Text("Parameter".into())]));
                assert_eq!(inlines[1], Inline::Text(" ".into()));
                assert_eq!(inlines[2], Inline::Code("foo".into()));
            }
            _ => panic!("expected paragraph for field"),
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
        let inl = parse_inlines(line);
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
        let inl = parse_inlines(line);
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
        let html = ast::join_inlines(&parse_inlines(line));
        assert_eq!(html, "<strong>bold <em>italic</em> text</strong>");
    }

    #[test]
    fn parses_link_with_nested_markup() {
        let line = "`**bold** link <https://example.com>`_";
        let inl = parse_inlines(line);
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
        let inl = parse_inlines(line);
        assert!(matches!(&inl[1], Inline::Code(s) if s == "**not bold**"));
    }

    #[test]
    fn multiple_levels_of_nesting() {
        let line = "**strong with *emphasis* inside** and *emphasis with **strong** inside*";
        let html = ast::join_inlines(&parse_inlines(line));
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
}
