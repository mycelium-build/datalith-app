use std::fmt::Write as _;

use yaml_serde::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frontmatter {
    pub(crate) properties: Vec<FrontmatterProperty>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontmatterProperty {
    pub(crate) key: String,
    pub(crate) values: Vec<FrontmatterValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrontmatterValue {
    Boolean(bool),
    Link { label: String, target: String },
    Text(String),
}

pub(super) fn extract_frontmatter(text: &str) -> (Option<String>, String) {
    if !text.starts_with("---") {
        return (None, text.to_string());
    }

    let Some(rest) = text.get(3..) else {
        return (None, text.to_string());
    };
    let Some(end) = rest.find("\n---") else {
        return (None, text.to_string());
    };
    let fm_content = rest.get(..end).unwrap_or_default();
    let body = rest.get(end.saturating_add(4)..).unwrap_or_default();
    (Some(fm_content.trim().to_string()), body.to_string())
}

pub(super) fn parse_frontmatter(content: &str) -> Frontmatter {
    let mapping = match yaml_serde::from_str(content) {
        Ok(Value::Mapping(mapping)) => mapping,
        Ok(_) => {
            return Frontmatter {
                properties: Vec::new(),
                error: Some(
                    "YAML frontmatter must be a mapping of property names to values".into(),
                ),
            };
        }
        Err(error) => {
            return Frontmatter {
                properties: Vec::new(),
                error: Some(format!("Invalid YAML frontmatter: {error}")),
            };
        }
    };
    Frontmatter {
        properties: mapping
            .into_iter()
            .filter_map(|(key, value)| {
                Some(FrontmatterProperty {
                    key: key.as_str()?.to_string(),
                    values: frontmatter_values(value),
                })
            })
            .collect(),
        error: None,
    }
}

fn frontmatter_values(value: Value) -> Vec<FrontmatterValue> {
    match value {
        Value::Null => Vec::new(),
        Value::Sequence(values) => values.into_iter().flat_map(frontmatter_values).collect(),
        Value::Bool(value) => vec![FrontmatterValue::Boolean(value)],
        Value::String(value) => vec![
            if let Some((label, target)) = parse_frontmatter_link(&value) {
                FrontmatterValue::Link {
                    label: label.to_string(),
                    target: target.to_string(),
                }
            } else {
                FrontmatterValue::Text(value)
            },
        ],
        Value::Number(value) => vec![FrontmatterValue::Text(value.to_string())],
        Value::Mapping(_) | Value::Tagged(_) => vec![FrontmatterValue::Text(
            yaml_serde::to_string(&value)
                .unwrap_or_default()
                .trim()
                .to_string(),
        )],
    }
}

fn parse_frontmatter_link(value: &str) -> Option<(&str, &str)> {
    if let Some(link) = value.strip_prefix("[[").and_then(|v| v.strip_suffix("]]")) {
        return Some(link.split_once('|').unwrap_or((link, link)));
    }
    let markdown = value.strip_prefix('[')?.strip_suffix(')')?;
    markdown.split_once("](")
}

fn needs_quoting(key: &str) -> bool {
    key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        || yaml_serde::from_str::<Value>(key)
            .is_ok_and(|parsed| !matches!(parsed, Value::String(_)))
}

fn quoted_yaml_scalar(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len().saturating_add(2));
    escaped.push('"');
    for c in value.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => {
                let code = u32::from(c);
                if code < 0x100 {
                    let _ = write!(escaped, "\\x{code:02x}");
                } else {
                    let _ = write!(escaped, "\\u{code:04x}");
                }
            }
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}

/// Serializes key/value pairs as YAML frontmatter lines, one `key: value` pair per line.
/// Keys are quoted only when they need it;
/// values are always double-quoted so plain scalars round-trip as strings
/// (never YAML booleans or numbers).
/// The output matches what the frontmatter parser accepts.
#[must_use]
pub fn serialize_properties(properties: &[(String, String)]) -> String {
    let mut lines = Vec::with_capacity(properties.len());
    for (key, value) in properties {
        let key = if needs_quoting(key) {
            quoted_yaml_scalar(key)
        } else {
            key.clone()
        };
        lines.push(format!("{key}: {}", quoted_yaml_scalar(value)));
    }
    lines.join("\n")
}

#[must_use]
pub fn build_note_document(properties: &[(String, String)], content: &str) -> String {
    if properties.is_empty() {
        return content.to_owned();
    }
    format!(
        "---\n{}\n---\n\n{}",
        serialize_properties(properties),
        content
    )
}

#[cfg(test)]
mod tests {

    use super::super::parse_markdown;
    use super::*;

    #[test]
    fn serializes_properties_that_round_trip_through_the_parser() {
        let properties = vec![
            ("title".to_owned(), "A: \"quoted\" value".to_owned()),
            ("tags".to_owned(), "true".to_owned()),
            ("multi".to_owned(), "line one\nline two".to_owned()),
            ("score".to_owned(), "3.14".to_owned()),
            ("odd key".to_owned(), "plain".to_owned()),
            ("2024".to_owned(), "year".to_owned()),
            ("true".to_owned(), "boolean-looking key".to_owned()),
        ];
        let document = parse_markdown(&build_note_document(&properties, "Body"));
        let frontmatter = document.frontmatter.expect("frontmatter");

        assert_eq!(
            frontmatter.properties,
            vec![
                FrontmatterProperty {
                    key: "title".into(),
                    values: vec![FrontmatterValue::Text("A: \"quoted\" value".into())],
                },
                FrontmatterProperty {
                    key: "tags".into(),
                    values: vec![FrontmatterValue::Text("true".into())],
                },
                FrontmatterProperty {
                    key: "multi".into(),
                    values: vec![FrontmatterValue::Text("line one\nline two".into())],
                },
                FrontmatterProperty {
                    key: "score".into(),
                    values: vec![FrontmatterValue::Text("3.14".into())],
                },
                FrontmatterProperty {
                    key: "odd key".into(),
                    values: vec![FrontmatterValue::Text("plain".into())],
                },
                FrontmatterProperty {
                    key: "2024".into(),
                    values: vec![FrontmatterValue::Text("year".into())],
                },
                FrontmatterProperty {
                    key: "true".into(),
                    values: vec![FrontmatterValue::Text("boolean-looking key".into())],
                },
            ]
        );
    }

    #[test]
    fn builds_content_only_document_without_properties() {
        assert_eq!(build_note_document(&[], "Body"), "Body");
    }

    #[test]
    fn parses_typed_frontmatter_properties() {
        let document = parse_markdown(
            "---\npublished: true\nrelated:\n  - \"[[Page|Alias]]\"\ntags: [rust, markdown]\n---\nBody",
        );
        let frontmatter = document.frontmatter.expect("frontmatter");

        assert_eq!(
            frontmatter.properties,
            vec![
                FrontmatterProperty {
                    key: "published".into(),
                    values: vec![FrontmatterValue::Boolean(true)],
                },
                FrontmatterProperty {
                    key: "related".into(),
                    values: vec![FrontmatterValue::Link {
                        label: "Page".into(),
                        target: "Alias".into(),
                    }],
                },
                FrontmatterProperty {
                    key: "tags".into(),
                    values: vec![
                        FrontmatterValue::Text("rust".into()),
                        FrontmatterValue::Text("markdown".into()),
                    ],
                },
            ]
        );
    }

    #[test]
    fn reports_invalid_yaml_frontmatter() {
        let document = parse_markdown("---\nd:\n 1. a\n 1. b\nloose text\n---\nBody");
        let frontmatter = document.frontmatter.expect("frontmatter");

        assert!(frontmatter.properties.is_empty());
        assert!(
            frontmatter
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with("Invalid YAML frontmatter:"))
        );
    }
}
