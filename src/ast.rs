/// Inline-level nodes produced by the parser.
///
/// These are rendered directly to HTML via [`std::fmt::Display`] and are reused
/// by both the HTML and Markdown pipelines.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Em(Vec<Inline>),
    Strong(Vec<Inline>),
    Code(String),
    Link { text: Vec<Inline>, url: String },
}

impl std::fmt::Display for Inline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Inline::Text(t) => write!(f, "{t}"),
            Inline::Em(children) => write!(f, "<em>{}</em>", join_inlines(children)),
            Inline::Strong(children) => write!(f, "<strong>{}</strong>", join_inlines(children)),
            Inline::Code(t) => write!(f, "<code>{}</code>", html_escape(t)),
            Inline::Link { text, url } => write!(f, "<a href=\"{url}\">{}</a>", join_inlines(text)),
        }
    }
}

pub fn join_inlines(v: &[Inline]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("")
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Block-level nodes in the parsed document tree.
///
/// Blocks embed [`Inline`] nodes where appropriate and carry the semantic shape
/// required for downstream renderers.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    List {
        kind: ListKind,
        items: Vec<Vec<Inline>>,
    },
    CodeBlock(String),
    Quote(Vec<Block>),
    LiteralBlock(String),
    Directive {
        name: String,
        argument: String,
        content: Vec<Block>,
    },
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Block::Heading { level, inlines } => {
                let tag = match level {
                    1 => "h1",
                    2 => "h2",
                    _ => "h2",
                };
                write!(f, "<{}>{}</{}>", tag, join_inlines(inlines), tag)
            }
            Block::Paragraph(inl) => write!(f, "<p>{}</p>", join_inlines(inl)),
            Block::List { kind, items } => {
                let tag = match kind {
                    ListKind::Unordered => "ul",
                    ListKind::Ordered => "ol",
                };
                write!(f, "<{tag}>")?;
                for it in items {
                    write!(f, "<li>{}</li>", join_inlines(it))?;
                }
                write!(f, "</{tag}>")
            }
            Block::CodeBlock(code) => write!(f, "<pre><code>{}</code></pre>", html_escape(code)),
            Block::Quote(children) => {
                write!(f, "<blockquote>")?;
                for b in children {
                    write!(f, "{b}")?;
                }
                write!(f, "</blockquote>")
            }
            Block::LiteralBlock(code) => {
                write!(f, "<pre><code>{}</code></pre>", html_escape(code))
            }
            Block::Directive { name, argument, content } => render_directive(f, name, argument, content),
        }
    }
}

/// Render directive to HTML based on directive type
fn render_directive(
    f: &mut std::fmt::Formatter<'_>, name: &str, argument: &str, content: &[Block],
) -> std::fmt::Result {
    match name {
        "note" | "warning" | "tip" | "caution" | "danger" | "attention" | "important" => {
            let class = name;
            write!(f, "<div class=\"admonition {class}\">")?;
            write!(f, "<p class=\"admonition-title\">{}</p>", capitalize(name))?;
            for block in content {
                write!(f, "{block}")?;
            }
            write!(f, "</div>")
        }
        "code-block" | "code" => {
            let lang = if argument.is_empty() { "" } else { argument };
            let lang_attr = if lang.is_empty() { String::new() } else { format!(" class=\"language-{lang}\"") };
            write!(f, "<pre><code{lang_attr}>")?;
            for block in content {
                if let Block::LiteralBlock(code) = block {
                    write!(f, "{}", html_escape(code))?;
                } else if let Block::Paragraph(inlines) = block {
                    write!(f, "{}", join_inlines(inlines))?;
                }
            }
            write!(f, "</code></pre>")
        }
        "image" => {
            let alt = if content.is_empty() { String::new() } else { "image".to_string() };
            write!(f, "<img src=\"{argument}\" alt=\"{alt}\" />")
        }
        _ => {
            // Unknown directive - render as div with class
            write!(f, "<div class=\"directive directive-{name}\">")?;
            if !argument.is_empty() {
                write!(f, "<p><code>{}</code></p>", html_escape(argument))?;
            }
            for block in content {
                write!(f, "{block}")?;
            }
            write!(f, "</div>")
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// List flavor used by [`Block::List`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Unordered,
    Ordered,
}
