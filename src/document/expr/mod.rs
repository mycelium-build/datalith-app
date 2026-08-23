use std::fmt;

use anyhow::{Result, bail};

mod lexer;
mod parser;
mod validate;

use parser::Parser;
pub use parser::parse_property_ref;
use validate::Validator;

/// A value reference usable inside Base expressions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyRef {
    /// A frontmatter property addressed by its YAML path segments.
    Note(Vec<String>),
    /// A catalog-backed file property.
    File(FileField),
    /// A named formula declared by the enclosing Base definition.
    Formula(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileField {
    Name,
    Ext,
    Path,
    Folder,
    Size,
    Mtime,
    Ctime,
    Links,
    Tags,
    Embeds,
    Backlinks,
    Properties,
}

/// A duration literal such as `"1M"` or `"2h"` used in date arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duration {
    pub amount: i64,
    pub unit: DurationUnit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
}

impl DurationUnit {
    fn parse(symbol: &str) -> Option<Self> {
        match symbol {
            "y" | "year" | "years" => Some(Self::Year),
            "M" | "month" | "months" => Some(Self::Month),
            "w" | "week" | "weeks" => Some(Self::Week),
            "d" | "day" | "days" => Some(Self::Day),
            "h" | "hour" | "hours" => Some(Self::Hour),
            "m" | "minute" | "minutes" => Some(Self::Minute),
            "s" | "second" | "seconds" => Some(Self::Second),
            _ => None,
        }
    }

    /// The Turso date-function modifier suffix for this unit.
    #[must_use]
    pub const fn modifier(self) -> &'static str {
        match self {
            Self::Year => "years",
            Self::Month => "months",
            Self::Week | Self::Day => "days",
            Self::Hour => "hours",
            Self::Minute => "minutes",
            Self::Second => "seconds",
        }
    }

    /// Multiplier converting this unit into days; weeks expand at compile time.
    #[must_use]
    pub const fn day_scale(self) -> i64 {
        match self {
            Self::Week => 7,
            _ => 1,
        }
    }
}

/// Parses a duration literal body such as `1M` or `2 hours`.
pub fn parse_duration(body: &str) -> Option<Duration> {
    let digits_end = body.find(|c: char| !c.is_ascii_digit())?;
    if digits_end == 0 {
        return None;
    }
    let amount = body.get(..digits_end)?.parse::<i64>().ok()?;
    let unit = DurationUnit::parse(body.get(digits_end..)?.trim())?;
    Some(Duration { amount, unit })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicOp {
    And,
    Or,
}

/// A parsed Base expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Property(PropertyRef),
    Not(Box<Self>),
    Neg(Box<Self>),
    Arithmetic {
        op: ArithOp,
        left: Box<Self>,
        right: Box<Self>,
    },
    Compare {
        op: CmpOp,
        left: Box<Self>,
        right: Box<Self>,
    },
    Logic {
        op: LogicOp,
        left: Box<Self>,
        right: Box<Self>,
    },
    Call(String, Vec<Self>),
    Method {
        subject: Box<Self>,
        name: String,
        args: Vec<Self>,
    },
}

/// The static value types the expression language reasons about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValType {
    Number,
    Text,
    Boolean,
    Date,
    List,
    Object,
    Any,
}

impl Expr {
    /// Parses an expression source string into a validated AST.
    ///
    /// # Errors
    /// Returns an error describing the first lexical, syntactic, or semantic problem found in the source.
    pub fn parse(source: &str) -> Result<Self> {
        let tokens = lexer::lex(source)?;
        let mut parser = Parser::new(tokens);
        let parsed = parser.parse_expression()?;
        if !parser.at_end() {
            bail!("unexpected trailing input in expression {source:?}");
        }
        let mut validator = Validator;
        validator.walk(&parsed)?;
        Ok(parsed)
    }

