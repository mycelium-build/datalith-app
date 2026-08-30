//! Graph view payload: display styles, physics forces, and classes.

use anyhow::{Result, bail};

pub const DEFAULT_CENTER_STRENGTH: f32 = 0.002;
pub const DEFAULT_REPULSION_STRENGTH: f32 = 1_024.0;
pub const DEFAULT_LINK_STRENGTH: f32 = 0.04;
pub const DEFAULT_LINK_DISTANCE: f32 = 128.0;

/// The graph-specific configuration of one `type: graph` view.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphConfig {
    pub display: GraphDisplay,
    pub physics: GraphPhysics,
    pub classes: Vec<GraphClass>,
}

/// Validates and assembles the graph payload of one view.
pub fn build(
    view_name: &str,
    display: Option<GraphDisplay>,
    physics: Option<GraphPhysics>,
    classes: Option<Vec<RawGraphClass>>,
) -> Result<crate::document::base::ViewKind> {
    let prefix = format!("view {view_name:?}");
    let display = display.unwrap_or_default();
    let physics = physics.unwrap_or_default();
    let mut class_names = std::collections::HashSet::new();
    let mut built = Vec::new();
    for class in classes.unwrap_or_default() {
        let name = class
            .name
            .ok_or_else(|| anyhow::anyhow!("{prefix}: every class must define a name"))?;
        if name.trim().is_empty() {
            bail!("{prefix}: class name must not be empty");
        }
        if !class_names.insert(name.clone()) {
            bail!("{prefix}: class name {name:?} is duplicated");
        }
        validate_style(
            class.node.size,
            &class.node.border,
            &class.node.hover,
            &format!("{prefix}.classes.{name}"),
        )?;
        if class.node == ClassStyle::default() {
            bail!("{prefix}: class {name:?} must define at least one style field");
        }
        built.push(GraphClass {
            name,
            filters: class.filters,
            node: class.node,
        });
    }
    validate_node_style(&display.node, &format!("{prefix}.display.node"))?;
    validate_edge_style(&display.edge, &format!("{prefix}.display.edge"))?;
    validate_node_style(
        &display.orphan.node,
        &format!("{prefix}.display.orphan.node"),
    )?;
    validate_non_negative(
        physics.center.strength,
        &format!("{prefix}.physics.center.strength"),
    )?;
    validate_non_negative(
        physics.repulsion.strength,
        &format!("{prefix}.physics.repulsion.strength"),
    )?;
    validate_non_negative(
        physics.link.strength,
        &format!("{prefix}.physics.link.strength"),
    )?;
    validate_positive(
        physics.link.distance,
        &format!("{prefix}.physics.link.distance"),
    )?;
    Ok(crate::document::base::ViewKind::Graph(GraphConfig {
        display,
        physics,
        classes: built,
    }))
}

fn validate_edge_style(style: &EdgeStyle, name: &str) -> Result<()> {
    validate_range(style.width, 0.5, 5.0, &format!("{name}.width"))?;
    for (label, hover) in [
        ("outgoing", &style.hover.direction.outgoing),
        ("incoming", &style.hover.direction.incoming),
        ("both", &style.hover.direction.both),
    ] {
        validate_range(
            hover.width,
            0.5,
            5.0,
            &format!("{name}.hover.{label}.width"),
        )?;
    }
    Ok(())
}

fn validate_node_style(style: &NodeStyle, name: &str) -> Result<()> {
    validate_style(style.size, &style.border, &style.hover, name)
}

fn validate_style(
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

fn validate_range(value: Option<f32>, minimum: f32, maximum: f32, name: &str) -> Result<()> {
    if value.is_some_and(|value| !value.is_finite() || value < minimum || value > maximum) {
        bail!("{name} must be between {minimum} and {maximum}");
    }
    Ok(())
}

mod color;
mod types;

pub use color::parse_color;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_view(source: &str) -> Result<crate::document::base::BaseDefinition> {
        crate::document::base::BaseDefinition::parse(source)
    }

    #[test]
    fn graph_views_parse_display_physics_and_classes() {
        let definition = parse_view(
            r##"
views:
  - type: graph
    name: Wiki
    limit: 100
    filters: 'file.inFolder("Notes")'
    display:
      node:
        color: oklch(70% 0.1 250)
        size: 1.5
      edge:
        arrow: true
        width: 2.0
      orphan:
        show: false
    physics:
      repulsion:
        strength: 512
    classes:
      - name: Projects
        filters: 'file.hasTag("project")'
        node:
          color: "#ff8800"
          border:
            width: 2.5
"##,
        )
        .unwrap();
        let crate::document::base::ViewKind::Graph(config) = &definition.views[0].kind else {
            panic!("expected graph kind");
        };
        assert_eq!(config.classes.len(), 1);
        assert_eq!(config.classes[0].name, "Projects");
        assert_eq!(config.display.node.size, Some(1.5));
        assert!(config.display.edge.arrow);
        assert!(!config.display.orphan.show);
        assert!((config.physics.repulsion.strength - 512.0).abs() < f32::EPSILON);
    }

    #[test]
    fn graph_legend_defaults_on_and_can_be_disabled() {
        let enabled = parse_view("views:\n  - type: graph\n    name: G").unwrap();
        let crate::document::base::ViewKind::Graph(config) = &enabled.views[0].kind else {
            panic!("expected graph kind");
        };
        assert!(config.display.legend);
        let disabled =
            parse_view("views:\n  - type: graph\n    name: G\n    display:\n      legend: false")
                .unwrap();
        let crate::document::base::ViewKind::Graph(config) = &disabled.views[0].kind else {
            panic!("expected graph kind");
        };
        assert!(!config.display.legend);
    }

    #[test]
    fn graph_classes_require_unique_names_and_styles() {
        assert!(parse_view(
            "views:\n  - type: graph\n    name: G\n    classes:\n      - name: A\n        filters: 'true'\n        node:\n          size: 1.2\n      - name: A\n        filters: 'true'\n        node:\n          size: 1.4",
        )
        .is_err());
        assert!(parse_view(
            "views:\n  - type: graph\n    name: G\n    classes:\n      - name: A\n        filters: 'true'",
        )
        .is_err());
        assert!(parse_view(
            "views:\n  - type: graph\n    name: G\n    classes:\n      - filters: 'true'\n        node:\n          size: 1.2",
        )
        .is_err());
    }

    #[test]
    fn graph_styles_reject_out_of_range_values() {
        for setting in [
            "display:\n  node:\n    size: 9",
            "display:\n  edge:\n    width: 9",
            "physics:\n  repulsion:\n    strength: -1",
            "physics:\n  link:\n    distance: 0",
        ] {
            let source = format!("views:\n  - type: graph\n    name: G\n    {setting}");
            assert!(parse_view(&source).is_err(), "{setting}");
        }
    }

    #[test]
    fn graph_defaults_apply_when_blocks_are_absent() {
        let definition = parse_view("views:\n  - type: graph\n    name: G").unwrap();
        let crate::document::base::ViewKind::Graph(config) = &definition.views[0].kind else {
            panic!("expected graph kind");
        };
        assert!(config.classes.is_empty());
        assert!(config.display.orphan.show);
        assert!((config.physics.center.strength - DEFAULT_CENTER_STRENGTH).abs() < f32::EPSILON);
    }
}
