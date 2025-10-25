# parseRST

A lightweight, recursive-descent **reStructuredText parser** written in Rust.

## Overview

`parserst` is a small, fast, and self-contained parser for [reStructuredText](https://docutils.sourceforge.io/rst.html) documents.
It aims to provide a clean, idiomatic Rust API for converting `.rst` content into structured AST nodes or HTML/Markdown.

This crate is ideal for:

- Building static site generators, documentation tools, or format converters.
- Integrating reStructuredText support into note-taking or content-processing apps.
- Experimenting with parsing techniques and markup language design.
- Parsing Python [docstrings](https://github.com/stormlightlabs/beacon)

## Features

| Category              | Description                                                                             |
| --------------------- | --------------------------------------------------------------------------------------- |
| **Inline parsing**    | Supports `*emphasis*`, `**strong**`, `` `code` ``, and `` `link <https://...>`_``.      |
| **Block parsing**     | Detects headings, paragraphs, lists (ordered/unordered), code fences, and quote blocks. |
| **Output**            | Render to **HTML** or **Markdown** using built-in formatters.                           |
| **AST Access**        | Exposes a clean, typed AST (`Block`, `Inline`, `ListKind`) for custom rendering.        |
| **Error Handling**    | Safe `Result<Vec<Block>, ParseError>` API with detailed line numbers.                   |
| **Zero dependencies** | No external parser frameworks or macros                                                 |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
parserst = "0.1"
```

Then import it:

```rust
use parserst::{html_of, parse};
```

## Example

```rust
use parserst::html_of;

fn main() {
    let rst = r#"
Heading
=======

This is *emphasized*, **bold**, and ``inline code``.

- Item 1
- Item 2
"#;

    let html = html_of(rst);
    println!("{}", html);
}
```

**Output:**

```html
<h1>Heading</h1>
<p>
    This is <em>emphasized</em>, <strong>bold</strong>, and
    <code>inline code</code>.
</p>
<ul>
    <li>Item 1</li>
    <li>Item 2</li>
</ul>
```

## Design

- **Recursive Descent** — every rule is expressed in idiomatic Rust, not macros.
- **Predictable** — prioritizes correctness over complete reStructuredText parity.
- **Composabe** — easy to extend or replace the renderer layer (e.g., to JSON, Markdown, or AST tools).
- **No Unsafe** — guaranteed safe Rust implementation.

## Testing

```bash
cargo test
```

You can also run style and performance checks:

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## API Overview

| Function                   | Description                                 |
| -------------------------- | ------------------------------------------- |
| `parse(input: &str)`       | Parses `.rst` text into a `Vec<Block>` AST. |
| `html_of(input: &str)`     | Parses and renders the input as HTML.       |
| `markdown_of(input: &str)` | Parses and renders the input as Markdown.   |

### Types

| Item         | Description                                                              |
| ------------ | ------------------------------------------------------------------------ |
| `Block`      | Top-level AST nodes such as headings, paragraphs, directives, etc.       |
| `Inline`     | Inline nodes nested inside `Block` variants                              |
| `ListKind`   | Enum describing list flavor (`Ordered` or `Unordered`) for `Block::List` |

## License

See [MIT License](./LICENSE) or learn [more here](https://opensource.org/license/mit)

## Roadmap

- [x] Nested inline markup (e.g. `*bold and *italic* inside*`)
- [x] Support for `::` literal blocks
- [x] Directive syntax (`.. note::`, `.. code-block::`)
- [ ] Comment and field list improvements
- [ ] Table parsing (simple & grid)
- [ ] Feature Flags
    - [ ] `serde` feature for AST serialization
    - [ ] Markdown behind flag
- [ ] Fixtures/examples for tests
    - [ ] > 90% coverage

---

Made with ⚡️ by Stormlight Labs.

Stormlight Labs is just me, [Owais](https://github.com/desertthunder). Support my work on [Ko-fi](https://ko-fi.com/desertthunder).

[![Brainmade](https://brainmade.org/88x31-dark.png)](https://brainmade.org)
