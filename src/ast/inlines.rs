use crate::Inline;

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
pub fn parse_inlines(text: &str) -> Vec<Inline> {
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
