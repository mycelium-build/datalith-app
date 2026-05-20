pub(crate) const INDEX_WRITER_BUDGET: usize = 50_000_000;

pub(crate) const MAX_SEARCH_RESULTS: usize = 25;
pub(crate) const MIN_SEARCH_QUERY_LENGTH: usize = 3;

pub(crate) const DEFAULT_FILE_NAME: &str = "untitled.md";
pub(crate) const DEFAULT_FOLDER_NAME: &str = "untitled";
pub(crate) const UNKNOWN_NAME: &str = "Unknown";
pub(crate) const VAULT_SELECT_MARKER: &str = "__open_new__";

pub(crate) const SUPPORTED_EXTENSIONS: &[&str] = &["txt", "md"];

pub(crate) const PALETTE_WIDTH: f32 = 600.0;
pub(crate) const PALETTE_ITEM_HEIGHT: f32 = 28.0;
pub(crate) const PALETTE_MAX_HEIGHT: f32 = 400.0;
pub(crate) const SIDEBAR_WIDTH: f32 = 260.0;

pub(crate) const DRAG_HOVER_EXPAND_DELAY_MS: u64 = 800;
pub(crate) const TREE_INDENT_PX: f32 = 16.0;
pub(crate) const TREE_PADDING_PX: f32 = 12.0;

pub(crate) const BORDER_WIDTH: f32 = 2.0;
pub(crate) const ICON_PADDING: f32 = 4.0;

pub(crate) const BASE_FONT_SIZE: f64 = 16.0;

pub(crate) const MD_LINE_HEIGHT: f32 = 1.6;
pub(crate) const MD_LIST_INDENT: &str = "  ";

pub(crate) const MD_HEADING_SIZES: [f32; 6] = [2.25, 2.0, 1.75, 1.5, 1.0, 1.25];
pub(crate) const MD_HEADING_MARGIN: f32 = 2.0;

pub(crate) const MD_CODE_FONT_SCALE: f32 = 0.9;
pub(crate) const MD_CODE_PADDING: f32 = 3.0;
pub(crate) const MD_CODE_RADIUS: f32 = 3.0;
pub(crate) const MD_CODE_BLOCK_PADDING: f32 = 3.0;
pub(crate) const MD_CODE_BLOCK_RADIUS: f32 = 4.0;

pub(crate) const MD_FRONTMATTER_FONT_SCALE: f32 = 0.75;
pub(crate) const MD_FRONTMATTER_PADDING: f32 = 3.0;
pub(crate) const MD_FRONTMATTER_RADIUS: f32 = 6.0;
pub(crate) const MD_FRONTMATTER_MARGIN: f32 = 3.0;

pub(crate) const MD_BLOCKQUOTE_PADDING: f32 = 4.0;
pub(crate) const MD_BLOCKQUOTE_BORDER: f32 = 3.0;
