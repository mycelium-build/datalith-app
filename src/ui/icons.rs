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
    Settings,
}

impl DatalithIcon {
    const fn asset(self) -> (&'static str, &'static str) {
        match self {
            Self::Note => icon_asset!("icons/note.svg"),
            Self::Todo => icon_asset!("icons/todo.svg"),
            Self::Graph => icon_asset!("icons/graph.svg"),
            Self::Image => icon_asset!("icons/image.svg"),
            Self::File => icon_asset!("icons/file.svg"),
            Self::Pen => icon_asset!("icons/pen.svg"),
            Self::Funnel => icon_asset!("icons/funnel.svg"),
            Self::ArrowUpAz => icon_asset!("icons/arrow-up-a-z.svg"),
            Self::ArrowDownAz => icon_asset!("icons/arrow-down-a-z.svg"),
            Self::Settings => icon_asset!("icons/gear.svg"),
        }
    }
}

impl IconNamed for DatalithIcon {
    fn path(self) -> SharedString {
        self.asset().0.into()
    }
}

pub const ICON_ASSETS: &[(&str, &str)] = &[
    // Custom icons, resolved through `DatalithIcon`.
    DatalithIcon::Note.asset(),
    DatalithIcon::Todo.asset(),
    DatalithIcon::Graph.asset(),
    DatalithIcon::Image.asset(),
    DatalithIcon::File.asset(),
    DatalithIcon::Pen.asset(),
    DatalithIcon::Funnel.asset(),
    DatalithIcon::ArrowUpAz.asset(),
    DatalithIcon::ArrowDownAz.asset(),
    DatalithIcon::Settings.asset(),
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
