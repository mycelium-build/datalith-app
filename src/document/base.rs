use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use yaml_serde::Value;

use crate::document::filter::{Filter, PropertyPath, parse_property};
use crate::vault::CatalogFilter;

pub const HARD_RESULT_LIMIT: usize = 50_000;

#[derive(Clone, Debug, PartialEq)]
pub struct BaseDefinition {
    pub(crate) filters: Filter,
    pub(crate) properties: BTreeMap<String, PropertyConfig>,
    pub(crate) views: Vec<BaseView>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropertyConfig {
    pub(crate) display_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BaseView {
    pub(crate) view_type: ViewType,
    pub(crate) name: String,
    pub(crate) filters: Filter,
    pub(crate) limit: Option<usize>,
    pub(crate) order: Vec<DisplayProperty>,
    pub(crate) sort: Vec<SortRule>,
    pub(crate) markers: ListMarkers,
    pub(crate) indent_properties: bool,
    pub(crate) separators: String,
    pub(crate) row_height: TableRowHeight,
    pub(crate) image: Option<DisplayProperty>,
    pub(crate) image_fit: CardImageFit,
    pub(crate) image_aspect_ratio: f32,
    pub(crate) card_size: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayProperty {
    pub(crate) source: String,
    pub(crate) path: PropertyPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortRule {
    pub(crate) source: String,
    pub(crate) path: PropertyPath,
    pub(crate) direction: SortDirection,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ViewType {
    List,
    Table,
    Cards,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CardImageFit {
    #[default]
    Cover,
    Contain,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ListMarkers {
    #[default]
    Bullets,
    Numbers,
    None,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum TableRowHeight {
    #[serde(rename = "short")]
    Short,
    #[default]
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "tall")]
    Tall,
    #[serde(rename = "extra tall")]
    ExtraTall,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    #[serde(rename = "ASC")]
    Asc,
    #[serde(rename = "DESC")]
    Desc,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawBaseDefinition {
    filters: Filter,
    properties: BTreeMap<String, RawPropertyConfig>,
    views: Option<Vec<RawBaseView>>,
    formulas: Option<Value>,
    summaries: Option<Value>,
}

impl Default for RawBaseDefinition {
    fn default() -> Self {
        Self {
            filters: Filter::MatchAll,
            properties: BTreeMap::new(),
            views: None,
            formulas: None,
            summaries: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawPropertyConfig {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawBaseView {
    #[serde(rename = "type")]
    view_type: Option<ViewType>,
    name: Option<String>,
    filters: Filter,
    limit: Option<usize>,
    order: Vec<String>,
    sort: Vec<RawSortRule>,
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
    #[serde(rename = "groupBy")]
    group_by: Option<Value>,
    summaries: Option<Value>,
}

impl Default for RawBaseView {
    fn default() -> Self {
        Self {
            view_type: None,
            name: None,
            filters: Filter::MatchAll,
            limit: None,
            order: Vec::new(),
            sort: Vec::new(),
            markers: None,
            indent_properties: None,
            separators: None,
            row_height: None,
            image: None,
            image_fit: None,
            image_aspect_ratio: None,
            card_size: None,
            group_by: None,
            summaries: None,
        }
    }
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
        if raw.formulas.is_some() {
            bail!("formulas are not supported yet");
        }
        if raw.summaries.is_some() {
            bail!("summaries are not supported yet");
        }

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
            .map(|view| {
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
                if view.group_by.is_some() {
                    bail!("groupBy is not supported yet");
                }
                if view.summaries.is_some() {
                    bail!("view summaries are not supported yet");
                }
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
                let markers = view.markers.unwrap_or_default();
                let indent_properties = view.indent_properties.unwrap_or(false);
                let has_separators = view.separators.is_some();
                let separators = view.separators.unwrap_or_else(|| ", ".into());
                if separators.is_empty() {
                    bail!("view {name:?}.separators must not be empty");
                }
                let row_height = view.row_height.unwrap_or_default();
                let image = view.image.as_deref().map(display_property).transpose()?;
                let image_fit = view.image_fit.unwrap_or_default();
                let image_aspect_ratio = view.image_aspect_ratio.unwrap_or(1.0);
                if !image_aspect_ratio.is_finite() || image_aspect_ratio <= 0.0 {
                    bail!("view {name:?}.imageAspectRatio must be greater than 0");
                }
                let card_size = view.card_size.unwrap_or(200.0);
                if !card_size.is_finite() || card_size <= 0.0 {
                    bail!("view {name:?}.cardSize must be greater than 0");
                }
                if view_type == ViewType::Table
                    && (view.markers.is_some()
                        || view.indent_properties.is_some()
                        || has_separators)
                {
                    bail!("list settings are only supported by list views");
                }
                if view_type == ViewType::List && view.row_height.is_some() {
                    bail!("rowHeight is only supported by table views");
                }
                if view_type != ViewType::Cards
                    && (view.image.is_some()
                        || view.image_fit.is_some()
                        || view.image_aspect_ratio.is_some()
                        || view.card_size.is_some())
                {
                    bail!("card settings are only supported by card views");
                }
                if view_type == ViewType::Cards
                    && (view.markers.is_some()
                        || view.indent_properties.is_some()
                        || has_separators
                        || view.row_height.is_some())
                {
                    bail!("list and table settings are not supported by card views");
                }
                Ok(BaseView {
                    view_type,
                    name,
                    filters: view.filters,
                    limit: view.limit,
                    order,
                    sort,
                    markers,
                    indent_properties,
                    separators,
                    row_height,
                    image,
                    image_fit,
                    image_aspect_ratio,
                    card_size,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            filters: raw.filters,
            properties,
            views,
        })
    }

    pub(crate) fn display_name<'a>(&'a self, property: &'a DisplayProperty) -> &'a str {
        self.properties
            .get(&property.source)
            .and_then(|config| config.display_name.as_deref())
            .unwrap_or(&property.source)
    }

    pub(crate) fn catalog_filter(&self, view: &BaseView) -> CatalogFilter {
        CatalogFilter::And(vec![
            self.filters.to_catalog_filter(),
            view.filters.to_catalog_filter(),
        ])
    }
}

fn display_property(source: &str) -> Result<DisplayProperty> {
    let path = validate_property_source(source)?;
    Ok(DisplayProperty {
        source: source.to_string(),
        path,
    })
}

fn validate_property_source(source: &str) -> Result<PropertyPath> {
    if source == "formula" || source.starts_with("formula.") {
        bail!("formula properties are not supported yet");
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_views_with_defaults_and_explicit_display_configuration() {
        let definition = BaseDefinition::parse(
            r#"
properties:
  file.name:
    displayName: Note
views:
  - type: list
    name: Reading
    order: [file.name, status]
    markers: numbers
    indentProperties: true
    separators: " | "
  - type: table
    name: Recent
    rowHeight: tall
    sort:
      - property: file.mtime
        direction: DESC
"#,
        )
        .unwrap();

        assert_eq!(definition.views.len(), 2);
        assert_eq!(definition.views[0].order[0].source, "file.name");
        assert_eq!(definition.views[0].markers, ListMarkers::Numbers);
        assert!(definition.views[0].indent_properties);
        assert_eq!(definition.views[1].row_height, TableRowHeight::Tall);
        assert_eq!(definition.views[1].sort[0].direction, SortDirection::Desc);
        assert_eq!(
            definition.display_name(&definition.views[0].order[0]),
            "Note"
        );
    }

    #[test]
    fn defaults_order_to_file_name_and_rejects_unsupported_sections() {
        let definition = BaseDefinition::parse("views:\n  - type: table\n    name: All").unwrap();
        assert_eq!(definition.views[0].order[0].source, "file.name");
        assert!(BaseDefinition::parse("formulas:\n  score: price\nviews: []").is_err());
        assert!(
            BaseDefinition::parse(
                "views:\n  - type: table\n    name: A\n  - type: list\n    name: A"
            )
            .is_err()
        );
    }

    #[test]
    fn requires_supported_named_views_and_valid_limits() {
        for source in [
            "filters: []",
            "views: []",
            "views:\n  - type: list\n    name: List\n    limit: 0",
            "views:\n  - type: table\n    name: Table\n    markers: bullets",
        ] {
            assert!(BaseDefinition::parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn parses_card_view_settings_and_rejects_them_on_other_views() {
        let definition = BaseDefinition::parse(
            r#"
views:
  - type: cards
    name: Gallery
    order: [file.name, cover, description]
    image: cover
    imageFit: contain
    imageAspectRatio: 1.5
    cardSize: 240
"#,
        )
        .unwrap();

        let view = &definition.views[0];
        assert_eq!(view.view_type, ViewType::Cards);
        assert_eq!(view.image.as_ref().unwrap().source, "cover");
        assert_eq!(view.image_fit, CardImageFit::Contain);
        assert_eq!(view.image_aspect_ratio, 1.5);
        assert_eq!(view.card_size, 240.0);
        assert!(
            BaseDefinition::parse("views:\n  - type: table\n    name: Table\n    cardSize: 240")
                .is_err()
        );
    }
}
