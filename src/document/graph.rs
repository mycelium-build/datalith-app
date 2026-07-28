use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Deserializer, de::Error};
use yaml_serde::Value;

use crate::vault::{
    CatalogComparison, CatalogFileField, CatalogFilter, CatalogProperty, CatalogScalar,
};

pub(crate) const DEFAULT_NODE_LIMIT: usize = 2_000;
pub(crate) const HARD_NODE_LIMIT: usize = 10_000;
pub(crate) const DEFAULT_CENTER_STRENGTH: f32 = 0.002;
pub(crate) const DEFAULT_REPULSION_STRENGTH: f32 = 1_024.0;
pub(crate) const DEFAULT_LINK_STRENGTH: f32 = 0.04;
pub(crate) const DEFAULT_LINK_DISTANCE: f32 = 128.0;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct GraphDefinition {
    pub(crate) limit: usize,
    pub(crate) filters: Filter,
    pub(crate) groups: Vec<GraphGroup>,
    pub(crate) display: GraphDisplay,
    pub(crate) physics: GraphPhysics,
}

impl Default for GraphDefinition {
    fn default() -> Self {
        Self {
            limit: DEFAULT_NODE_LIMIT,
            filters: Filter::MatchAll,
            groups: Vec::new(),
            display: GraphDisplay::default(),
            physics: GraphPhysics::default(),
        }
    }
}

