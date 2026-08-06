use serde::Deserialize;

use super::color::parse_color;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GraphDisplay {
    pub(crate) node: NodeStyle,
    pub(crate) edge: EdgeStyle,
    pub(crate) orphan: OrphanStyle,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct NodeStyle {
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
pub struct BorderStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) width: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct HoverStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) size: Option<f32>,
    pub(crate) border: BorderStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) width: Option<f32>,
    pub(crate) arrow: bool,
    pub(crate) hover: EdgeHoverStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeHoverStyle {
    pub(crate) direction: EdgeHoverDirectionStyles,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeHoverDirectionStyles {
    pub(crate) outgoing: DirectionalEdgeHoverStyle,
    pub(crate) incoming: DirectionalEdgeHoverStyle,
    pub(crate) both: DirectionalEdgeHoverStyle,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DirectionalEdgeHoverStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) width: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct OrphanStyle {
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

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GroupNodeStyle {
    pub(crate) color: Option<GraphColor>,
    pub(crate) size: Option<f32>,
    pub(crate) border: BorderStyle,
    pub(crate) hover: HoverStyle,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GraphPhysics {
    pub(crate) center: CenterForce,
    pub(crate) repulsion: RepulsionForce,
    pub(crate) link: LinkForce,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CenterForce {
    pub(crate) strength: f32,
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
    pub(crate) strength: f32,
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
    pub(crate) strength: f32,
    pub(crate) distance: f32,
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
    pub(crate) red: f32,
    pub(crate) green: f32,
    pub(crate) blue: f32,
    pub(crate) alpha: f32,
}

impl<'de> Deserialize<'de> for GraphColor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        parse_color(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