    /// Infers the static value type produced by this expression.
    #[must_use]
    pub fn infer(&self) -> ValType {
        match self {
            Self::Number(_) => ValType::Number,
            Self::Text(_) => ValType::Text,
            Self::Bool(_) | Self::Not(_) | Self::Compare { .. } | Self::Logic { .. } => {
                ValType::Boolean
            }
            Self::Null => ValType::Any,
            Self::Property(reference) => match reference {
                PropertyRef::Note(_) | PropertyRef::Formula(_) => ValType::Any,
                PropertyRef::File(field) => match field {
                    FileField::Size => ValType::Number,
                    FileField::Mtime | FileField::Ctime => ValType::Date,
                    FileField::Name | FileField::Ext | FileField::Path | FileField::Folder => {
                        ValType::Text
                    }
                    FileField::Links
                    | FileField::Tags
                    | FileField::Embeds
                    | FileField::Backlinks => ValType::List,
                    FileField::Properties => ValType::Object,
                },
            },
            Self::Neg(inner) => inner.infer(),
            Self::Arithmetic { op, left, right } => {
                let left_type = left.infer();
                let right_type = right.infer();
                if (*op == ArithOp::Add || *op == ArithOp::Subtract)
                    && (left_type == ValType::Date || right_type == ValType::Date)
                {
                    // date ± duration stays a date; date - date yields milliseconds.
                    if *op == ArithOp::Subtract
                        && left_type == ValType::Date
                        && right_type == ValType::Date
                    {
                        return ValType::Number;
                    }
                    return ValType::Date;
                }
                ValType::Number
            }
            Self::Call(name, _) => infer_call(name),
            Self::Method { subject, name, .. } => infer_method(subject.infer(), name),
        }
    }
}

fn infer_call(name: &str) -> ValType {
    match name {
        "today" | "now" | "date" | "datetime" => ValType::Date,
        "julianday" | "abs" | "round" => ValType::Number,
        "length" | "lower" | "upper" | "trim" | "ltrim" | "rtrim" | "replace" | "substr"
        | "instr" | "time" => ValType::Text,
        _ => ValType::Any,
    }
}

// Matching on `str` is not yet allowed in const fn on stable Rust.
#[allow(clippy::missing_const_for_fn)]
fn infer_method(subject: ValType, name: &str) -> ValType {
    let _ = subject;
    match name {
        "contains" | "startsWith" | "endsWith" => ValType::Boolean,
        "toFixed" | "round" => ValType::Number,
        "date" => ValType::Date,
        "format" | "lower" | "upper" | "trim" => ValType::Text,
        _ => ValType::Any,
    }
}

pub fn coerce_duration(expression: &Expr) -> Option<Duration> {
    match expression {
        Expr::Text(body) => parse_duration(body),
        _ => None,
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.amount, modifier_symbol(self.unit))
    }
}

