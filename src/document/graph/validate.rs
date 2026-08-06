use anyhow::{Result, anyhow, bail};

use super::types::*;
use super::{HARD_NODE_LIMIT, GraphDefinition};

pub(crate) fn parse_definition(source: &str) -> Result<GraphDefinition> {
    let definition: GraphDefinition = if source.trim().is_empty() {
        GraphDefinition::default()
    } else {
        yaml_serde::from_str(source).map_err(|error| anyhow!(format_yaml_error(error)))?
    };
    if !(1..=HARD_NODE_LIMIT).contains(&definition.limit) {
        bail!("limit must be between 1 and {HARD_NODE_LIMIT}");
    }

    let mut names = std::collections::HashSet::new();
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::color::parse_color;
    use super::super::matches_definition;
    use std::path::Path;
    use yaml_serde::Value;

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
    fn validates_strict_schema_groups_limits_and_colors() {
        assert!(
            parse_definition("fitlers: []")
                .unwrap_err()
                .to_string()
                .contains("unknown field")
        );
        assert!(parse_definition("limit: 50001").is_err());
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

        assert_eq!(definition.physics.center.strength, super::super::DEFAULT_CENTER_STRENGTH);
        assert_eq!(
            definition.physics.repulsion.strength,
            super::super::DEFAULT_REPULSION_STRENGTH
        );
        assert_eq!(definition.physics.link.strength, super::super::DEFAULT_LINK_STRENGTH);
        assert_eq!(definition.physics.link.distance, super::super::DEFAULT_LINK_DISTANCE);
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
}
