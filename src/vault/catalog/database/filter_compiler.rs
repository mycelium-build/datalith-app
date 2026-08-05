use turso::Value;

use super::escape_like_pattern;
use crate::vault::catalog::{
    CatalogComparison, CatalogFileField, CatalogFilter, CatalogProperty, CatalogScalar,
};

#[derive(Default)]
pub(super) struct FilterCompiler {
    pub(super) parameters: Vec<Value>,
}

impl FilterCompiler {
    fn parameter(&mut self, value: Value) -> &'static str {
        self.parameters.push(value);
        "?"
    }

    pub(super) fn compile(&mut self, filter: &CatalogFilter) -> String {
        match filter {
            CatalogFilter::MatchAll => "1".into(),
            CatalogFilter::And(filters) => self.join(filters, "AND", "1"),
            CatalogFilter::Or(filters) => self.join(filters, "OR", "0"),
            CatalogFilter::Not(filter) => {
                format!("COALESCE(({}), 0) = 0", self.compile(filter))
            }
            CatalogFilter::InFolder(folder) => {
                self.parameters.push(Value::Text(folder.clone()));
                self.parameters
                    .push(Value::Text(format!("{}/%", escape_like_pattern(folder))));
                "(folder = ? OR folder LIKE ? ESCAPE '\\')".into()
            }
            CatalogFilter::Contains { property, value } => {
                let CatalogProperty::Metadata(parts) = property else {
                    return "0".into();
                };
                let path = json_path(parts);
                self.parameters.push(Value::Text(path.clone()));
                self.parameters.push(Value::Text(path));
                let value_clause = self.each_value_clause(value);
                format!(
                    "(json_type(metadata, ?) = 'array' AND EXISTS \
                     (SELECT 1 FROM json_each(metadata, ?) AS each \
                      WHERE {value_clause}))"
                )
            }
            CatalogFilter::Compare {
                property,
                comparison,
                value,
            } => self.compare(property, *comparison, value),
        }
    }

    fn join(&mut self, filters: &[CatalogFilter], operator: &str, empty: &str) -> String {
        if filters.is_empty() {
            return empty.into();
        }
        let values = filters
            .iter()
            .map(|filter| format!("({})", self.compile(filter)))
            .collect::<Vec<_>>();
        values.join(&format!(" {operator} "))
    }

    fn compare(
        &mut self,
        property: &CatalogProperty,
        comparison: CatalogComparison,
        value: &CatalogScalar,
    ) -> String {
        match property {
            CatalogProperty::File(field) => {
                let column = match field {
                    CatalogFileField::Name => super::FILE_NAME_SQL,
                    CatalogFileField::Extension => "extension",
                    CatalogFileField::Path => "path",
                    CatalogFileField::Folder => "folder",
                };
                self.file_compare(column, comparison, value)
            }
            CatalogProperty::Metadata(parts) => {
                let path = json_path(parts);
                match (comparison, value) {
                    (CatalogComparison::Equal, CatalogScalar::Null) => {
                        self.push_path(&path, 2);
                        "(json_type(metadata, ?) IS NULL OR json_type(metadata, ?) = 'null')".into()
                    }
                    (CatalogComparison::NotEqual, CatalogScalar::Null) => {
                        self.push_path(&path, 2);
                        "(json_type(metadata, ?) IS NOT NULL AND json_type(metadata, ?) != 'null')"
                            .into()
                    }
                    (CatalogComparison::NotEqual, value) => {
                        self.push_path(&path, 3);
                        let Some(type_clause) = json_type_clause(value) else {
                            return "1".into();
                        };
                        self.scalar_parameter(value);
                        format!(
                            "(json_type(metadata, ?) IS NULL OR NOT \
                             ({type_clause} AND json_extract(metadata, ?) = ?))"
                        )
                    }
                    (CatalogComparison::Equal, value) => {
                        self.push_path(&path, 2);
                        let Some(type_clause) = json_type_clause(value) else {
                            return "0".into();
                        };
                        self.scalar_parameter(value);
                        format!("({type_clause} AND json_extract(metadata, ?) = ?)")
                    }
                    (comparison, CatalogScalar::Number(value)) => {
                        self.push_path(&path, 2);
                        self.parameters.push(Value::Real(*value));
                        format!(
                            "(json_type(metadata, ?) IN ('integer', 'real') AND \
                             json_extract(metadata, ?) {} ?)",
                            comparison_sql(comparison)
                        )
                    }
                    _ => "0".into(),
                }
            }
        }
    }

    fn file_compare(
        &mut self,
        column: &str,
        comparison: CatalogComparison,
        value: &CatalogScalar,
    ) -> String {
        let CatalogScalar::String(value) = value else {
            return match (comparison, value) {
                (CatalogComparison::NotEqual, CatalogScalar::Null) => "1".into(),
                _ => "0".into(),
            };
        };
        self.parameters.push(Value::Text(value.clone()));
        format!("{column} {} ?", comparison_sql(comparison))
    }

    fn scalar_parameter(&mut self, scalar: &CatalogScalar) -> &'static str {
        let value = match scalar {
            CatalogScalar::Null => Value::Null,
            CatalogScalar::Bool(value) => Value::Integer(i64::from(*value)),
            CatalogScalar::Number(value) => Value::Real(*value),
            CatalogScalar::String(value) => Value::Text(value.clone()),
        };
        self.parameter(value)
    }

    fn push_path(&mut self, path: &str, count: usize) {
        self.parameters
            .extend((0..count).map(|_| Value::Text(path.to_string())));
    }

    fn each_value_clause(&mut self, scalar: &CatalogScalar) -> String {
        match scalar {
            CatalogScalar::Null => "each.type = 'null'".into(),
            CatalogScalar::Bool(value) => {
                let kind = if *value { "true" } else { "false" };
                format!("each.type = '{kind}'")
            }
            CatalogScalar::Number(value) => {
                self.parameters.push(Value::Real(*value));
                "each.type IN ('integer', 'real') AND each.atom = ?".into()
            }
            CatalogScalar::String(value) => {
                self.parameters.push(Value::Text(value.clone()));
                "each.type = 'text' AND each.atom = ?".into()
            }
        }
    }
}

fn json_type_clause(value: &CatalogScalar) -> Option<&'static str> {
    match value {
        CatalogScalar::Null => None,
        CatalogScalar::Bool(true) => Some("json_type(metadata, ?) = 'true'"),
        CatalogScalar::Bool(false) => Some("json_type(metadata, ?) = 'false'"),
        CatalogScalar::Number(_) => Some("json_type(metadata, ?) IN ('integer', 'real')"),
        CatalogScalar::String(_) => Some("json_type(metadata, ?) = 'text'"),
    }
}

fn comparison_sql(comparison: CatalogComparison) -> &'static str {
    match comparison {
        CatalogComparison::Equal => "=",
        CatalogComparison::NotEqual => "!=",
        CatalogComparison::Greater => ">",
        CatalogComparison::GreaterEqual => ">=",
        CatalogComparison::Less => "<",
        CatalogComparison::LessEqual => "<=",
    }
}

fn json_path(parts: &[String]) -> String {
    parts.iter().fold("$".to_string(), |mut path, part| {
        path.push('.');
        path.push_str(&serde_json::to_string(part).unwrap_or_else(|_| "\"\"".into()));
        path
    })
}
