use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use yaml_serde::Value;

use crate::vault::{
    CatalogComparison, CatalogFileField, CatalogFilter, CatalogProperty, CatalogScalar,
};

use std::path::Path;

pub struct DocumentFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) properties: &'a Value,
    pub(crate) size_bytes: Option<i64>,
    pub(crate) modified_ns: Option<i64>,
    pub(crate) links: &'a [String],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Filter {
    #[default]
    MatchAll,
    Expression(Expression),
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
}

impl Filter {
    pub(crate) fn to_catalog_filter(&self) -> CatalogFilter {
        match self {
            Self::MatchAll => CatalogFilter::MatchAll,
            Self::Expression(expression) => expression.to_catalog_filter(),
            Self::And(filters) => {
                CatalogFilter::And(filters.iter().map(Self::to_catalog_filter).collect())
            }
            Self::Or(filters) => {
                CatalogFilter::Or(filters.iter().map(Self::to_catalog_filter).collect())
            }
            Self::Not(filter) => CatalogFilter::Not(Box::new(filter.to_catalog_filter())),
        }
    }

    pub(crate) fn matches(&self, file: &DocumentFile<'_>) -> bool {
        match self {
            Self::Expression(expression) => expression.matches(file),
            Self::And(filters) => filters.iter().all(|filter| filter.matches(file)),
            Self::Or(filters) => filters.iter().any(|filter| filter.matches(file)),
            Self::Not(filter) => !filter.matches(file),
            Self::MatchAll => true,
        }
    }
}

