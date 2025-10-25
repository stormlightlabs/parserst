use super::{Block, Inline, list_kind};
use crate::{Lines, ParseError, is_blank, leading_indent, parse, strip_indent_preserve};

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

pub fn parse_definition_entries(ls: &mut Lines<'_>) -> Result<Option<Vec<Block>>, ParseError> {
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

pub fn parse_field_entries(ls: &mut Lines<'_>) -> Result<Option<Vec<Block>>, ParseError> {
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