const fn modifier_symbol(unit: DurationUnit) -> &'static str {
    match unit {
        DurationUnit::Year => "y",
        DurationUnit::Month => "M",
        DurationUnit::Week => "w",
        DurationUnit::Day => "d",
        DurationUnit::Hour => "h",
        DurationUnit::Minute => "m",
        DurationUnit::Second => "s",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(source: &str) -> Expr {
        Expr::parse(source).unwrap_or_else(|error| panic!("test setup: {error}"))
    }

    fn parse_err(source: &str) -> String {
        Expr::parse(source)
            .map_err(|error| error.to_string())
            .expect_err("test setup: expected parse failure")
    }

    #[test]
    fn parses_arithmetic_precedence_and_parentheses() {
        let expression = parse_ok("radius * (2 + 3.5)");
        assert_eq!(
            expression,
            Expr::Arithmetic {
                op: ArithOp::Multiply,
                left: Box::new(Expr::Property(PropertyRef::Note(vec!["radius".into()]))),
                right: Box::new(Expr::Arithmetic {
                    op: ArithOp::Add,
                    left: Box::new(Expr::Number(2.0)),
                    right: Box::new(Expr::Number(3.5)),
                }),
            }
        );
    }

    #[test]
    fn parses_inline_boolean_operators_and_comparisons() {
        let expression = parse_ok("!done && price > 5 || status == \"todo\"");
        assert!(matches!(expression, Expr::Logic { .. }));
        assert_eq!(parse_ok("a != null").infer(), ValType::Boolean);
    }

    #[test]
    fn parses_property_chains_brackets_and_formulas() {
        assert_eq!(
            parse_ok("note.project.owner.name"),
            Expr::Property(PropertyRef::Note(vec![
                "project".into(),
                "owner".into(),
                "name".into()
            ]))
        );
        assert_eq!(
            parse_ok("note[\"project status\"]"),
            Expr::Property(PropertyRef::Note(vec!["project status".into()]))
        );
        assert_eq!(
            parse_ok("formula.ppu"),
            Expr::Property(PropertyRef::Formula("ppu".into()))
        );
        assert_eq!(
            parse_ok("author"),
            Expr::Property(PropertyRef::Note(vec!["author".into()]))
        );
    }

    #[test]
    fn parses_file_predicates_and_methods() {
        assert_eq!(
            parse_ok("file.hasTag(\"reading\")"),
            Expr::Call("file.hasTag".into(), vec![Expr::Text("reading".into())])
        );
        assert!(matches!(parse_ok("price.toFixed(2)"), Expr::Method { .. }));
        assert!(matches!(
            parse_ok("status.contains(\"done\")"),
            Expr::Method { .. }
        ));
    }

    #[test]
    fn accepts_date_arithmetic_with_duration_literals() {
        let expression = parse_ok("file.mtime > now() - \"1 week\"");
        assert_eq!(expression.infer(), ValType::Boolean);
        parse_ok("date(\"2024-12-01\") + \"1M\" + \"4h\"");
        parse_ok("file.ctime + \"3 days\"");
    }

    #[test]
    fn rejects_misplaced_durations_and_unknown_functions() {
        // A duration-shaped string is plain text outside date arithmetic.
        assert!(parse_ok("\"1M\"").infer() == ValType::Text);
        let message = parse_err("price + \"1M\"");
        assert!(message.contains("duration"), "{message}");
        let message = parse_err("price.toFixed(price)");
        assert!(message.contains("numeric literal"), "{message}");
        let message = parse_err("unknownFn(a)");
        assert!(message.contains("unsupported function"), "{message}");
        let message = parse_err("file.bogus");
        assert!(message.contains("unknown file property"), "{message}");
        let message = parse_err("a === b");
        assert!(message.to_lowercase().contains("expected"), "{message}");
    }

    #[test]
    fn infers_static_types_for_dates_numbers_and_text() {
        assert_eq!(parse_ok("file.mtime").infer(), ValType::Date);
        assert_eq!(parse_ok("file.size").infer(), ValType::Number);
        assert_eq!(parse_ok("file.tags").infer(), ValType::List);
        assert_eq!(parse_ok("today()").infer(), ValType::Date);
        assert_eq!(
            parse_ok("(price / age).toFixed(2)").infer(),
            ValType::Number
        );
        assert_eq!(
            parse_ok("now() - file.ctime").infer(),
            ValType::Number,
            "date subtraction yields milliseconds"
        );
    }

    #[test]
    fn parses_standalone_property_references() {
        assert_eq!(
            parse_property_ref("file.mtime").unwrap(),
            PropertyRef::File(FileField::Mtime)
        );
        assert_eq!(
            parse_property_ref("note.a.b").unwrap(),
            PropertyRef::Note(vec!["a".into(), "b".into()])
        );
        assert!(parse_property_ref("price + 1").is_err());
    }

    #[test]
    fn parses_duration_bodies() {
        assert_eq!(
            parse_duration("1M"),
            Some(Duration {
                amount: 1,
                unit: DurationUnit::Month
            })
        );
        assert_eq!(
            parse_duration("2 hours"),
            Some(Duration {
                amount: 2,
                unit: DurationUnit::Hour
            })
        );
        assert_eq!(parse_duration("M"), None);
        assert_eq!(parse_duration("1x"), None);
    }
}