impl<'de> Deserialize<'de> for Filter {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        parse_filter(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expression {
    pub(super) left: PropertyPath,
    pub(super) operation: Operation,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Operation {
    Compare(Comparison, Scalar),
    Contains(Scalar),
    InFolder(String),
    HasTag(String),
    HasLink(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Comparison {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyPath {
    Note(Vec<String>),
    File(FileField),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileField {
    Name,
    Ext,
    Path,
    Folder,
    Size,
    Mtime,
    Links,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Scalar {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

impl Expression {
    fn to_catalog_filter(&self) -> CatalogFilter {
        let property = match &self.left {
            PropertyPath::Note(parts) => CatalogProperty::Metadata(parts.clone()),
            PropertyPath::File(field) => CatalogProperty::File(match field {
                FileField::Name => CatalogFileField::Name,
                FileField::Ext => CatalogFileField::Extension,
                FileField::Path | FileField::Links => CatalogFileField::Path,
                FileField::Folder => CatalogFileField::Folder,
                FileField::Size => CatalogFileField::Size,
                FileField::Mtime => CatalogFileField::Modified,
            }),
        };
        match &self.operation {
            Operation::InFolder(folder) => CatalogFilter::InFolder(folder.clone()),
            Operation::HasTag(tag) => CatalogFilter::HasTag(tag.clone()),
            Operation::HasLink(link) => CatalogFilter::HasLink(link.clone()),
            Operation::Compare(comparison, value) => CatalogFilter::Compare {
                property,
                comparison: match comparison {
                    Comparison::Equal => CatalogComparison::Equal,
                    Comparison::NotEqual => CatalogComparison::NotEqual,
                    Comparison::Greater => CatalogComparison::Greater,
                    Comparison::GreaterEqual => CatalogComparison::GreaterEqual,
                    Comparison::Less => CatalogComparison::Less,
                    Comparison::LessEqual => CatalogComparison::LessEqual,
                },
                value: value.to_catalog_scalar(),
            },
            Operation::Contains(value) => CatalogFilter::Contains {
                property,
                value: value.to_catalog_scalar(),
            },
        }
    }

    pub(crate) fn matches(&self, file: &DocumentFile<'_>) -> bool {
        match &self.operation {
            Operation::InFolder(folder) => {
                let parent = file
                    .path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(""))
                    .to_string_lossy()
                    .replace('\\', "/");
                parent == *folder || parent.starts_with(&format!("{folder}/"))
            }
            Operation::Contains(expected) => value_at(&self.left, file).is_some_and(|value| {
                value
                    .as_sequence()
                    .is_some_and(|values| values.iter().any(|value| scalar_equals(value, expected)))
            }),
            Operation::HasTag(expected) => {
                value_at(&PropertyPath::Note(vec!["tags".to_string()]), file).is_some_and(|value| {
                    value.as_sequence().is_some_and(|values| {
                        values
                            .iter()
                            .any(|value| scalar_equals(value, &Scalar::String(expected.clone())))
                    }) || scalar_equals(value, &Scalar::String(expected.clone()))
                })
            }
            Operation::HasLink(expected) => file.links.iter().any(|link| {
                link == expected
                    || link
                        .strip_suffix(".md")
                        .is_some_and(|without_extension| without_extension == expected)
            }),
            Operation::Compare(comparison, expected) => file_scalar(&self.left, file).map_or_else(
                || compare(value_at(&self.left, file), *comparison, expected),
                |actual| compare_scalars(&actual, *comparison, expected),
            ),
        }
    }
}

impl Scalar {
    fn to_catalog_scalar(&self) -> CatalogScalar {
        match self {
            Self::Null => CatalogScalar::Null,
            Self::Bool(value) => CatalogScalar::Bool(*value),
            Self::Number(value) => CatalogScalar::Number(*value),
            Self::String(value) => CatalogScalar::String(value.clone()),
        }
    }
}

fn value_at<'a>(path: &PropertyPath, file: &'a DocumentFile<'_>) -> Option<&'a Value> {
    match path {
        PropertyPath::Note(parts) => parts
            .iter()
            .try_fold(file.properties, |value, part| value.get(part)),
        PropertyPath::File(_) => None,
    }
}

fn file_scalar(path: &PropertyPath, file: &DocumentFile<'_>) -> Option<Scalar> {
    let PropertyPath::File(field) = path else {
        return None;
    };
    let string = match field {
        FileField::Name => file.path.file_stem()?.to_str()?.to_string(),
        FileField::Ext => file.path.extension()?.to_str()?.to_string(),
        FileField::Path => file.path.to_string_lossy().replace('\\', "/"),
        FileField::Folder => file
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""))
            .to_string_lossy()
            .replace('\\', "/"),
        FileField::Size => {
            return file
                .size_bytes
                .and_then(|value| value.to_string().parse().ok())
                .map(Scalar::Number);
        }
        FileField::Mtime => {
            return file
                .modified_ns
                .and_then(|value| value.to_string().parse().ok())
                .map(Scalar::Number);
        }
        FileField::Links => return None,
    };
    Some(Scalar::String(string))
}

fn compare(actual: Option<&Value>, comparison: Comparison, expected: &Scalar) -> bool {
    let Some(actual) = actual else {
        if matches!(expected, Scalar::Null) {
            return matches!(comparison, Comparison::Equal);
        }
        return matches!(comparison, Comparison::NotEqual);
    };
    if matches!(expected, Scalar::Null) {
        return matches!(comparison, Comparison::NotEqual) && !actual.is_null()
            || matches!(comparison, Comparison::Equal) && actual.is_null();
    }
    match comparison {
        Comparison::Equal => scalar_equals(actual, expected),
        Comparison::NotEqual => !scalar_equals(actual, expected),
        Comparison::Greater
        | Comparison::GreaterEqual
        | Comparison::Less
        | Comparison::LessEqual => {
            let Some(actual) = actual.as_f64() else {
                return false;
            };
            let Scalar::Number(expected) = expected else {
                return false;
            };
            match comparison {
                Comparison::Greater => actual > *expected,
                Comparison::GreaterEqual => actual >= *expected,
                Comparison::Less => actual < *expected,
                Comparison::LessEqual => actual <= *expected,
                _ => false,
            }
        }
    }
}

fn compare_scalars(actual: &Scalar, comparison: Comparison, expected: &Scalar) -> bool {
    match comparison {
        Comparison::Equal => actual == expected,
        Comparison::NotEqual => actual != expected,
        Comparison::Greater
        | Comparison::GreaterEqual
        | Comparison::Less
        | Comparison::LessEqual => {
            let (Scalar::Number(actual), Scalar::Number(expected)) = (actual, expected) else {
                return false;
            };
            match comparison {
                Comparison::Greater => actual > expected,
                Comparison::GreaterEqual => actual >= expected,
                Comparison::Less => actual < expected,
                Comparison::LessEqual => actual <= expected,
                _ => false,
            }
        }
    }
}

fn scalar_equals(actual: &Value, expected: &Scalar) -> bool {
    match expected {
        Scalar::Null => actual.is_null(),
        Scalar::Bool(expected) => actual.as_bool() == Some(*expected),
        Scalar::Number(expected) => actual.as_f64() == Some(*expected),
        Scalar::String(expected) => actual.as_str() == Some(expected),
    }
}

pub(super) fn parse_filter(value: &Value) -> Result<Filter> {
    match value {
        Value::Null => Ok(Filter::MatchAll),
        Value::String(expression) => Ok(Filter::Expression(parse_expression(expression)?)),
        Value::Sequence(filters) if filters.is_empty() => Ok(Filter::MatchAll),
        Value::Mapping(map) if map.len() == 1 => {
            let Some((key, value)) = map.iter().next() else {
                bail!("filter operator must be a string");
            };
            let key = key
                .as_str()
                .ok_or_else(|| anyhow!("filter operator must be a string"))?;
            match key {
                "and" | "or" => {
                    let values = value
                        .as_sequence()
                        .ok_or_else(|| anyhow!("{key} must contain a list"))?;
                    let filters = values
                        .iter()
                        .map(parse_filter)
                        .collect::<Result<Vec<_>>>()?;
                    if key == "and" {
                        Ok(Filter::And(filters))
                    } else {
                        Ok(Filter::Or(filters))
                    }
                }
                "not" => Ok(Filter::Not(Box::new(parse_filter(value)?))),
                _ => bail!("unknown filter operator {key:?}"),
            }
        }
        _ => bail!("filter must be an expression or an and/or/not object"),
    }
}

fn parse_expression(source: &str) -> Result<Expression> {
    let source = source.trim();
    for (function, operation) in [("file.hasTag(", true), ("file.hasLink(", false)] {
        if let Some(argument) = source
            .strip_prefix(function)
            .and_then(|value| value.strip_suffix(')'))
        {
            let argument = parse_string(argument.trim())?;
            return Ok(Expression {
                left: if operation {
                    PropertyPath::Note(vec!["tags".to_string()])
                } else {
                    PropertyPath::File(FileField::Links)
                },
                operation: if operation {
                    Operation::HasTag(argument)
                } else {
                    Operation::HasLink(argument)
                },
            });
        }
    }
    if let Some(argument) = source
        .strip_prefix("file.inFolder(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return Ok(Expression {
            left: PropertyPath::File(FileField::Folder),
            operation: Operation::InFolder(parse_string(argument.trim())?),
        });
    }
    if let Some(prefix) = source.strip_suffix(')')
        && let Some((property, argument)) = prefix.split_once(".contains(")
    {
        return Ok(Expression {
            left: parse_property(property.trim())?,
            operation: Operation::Contains(parse_scalar(argument.trim())?),
        });
    }
    for (symbol, comparison) in [
        (">=", Comparison::GreaterEqual),
        ("<=", Comparison::LessEqual),
        ("!=", Comparison::NotEqual),
        ("==", Comparison::Equal),
        (">", Comparison::Greater),
        ("<", Comparison::Less),
    ] {
        if let Some((left, right)) = source.split_once(symbol) {
            let left = parse_property(left.trim())?;
            let expected = parse_scalar(right.trim())?;
            return Ok(Expression {
                left,
                operation: Operation::Compare(comparison, expected),
            });
        }
    }
    bail!("unsupported filter expression {source:?}")
}

pub fn parse_property(source: &str) -> Result<PropertyPath> {
    if let Some(field) = source.strip_prefix("file.") {
        return Ok(PropertyPath::File(match field {
            "name" => FileField::Name,
            "ext" => FileField::Ext,
            "path" => FileField::Path,
            "folder" => FileField::Folder,
            "size" => FileField::Size,
            "mtime" => FileField::Mtime,
            "links" => FileField::Links,
            _ => bail!("unknown file property {field:?}"),
        }));
    }
    let source = source.strip_prefix("note.").unwrap_or(source);
    let mut parts = Vec::new();
    let mut rest = source;
    while !rest.is_empty() {
        if let Some(bracket) = rest
            .strip_prefix("note[")
            .or_else(|| rest.strip_prefix('['))
        {
            let Some((content, after)) = bracket.split_once(']') else {
                bail!("unterminated property bracket");
            };
            parts.push(parse_string(content)?);
            rest = after.trim_start_matches('.');
        } else {
            let end = rest.find(['.', '[']).unwrap_or(rest.len());
            let Some(part) = rest.get(..end) else {
                bail!("invalid property name");
            };
            if part.is_empty() {
                bail!("empty property name");
            }
            parts.push(part.to_string());
            let Some(after) = rest.get(end..) else {
                bail!("invalid property name");
            };
            rest = after.trim_start_matches('.');
        }
    }
    if parts.is_empty() {
        bail!("property name must not be empty");
    }
    Ok(PropertyPath::Note(parts))
}

fn parse_scalar(source: &str) -> Result<Scalar> {
    match source {
        "null" => Ok(Scalar::Null),
        "true" => Ok(Scalar::Bool(true)),
        "false" => Ok(Scalar::Bool(false)),
        _ if source.starts_with(['\'', '"']) => Ok(Scalar::String(parse_string(source)?)),
        _ => source
            .parse::<f64>()
            .map(Scalar::Number)
            .map_err(|_| anyhow!("expected a string, number, boolean, or null, got {source:?}")),
    }
}

fn parse_string(source: &str) -> Result<String> {
    let bytes = source.as_bytes();
    if bytes.len() < 2 {
        bail!("expected quoted string");
    }
    let first = bytes.first().copied();
    let last = bytes.last().copied();
    if !matches!(first, Some(b'\'' | b'"')) || first != last {
        bail!("expected quoted string");
    }
    let content = source
        .get(1..bytes.len().saturating_sub(1))
        .ok_or_else(|| anyhow!("expected quoted string"))?;
    Ok(content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn supports_missing_values_nested_and_bracket_properties() {
        let properties: Value =
            yaml_serde::from_str("project:\n  owner:\n    name: Romain\nproject status: done")
                .unwrap();
        for source in [
            "note.project.owner.name == \"Romain\"",
            "note[\"project status\"] == \"done\"",
            "missing != \"done\"",
            "missing == null",
        ] {
            let filter = parse_filter(&Value::String(source.into())).unwrap();
            assert!(
                filter.matches(&DocumentFile {
                    path: Path::new("Note.md"),
                    properties: &properties,
                    size_bytes: None,
                    modified_ns: None,
                    links: &[],
                }),
                "{source}"
            );
        }
    }

    #[test]
    fn matches_catalog_backed_tag_and_link_predicates() {
        let properties: Value = yaml_serde::from_str("tags: [reading, rust]").unwrap();
        let links = ["Index.md".to_string()];
        for source in ["file.hasTag(\"reading\")", "file.hasLink(\"Index.md\")"] {
            let filter = parse_filter(&Value::String(source.into())).unwrap();
            assert!(
                filter.matches(&DocumentFile {
                    path: Path::new("Note.md"),
                    properties: &properties,
                    size_bytes: Some(128),
                    modified_ns: Some(256),
                    links: &links,
                }),
                "{source}"
            );
        }
    }
}
