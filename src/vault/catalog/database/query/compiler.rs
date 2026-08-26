//! Compiles Bases expressions into Turso SQL with numbered placeholders.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use turso::Value;

use crate::document::expr::{
    self, ArithOp, CmpOp, DurationUnit, Expr, FileField, LogicOp, PropertyRef, ValType,
};
use crate::document::filter::Filter;

use super::super::{FILE_NAME_SQL, escape_like_pattern};
use super::parenthesize;

const FORMULA_SUBSTITUTION_DEPTH: u32 = 32;

fn truthy(sql: &str) -> String {
    format!("COALESCE(({}), 0) <> 0", parenthesize(sql))
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
        match filter {
            Filter::MatchAll => Ok("1".into()),
            Filter::And(filters) => self.join(filters.iter(), "AND", "1"),
            Filter::Or(filters) => self.join(filters.iter(), "OR", "0"),
            Filter::Not(filter) => {
                let inner = self.compile_filter(filter)?;
                Ok(format!("NOT {}", truthy(&inner)))
            }
            Filter::Expression(expression) => {
                let sql = self.compile_expr(&expression.expr)?;
                Ok(truthy(&sql))
            }
        }
    }

    fn join<'a>(
        &mut self,
        filters: impl Iterator<Item = &'a Filter>,
        operator: &str,
        empty: &str,
    ) -> Result<String> {
        let parts = filters
            .map(|filter| Ok(format!("({})", self.compile_filter(filter)?)))
            .collect::<Result<Vec<_>>>()?;
        if parts.is_empty() {
            return Ok(empty.into());
        }
        Ok(parts.join(&format!(" {operator} ")))
    }

    fn compile_expr(&mut self, expression: &Expr) -> Result<String> {
        match expression {
            Expr::Null => Ok("NULL".into()),
            Expr::Bool(value) => Ok(if *value { "1" } else { "0" }.into()),
            Expr::Number(value) => {
                let placeholder = self.bind(Value::Real(*value));
                Ok(placeholder)
            }
            Expr::Text(text) => {
                let placeholder = self.bind(Value::Text(text.clone()));
                Ok(placeholder)
            }
            // Duration literals are consumed by date arithmetic at the
            // Arithmetic node below and never reach this arm in valid trees. => bail!("misplaced duration literal"),
            Expr::Property(reference) => self.compile_property(reference),
            Expr::Not(inner) => {
                let sql = self.compile_expr(inner)?;
                Ok(format!("(NOT {})", parenthesize(&sql)))
            }
            Expr::Neg(inner) => Ok(format!("(-{})", parenthesize(&self.compile_expr(inner)?))),
            Expr::Arithmetic { op, left, right } => self.compile_arithmetic(*op, left, right),
            Expr::Compare { op, left, right } => self.compile_compare(*op, left, right),
            Expr::Logic { op, left, right } => {
                let operator = match op {
                    LogicOp::And => "AND",
                    LogicOp::Or => "OR",
                };
                let left_sql = truthy(&self.compile_expr(left)?);
                let right_sql = truthy(&self.compile_expr(right)?);
                Ok(format!("({left_sql} {operator} {right_sql})"))
            }
            Expr::Call(name, args) => self.compile_call(name, args),
            Expr::Method {
                subject,
                name,
                args,
            } => self.compile_method(subject, name, args),
        }
    }

    fn compile_arithmetic(&mut self, op: ArithOp, left: &Expr, right: &Expr) -> Result<String> {
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
                return Ok(format!(
                    "datetime({}, {placeholder})",
                    parenthesize(&left_sql)
                ));
            }
            if op == ArithOp::Add
                && let Some(duration) = expr::coerce_duration(left)
                && right_type == ValType::Date
            {
                let modifier = duration_modifier(duration, false)?;
                let right_sql = self.compile_expr(right)?;
                let placeholder = self.bind(Value::Text(modifier));
                return Ok(format!(
                    "datetime({}, {placeholder})",
                    parenthesize(&right_sql)
                ));
            }
            if op == ArithOp::Subtract && left_type == ValType::Date && right_type == ValType::Date
            {
                let left_sql = self.compile_expr(left)?;
                let right_sql = self.compile_expr(right)?;
                return Ok(format!(
                    "((julianday({}) - julianday({})) * 86400000.0)",
                    parenthesize(&left_sql),
                    parenthesize(&right_sql)
                ));
            }
        }
        let left_sql = self.compile_expr(left)?;
        let right_sql = self.compile_expr(right)?;
        // SQLite divides integers with truncation,
        // but the documented expression semantics are real-valued.
        if op == ArithOp::Divide {
            return Ok(format!(
                "((CAST({} AS REAL)) / (CAST({} AS REAL)))",
                parenthesize(&left_sql),
                parenthesize(&right_sql)
            ));
        }
        let operator = match op {
            ArithOp::Add => "+",
            ArithOp::Subtract => "-",
            ArithOp::Multiply => "*",
            ArithOp::Divide => "/",
            ArithOp::Modulo => "%",
        };
        Ok(format!(
            "({} {operator} {})",
            parenthesize(&left_sql),
            parenthesize(&right_sql)
        ))
    }

    fn compile_compare(&mut self, op: CmpOp, left: &Expr, right: &Expr) -> Result<String> {
        // Missing properties are SQL NULL;
        // comparisons against the null literal follow the documented missing-value semantics.
        if matches!(right, Expr::Null) {
            let left_sql = self.compile_expr(left)?;
            return match op {
                CmpOp::Equal => Ok(format!("({}) IS NULL", parenthesize(&left_sql))),
                CmpOp::NotEqual => Ok(format!("({}) IS NOT NULL", parenthesize(&left_sql))),
                _ => bail!("null only supports equality comparisons"),
            };
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
        Ok(format!(
            "({} {operator} {})",
            parenthesize(&left_sql),
            parenthesize(&right_sql)
        ))
    }

    fn compile_call(&mut self, name: &str, args: &[Expr]) -> Result<String> {
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
                Ok(format!(
                    "EXISTS (SELECT 1 FROM wiki_links WHERE source_path = documents.path \
                     AND target_path IS NOT NULL AND (target = {target} OR target_path = {target_path}))"
                ))
            }
            "file.inFolder" => {
                let [Expr::Text(folder)] = args else {
                    bail!("file.inFolder takes one string argument")
                };
                let exact = self.bind(Value::Text(folder.clone()));
                let prefix = self.bind(Value::Text(format!("{}/%", escape_like_pattern(folder))));
                Ok(format!(
                    "(folder = {exact} OR folder LIKE {prefix} ESCAPE '\\')"
                ))
            }
            "today" | "now" => {
                // Frozen per refresh: the executor overwrites this payload once.
                Ok(self.bind(Value::Text(String::new())))
            }

            "if" => {
                let [condition, then, otherwise] = args else {
                    bail!("if takes three arguments")
                };
                let condition_sql = truthy(&self.compile_expr(condition)?);
                let then_sql = self.compile_expr(then)?;
                let otherwise_sql = self.compile_expr(otherwise)?;
                Ok(format!(
                    "(CASE WHEN {condition_sql} THEN {} ELSE {} END)",
                    parenthesize(&then_sql),
                    parenthesize(&otherwise_sql)
                ))
            }
            "min" | "max" => {
                let sqls = args
                    .iter()
                    .map(|arg| Ok(parenthesize(&self.compile_expr(arg)?)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(format!("{name}({})", sqls.join(", ")))
            }
            "date" | "datetime" | "time" | "julianday" | "abs" | "round" | "length" | "lower"
            | "upper" | "trim" | "ltrim" | "rtrim" | "instr" => {
                let [inner] = args else {
                    bail!("{name} takes one argument")
                };
                let inner_sql = self.compile_expr(inner)?;
                Ok(format!("{name}({})", parenthesize(&inner_sql)))
            }
            "replace" | "substr" => {
                let sqls = args
                    .iter()
                    .map(|arg| Ok(parenthesize(&self.compile_expr(arg)?)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(format!("{name}({})", sqls.join(", ")))
            }
            _ => bail!("unsupported function {name:?}"),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn compile_method(&mut self, subject: &Expr, name: &str, args: &[Expr]) -> Result<String> {
        match name {
            "contains" => {
                // Lists check element membership; strings check substrings.
                let subject_sql = self.compile_expr(subject)?;
                let Some(arg) = args.first() else {
                    bail!(".contains takes one argument")
                };
                let arg_sql = self.compile_expr(arg)?;
                Ok(format!(
                    "(CASE WHEN json_type({}) = 'array' \
                     THEN EXISTS (SELECT 1 FROM json_each({}) AS each \
                     WHERE ({arg_sql}) = each.atom) \
                     ELSE instr(COALESCE({}, ''), ({arg_sql})) > 0 END)",
                    parenthesize(&subject_sql),
                    parenthesize(&subject_sql),
                    parenthesize(&subject_sql)
                ))
            }
            "toFixed" => {
                let subject_sql = self.compile_expr(subject)?;
                let digits = integer_argument(args.first(), ".toFixed")?;
                self.parameters.push(Value::Text(format!("%.{digits}f")));
                Ok(format!("format(?, {})", parenthesize(&subject_sql)))
            }
            "round" => {
                let subject_sql = self.compile_expr(subject)?;
                let digits = integer_argument(args.first(), ".round")?;
                let placeholder = self.bind(Value::Integer(i64::from(digits)));
                Ok(format!(
                    "round({}, {placeholder})",
                    parenthesize(&subject_sql)
                ))
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
                Ok(format!(
                    "({} GLOB {placeholder})",
                    parenthesize(&subject_sql)
                ))
            }
            "lower" | "upper" | "trim" => {
                let subject_sql = self.compile_expr(subject)?;
                Ok(format!("{name}({})", parenthesize(&subject_sql)))
            }
            "date" => {
                let subject_sql = self.compile_expr(subject)?;
                Ok(format!("date({})", parenthesize(&subject_sql)))
            }
            "format" => {
                let Some(Expr::Text(pattern)) = args.first() else {
                    bail!(".format requires a string literal")
                };
                let strftime_pattern = moment_to_strftime(pattern);
                let subject_sql = self.compile_expr(subject)?;
                let placeholder = self.bind(Value::Text(strftime_pattern));
                Ok(format!(
                    "strftime({placeholder}, {})",
                    parenthesize(&subject_sql)
                ))
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
        self.compile_expr(&resolved)
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
