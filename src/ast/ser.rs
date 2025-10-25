//! Serialization support for the AST types.
//!
//! When the `serde` feature is enabled, all AST types ([`Inline`], [`Block`], [`Field`], and [`ListKind`])
//! implement [`serde::Serialize`] and [`serde::Deserialize`] via derive macros.
//!
//! This allows you to serialize parsed documents to JSON, YAML, or any other format supported by serde.
//!
//! ## Example
//!
//! ```ignore
//! use parserst::{parse, Block};
//!
//! let doc = "# Heading\n\nParagraph text.";
//! let ast = parse(doc).unwrap();
//!
//! // Serialize to JSON
//! let json = serde_json::to_string_pretty(&ast).unwrap();
//! println!("{}", json);
//!
//! // Deserialize from JSON
//! let parsed: Vec<Block> = serde_json::from_str(&json).unwrap();
//! assert_eq!(ast, parsed);
//! ```

#[cfg(feature = "serde")]
pub use serde::{Deserialize, Serialize};

#[cfg(all(test, feature = "serde"))]
mod tests {
    use crate::{Block, Field, Inline, ListKind, parse};

    #[test]
    fn roundtrip_inline_text_json() {
        let inline = Inline::Text("Hello, world!".to_string());
        let json = serde_json::to_string(&inline).unwrap();
        let deserialized: Inline = serde_json::from_str(&json).unwrap();
        assert_eq!(inline, deserialized);
    }

    #[test]
    fn roundtrip_inline_nested_json() {
        let inline = Inline::Strong(vec![
            Inline::Text("bold ".to_string()),
            Inline::Em(vec![Inline::Text("italic".to_string())]),
            Inline::Text(" text".to_string()),
        ]);
        let json = serde_json::to_string(&inline).unwrap();
        let deserialized: Inline = serde_json::from_str(&json).unwrap();
        assert_eq!(inline, deserialized);
    }

    #[test]
    fn roundtrip_inline_link_json() {
        let inline =
            Inline::Link { text: vec![Inline::Text("example".to_string())], url: "https://example.com".to_string() };
        let json = serde_json::to_string(&inline).unwrap();
        let deserialized: Inline = serde_json::from_str(&json).unwrap();
        assert_eq!(inline, deserialized);
    }

