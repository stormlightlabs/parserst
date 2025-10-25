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
| **Output**            | Render to **HTML** (always available) or **Markdown** (requires `markdown` feature).    |
| **AST Access**        | Exposes a clean, typed AST (`Block`, `Inline`, `ListKind`) for custom rendering.        |
| **Error Handling**    | Safe `Result<Vec<Block>, ParseError>` API with detailed line numbers.                   |
| **Zero dependencies** | No external parser frameworks or macros (Markdown support is optional)                  |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
parserst = "0.1"

# Or with markdown support
parserst = { version = "0.1", features = ["markdown"] }
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

Test with specific feature configurations:

```bash
# Test without markdown feature
cargo test --no-default-features

# Test with markdown feature
cargo test --features markdown

# Test with all features
cargo test --all-features
```

You can also run style and performance checks:

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Building

Build the library with different feature configurations:

```bash
# Default build (no markdown support)
cargo build

# Build with markdown feature
cargo build --features markdown

# Build without any features
cargo build --no-default-features

# Release build with all features
cargo build --release --all-features
```

Using `just` (if you have [just](https://github.com/casey/just) installed):

```bash
# Run all tests
just test

# Test specific configurations
just test-no-markdown
just test-markdown
just test-all

# Build variants
just build              # Default build
just build-markdown     # With markdown feature
just build-no-markdown  # Without any features
just build-all          # With all features
just build-release      # Release build
just build-release-all  # Release with all features

# Other commands
just lint               # Run clippy
just fmt                # Format code
just coverage           # Generate coverage report
```

## API Overview

| Function                   | Description                                                     |
| -------------------------- | --------------------------------------------------------------- |
| `parse(input: &str)`       | Parses `.rst` text into a `Vec<Block>` AST.                     |
| `html_of(input: &str)`     | Parses and renders the input as HTML.                           |
| `markdown_of(input: &str)` | Parses and renders the input as Markdown (requires `markdown` feature). |

### Types

| Item         | Description                                                              |
| ------------ | ------------------------------------------------------------------------ |
| `Block`      | Top-level AST nodes such as headings, paragraphs, directives, etc.       |
| `Inline`     | Inline nodes nested inside `Block` variants                              |
| `ListKind`   | Enum describing list flavor (`Ordered` or `Unordered`) for `Block::List` |

## License

See [MIT License](./LICENSE) or learn [more here](https://opensource.org/license/mit)

## Roadmap

- [x] Feature Flags
    - [x] `markdown` - Markdown support behind feature flag
    - [ ] `serde` - AST serialization support

---

Made with ⚡️ by Stormlight Labs.

Stormlight Labs is just me, [Owais](https://github.com/desertthunder). Support my work on [Ko-fi](https://ko-fi.com/desertthunder).

[![Brainmade](https://brainmade.org/88x31-dark.png)](https://brainmade.org)
