use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use yaml_serde::Value;

use crate::document::expr::{Expr, PropertyRef};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Filter {
    #[default]
    MatchAll,
    Expression(Expression),
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
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
    pub(crate) source: String,
    pub(crate) expr: Expr,
}

impl Expression {
    pub(crate) fn parse(source: &str) -> Result<Self> {
        let expr = Expr::parse(source)?;
        Ok(Self {
            source: source.to_string(),
            expr,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyPath {
    Note(Vec<String>),
    File(FileField),
    Formula(String),
}

pub use crate::document::expr::FileField;

pub fn parse_property(source: &str) -> Result<PropertyPath> {
    match crate::document::expr::parse_property_ref(source)? {
        PropertyRef::Note(parts) => Ok(PropertyPath::Note(parts)),
        PropertyRef::File(field) => Ok(PropertyPath::File(field)),
        PropertyRef::Formula(name) => Ok(PropertyPath::Formula(name)),
    }
}

fn parse_filter(value: &Value) -> Result<Filter> {
    match value {
        Value::Null => Ok(Filter::MatchAll),
        Value::String(expression) => Ok(Filter::Expression(Expression::parse(expression)?)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_expression_strings_and_boolean_combinators() {
        for source in ["status == 'done'", "a > 1 && b < 2", "price / count >= 2"] {
            let value = Value::String(source.into());
            assert!(parse_filter(&value).is_ok(), "{source}");
        }
        let nested: Value =
            yaml_serde::from_str("{or: [{not: \"a == b\"}, {and: [\"c\", \"d\"]}]}").unwrap();
        assert!(parse_filter(&nested).is_ok());
    }

    #[test]
    fn empty_filters_and_null_match_everything() {
        assert!(matches!(parse_filter(&Value::Null), Ok(Filter::MatchAll)));
        let empty: Value = yaml_serde::from_str("[]").unwrap();
        assert!(matches!(parse_filter(&empty), Ok(Filter::MatchAll)));
    }

    #[test]
    fn rejects_unknown_operators_and_malformed_shapes() {
        let bad: Value = yaml_serde::from_str("{bogus: []}").unwrap();
        assert!(parse_filter(&bad).is_err());
        let multi: Value = yaml_serde::from_str("{and: [], or: []}").unwrap();
        assert!(parse_filter(&multi).is_err());
        let scalar: Value = yaml_serde::from_str("42").unwrap();
        assert!(parse_filter(&scalar).is_err());
    }

    #[test]
    fn parses_formula_and_new_file_property_references() {
        assert_eq!(
            parse_property("formula.ppu").unwrap(),
            PropertyPath::Formula("ppu".into())
        );
        assert_eq!(
            parse_property("file.ctime").unwrap(),
            PropertyPath::File(FileField::Ctime)
        );
        assert_eq!(
            parse_property("file.backlinks").unwrap(),
            PropertyPath::File(FileField::Backlinks)
        );
    }
}
