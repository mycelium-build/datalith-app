use conv::{ConvUtil, UnwrapOrInf};
use gpui::{
    App, BorderStyle, Bounds, Corners, Edges, Element, ElementId, Hsla, IntoElement, LayoutId,
    Length, Pixels, Size, Style, Window, point, px, quad, size, solid_background,
    transparent_black,
};

/// The monolith mark source art, in the tier color encoding
/// (`M` border, `1` slightly whiter, `2` closest to white, `I` inscription, `0` top side)
/// on a 32×32 canvas.
pub const LOGO_SRC: &str = include_str!("../../assets/datalith.txt");

/// Whitening applied to the primary color for the monolith's two sides.
/// Values mirror `scripts/app_icon.py` (`RIGHT_SIDE_WHITEN` / `LEFT_SIDE_WHITEN`);
/// keep both in sync so the app icon and the in-app mark match.
pub const RIGHT_SIDE_WHITEN: f32 = 0.35;
pub const LEFT_SIDE_WHITEN: f32 = 0.7;

pub fn whiten(color: Hsla, amount: f32) -> Hsla {
    Hsla {
        h: color.h,
        s: color.s * (1.0 - amount),
        l: (1.0 - color.l).mul_add(amount, color.l),
        a: color.a,
    }
}

#[must_use]
pub fn tier_color(tier: Tier, primary: Hsla) -> Hsla {
    match tier {
        Tier::Light => primary,
        Tier::RightSide => whiten(primary, RIGHT_SIDE_WHITEN),
        Tier::LeftSide => whiten(primary, LEFT_SIDE_WHITEN),
        Tier::Inscription | Tier::Top => gpui::white(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Light,
    RightSide,
    LeftSide,
    Inscription,
    Top,
}

#[derive(Clone)]
pub struct Cell {
    pub col: f32,
    pub row: f32,
    pub tier: Tier,
}

#[derive(Clone)]
pub struct LogoGrid {
    pub width: f32,
    pub height: f32,
    pub cells: Vec<Cell>,
}

pub fn parse_logo(source: &str) -> LogoGrid {
    let mut cells = Vec::new();
    let mut width: usize = 0;
    let mut height: usize = 0;
    for line in source.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut row_width: usize = 0;
        for (col, ch) in line.chars().enumerate() {
            let tier = match ch {
                'M' => Some(Tier::Light),
                '1' => Some(Tier::RightSide),
                '2' => Some(Tier::LeftSide),
                'I' => Some(Tier::Inscription),
                '0' => Some(Tier::Top),
                _ => None,
            };
            if let Some(tier) = tier {
                cells.push(Cell {
                    col: col.approx_as::<f32>().unwrap_or_inf(),
                    row: height.approx_as::<f32>().unwrap_or_inf(),
                    tier,
                });
            }
            row_width = row_width.max(col.saturating_add(1));
        }
        width = width.max(row_width);
        height = height.saturating_add(1);
    }
    LogoGrid {
        width: width.approx_as::<f32>().unwrap_or_inf(),
        height: height.approx_as::<f32>().unwrap_or_inf(),
        cells,
    }
}

/// Render monolith mark at the given cell size.
pub fn monolith_mark(cell: f32, color: Hsla) -> MonolithMark {
    MonolithMark::new(parse_logo(LOGO_SRC), cell, color)
}

pub struct MonolithMark {
    logo: LogoGrid,
    cell: f32,
    color: Hsla,
}

impl MonolithMark {
    pub const fn new(logo: LogoGrid, cell: f32, color: Hsla) -> Self {
        Self { logo, cell, color }
    }
}

impl IntoElement for MonolithMark {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for MonolithMark {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let width: Length = px(self.logo.width * self.cell).into();
        let height: Length = px(self.logo.height * self.cell).into();
        let style = Style {
            size: Size::new(width, height),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let origin_x = bounds.origin.x.as_f32();
        let origin_y = bounds.origin.y.as_f32();
        for cell in &self.logo.cells {
            let color = tier_color(cell.tier, self.color);
            let x = cell.col.mul_add(self.cell, origin_x);
            let y = cell.row.mul_add(self.cell, origin_y);
            let cell_bounds = Bounds::new(point(px(x), px(y)), size(px(self.cell), px(self.cell)));
            window.paint_quad(quad(
                cell_bounds,
                Corners::default(),
                solid_background(color),
                Edges::default(),
                transparent_black(),
                BorderStyle::default(),
            ));
        }
    }
}
