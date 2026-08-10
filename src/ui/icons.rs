use gpui::SharedString;
use gpui_component::IconNamed;

// Pixel-art icons shipped by Datalith.
// Paths that collide with gpui-component's lucide icons override them
// (the same key is resolved by `DatalithAssets::load` before falling back to the component assets),
// and the remaining paths add icons gpui-component does not ship.

macro_rules! icon_asset {
    ($path:literal) => {
        ($path, include_str!(concat!("../../assets/", $path)))
    };
}

pub const ICON_ASSETS: &[(&str, &str)] = &[
    // Custom icons, referenced through `DatalithIcon`.
    icon_asset!("icons/note.svg"),
    icon_asset!("icons/todo.svg"),
    icon_asset!("icons/graph.svg"),
    icon_asset!("icons/image.svg"),
    icon_asset!("icons/file.svg"),
    icon_asset!("icons/pen.svg"),
    icon_asset!("icons/funnel.svg"),
    icon_asset!("icons/arrow-up-a-z.svg"),
    icon_asset!("icons/arrow-down-a-z.svg"),
    // Overrides of gpui-component's lucide icons.
    icon_asset!("icons/search.svg"),
    icon_asset!("icons/layout-dashboard.svg"),
    icon_asset!("icons/folder.svg"),
    icon_asset!("icons/folder-open.svg"),
    icon_asset!("icons/arrow-up.svg"),
    icon_asset!("icons/arrow-down.svg"),
    icon_asset!("icons/arrow-left.svg"),
    icon_asset!("icons/arrow-right.svg"),
    icon_asset!("icons/chevron-up.svg"),
    icon_asset!("icons/chevron-down.svg"),
    icon_asset!("icons/chevron-left.svg"),
    icon_asset!("icons/chevron-right.svg"),
    icon_asset!("icons/plus.svg"),
    icon_asset!("icons/close.svg"),
    icon_asset!("icons/check.svg"),
    icon_asset!("icons/eye.svg"),
    icon_asset!("icons/inbox.svg"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatalithIcon {
    Note,
    Todo,
    Graph,
    Image,
    File,
    Pen,
    Funnel,
    ArrowUpAz,
    ArrowDownAz,
}

/// Paths must stay in sync with `ICON_ASSETS`.
impl IconNamed for DatalithIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Note => "icons/note.svg",
            Self::Todo => "icons/todo.svg",
            Self::Graph => "icons/graph.svg",
            Self::Image => "icons/image.svg",
            Self::File => "icons/file.svg",
            Self::Pen => "icons/pen.svg",
            Self::Funnel => "icons/funnel.svg",
            Self::ArrowUpAz => "icons/arrow-up-a-z.svg",
            Self::ArrowDownAz => "icons/arrow-down-a-z.svg",
        }
        .into()
    }
}