    #[test]
    fn roundtrip_block_heading_json() {
        let block = Block::Heading { level: 1, inlines: vec![Inline::Text("Title".to_string())] };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn roundtrip_block_paragraph_json() {
        let block = Block::Paragraph(vec![
            Inline::Text("Some ".to_string()),
            Inline::Em(vec![Inline::Text("text".to_string())]),
        ]);
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn roundtrip_block_list_json() {
        let block = Block::List {
            kind: ListKind::Unordered,
            items: vec![
                vec![Inline::Text("Item 1".to_string())],
                vec![Inline::Text("Item 2".to_string())],
                vec![Inline::Text("Item 3".to_string())],
            ],
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn roundtrip_block_table_json() {
        let block = Block::Table {
            headers: vec![
                vec![Inline::Text("Col1".to_string())],
                vec![Inline::Text("Col2".to_string())],
            ],
            rows: vec![
                vec![
                    vec![Inline::Text("val1".to_string())],
                    vec![Inline::Text("val2".to_string())],
                ],
                vec![
                    vec![Inline::Text("val3".to_string())],
                    vec![Inline::Text("val4".to_string())],
                ],
            ],
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn roundtrip_block_directive_json() {
        let block = Block::Directive {
            name: "note".to_string(),
            argument: "".to_string(),
            content: vec![Block::Paragraph(vec![Inline::Text("Note content".to_string())])],
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn roundtrip_field_json() {
        let field = Field {
            name: "param".to_string(),
            argument: "x".to_string(),
            body: vec![Block::Paragraph(vec![Inline::Text("Description".to_string())])],
        };
        let json = serde_json::to_string(&field).unwrap();
        let deserialized: Field = serde_json::from_str(&json).unwrap();
        assert_eq!(field, deserialized);
    }

    #[test]
    fn roundtrip_block_field_list_json() {
        let block = Block::FieldList {
            fields: vec![
                Field {
                    name: "param".to_string(),
                    argument: "x".to_string(),
                    body: vec![Block::Paragraph(vec![Inline::Text("X value".to_string())])],
                },
                Field {
                    name: "returns".to_string(),
                    argument: "".to_string(),
                    body: vec![Block::Paragraph(vec![Inline::Text("Result".to_string())])],
                },
            ],
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn roundtrip_list_kind_json() {
        let ordered = ListKind::Ordered;
        let json = serde_json::to_string(&ordered).unwrap();
        let deserialized: ListKind = serde_json::from_str(&json).unwrap();
        assert_eq!(ordered, deserialized);

        let unordered = ListKind::Unordered;
        let json = serde_json::to_string(&unordered).unwrap();
        let deserialized: ListKind = serde_json::from_str(&json).unwrap();
        assert_eq!(unordered, deserialized);
    }

    #[test]
    fn roundtrip_parsed_document_json() {
        let doc = r#"
Title
=====

A paragraph with *emphasis* and **strong** text.

- Item 1
- Item 2

:param x: The x parameter
:returns: The result
"#;
        let ast = parse(doc).unwrap();
        let json = serde_json::to_string_pretty(&ast).unwrap();
        let deserialized: Vec<Block> = serde_json::from_str(&json).unwrap();
        assert_eq!(ast, deserialized);
    }

    #[test]
    fn roundtrip_complex_document_json() {
        let doc = r#"
Heading
-------

> A quote block with *emphasis*.

```
code block
```

.. note::

   A note directive.

====  ====
Col1  Col2
====  ====
val1  val2
====  ====
"#;
        let ast = parse(doc).unwrap();
        let json = serde_json::to_string_pretty(&ast).unwrap();
        let deserialized: Vec<Block> = serde_json::from_str(&json).unwrap();
        assert_eq!(ast, deserialized);
    }

    #[test]
    fn roundtrip_inline_text_yaml() {
        let inline = Inline::Text("Hello, world!".to_string());
        let yaml = serde_yml::to_string(&inline).unwrap();
        let deserialized: Inline = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(inline, deserialized);
    }

    #[test]
    fn roundtrip_block_heading_yaml() {
        let block = Block::Heading { level: 1, inlines: vec![Inline::Text("Title".to_string())] };
        let yaml = serde_yml::to_string(&block).unwrap();
        let deserialized: Block = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn roundtrip_parsed_document_yaml() {
        let doc = r#"
Title
=====

A paragraph with *emphasis*.

- Item 1
- Item 2
"#;
        let ast = parse(doc).unwrap();
        let yaml = serde_yml::to_string(&ast).unwrap();
        let deserialized: Vec<Block> = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(ast, deserialized);
    }

    #[test]
    fn roundtrip_deeply_nested_structure() {
        let block = Block::Quote(vec![
            Block::Paragraph(vec![Inline::Strong(vec![
                Inline::Text("Bold with ".to_string()),
                Inline::Em(vec![Inline::Text("nested italic".to_string())]),
                Inline::Text(" and ".to_string()),
                Inline::Link {
                    text: vec![Inline::Code("code link".to_string())],
                    url: "https://example.com".to_string(),
                },
            ])]),
            Block::List {
                kind: ListKind::Ordered,
                items: vec![
                    vec![
                        Inline::Text("Item with ".to_string()),
                        Inline::Em(vec![Inline::Text("emphasis".to_string())]),
                    ],
                    vec![Inline::Code("code item".to_string())],
                ],
            },
        ]);
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn roundtrip_empty_collections() {
        let empty_paragraph = Block::Paragraph(vec![]);
        let json = serde_json::to_string(&empty_paragraph).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(empty_paragraph, deserialized);

        let empty_list = Block::List { kind: ListKind::Unordered, items: vec![] };
        let json = serde_json::to_string(&empty_list).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(empty_list, deserialized);

        let empty_table = Block::Table { headers: vec![], rows: vec![] };
        let json = serde_json::to_string(&empty_table).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(empty_table, deserialized);
    }

    #[test]
    fn json_format_is_readable() {
        let block = Block::Paragraph(vec![
            Inline::Text("Hello ".to_string()),
            Inline::Em(vec![Inline::Text("world".to_string())]),
        ]);
        let json = serde_json::to_string_pretty(&block).unwrap();

        assert!(json.contains("Paragraph"));
        assert!(json.contains("Text"));
        assert!(json.contains("Em"));
        assert!(json.contains("Hello"));
        assert!(json.contains("world"));
    }

    #[test]
    fn yaml_format_is_readable() {
        let block = Block::Paragraph(vec![
            Inline::Text("Hello ".to_string()),
            Inline::Em(vec![Inline::Text("world".to_string())]),
        ]);
        let yaml = serde_yml::to_string(&block).unwrap();

        assert!(yaml.contains("Paragraph"));
        assert!(yaml.contains("Text"));
        assert!(yaml.contains("Em"));
        assert!(yaml.contains("Hello"));
        assert!(yaml.contains("world"));
    }
}
