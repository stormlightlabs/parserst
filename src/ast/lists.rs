use crate::{Block, Inline, Lines};

/// List flavor used by [`Block::List`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Unordered,
    Ordered,
}

/// Try to parse a list (ordered or unordered)
pub fn try_parse_list(ls: &mut Lines<'_>) -> Option<Block> {
    let l = ls.peek()?;
    let kind = list_kind(l.raw)?;

    let mut items: Vec<Vec<Inline>> = Vec::new();
    while let Some(it) = ls.peek() {
        match list_kind(it.raw) {
            Some(next_kind) if next_kind == kind => {
                let line = ls.next().unwrap();
                let content = strip_list_marker(line.raw, kind).unwrap().trim_end();
                items.push(super::parse_inlines(content));
            }
            _ => break,
        }
    }
    Some(Block::List { kind, items })
}

pub fn list_kind(s: &str) -> Option<ListKind> {
    if is_bullet(s) {
        Some(ListKind::Unordered)
    } else if is_ordered_bullet(s) {
        Some(ListKind::Ordered)
    } else {
        None
    }
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

fn strip_list_marker(s: &str, kind: ListKind) -> Option<&str> {
    match kind {
        ListKind::Unordered => strip_bullet(s),
        ListKind::Ordered => strip_ordered_bullet(s),
    }
}
