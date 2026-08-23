//! YAML parsing and validation for Base definitions.

use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, anyhow, bail};
use serde::Deserialize;

use super::{
    AggregateFn, BaseDefinition, BaseView, CardImageFit, CustomSummary, DisplayProperty, GroupRule,
    HARD_RESULT_LIMIT, ListMarkers, PostFn, PropertyConfig, RawGraphClass, RawGraphDisplay,
    RawGraphPhysics, SortDirection, SortRule, Summary, TableRowHeight, ViewType,
};
use crate::document::expr::{Expr, PropertyRef};
use crate::document::filter::{Filter, PropertyPath, parse_property};

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawBaseDefinition {
    filters: Filter,
    formulas: BTreeMap<String, String>,
    summaries: BTreeMap<String, String>,
    properties: BTreeMap<String, RawPropertyConfig>,
    views: Option<Vec<RawBaseView>>,
}

impl Default for RawBaseDefinition {
    fn default() -> Self {
        Self {
            filters: Filter::MatchAll,
            formulas: BTreeMap::new(),
            summaries: BTreeMap::new(),
            properties: BTreeMap::new(),
            views: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawPropertyConfig {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawGroupBy {
    property: Option<String>,
    direction: SortDirection,
}

/// Every view shares the selection fields;
/// kind-specific keys are extracted by the matching module in `document/base/`
/// and silently ignored elsewhere.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawBaseView {
    #[serde(rename = "type")]
    view_type: Option<ViewType>,
    name: Option<String>,
    filters: Filter,
    limit: Option<usize>,
    order: Vec<String>,
    sort: Vec<RawSortRule>,
    #[serde(rename = "groupBy")]
    group_by: Option<RawGroupBy>,
    summaries: BTreeMap<String, String>,
    markers: Option<ListMarkers>,
    #[serde(rename = "indentProperties")]
    indent_properties: Option<bool>,
    separators: Option<String>,
    #[serde(rename = "rowHeight")]
    row_height: Option<TableRowHeight>,
    image: Option<String>,
    #[serde(rename = "imageFit")]
    image_fit: Option<CardImageFit>,
    #[serde(rename = "imageAspectRatio")]
    image_aspect_ratio: Option<f32>,
    #[serde(rename = "cardSize")]
    card_size: Option<f32>,
    display: Option<RawGraphDisplay>,
    physics: Option<RawGraphPhysics>,
    classes: Option<Vec<RawGraphClass>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSortRule {
    property: Option<String>,
    #[serde(default)]
    direction: SortDirection,
}

impl BaseDefinition {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn parse(source: &str) -> Result<Self> {
        let raw: RawBaseDefinition = if source.trim().is_empty() {
            RawBaseDefinition::default()
        } else {
            yaml_serde::from_str(source).map_err(|error| anyhow!(format_yaml_error(&error)))?
        };

        let formulas = parse_formulas(&raw.formulas)?;
        let summaries = raw
            .summaries
            .iter()
            .map(|(name, body)| Ok((name.clone(), parse_custom_summary(body)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;

        let views = raw
            .views
            .ok_or_else(|| anyhow!("views must contain at least one view"))?;
        if views.is_empty() {
            bail!("views must contain at least one view");
        }

        let properties = raw
            .properties
            .into_iter()
            .map(|(source, config)| {
                validate_property_source(&source)?;
                if config
                    .display_name
                    .as_deref()
                    .is_some_and(|name| name.trim().is_empty())
                {
                    bail!("displayName for {source:?} must not be empty");
                }
                Ok((
                    source,
                    PropertyConfig {
                        display_name: config.display_name,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        let mut names = HashSet::new();
        let views = views
            .into_iter()
            .map(|view| Self::parse_view(view, &formulas, &summaries, &mut names))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            filters: raw.filters,
            formulas,
            summaries,
            properties,
            views,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn parse_view(
        view: RawBaseView,
        _formulas: &BTreeMap<String, Expr>,
        summaries: &BTreeMap<String, CustomSummary>,
        names: &mut HashSet<String>,
    ) -> Result<BaseView> {
        let name = view
            .name
            .ok_or_else(|| anyhow!("view name must not be empty"))?;
        let name = name.trim().to_string();
        if name.is_empty() {
            bail!("view name must not be empty");
        }
        if !names.insert(name.clone()) {
            bail!("view name {name:?} is duplicated");
        }
        let view_type = view
            .view_type
            .ok_or_else(|| anyhow!("view {name:?} must define type"))?;
        validate_limit(view.limit, &format!("view {name:?}.limit"))?;
        let order = if view.order.is_empty() {
            vec![display_property("file.name")?]
        } else {
            view.order
                .iter()
                .map(|source| display_property(source))
                .collect::<Result<Vec<_>>>()?
        };
        let sort = view
            .sort
            .into_iter()
            .map(|sort| {
                let source = sort
                    .property
                    .ok_or_else(|| anyhow!("sort property must not be empty"))?;
                let path = validate_property_source(&source)?;
                Ok(SortRule {
                    source,
                    path,
                    direction: sort.direction,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let group_by = match view.group_by {
            Some(raw) => {
                let source = raw
                    .property
                    .ok_or_else(|| anyhow!("view {name:?}.groupBy must declare a property"))?;
                let property = display_property(&source)?;
                Some(GroupRule {
                    property,
                    direction: raw.direction,
                })
            }
            None => None,
        };
        let group_direction = group_by
            .as_ref()
            .map_or(SortDirection::Asc, |rule| rule.direction);
        let resolved_summaries = view
            .summaries
            .iter()
            .map(|(source, summary)| {
                validate_property_source(source)?;
                resolve_summary(summary, summaries)?;
                Ok((source.clone(), summary.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        // Kind settings are extracted by their owning module;
        // anything a different kind declares is silently ignored.
        let kind = match view_type {
            ViewType::List => {
                super::list::build(view.markers, view.indent_properties, view.separators, &name)?
            }
            ViewType::Table => super::table::build(view.row_height),
            ViewType::Cards => super::cards::build(
                view.image,
                view.image_fit,
                view.image_aspect_ratio,
                view.card_size,
            )?,
            ViewType::Graph => {
                super::graph::build(&name, view.display, view.physics, view.classes)?
            }
        };
        Ok(BaseView {
            view_type,
            name,
            filters: view.filters,
            limit: view.limit,
            order,
            sort,
            group_by,
            group_direction,
            summaries: resolved_summaries,
            kind,
        })
    }
}

fn parse_formulas(formulas: &BTreeMap<String, String>) -> Result<BTreeMap<String, Expr>> {
    let parsed = formulas
        .iter()
        .map(|(name, body)| {
            if name.trim().is_empty() {
                bail!("formula names must not be empty");
            }
            let expr = Expr::parse(body)?;
            Ok((name.clone(), expr))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    check_formula_cycles(&parsed)?;
    Ok(parsed)
}

fn check_formula_cycles(formulas: &BTreeMap<String, Expr>) -> Result<()> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        InProgress,
        Done,
    }
    fn visit(
        name: &str,
        formulas: &BTreeMap<String, Expr>,
        marks: &mut BTreeMap<String, Mark>,
        stack: &mut Vec<String>,
    ) -> Result<()> {
        match marks.get(name) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::InProgress) => {
                stack.push(name.to_string());
                bail!("circular formula reference: {}", stack.join(" -> "));
            }
            None => {}
        }
        marks.insert(name.to_string(), Mark::InProgress);
        stack.push(name.to_string());
        let Some(expr) = formulas.get(name) else {
            bail!("formula {name:?} references an undeclared formula");
        };
        let mut references = Vec::new();
        walk_formula_refs(expr, &mut |reference| {
            references.push(reference.to_string());
        });
        for reference in references {
            visit(&reference, formulas, marks, stack)?;
        }
        stack.pop();
        marks.insert(name.to_string(), Mark::Done);
        Ok(())
    }
    let mut marks = BTreeMap::new();
    let mut stack = Vec::new();
    for name in formulas.keys() {
        visit(name, formulas, &mut marks, &mut stack)?;
    }
    Ok(())
}

fn walk_formula_refs(expr: &Expr, visitor: &mut impl FnMut(&str)) {
    match expr {
        Expr::Null
        | Expr::Bool(_)
        | Expr::Number(_)
        | Expr::Text(_)
        | Expr::Property(PropertyRef::Note(_) | PropertyRef::File(_)) => {}
        Expr::Property(PropertyRef::Formula(name)) => visitor(name),
        Expr::Not(inner) | Expr::Neg(inner) => walk_formula_refs(inner, visitor),
        Expr::Arithmetic { left, right, .. }
        | Expr::Compare { left, right, .. }
        | Expr::Logic { left, right, .. } => {
            walk_formula_refs(left, visitor);
            walk_formula_refs(right, visitor);
        }
        Expr::Call(_, args) => {
            for arg in args {
                walk_formula_refs(arg, visitor);
            }
        }
        Expr::Method { subject, args, .. } => {
            walk_formula_refs(subject, visitor);
            for arg in args {
                walk_formula_refs(arg, visitor);
            }
        }
    }
}

fn parse_custom_summary(body: &str) -> Result<CustomSummary> {
    let expr = Expr::parse(body)?;
    let mut posts = Vec::new();
    let mut current = &expr;
    loop {
        let Expr::Method {
            subject,
            name,
            args,
        } = current
        else {
            bail!(
                "custom summaries must aggregate the values keyword, \
                 e.g. values.mean().round(3)"
            );
        };
        match name.as_str() {
            "round" => {
                let amount = match args.first() {
                    Some(Expr::Number(value)) if value.fract() == 0.0 && *value >= 0.0 => {
                        // The guard above guarantees an integer-valued f64.
                        value.to_string().parse::<u32>().ok()
                    }
                    _ => None,
                };
                let digits = amount.ok_or_else(|| anyhow!(".round requires a whole number"))?;
                posts.push(PostFn::Round(digits));
            }
            "abs" => posts.push(PostFn::Abs),
            "mean" | "min" | "max" | "sum" | "count" => {
                match subject.as_ref() {
                    Expr::Property(crate::document::expr::PropertyRef::Note(parts))
                        if parts.as_slice() == ["values"] => {}
                    _ => bail!("summary aggregates must operate on values"),
                }
                let aggregate = match name.as_str() {
                    "mean" => AggregateFn::Mean,
                    "min" => AggregateFn::Min,
                    "max" => AggregateFn::Max,
                    "sum" => AggregateFn::Sum,
                    _ => AggregateFn::Count,
                };
                posts.reverse();
                return Ok(CustomSummary {
                    aggregate,
                    post: posts,
                });
            }
            other => bail!("unsupported summary function .{other}"),
        }
        current = subject.as_ref();
    }
}

fn resolve_summary(name: &str, custom: &BTreeMap<String, CustomSummary>) -> Result<()> {
    if Summary::named(name).is_some() || custom.contains_key(name) {
        return Ok(());
    }
    bail!("unknown summary {name:?}");
}

pub(super) fn display_property(source: &str) -> Result<DisplayProperty> {
    let path = validate_property_source(source)?;
    Ok(DisplayProperty {
        source: source.to_string(),
        path,
    })
}

pub(super) fn validate_property_source(source: &str) -> Result<PropertyPath> {
    parse_property(source).map_err(|error| anyhow!("invalid property {source:?}: {error}"))
}

fn validate_limit(limit: Option<usize>, name: &str) -> Result<()> {
    if limit.is_some_and(|limit| !(1..=HARD_RESULT_LIMIT).contains(&limit)) {
        bail!("{name} must be between 1 and {HARD_RESULT_LIMIT}");
    }
    Ok(())
}

fn format_yaml_error(error: &yaml_serde::Error) -> String {
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