impl GraphDefinition {
    pub(crate) fn catalog_filter(&self) -> CatalogFilter {
        self.filters.to_catalog_filter()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Filter {
    MatchAll,
    Expression(Expression),
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Box<Filter>),
}

impl Default for Filter {
    fn default() -> Self {
        Self::MatchAll
    }
}

impl Filter {
    fn to_catalog_filter(&self) -> CatalogFilter {
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
}

impl<'de> Deserialize<'de> for Filter {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        parse_filter(&value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct GraphGroup {
    pub(crate) name: String,
    pub(crate) filters: Filter,
    pub(crate) node: GroupNodeStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct GroupNodeStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) size: Option<f32>,
    pub(crate) border: BorderStyle,
    pub(crate) hover: HoverStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct GraphDisplay {
    pub(crate) node: NodeStyle,
    pub(crate) edge: EdgeStyle,
    pub(crate) orphan: OrphanStyle,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct NodeStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) size: Option<f32>,
    pub(crate) propertional: bool,
    pub(crate) border: BorderStyle,
    pub(crate) hover: HoverStyle,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            color: None,
            size: None,
            propertional: true,
            border: BorderStyle::default(),
            hover: HoverStyle::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct BorderStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) width: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct HoverStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) size: Option<f32>,
    pub(crate) border: BorderStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct EdgeStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) width: Option<f32>,
    pub(crate) arrow: bool,
    pub(crate) hover: EdgeHoverStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct EdgeHoverStyle {
    pub(crate) direction: EdgeHoverDirectionStyles,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct EdgeHoverDirectionStyles {
    pub(crate) outgoing: DirectionalEdgeHoverStyle,
    pub(crate) incoming: DirectionalEdgeHoverStyle,
    pub(crate) both: DirectionalEdgeHoverStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DirectionalEdgeHoverStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) width: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct OrphanStyle {
    pub(crate) show: bool,
    pub(crate) node: NodeStyle,
}

impl Default for OrphanStyle {
    fn default() -> Self {
        Self {
            show: true,
            node: NodeStyle::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct GraphPhysics {
    pub(crate) center: CenterForce,
    pub(crate) repulsion: RepulsionForce,
    pub(crate) link: LinkForce,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CenterForce {
    pub(crate) strength: f32,
}

impl Default for CenterForce {
    fn default() -> Self {
        Self {
            strength: DEFAULT_CENTER_STRENGTH,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct RepulsionForce {
    pub(crate) strength: f32,
}

impl Default for RepulsionForce {
    fn default() -> Self {
        Self {
            strength: DEFAULT_REPULSION_STRENGTH,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LinkForce {
    pub(crate) strength: f32,
    pub(crate) distance: f32,
}

impl Default for LinkForce {
    fn default() -> Self {
        Self {
            strength: DEFAULT_LINK_STRENGTH,
            distance: DEFAULT_LINK_DISTANCE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GraphColor {
    pub(crate) red: f32,
    pub(crate) green: f32,
    pub(crate) blue: f32,
    pub(crate) alpha: f32,
}

impl<'de> Deserialize<'de> for GraphColor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        parse_color(&String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Expression {
    left: PropertyPath,
    operation: Operation,
}

#[derive(Clone, Debug, PartialEq)]
enum Operation {
    Compare(Comparison, Scalar),
    Contains(Scalar),
    InFolder(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Comparison {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Clone, Debug, PartialEq)]
enum PropertyPath {
    Note(Vec<String>),
    File(FileField),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FileField {
    Name,
    Ext,
    Path,
    Folder,
}

#[derive(Clone, Debug, PartialEq)]
enum Scalar {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

pub(crate) struct GraphFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) properties: &'a Value,
}

pub(crate) fn parse_definition(source: &str) -> Result<GraphDefinition> {
    let definition: GraphDefinition = if source.trim().is_empty() {
        GraphDefinition::default()
    } else {
        yaml_serde::from_str(source).map_err(|error| anyhow!(format_yaml_error(error)))?
    };
    if !(1..=HARD_NODE_LIMIT).contains(&definition.limit) {
        bail!("limit must be between 1 and {HARD_NODE_LIMIT}");
    }

    let mut names = HashSet::new();
    for group in &definition.groups {
        if group.name.trim().is_empty() {
            bail!("group name must not be empty");
        }
        if !names.insert(&group.name) {
            bail!("group name {:?} is duplicated", group.name);
        }
        if group.node == GroupNodeStyle::default() {
            bail!("group {:?} must define at least one node style", group.name);
        }
        validate_group_node_style(&group.node, &format!("group {:?}.node", group.name))?;
    }
    validate_node_style(&definition.display.node, "display.node")?;
    validate_range(
        definition.display.edge.width,
        0.5,
        5.0,
        "display.edge.width",
    )?;
    validate_range(
        definition.display.edge.hover.direction.outgoing.width,
        0.5,
        5.0,
        "display.edge.hover.direction.outgoing.width",
    )?;
    validate_range(
        definition.display.edge.hover.direction.incoming.width,
        0.5,
        5.0,
        "display.edge.hover.direction.incoming.width",
    )?;
    validate_range(
        definition.display.edge.hover.direction.both.width,
        0.5,
        5.0,
        "display.edge.hover.direction.both.width",
    )?;
    validate_node_style(&definition.display.orphan.node, "display.orphan.node")?;
    validate_non_negative(
        definition.physics.center.strength,
        "physics.center.strength",
    )?;
    validate_non_negative(
        definition.physics.repulsion.strength,
        "physics.repulsion.strength",
    )?;
    validate_non_negative(definition.physics.link.strength, "physics.link.strength")?;
    validate_positive(definition.physics.link.distance, "physics.link.distance")?;
    Ok(definition)
}

fn validate_node_style(style: &NodeStyle, name: &str) -> Result<()> {
    validate_node_style_fields(style.size, &style.border, &style.hover, name)
}

fn validate_group_node_style(style: &GroupNodeStyle, name: &str) -> Result<()> {
    validate_node_style_fields(style.size, &style.border, &style.hover, name)
}

fn validate_node_style_fields(
    size: Option<f32>,
    border: &BorderStyle,
    hover: &HoverStyle,
    name: &str,
) -> Result<()> {
    validate_range(size, 0.5, 3.0, &format!("{name}.size"))?;
    validate_range(border.width, 0.0, 5.0, &format!("{name}.border.width"))?;
    validate_range(hover.size, 0.5, 3.0, &format!("{name}.hover.size"))?;
    validate_range(
        hover.border.width,
        0.0,
        5.0,
        &format!("{name}.hover.border.width"),
    )?;
    Ok(())
}

fn validate_non_negative(value: f32, name: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        bail!("{name} must be a finite non-negative number");
    }
    Ok(())
}

fn validate_positive(value: f32, name: &str) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        bail!("{name} must be a finite positive number");
    }
    Ok(())
}

fn format_yaml_error(error: yaml_serde::Error) -> String {
    error.location().map_or_else(
        || error.to_string(),
        |location| {
            format!(
                "line {}, column {}: {error}",
                location.line(),
                location.column()
            )
        },
    )
}

fn validate_range(
    value: Option<f32>,
    minimum: f32,
    maximum: f32,
    name: &str,
) -> Result<Option<f32>> {
    if value.is_some_and(|value| !value.is_finite() || value < minimum || value > maximum) {
        bail!("{name} must be between {minimum} and {maximum}");
    }
    Ok(value)
}

fn parse_filter(value: &Value) -> Result<Filter> {
    match value {
        Value::Null => Ok(Filter::MatchAll),
        Value::String(expression) => Ok(Filter::Expression(parse_expression(expression)?)),
        Value::Sequence(filters) if filters.is_empty() => Ok(Filter::MatchAll),
        Value::Mapping(map) if map.len() == 1 => {
            let (key, value) = map.iter().next().unwrap();
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

impl Expression {
    fn to_catalog_filter(&self) -> CatalogFilter {
        if let Operation::InFolder(folder) = &self.operation {
            return CatalogFilter::InFolder(folder.clone());
        }
        let property = match &self.left {
            PropertyPath::Note(parts) => CatalogProperty::Metadata(parts.clone()),
            PropertyPath::File(field) => CatalogProperty::File(match field {
                FileField::Name => CatalogFileField::Name,
                FileField::Ext => CatalogFileField::Extension,
                FileField::Path => CatalogFileField::Path,
                FileField::Folder => CatalogFileField::Folder,
            }),
        };
        match &self.operation {
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
            Operation::InFolder(_) => unreachable!(),
        }
    }

    fn matches(&self, file: &GraphFile<'_>) -> bool {
        match &self.operation {
            Operation::InFolder(folder) => {
                let parent = file
                    .path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_string_lossy()
                    .replace('\\', "/");
                parent == *folder || parent.starts_with(&format!("{folder}/"))
            }
            Operation::Contains(expected) => value_at(&self.left, file).is_some_and(|value| {
                value
                    .as_sequence()
                    .is_some_and(|values| values.iter().any(|value| scalar_equals(value, expected)))
            }),
            Operation::Compare(comparison, expected) => {
                compare(value_at(&self.left, file), *comparison, expected)
            }
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

fn value_at<'a>(path: &PropertyPath, file: &'a GraphFile<'_>) -> Option<&'a Value> {
    match path {
        PropertyPath::Note(parts) => parts
            .iter()
            .try_fold(file.properties, |value, part| value.get(part)),
        PropertyPath::File(_) => None,
    }
}

fn file_scalar(path: &PropertyPath, file: &GraphFile<'_>) -> Option<Scalar> {
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
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy()
            .replace('\\', "/"),
    };
    Some(Scalar::String(string))
}

fn compare(actual: Option<&Value>, comparison: Comparison, expected: &Scalar) -> bool {
    if actual.is_none() {
        return match (comparison, expected) {
            (Comparison::Equal, Scalar::Null) => true,
            (Comparison::NotEqual, Scalar::Null) => false,
            (Comparison::NotEqual, _) => true,
            _ => false,
        };
    }
    let actual = actual.unwrap();
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

fn scalar_equals(actual: &Value, expected: &Scalar) -> bool {
    match expected {
        Scalar::Null => actual.is_null(),
        Scalar::Bool(expected) => actual.as_bool() == Some(*expected),
        Scalar::Number(expected) => actual.as_f64() == Some(*expected),
        Scalar::String(expected) => actual.as_str() == Some(expected),
    }
}

fn parse_expression(source: &str) -> Result<Expression> {
    let source = source.trim();
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
            if matches!(left, PropertyPath::File(_)) {
                return Ok(Expression {
                    left,
                    operation: Operation::Compare(comparison, expected),
                });
            }
            return Ok(Expression {
                left,
                operation: Operation::Compare(comparison, expected),
            });
        }
    }
    bail!("unsupported filter expression {source:?}")
}

fn parse_property(source: &str) -> Result<PropertyPath> {
    if let Some(field) = source.strip_prefix("file.") {
        return Ok(PropertyPath::File(match field {
            "name" => FileField::Name,
            "ext" => FileField::Ext,
            "path" => FileField::Path,
            "folder" => FileField::Folder,
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
            let end = bracket
                .find(']')
                .ok_or_else(|| anyhow!("unterminated property bracket"))?;
            parts.push(parse_string(&bracket[..end])?);
            rest = bracket[end + 1..].trim_start_matches('.');
        } else {
            let end = rest.find(['.', '[']).unwrap_or(rest.len());
            let part = &rest[..end];
            if part.is_empty() {
                bail!("empty property name");
            }
            parts.push(part.to_string());
            rest = rest[end..].trim_start_matches('.');
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
    if source.len() < 2 {
        bail!("expected quoted string");
    }
    let quote = source.as_bytes()[0];
    if !matches!(quote, b'\'' | b'"') || source.as_bytes()[source.len() - 1] != quote {
        bail!("expected quoted string");
    }
    Ok(source[1..source.len() - 1].to_string())
}

pub(crate) fn matches_definition(
    definition: &GraphDefinition,
    path: &Path,
    properties: &Value,
) -> bool {
    let file = GraphFile { path, properties };
    match &definition.filters {
        Filter::Expression(expression) if matches!(expression.left, PropertyPath::File(_)) => {
            expression_matches_with_file(expression, &file)
        }
        filter => filter_matches_with_file(filter, &file),
    }
}

fn filter_matches_with_file(filter: &Filter, file: &GraphFile<'_>) -> bool {
    match filter {
        Filter::Expression(expression) => expression_matches_with_file(expression, file),
        Filter::And(filters) => filters
            .iter()
            .all(|filter| filter_matches_with_file(filter, file)),
        Filter::Or(filters) => filters
            .iter()
            .any(|filter| filter_matches_with_file(filter, file)),
        Filter::Not(filter) => !filter_matches_with_file(filter, file),
        Filter::MatchAll => true,
    }
}

fn expression_matches_with_file(expression: &Expression, file: &GraphFile<'_>) -> bool {
    if let Operation::Compare(comparison, expected) = &expression.operation
        && let Some(actual) = file_scalar(&expression.left, file)
    {
        return match comparison {
            Comparison::Equal => actual == *expected,
            Comparison::NotEqual => actual != *expected,
            _ => match (actual, expected) {
                (Scalar::Number(actual), Scalar::Number(expected)) => match comparison {
                    Comparison::Greater => actual > *expected,
                    Comparison::GreaterEqual => actual >= *expected,
                    Comparison::Less => actual < *expected,
                    Comparison::LessEqual => actual <= *expected,
                    _ => false,
                },
                _ => false,
            },
        };
    }
    expression.matches(file)
}

pub(crate) fn matching_group<'a>(
    definition: &'a GraphDefinition,
    path: &Path,
    properties: &Value,
) -> Option<&'a GraphGroup> {
    let file = GraphFile { path, properties };
    definition
        .groups
        .iter()
        .find(|group| filter_matches_with_file(&group.filters, &file))
}

pub(crate) fn parse_color(source: &str) -> Result<GraphColor> {
    let source = source.trim();
    if let Some(hex) = source.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(args) = function_args(source, "rgb").or_else(|| function_args(source, "rgba")) {
        return parse_rgb(args);
    }
    if let Some(args) = function_args(source, "hsl").or_else(|| function_args(source, "hsla")) {
        return parse_hsl(args);
    }
    if let Some(args) = function_args(source, "oklch") {
        return parse_oklch(args);
    }
    bail!("unsupported color {source:?}")
}

fn function_args<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    source
        .strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')
}

fn parse_hex(hex: &str) -> Result<GraphColor> {
    let expanded = match hex.len() {
        3 => format!(
            "{}{}{}{}{}{}ff",
            &hex[0..1],
            &hex[0..1],
            &hex[1..2],
            &hex[1..2],
            &hex[2..3],
            &hex[2..3]
        ),
        6 => format!("{hex}ff"),
        8 => hex.to_string(),
        _ => bail!("hex colors must use #RGB, #RRGGBB, or #RRGGBBAA"),
    };
    let bytes = (0..4)
        .map(|i| u8::from_str_radix(&expanded[i * 2..i * 2 + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(GraphColor {
        red: bytes[0] as f32 / 255.0,
        green: bytes[1] as f32 / 255.0,
        blue: bytes[2] as f32 / 255.0,
        alpha: bytes[3] as f32 / 255.0,
    })
}

fn color_parts(args: &str) -> Vec<String> {
    args.replace(',', " ")
        .split_whitespace()
        .filter(|part| *part != "/")
        .map(str::to_string)
        .collect()
}

fn parse_rgb(args: &str) -> Result<GraphColor> {
    let parts = color_parts(args);
    if !(3..=4).contains(&parts.len()) {
        bail!("rgb requires three channels and optional alpha");
    }
    let channel = |part: &str| -> Result<f32> {
        if let Some(percent) = part.strip_suffix('%') {
            Ok(percent.parse::<f32>()? / 100.0)
        } else {
            Ok(part.parse::<f32>()? / 255.0)
        }
    };
    make_color(
        channel(&parts[0])?,
        channel(&parts[1])?,
        channel(&parts[2])?,
        parts.get(3).map(|v| alpha(v)).transpose()?.unwrap_or(1.0),
    )
}

fn parse_hsl(args: &str) -> Result<GraphColor> {
    let parts = color_parts(args);
    if !(3..=4).contains(&parts.len()) {
        bail!("hsl requires hue, saturation, lightness, and optional alpha");
    }
    let h = parts[0]
        .trim_end_matches("deg")
        .parse::<f32>()?
        .rem_euclid(360.0)
        / 360.0;
    let s = percentage(&parts[1])?;
    let l = percentage(&parts[2])?;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| {
        if t < 0.0 {
            t += 1.0
        }
        if t > 1.0 {
            t -= 1.0
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    make_color(
        hue(h + 1.0 / 3.0),
        hue(h),
        hue(h - 1.0 / 3.0),
        parts.get(3).map(|v| alpha(v)).transpose()?.unwrap_or(1.0),
    )
}

#[allow(clippy::excessive_precision)] // Published OKLab conversion coefficients.
fn parse_oklch(args: &str) -> Result<GraphColor> {
    let parts = color_parts(args);
    if !(3..=4).contains(&parts.len()) {
        bail!("oklch requires lightness, chroma, hue, and optional alpha");
    }
    let l = if parts[0].ends_with('%') {
        percentage(&parts[0])?
    } else {
        parts[0].parse()?
    };
    let c: f32 = parts[1].parse()?;
    let h = parts[2]
        .trim_end_matches("deg")
        .parse::<f32>()?
        .to_radians();
    if !(0.0..=1.0).contains(&l) || c < 0.0 || !c.is_finite() {
        bail!("oklch lightness or chroma is outside its valid range");
    }
    let a = c * h.cos();
    let b = c * h.sin();
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.291485548 * b;
    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;
    let linear = [
        4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
        -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
        -0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3,
    ];
    let gamma = |v: f32| {
        if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    // CSS maps out-of-gamut OKLCH colors into the output gamut. A channel
    // clamp is deterministic and adequate for the native sRGB renderer.
    make_color(
        gamma(linear[0]).clamp(0.0, 1.0),
        gamma(linear[1]).clamp(0.0, 1.0),
        gamma(linear[2]).clamp(0.0, 1.0),
        parts.get(3).map(|v| alpha(v)).transpose()?.unwrap_or(1.0),
    )
}

fn percentage(value: &str) -> Result<f32> {
    Ok(value
        .strip_suffix('%')
        .ok_or_else(|| anyhow!("expected percentage"))?
        .parse::<f32>()?
        / 100.0)
}
fn alpha(value: &str) -> Result<f32> {
    if value.ends_with('%') {
        percentage(value)
    } else {
        Ok(value.parse()?)
    }
}
fn make_color(red: f32, green: f32, blue: f32, alpha: f32) -> Result<GraphColor> {
    if [red, green, blue, alpha]
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        bail!("color channel is outside its valid range");
    }
    Ok(GraphColor {
        red,
        green,
        blue,
        alpha,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct GraphNode {
    pub(crate) path: PathBuf,
    pub(crate) properties: Value,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GraphEdge {
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
}

pub(crate) fn select_nodes(
    definition: &GraphDefinition,
    candidates: impl IntoIterator<Item = GraphNode>,
) -> Result<Vec<GraphNode>> {
    let nodes: Vec<_> = candidates
        .into_iter()
        .filter(|node| matches_definition(definition, &node.path, &node.properties))
        .collect();
    if nodes.len() > definition.limit {
        bail!(
            "graph matches {} nodes, exceeding limit {}; narrow filters or raise limit",
            nodes.len(),
            definition.limit
        );
    }
    Ok(nodes)
}

pub(crate) fn deduplicate_edges(
    edges: impl IntoIterator<Item = GraphEdge>,
    selected: &HashSet<PathBuf>,
) -> Vec<GraphEdge> {
    let mut unique = HashSet::new();
    edges
        .into_iter()
        .filter(|edge| {
            edge.source != edge.target
                && selected.contains(&edge.source)
                && selected.contains(&edge.target)
                && unique.insert(edge.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_approved_definition_and_filters_typed_properties() {
        let definition = parse_definition(
            r##"
limit: 2000
filters:
  and:
    - 'file.inFolder("Inbox")'
    - 'priority >= 3'
    - 'tags.contains("project")'
groups:
  - name: Done
    filters: 'status == "done"'
    node:
      color: '#ff000080'
      size: 1.25
      border:
        color: '#112233'
        width: 1.5
      hover:
        color: '#445566'
        size: 1.5
        border:
          color: '#778899'
          width: 2.0
display:
  orphan:
    show: false
  edge:
    arrow: true
"##,
        )
        .unwrap();
        let properties: Value =
            yaml_serde::from_str("priority: 4\ntags: [project, rust]\nstatus: done").unwrap();
        assert!(matches_definition(
            &definition,
            Path::new("Inbox/Nested/Note.md"),
            &properties
        ));
        assert!(!definition.display.orphan.show);
        assert!(definition.display.edge.arrow);
        assert!(definition.display.node.propertional);
        let group_node = &definition.groups[0].node;
        assert_eq!(group_node.color.unwrap().alpha, 128.0 / 255.0);
        assert_eq!(group_node.size, Some(1.25));
        assert_eq!(group_node.border.width, Some(1.5));
        assert_eq!(group_node.hover.size, Some(1.5));
        assert_eq!(group_node.hover.border.width, Some(2.0));
        assert!(
            parse_definition("groups:\n  - name: Old\n    filters: []\n    color: '#ff0000'",)
                .is_err()
        );
        assert!(
            parse_definition(
                "groups:\n  - name: Invalid\n    filters: []\n    node:\n      propertional: false",
            )
            .is_err()
        );
    }

    #[test]
    fn supports_missing_values_nested_and_bracket_properties() {
        let properties: Value =
            yaml_serde::from_str("project:\n  owner:\n    name: Romain\nproject status: done")
                .unwrap();
        for filter in [
            "note.project.owner.name == \"Romain\"",
            "note[\"project status\"] == \"done\"",
            "missing != \"done\"",
            "missing == null",
        ] {
            let definition = parse_definition(&format!("filters: '{filter}'")).unwrap();
            assert!(
                matches_definition(&definition, Path::new("Note.md"), &properties),
                "{filter}"
            );
        }
    }

    #[test]
    fn validates_strict_schema_groups_limits_and_colors() {
        assert!(
            parse_definition("fitlers: []")
                .unwrap_err()
                .to_string()
                .contains("unknown field")
        );
        assert!(parse_definition("limit: 10001").is_err());
        assert!(parse_definition("groups:\n  - name: Empty\n    filters: []").is_err());
        assert!(
            parse_definition("groups:\n  - name: Empty\n    filters: []\n    node: {}").is_err()
        );
        assert!(
            !parse_definition("display:\n  node:\n    propertional: false")
                .unwrap()
                .display
                .node
                .propertional
        );
        assert_eq!(parse_color("#00000000").unwrap().alpha, 0.0);
        assert!(parse_color("rgb(300 0 0)").is_err());
    }

    #[test]
    fn parses_node_interaction_styles_and_physics() {
        let definition = parse_definition(
            r##"
display:
  node:
    border:
      color: '#112233'
      width: 1.5
    hover:
      color: '#445566'
      size: 1.25
      border:
        color: '#778899'
        width: 2.5
  orphan:
    node:
      border:
        width: 0.5
      hover:
        size: 1.5
physics:
  center:
    strength: 0.004
  repulsion:
    strength: 2048.0
  link:
    strength: 0.08
    distance: 96.0
"##,
        )
        .unwrap();

        assert_eq!(definition.display.node.border.width, Some(1.5));
        assert_eq!(
            definition.display.node.border.color.unwrap(),
            parse_color("#112233").unwrap()
        );
        assert_eq!(definition.display.node.hover.size, Some(1.25));
        assert_eq!(
            definition.display.node.hover.color.unwrap(),
            parse_color("#445566").unwrap()
        );
        assert_eq!(definition.display.node.hover.border.width, Some(2.5));
        assert_eq!(definition.display.orphan.node.border.width, Some(0.5));
        assert_eq!(definition.display.orphan.node.hover.size, Some(1.5));
        assert_eq!(definition.physics.center.strength, 0.004);
        assert_eq!(definition.physics.repulsion.strength, 2048.0);
        assert_eq!(definition.physics.link.strength, 0.08);
        assert_eq!(definition.physics.link.distance, 96.0);
    }

    #[test]
    fn parses_edge_hover_style() {
        let definition = parse_definition(
            r##"
display:
  edge:
    hover:
      direction:
        outgoing:
          color: '#abcdef'
          width: 2.5
        incoming:
          color: '#123456'
          width: 3.5
        both:
          color: '#fedcba'
          width: 4.5
"##,
        )
        .unwrap();

        assert_eq!(
            definition
                .display
                .edge
                .hover
                .direction
                .outgoing
                .color
                .unwrap(),
            parse_color("#abcdef").unwrap()
        );
        assert_eq!(
            definition.display.edge.hover.direction.outgoing.width,
            Some(2.5)
        );
        assert_eq!(
            definition
                .display
                .edge
                .hover
                .direction
                .incoming
                .color
                .unwrap(),
            parse_color("#123456").unwrap()
        );
        assert_eq!(
            definition.display.edge.hover.direction.incoming.width,
            Some(3.5)
        );
        assert_eq!(
            definition.display.edge.hover.direction.both.color.unwrap(),
            parse_color("#fedcba").unwrap()
        );
        assert_eq!(
            definition.display.edge.hover.direction.both.width,
            Some(4.5)
        );
        assert!(
            parse_definition(
                "display:\n  edge:\n    hover:\n      direction:\n        outgoing:\n          width: 0.1",
            )
            .is_err()
        );
        assert!(
            parse_definition(
                "display:\n  edge:\n    hover:\n      direction:\n        incoming:\n          width: 5.1",
            )
            .is_err()
        );
        assert!(
            parse_definition(
                "display:\n  edge:\n    hover:\n      direction:\n        both:\n          width: 0.1",
            )
            .is_err()
        );
        assert!(
            parse_definition("display:\n  edge:\n    hover:\n      outgoing:\n        width: 2")
                .is_err()
        );
    }

    #[test]
    fn rejects_removed_plural_orphan_and_arrow_styles() {
        assert!(parse_definition("display:\n  orphans:\n    show: false").is_err());
        assert!(parse_definition("display:\n  arrows:\n    show: true").is_err());
        assert!(parse_definition("display:\n  edge:\n    arrow:\n      color: '#abcdef'").is_err());
    }

    #[test]
    fn graph_physics_defaults_match_the_tuned_values() {
        let definition = parse_definition("").unwrap();

        assert_eq!(definition.physics.center.strength, DEFAULT_CENTER_STRENGTH);
        assert_eq!(
            definition.physics.repulsion.strength,
            DEFAULT_REPULSION_STRENGTH
        );
        assert_eq!(definition.physics.link.strength, DEFAULT_LINK_STRENGTH);
        assert_eq!(definition.physics.link.distance, DEFAULT_LINK_DISTANCE);
    }

    #[test]
    fn rejects_invalid_node_interaction_styles_and_physics() {
        for source in [
            "display:\n  node:\n    border:\n      width: -0.1",
            "display:\n  node:\n    hover:\n      size: 4.0",
            "groups:\n  - name: Invalid\n    filters: []\n    node:\n      border:\n        width: 6.0",
            "physics:\n  center:\n    strength: -0.1",
            "physics:\n  repulsion:\n    strength: .inf",
            "physics:\n  link:\n    strength: -0.1",
            "physics:\n  link:\n    distance: 0",
        ] {
            assert!(parse_definition(source).is_err(), "accepted {source}");
        }
    }

    #[test]
    fn empty_boolean_filters_have_defined_identities() {
        let properties = Value::Mapping(Default::default());
        assert!(matches_definition(
            &parse_definition("filters: []").unwrap(),
            Path::new("A.md"),
            &properties
        ));
        assert!(matches_definition(
            &parse_definition("filters:\n  and: []").unwrap(),
            Path::new("A.md"),
            &properties
        ));
        assert!(!matches_definition(
            &parse_definition("filters:\n  or: []").unwrap(),
            Path::new("A.md"),
            &properties
        ));
    }

    #[test]
    fn selects_with_limit_uses_first_group_and_deduplicates_visible_edges() {
        let definition = parse_definition(
            r##"
limit: 1
groups:
  - name: First
    filters: 'status == "done"'
    node:
      color: '#ff0000'
  - name: Second
    filters: 'status == "done"'
    node:
      color: '#00ff00'
"##,
        )
        .unwrap();
        let properties: Value = yaml_serde::from_str("status: done").unwrap();
        let node = GraphNode {
            path: PathBuf::from("A.md"),
            properties,
        };
        let nodes = select_nodes(&definition, [node]).unwrap();
        assert_eq!(
            matching_group(&definition, &nodes[0].path, &nodes[0].properties)
                .unwrap()
                .name,
            "First"
        );

        let selected = HashSet::from([PathBuf::from("A.md"), PathBuf::from("B.md")]);
        let edge = GraphEdge {
            source: PathBuf::from("A.md"),
            target: PathBuf::from("B.md"),
        };
        let self_edge = GraphEdge {
            source: PathBuf::from("A.md"),
            target: PathBuf::from("A.md"),
        };
        assert_eq!(
            deduplicate_edges([edge.clone(), edge, self_edge], &selected).len(),
            1
        );
    }
}
