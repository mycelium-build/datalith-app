//! Compiles Bases expressions into Turso SQL with numbered placeholders.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use turso::Value;

use crate::document::expr::{
    self, ArithOp, CmpOp, DurationUnit, Expr, FileField, LogicOp, PropertyRef, ValType,
};
use crate::document::filter::Filter;

use super::super::{FILE_NAME_SQL, escape_like_pattern};

const FORMULA_SUBSTITUTION_DEPTH: u32 = 32;

/// SQL binding strength of a fragment's top-level operator,
/// following `SQLite` operator precedence.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Or = 1,
    And = 2,
    Not = 3,
    Comparison = 4,
    Additive = 5,
    Multiplicative = 6,
    Unary = 7,
    Atom = 9,
}

/// A compiled SQL fragment plus the precedence of its top-level operator,
/// so parents parenthesize children only when binding requires it.
struct Sql {
    text: String,
    precedence: Precedence,
}

impl Sql {
    fn atom(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            precedence: Precedence::Atom,
        }
    }

    /// Renders as a left operand binding at least `min`.
    fn emit(&self, min: Precedence) -> String {
        if self.precedence < min {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }

    /// Renders as a right operand: left-associative chains must stay grouped.
    fn emit_strict(&self, min: Precedence) -> String {
        if self.precedence <= min {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }
}

/// Folds fragments into a left-associative chain of `operator`.
fn fold_binary(parts: Vec<Sql>, operator: &str, precedence: Precedence) -> Option<Sql> {
    let mut parts = parts.into_iter();
    let first = parts.next()?;
    let mut text = first.emit(precedence);
    for part in parts {
        text = format!("{text} {operator} {}", part.emit_strict(precedence));
    }
    Some(Sql { text, precedence })
}

/// Coerces a fragment to the filter truth domain:
/// NULL (a missing property) counts as false even under NOT.
fn truthy(fragment: &Sql) -> Sql {
    Sql {
        text: format!("COALESCE({}, 0) <> 0", fragment.emit(Precedence::Or)),
        precedence: Precedence::Comparison,
    }
}

pub(super) struct BaseQueryCompiler {
    pub(super) parameters: Vec<Value>,
    number_offset: usize,
}

impl BaseQueryCompiler {
    pub(super) const fn new(number_offset: usize) -> Self {
        Self {
            parameters: Vec::new(),
            number_offset,
        }
    }

    /// Binds a value and returns its numbered placeholder (`?N`),
    /// so compiled fragments can be recombined in any statement order.
    fn bind(&mut self, value: Value) -> String {
        self.parameters.push(value);
        let index = self.number_offset.saturating_add(self.parameters.len());
        format!("?{index}")
    }

    pub(super) fn compile_filter(&mut self, filter: &Filter) -> Result<String> {
        Ok(self.compile_filter_fragment(filter)?.text)
    }

    fn compile_filter_fragment(&mut self, filter: &Filter) -> Result<Sql> {
        match filter {
            Filter::MatchAll => Ok(Sql::atom("1")),
            Filter::And(filters) => {
                let parts = filters
                    .iter()
                    // A match-all conjunct is a no-op.
                    .filter(|filter| !matches!(filter, Filter::MatchAll))
                    .map(|filter| self.compile_filter_fragment(filter))
                    .collect::<Result<Vec<_>>>()?;
                Ok(fold_binary(parts, "AND", Precedence::And).unwrap_or_else(|| Sql::atom("1")))
            }
            Filter::Or(filters) => {
                // One true disjunct makes the whole OR true.
                if filters
                    .iter()
                    .any(|filter| matches!(filter, Filter::MatchAll))
                {
                    return Ok(Sql::atom("1"));
                }
                let parts = filters
                    .iter()
                    .map(|filter| self.compile_filter_fragment(filter))
                    .collect::<Result<Vec<_>>>()?;
                Ok(fold_binary(parts, "OR", Precedence::Or).unwrap_or_else(|| Sql::atom("0")))
            }
            Filter::Not(filter) => {
                let inner = truthy(&self.compile_filter_fragment(filter)?);
                Ok(Sql {
                    text: format!("NOT {}", inner.emit(Precedence::Comparison)),
                    precedence: Precedence::Not,
                })
            }
            Filter::Expression(expression) => Ok(truthy(&self.compile_expr(&expression.expr)?)),
        }
    }

    fn compile_expr(&mut self, expression: &Expr) -> Result<Sql> {
        match expression {
            Expr::Null => Ok(Sql::atom("NULL")),
            Expr::Bool(value) => Ok(Sql::atom(if *value { "1" } else { "0" })),
            Expr::Number(value) => Ok(Sql::atom(self.bind(Value::Real(*value)))),
            Expr::Text(text) => Ok(Sql::atom(self.bind(Value::Text(text.clone())))),
            // Duration literals are consumed by date arithmetic at the
            // Arithmetic node below and never reach this arm in valid trees. => bail!("misplaced duration literal"),
            Expr::Property(reference) => Ok(Sql::atom(self.compile_property(reference)?)),
            Expr::Not(inner) => {
                let operand = self.compile_expr(inner)?;
                Ok(Sql {
                    text: format!("NOT {}", operand.emit(Precedence::Comparison)),
                    precedence: Precedence::Not,
                })
            }
            Expr::Neg(inner) => {
                let operand = self.compile_expr(inner)?;
                // Unary minus binds tightest; nested unary needs explicit grouping.
                let text = if operand.precedence >= Precedence::Unary {
                    format!("-({})", operand.text)
                } else {
                    format!("-{}", operand.text)
                };
                Ok(Sql {
                    text,
                    precedence: Precedence::Unary,
                })
            }
            Expr::Arithmetic { op, left, right } => self.compile_arithmetic(*op, left, right),
            Expr::Compare { op, left, right } => self.compile_compare(*op, left, right),
            Expr::Logic { op, left, right } => {
                let operator = match op {
                    LogicOp::And => "AND",
                    LogicOp::Or => "OR",
                };
                let precedence = match op {
                    LogicOp::And => Precedence::And,
                    LogicOp::Or => Precedence::Or,
                };
                let left_sql = truthy(&self.compile_expr(left)?);
                let right_sql = truthy(&self.compile_expr(right)?);
                Ok(fold_binary(vec![left_sql, right_sql], operator, precedence)
                    .unwrap_or_else(|| Sql::atom("1")))
            }
            Expr::Call(name, args) => self.compile_call(name, args),
            Expr::Method {
                subject,
                name,
                args,
            } => self.compile_method(subject, name, args),
        }
    }

    fn compile_arithmetic(&mut self, op: ArithOp, left: &Expr, right: &Expr) -> Result<Sql> {
        let left_type = left.infer();
        let right_type = right.infer();
        if matches!(op, ArithOp::Add | ArithOp::Subtract)
            && (left_type == ValType::Date || right_type == ValType::Date)
        {
            // Duration literals are text at this point;
            // convert them into a Turso datetime modifier bound as a parameter.
            if let Some(duration) = expr::coerce_duration(right)
                && left_type == ValType::Date
            {
                let modifier = duration_modifier(duration, op == ArithOp::Subtract)?;
                let left_sql = self.compile_expr(left)?;
                let placeholder = self.bind(Value::Text(modifier));
                return Ok(Sql::atom(format!(
                    "datetime({}, {placeholder})",
                    left_sql.emit(Precedence::Or)
                )));
            }
            if op == ArithOp::Add
                && let Some(duration) = expr::coerce_duration(left)
                && right_type == ValType::Date
            {
                let modifier = duration_modifier(duration, false)?;
                let right_sql = self.compile_expr(right)?;
                let placeholder = self.bind(Value::Text(modifier));
                return Ok(Sql::atom(format!(
                    "datetime({}, {placeholder})",
                    right_sql.emit(Precedence::Or)
                )));
            }
            if op == ArithOp::Subtract && left_type == ValType::Date && right_type == ValType::Date
            {
                let left_sql = self.compile_expr(left)?;
                let right_sql = self.compile_expr(right)?;
                let difference = Sql {
                    text: format!(
                        "julianday({}) - julianday({})",
                        left_sql.emit(Precedence::Or),
                        right_sql.emit(Precedence::Or)
                    ),
                    precedence: Precedence::Additive,
                };
                return Ok(Sql {
                    text: format!(
                        "{} * 86400000.0",
                        difference.emit(Precedence::Multiplicative)
                    ),
                    precedence: Precedence::Multiplicative,
                });
            }
        }
        let left_sql = self.compile_expr(left)?;
        let right_sql = self.compile_expr(right)?;
        // SQLite divides integers with truncation,
        // but the documented expression semantics are real-valued.
        if op == ArithOp::Divide {
            return Ok(Sql {
                text: format!(
                    "CAST({} AS REAL) / CAST({} AS REAL)",
                    left_sql.emit(Precedence::Or),
                    right_sql.emit(Precedence::Or)
                ),
                precedence: Precedence::Multiplicative,
            });
        }
        // Divide already returned above, so its arm here is a formality.
        let (operator, precedence) = match op {
            ArithOp::Add => ("+", Precedence::Additive),
            ArithOp::Subtract => ("-", Precedence::Additive),
            ArithOp::Multiply | ArithOp::Divide => ("*", Precedence::Multiplicative),
            ArithOp::Modulo => ("%", Precedence::Multiplicative),
        };
        Ok(fold_binary(vec![left_sql, right_sql], operator, precedence)
            .unwrap_or_else(|| Sql::atom("0")))
    }

    fn compile_compare(&mut self, op: CmpOp, left: &Expr, right: &Expr) -> Result<Sql> {
        // Missing properties are SQL NULL;
        // comparisons against the null literal follow the documented missing-value semantics.
        if matches!(right, Expr::Null) {
            let left_sql = self.compile_expr(left)?;
            let suffix = match op {
                CmpOp::Equal => "IS NULL",
                CmpOp::NotEqual => "IS NOT NULL",
                _ => bail!("null only supports equality comparisons"),
            };
            return Ok(Sql {
                text: format!("{} {suffix}", left_sql.emit(Precedence::Comparison)),
                precedence: Precedence::Comparison,
            });
        }
        if matches!(left, Expr::Null) {
            let swapped = match op {
                CmpOp::Greater => CmpOp::Less,
                CmpOp::Less => CmpOp::Greater,
                other => other,
            };
            return self.compile_compare(swapped, right, left);
        }
        let operator = match op {
            CmpOp::Equal => "=",
            CmpOp::NotEqual => "!=",
            CmpOp::Greater => ">",
            CmpOp::GreaterEqual => ">=",
            CmpOp::Less => "<",
            CmpOp::LessEqual => "<=",
        };
        let left_sql = self.compile_expr(left)?;
        let right_sql = self.compile_expr(right)?;
        Ok(
            fold_binary(vec![left_sql, right_sql], operator, Precedence::Comparison)
                .unwrap_or_else(|| Sql::atom("0")),
        )
    }

    fn compile_call(&mut self, name: &str, args: &[Expr]) -> Result<Sql> {
        match name {
            "file.hasTag" => {
                let [Expr::Text(tag)] = args else {
                    bail!("file.hasTag takes one string argument")
                };
                let contains = Expr::Method {
                    subject: Box::new(Expr::Property(PropertyRef::Note(vec!["tags".to_string()]))),
                    name: "contains".to_string(),
                    args: vec![Expr::Text(tag.clone())],
                };
                self.compile_expr(&contains)
            }
            "file.hasLink" => {
                let [Expr::Text(link)] = args else {
                    bail!("file.hasLink takes one string argument")
                };
                let target = self.bind(Value::Text(link.clone()));
                let target_path = self.bind(Value::Text(link.clone()));
                Ok(Sql::atom(format!(
                    "EXISTS (SELECT 1 FROM wiki_links WHERE source_path = documents.path \
                     AND target_path IS NOT NULL AND (target = {target} OR target_path = {target_path}))"
                )))
            }
            "file.inFolder" => {
                let [Expr::Text(folder)] = args else {
                    bail!("file.inFolder takes one string argument")
                };
                let exact = self.bind(Value::Text(folder.clone()));
                let prefix = self.bind(Value::Text(format!("{}/%", escape_like_pattern(folder))));
                Ok(Sql {
                    text: format!("folder = {exact} OR folder LIKE {prefix} ESCAPE '\\'"),
                    precedence: Precedence::Or,
                })
            }
            "today" | "now" => {
                // Frozen per refresh: the executor overwrites this payload once.
                Ok(Sql::atom(self.bind(Value::Text(String::new()))))
            }

            "if" => {
                let [condition, then, otherwise] = args else {
                    bail!("if takes three arguments")
                };
                let condition_sql = truthy(&self.compile_expr(condition)?);
                let then_sql = self.compile_expr(then)?;
                let otherwise_sql = self.compile_expr(otherwise)?;
                Ok(Sql::atom(format!(
                    "CASE WHEN {} THEN {} ELSE {} END",
                    condition_sql.emit(Precedence::Or),
                    then_sql.emit(Precedence::Or),
                    otherwise_sql.emit(Precedence::Or)
                )))
            }
            "min" | "max" => {
                let sqls = args
                    .iter()
                    .map(|arg| Ok(self.compile_expr(arg)?.emit(Precedence::Or)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Sql::atom(format!("{name}({})", sqls.join(", "))))
            }
            "date" | "datetime" | "time" | "julianday" | "abs" | "round" | "length" | "lower"
            | "upper" | "trim" | "ltrim" | "rtrim" | "instr" => {
                let [inner] = args else {
                    bail!("{name} takes one argument")
                };
                let inner_sql = self.compile_expr(inner)?;
                Ok(Sql::atom(format!(
                    "{name}({})",
                    inner_sql.emit(Precedence::Or)
                )))
            }
            "replace" | "substr" => {
                let sqls = args
                    .iter()
                    .map(|arg| Ok(self.compile_expr(arg)?.emit(Precedence::Or)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Sql::atom(format!("{name}({})", sqls.join(", "))))
            }
            _ => bail!("unsupported function {name:?}"),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn compile_method(&mut self, subject: &Expr, name: &str, args: &[Expr]) -> Result<Sql> {
        match name {
            "contains" => {
                // Lists check element membership; strings check substrings.
                let subject_sql = self.compile_expr(subject)?;
                let Some(arg) = args.first() else {
                    bail!(".contains takes one argument")
                };
                let arg_sql = self.compile_expr(arg)?;
                let subject = subject_sql.emit(Precedence::Or);
                let arg = arg_sql.emit(Precedence::Comparison);
                Ok(Sql::atom(format!(
                    "CASE WHEN json_type({subject}) = 'array' \
                     THEN EXISTS (SELECT 1 FROM json_each({subject}) AS each \
                     WHERE {arg} = each.atom) \
                     ELSE instr(COALESCE({subject}, ''), {arg}) > 0 END"
                )))
            }
            "toFixed" => {
                let subject_sql = self.compile_expr(subject)?;
                let digits = integer_argument(args.first(), ".toFixed")?;
                self.parameters.push(Value::Text(format!("%.{digits}f")));
                Ok(Sql::atom(format!(
                    "format(?, {})",
                    subject_sql.emit(Precedence::Or)
                )))
            }
            "round" => {
                let subject_sql = self.compile_expr(subject)?;
                let digits = integer_argument(args.first(), ".round")?;
                let placeholder = self.bind(Value::Integer(i64::from(digits)));
                Ok(Sql::atom(format!(
                    "round({}, {placeholder})",
                    subject_sql.emit(Precedence::Or)
                )))
            }
            "startsWith" | "endsWith" => {
                let subject_sql = self.compile_expr(subject)?;
                let Some(Expr::Text(pattern)) = args.first() else {
                    bail!(".{name} requires a string literal")
                };
                let glob = match name {
                    "startsWith" => format!("{}*", escape_glob(pattern)),
                    _ => format!("*{}", escape_glob(pattern)),
                };
                let placeholder = self.bind(Value::Text(glob));
                Ok(Sql {
                    text: format!(
                        "{} GLOB {placeholder}",
                        subject_sql.emit(Precedence::Comparison)
                    ),
                    precedence: Precedence::Comparison,
                })
            }
            "lower" | "upper" | "trim" => {
                let subject_sql = self.compile_expr(subject)?;
                Ok(Sql::atom(format!(
                    "{name}({})",
                    subject_sql.emit(Precedence::Or)
                )))
            }
            "date" => {
                let subject_sql = self.compile_expr(subject)?;
                Ok(Sql::atom(format!(
                    "date({})",
                    subject_sql.emit(Precedence::Or)
                )))
            }
            "format" => {
                let Some(Expr::Text(pattern)) = args.first() else {
                    bail!(".format requires a string literal")
                };
                let strftime_pattern = moment_to_strftime(pattern);
                let subject_sql = self.compile_expr(subject)?;
                let placeholder = self.bind(Value::Text(strftime_pattern));
                Ok(Sql::atom(format!(
                    "strftime({placeholder}, {})",
                    subject_sql.emit(Precedence::Or)
                )))
            }
            _ => bail!("unsupported method .{name}"),
        }
    }

    fn compile_property(&mut self, reference: &PropertyRef) -> Result<String> {
        match reference {
            PropertyRef::Note(parts) => {
                let placeholder = self.bind(Value::Text(json_path(parts)));
                Ok(format!("json_extract(metadata, {placeholder})"))
            }
            PropertyRef::File(field) => Ok(match field {
                FileField::Name => FILE_NAME_SQL.to_string(),
                FileField::Ext => "extension".into(),
                FileField::Path => "path".into(),
                FileField::Folder => "folder".into(),
                FileField::Size => "size_bytes".into(),
                FileField::Mtime => "datetime(modified_ns / 1000000000, 'unixepoch')".into(),
                FileField::Ctime => "datetime(created_ns / 1000000000, 'unixepoch')".into(),
                FileField::Links => "(SELECT group_concat(target_path, ', ') FROM wiki_links \
                     WHERE source_path = documents.path \
                     AND target_path IS NOT NULL AND is_embed = 0)"
                    .into(),
                FileField::Embeds => "(SELECT group_concat(target_path, ', ') FROM wiki_links \
                     WHERE source_path = documents.path \
                     AND target_path IS NOT NULL AND is_embed = 1)"
                    .into(),
                FileField::Backlinks => "(SELECT group_concat(source_path, ', ') FROM wiki_links \
                     WHERE target_path = documents.path)"
                    .into(),
                // `tags` is an ordinary frontmatter key (a YAML list, or a
                // single text when one tag); read it like any note property.
                FileField::Tags => {
                    self.compile_property(&PropertyRef::Note(vec!["tags".to_string()]))?
                }
                FileField::Properties => "json(metadata)".into(),
            }),
            PropertyRef::Formula(_) => bail!("unresolved formula reference"),
        }
    }

    /// Compiles a property source string
    /// (note path, file property, or formula name)
    /// into SQL with all formula references substituted.
    pub(super) fn compile_source(
        &mut self,
        source: &str,
        formulas: &BTreeMap<String, Expr>,
    ) -> Result<String> {
        let reference = expr::parse_property_ref(source)
            .map_err(|error| anyhow::anyhow!("invalid property {source:?}: {error}"))?;
        let resolved = match &reference {
            PropertyRef::Formula(name) => substitute_formula(name, formulas, 0)?,
            PropertyRef::Note(parts) => Expr::Property(PropertyRef::Note(parts.clone())),
            PropertyRef::File(field) => Expr::Property(PropertyRef::File(*field)),
        };
        Ok(self.compile_expr(&resolved)?.text)
    }
}

fn integer_argument(arg: Option<&Expr>, method: &str) -> Result<u8> {
    let Some(Expr::Number(value)) = arg else {
        bail!("{method} requires a numeric literal");
    };
    if value.fract() != 0.0 || *value < 0.0 || *value > 30.0 {
        bail!("{method} requires a whole number between 0 and 30");
    }
    value
        .to_string()
        .parse::<u8>()
        .map_err(|_| anyhow::anyhow!("{method} requires a whole number"))
}

fn duration_modifier(duration: crate::document::expr::Duration, negative: bool) -> Result<String> {
    let sign = if negative { '-' } else { '+' };
    let unit = duration.unit;
    let amount = if unit == DurationUnit::Week {
        duration
            .amount
            .checked_mul(unit.day_scale())
            .ok_or_else(|| anyhow::anyhow!("duration amount overflows"))?
    } else {
        duration.amount
    };
    Ok(format!("{sign}{amount} {}", unit.modifier()))
}

fn escape_glob(pattern: &str) -> String {
    pattern
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('?', "\\?")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

/// Maps common moment.js tokens onto strftime directives;
/// literal text passes through with `%` escaped.
#[must_use]
pub(super) fn moment_to_strftime(pattern: &str) -> String {
    let mut output = String::new();
    for character in pattern.chars() {
        if character == '%' {
            output.push_str("%%");
        } else {
            output.push(character);
        }
    }
    // Token rewriting happens per known token below;
    // the loop above only escapes stray directives so the mapping pass can insert them safely.
    rewrite_tokens(&output)
}

fn rewrite_tokens(input: &str) -> String {
    let mut output = input.to_string();
    for (token, directive) in TOKENS {
        output = output.replace(token, directive);
    }
    output
}

const TOKENS: &[(&str, &str)] = &[
    ("YYYY", "%Y"),
    ("YY", "%y"),
    ("MM", "%m"),
    ("DD", "%d"),
    ("HH", "%H"),
    ("mm", "%M"),
    ("ss", "%S"),
];

fn json_path(parts: &[String]) -> String {
    parts.iter().fold("$".to_string(), |mut path, part| {
        path.push('.');
        path.push_str(&serde_json::to_string(part).unwrap_or_else(|_| "\"\"".into()));
        path
    })
}

fn substitute_formula(name: &str, formulas: &BTreeMap<String, Expr>, depth: u32) -> Result<Expr> {
    if depth > FORMULA_SUBSTITUTION_DEPTH {
        bail!("formula references nest too deeply at {name:?}");
    }
    let body = formulas
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("undeclared formula {name:?}"))?
        .clone();
    substitute_formula_in(body, formulas, depth.saturating_add(1))
}

fn substitute_formula_in(
    expression: Expr,
    formulas: &BTreeMap<String, Expr>,
    depth: u32,
) -> Result<Expr> {
    if depth > FORMULA_SUBSTITUTION_DEPTH {
        bail!("formula references nest too deeply");
    }
    Ok(match expression {
        Expr::Property(PropertyRef::Formula(name)) => substitute_formula(&name, formulas, depth)?,
        Expr::Property(_) | Expr::Null | Expr::Bool(_) | Expr::Number(_) | Expr::Text(_) => {
            expression
        }
        Expr::Not(inner) => Expr::Not(Box::new(substitute_formula_in(*inner, formulas, depth)?)),
        Expr::Neg(inner) => Expr::Neg(Box::new(substitute_formula_in(*inner, formulas, depth)?)),
        Expr::Arithmetic { op, left, right } => Expr::Arithmetic {
            op,
            left: Box::new(substitute_formula_in(*left, formulas, depth)?),
            right: Box::new(substitute_formula_in(*right, formulas, depth)?),
        },
        Expr::Compare { op, left, right } => Expr::Compare {
            op,
            left: Box::new(substitute_formula_in(*left, formulas, depth)?),
            right: Box::new(substitute_formula_in(*right, formulas, depth)?),
        },
        Expr::Logic { op, left, right } => Expr::Logic {
            op,
            left: Box::new(substitute_formula_in(*left, formulas, depth)?),
            right: Box::new(substitute_formula_in(*right, formulas, depth)?),
        },
        Expr::Call(name, args) => Expr::Call(
            name,
            args.into_iter()
                .map(|arg| substitute_formula_in(arg, formulas, depth))
                .collect::<Result<Vec<_>>>()?,
        ),
        Expr::Method {
            subject,
            name,
            args,
        } => Expr::Method {
            subject: Box::new(substitute_formula_in(*subject, formulas, depth)?),
            name,
            args: args
                .into_iter()
                .map(|arg| substitute_formula_in(arg, formulas, depth))
                .collect::<Result<Vec<_>>>()?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{catalog_with, expression};
    use super::*;
    use crate::document::base::SortDirection;
    use crate::vault::catalog::BaseQuery;
    use std::collections::BTreeMap;

    fn source(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn filters_compile_with_minimal_parenthesization() {
        let mut compiler = BaseQueryCompiler::new(0);
        let sql = compiler
            .compile_filter(&expression("file.inFolder(\"Notes\")"))
            .unwrap();
        assert_eq!(
            sql,
            "COALESCE(folder = ?1 OR folder LIKE ?2 ESCAPE '\\', 0) <> 0"
        );

        // MatchAll conjuncts vanish; parameters keep numbering across filters.
        let filter = Filter::And(vec![
            expression("price > 5"),
            expression("!title.startsWith(\"Draft\")"),
            Filter::MatchAll,
        ]);
        let sql = compiler.compile_filter(&filter).unwrap();
        assert_eq!(
            sql,
            "COALESCE(json_extract(metadata, ?3) > ?4, 0) <> 0 \
             AND COALESCE(NOT json_extract(metadata, ?5) GLOB ?6, 0) <> 0"
        );
    }

    #[test]
    fn arithmetic_compiles_as_minimal_precedence_chains() {
        let mut formulas = BTreeMap::new();
        formulas.insert(
            "completion".to_string(),
            Expr::parse("progress / pages * 100").unwrap(),
        );
        let mut compiler = BaseQueryCompiler::new(0);
        let sql = compiler
            .compile_source("formula.completion", &formulas)
            .unwrap();
        assert_eq!(
            sql,
            "CAST(json_extract(metadata, ?1) AS REAL) \
             / CAST(json_extract(metadata, ?2) AS REAL) * ?3"
        );
    }

    #[test]
    fn file_predicates_compile_and_match() {
        pollster::block_on(async {
            let (database, root) = catalog_with(&[
                ("Notes/A.md", "---\ntags: [front]\n---\n[[Target]]"),
                ("Other/B.md", "---\n---\nplain"),
                ("Target.md", "---\n---\n"),
            ])
            .await;
            for (filter_source, expected) in [
                ("file.hasTag(\"front\")", 1usize),
                ("file.hasTag(\"missing-tag\")", 0),
                ("file.hasLink(\"Target\")", 1),
                ("file.hasLink(\"Target.md\")", 1),
                ("file.inFolder(\"Notes\")", 1),
                ("file.inFolder(\"Missing\")", 0),
            ] {
                let query = BaseQuery {
                    filters: expression(filter_source),
                    formulas: BTreeMap::new(),
                    projections: vec![source("file.name")],
                    sort: vec![],
                    group_by: None,
                    summaries: Vec::new(),
                    classes: Vec::new(),
                    limit: None,
                };
                let selection = database
                    .query_base(query)
                    .await
                    .unwrap_or_else(|error| panic!("test setup: {filter_source}: {error}"));
                assert_eq!(selection.total_matched, expected, "{filter_source}");
            }
            drop(database);
            let _ = std::fs::remove_dir_all(root);
        });
    }

    #[test]
    fn formulas_project_and_group_by_buckets_rows() {
        pollster::block_on(async {
            let (database, root) = catalog_with(&[
                ("A.md", "---\nprice: 4\ncount: 2\n---\n"),
                ("B.md", "---\nprice: 8\ncount: 2\n---\n"),
                ("C.md", "---\nprice: 1\ncount: 5\n---\n"),
            ])
            .await;
            let mut formulas = BTreeMap::new();
            formulas.insert("ppu".to_string(), Expr::parse("price / count").unwrap());
            let query = BaseQuery {
                filters: Filter::MatchAll,
                formulas,
                projections: vec![source("formula.ppu"), source("count")],
                sort: vec![],
                group_by: Some((source("count"), SortDirection::Asc)),
                summaries: vec![(
                    source("formula.ppu"),
                    crate::vault::SummaryRef::Default(crate::document::base::Summary::Average),
                )],
                limit: Some(50),
                classes: Vec::new(),
            };
            let selection = database.query_base(query).await.unwrap();
            assert_eq!(selection.total_matched, 3);
            let first = &selection.documents[0].values[0];
            assert_eq!(first.as_f64(), Some(2.0), "ppu for A is price/count=2");
            let last = selection.documents.last().unwrap();
            assert!(last.path.ends_with("C.md"));
            assert_eq!(last.values[0].as_f64(), Some(0.2));
            let average = selection.summaries[0].as_f64().unwrap();
            assert!((average - 2.066_666_7).abs() < 1e-4, "{average}");
            drop(database);
            let _ = std::fs::remove_dir_all(root);
        });
    }
}
