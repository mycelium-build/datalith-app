//! Style, force, and class types for graph views.

use serde::Deserialize;

use super::parse_color;
use crate::document::filter::Filter;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GraphDisplay {
    pub node: NodeStyle,
    pub edge: EdgeStyle,
    pub orphan: OrphanStyle,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct NodeStyle {
    pub color: Option<GraphColor>,
    pub size: Option<f32>,
    pub propertional: bool,
    pub border: BorderStyle,
    pub hover: HoverStyle,
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
pub struct BorderStyle {
    pub color: Option<GraphColor>,
    pub width: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct HoverStyle {
    pub color: Option<GraphColor>,
    pub size: Option<f32>,
    pub border: BorderStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeStyle {
    pub color: Option<GraphColor>,
    pub width: Option<f32>,
    pub arrow: bool,
    pub hover: EdgeHoverStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeHoverStyle {
    pub direction: EdgeHoverDirectionStyles,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeHoverDirectionStyles {
    pub outgoing: DirectionalEdgeHoverStyle,
    pub incoming: DirectionalEdgeHoverStyle,
    pub both: DirectionalEdgeHoverStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DirectionalEdgeHoverStyle {
    pub color: Option<GraphColor>,
    pub width: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct OrphanStyle {
    pub show: bool,
    pub node: NodeStyle,
}

impl Default for OrphanStyle {
    fn default() -> Self {
        Self {
            show: true,
            node: NodeStyle::default(),
        }
    }
}

/// One named class of a graph view: a filter paired with the node style
/// applied to notes matching it first.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphClass {
    pub name: String,
    pub filters: Filter,
    pub node: ClassStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RawGraphClass {
    pub(super) name: Option<String>,
    pub(super) filters: Filter,
    pub(super) node: ClassStyle,
}

/// The node style override declared by a class; unset fields inherit from the view's `display.node`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ClassStyle {
    pub color: Option<GraphColor>,
    pub size: Option<f32>,
    pub border: BorderStyle,
    pub hover: HoverStyle,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GraphPhysics {
    pub center: CenterForce,
    pub repulsion: RepulsionForce,
    pub link: LinkForce,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CenterForce {
    pub strength: f32,
}

impl Default for CenterForce {
    fn default() -> Self {
        Self {
            strength: super::DEFAULT_CENTER_STRENGTH,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RepulsionForce {
    pub strength: f32,
}

impl Default for RepulsionForce {
    fn default() -> Self {
        Self {
            strength: super::DEFAULT_REPULSION_STRENGTH,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LinkForce {
    pub strength: f32,
    pub distance: f32,
}

impl Default for LinkForce {
    fn default() -> Self {
        Self {
            strength: super::DEFAULT_LINK_STRENGTH,
            distance: super::DEFAULT_LINK_DISTANCE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl<'de> Deserialize<'de> for GraphColor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        parse_color(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
