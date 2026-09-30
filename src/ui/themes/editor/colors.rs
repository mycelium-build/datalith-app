//! Human-facing navigation for the complete color schema. Keys come from the
//! serialized dependency types, so optional colors are never omitted here.
pub(super) const CORE_GROUP: &str = "Base colors";
pub(super) const ESSENTIAL_GROUPS: &[&str] = &[
    "All colors",
    CORE_GROUP,
    "Primary & selection",
    "Secondary & accent",
    "Text & surfaces",
];

pub(super) fn essential_group(token: &str) -> &'static str {
    match token {
        "primary.foreground"
        | "primary.hover.background"
        | "primary.active.background"
        | "link"
        | "selection.background"
        | "list.active.background"
        | "list.active.border" => "Primary & selection",
        "secondary.foreground"
        | "secondary.hover.background"
        | "secondary.active.background"
        | "accent.background"
        | "accent.foreground"
        | "list.hover.background"
        | "scrollbar.thumb.background" => "Secondary & accent",
        "muted.foreground"
        | "input.border"
        | "ring"
        | "list.background"
        | "list.head.background" => "Text & surfaces",
        _ => CORE_GROUP,
    }
}

pub(super) const GROUPS: &[&str] = &[
    "All areas",
    "Workspace",
    "Navigation",
    "Buttons",
    "Inputs & selection",
    "Lists & tables",
    "Feedback",
    "Source editor",
    "Syntax",
    "Base palette",
    "Other components",
];

pub(super) const FAMILIES: &[&str] = &[
    "All families",
    "General",
    "Sidebar",
    "Tabs",
    "Title bar",
    "Window",
    "Scrollbars",
    "Buttons",
    "Inputs & selection",
    "Links",
    "Menus & popovers",
    "Lists",
    "Tables",
    "Feedback",
    "Charts",
    "Status bar",
    "Source editor",
    "Syntax",
    "Palette",
    "Other components",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ColorOrigin {
    ThemeDefined,
    ComponentDefault,
}

impl ColorOrigin {
    pub(super) const ALL: [Self; 2] = [Self::ThemeDefined, Self::ComponentDefault];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::ThemeDefined => "theme-defined",
            Self::ComponentDefault => "component-default",
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::ThemeDefined => "Theme defined",
            Self::ComponentDefault => "Component Default",
        }
    }
}

pub(super) const fn origin(theme_defined: bool) -> ColorOrigin {
    if theme_defined {
        ColorOrigin::ThemeDefined
    } else {
        ColorOrigin::ComponentDefault
    }
}

pub(super) fn family(token: &str) -> &'static str {
    if token.starts_with("highlight:syntax.") {
        return "Syntax";
    }
    if token.starts_with("highlight:") {
        return "Source editor";
    }
    match token.split('.').next().unwrap_or_default() {
        "base" => "Palette",
        "sidebar" => "Sidebar",
        "tab" | "tab_bar" => "Tabs",
        "title_bar" => "Title bar",
        "window" => "Window",
        "scrollbar" => "Scrollbars",
        "button" | "primary" | "secondary" => "Buttons",
        "input" | "caret" | "ring" | "selection" | "switch" | "slider" => "Inputs & selection",
        "link" => "Links",
        "popover" | "overlay" | "accent" => "Menus & popovers",
        "list" => "Lists",
        "table" => "Tables",
        "danger" | "info" | "warning" | "success" | "progress" => "Feedback",
        "chart" | "chart_bullish" | "chart_bearish" => "Charts",
        "status_bar" => "Status bar",
        "background" | "foreground" | "border" | "muted" | "drag" | "drop_target" => "General",
        _ => "Other components",
    }
}

pub(super) fn group(token: &str) -> &'static str {
    if token.starts_with("highlight:syntax.") {
        return "Syntax";
    }
    if token.starts_with("highlight:") {
        return "Source editor";
    }
    match token.split('.').next().unwrap_or_default() {
        "base" => "Base palette",
        "sidebar" | "tab" | "tab_bar" | "title_bar" | "scrollbar" => "Navigation",
        "button" | "primary" | "secondary" => "Buttons",
        "input" | "caret" | "ring" | "selection" | "switch" | "slider" => "Inputs & selection",
        "list" | "table" => "Lists & tables",
        "danger" | "info" | "warning" | "success" | "progress" => "Feedback",
        "background" | "foreground" | "border" | "muted" | "accent" | "popover" | "overlay"
        | "link" | "drag" | "drop_target" => "Workspace",
        _ => "Other components",
    }
}

pub(super) fn label(token: &str) -> String {
    let token = token
        .strip_prefix("highlight:syntax.")
        .or_else(|| token.strip_prefix("highlight:"))
        .unwrap_or(token);
    token
        .split('.')
        .map(|part| {
            let label = match part {
                "foreground" => "text",
                "active" => "selected / pressed",
                "head" => "header",
                "foot" => "footer",
                "ring" => "focus outline",
                "caret" => "text cursor",
                "base" => "palette",
                other => other,
            }
            .replace('_', " ");
            let mut chars = label.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(super) fn description(token: &str) -> &'static str {
    if let Some(syntax) = token.strip_prefix("highlight:syntax.") {
        return syntax_description(syntax);
    }
    if let Some(description) = general_role_description(token) {
        return description;
    }
    match token {
        "highlight:editor.background" => {
            "Source editor surface; separate from the rendered note background"
        }
        "highlight:editor.foreground" => {
            "Stored but not applied by the current editor; use Text (foreground) instead"
        }
        "highlight:editor.active_line.background" => {
            "Source editor: background of the line containing the cursor"
        }
        "highlight:editor.invisible" => "Source editor: whitespace markers when enabled",
        "highlight:editor.line_number"
        | "highlight:editor.active_line_number"
        | "highlight:editor.gutter.background" => {
            "Stored for the editor; line numbers are currently hidden in Datalith"
        }
        "group_box.title.foreground" => {
            "Accepted by the schema, but not applied by the component library"
        }
        "window.border" => "Window border on Linux only; no effect on macOS",
        "tab.background" => {
            "Stored by the schema; the current tab strip uses tab_bar.background and tab.active.*"
        }
        "link" | "link.active" | "link.hover" => "Rendered Markdown note, property and Base links",
        _ if token.starts_with("highlight:") => {
            "Source diagnostics when provided; Datalith currently has no diagnostic provider"
        }
        _ if token.starts_with("sidebar.primary.") => {
            "Stored by the schema; Datalith file rows use list.* colors"
        }
        _ if token.starts_with("sidebar.accent.") => {
            "Stored by the schema; Datalith file rows use list.* colors"
        }
        _ if token.starts_with("sidebar.") => "File sidebar panel, text, border and vault label",
        _ if token.starts_with("tab") => "Workspace tabs and the tab strip",
        _ if token.starts_with("title_bar.") => "The window title bar and its border",
        _ if token.starts_with("scrollbar.") => "Scroll track and draggable thumb",
        _ if token.starts_with("button.") => {
            "Button-specific override; takes precedence over its matching accent color"
        }
        _ if token.starts_with("primary.") => {
            "Primary action hover and pressed states; button-specific overrides take precedence"
        }
        _ if token.starts_with("secondary.") => {
            "Secondary button hover and pressed states; component-specific overrides take precedence"
        }
        "table.head.background" | "table.head.foreground" => {
            "Headers in rendered Markdown and Base tables"
        }
        "table.foot.background" | "table.foot.foreground" => {
            "Base table footer when summaries are shown"
        }
        "table.active.background"
        | "table.active.border"
        | "table.even.background"
        | "table.hover.background" => {
            "DataTable state color; not used by Datalith's current table views"
        }
        "table.background" => "Surface of rendered Markdown tables",
        "table.row.border" => "Row dividers in rendered Markdown and Shortcuts tables",
        "list.hover.background" => {
            "Hovered items in Select and List components, including file rows"
        }
        "list.active.background" => {
            "Selected Todo tasks and GPUI List items, including selected file rows"
        }
        _ if token.starts_with("list.") => "GPUI List component colors",
        _ if token.starts_with("base.") => {
            "Color-picker palette; also used by component fallbacks when specific colors are unset"
        }
        _ if token.starts_with("popover.") => "Floating menus and popovers",
        "overlay" => "The dimmed backdrop behind dialogs",
        "selection.background" => "Selected text in input fields and source editors",
        "caret" => "The blinking text cursor",
        "ring" => "Keyboard focus outline around controls",
        "input.border" => "Borders of text fields and select controls",
        _ if token.starts_with("switch.") => "Toggle switch track and thumb",
        _ if token.starts_with("slider.") => "Slider track and thumb, including display scale",
        _ if group(token) == "Feedback" => {
            "Status messages, progress indicators and semantic action colors"
        }
        _ if group(token) == "Other components" => {
            "Component-library setting; this component is not currently shown in Datalith"
        }
        _ => "Shared workspace color; specific component colors can override it",
    }
}

fn syntax_description(syntax: &str) -> &'static str {
    match syntax {
        "keyword" => "Source syntax: keywords such as let, if and return",
        "comment" | "comment_doc" => "Source syntax: comments and documentation",
        "string" | "text.literal" | "text.code.span" => {
            "Source syntax: quoted text and literal values"
        }
        "number" | "boolean" | "constant" => "Source syntax: numbers, true / false and constants",
        "function" | "constructor" => "Source syntax: function names and constructors",
        "link_text" => "Markdown source: the visible text of a link",
        "link_uri" => "Markdown source: the destination of a link",
        "title" => "Markdown source: heading text",
        "emphasis" | "emphasis.strong" => "Markdown source: emphasized text",
        _ => "Source syntax: used when the language grammar recognizes this token",
    }
}

fn general_role_description(token: &str) -> Option<&'static str> {
    match token {
        "primary.background" => Some("Primary action background"),
        "primary.foreground" => Some("Text on primary actions; fallback for status text"),
        "secondary.background" => Some("Secondary action background"),
        "secondary.foreground" => Some(
            "Text on secondary buttons and supporting surfaces; button-specific overrides take precedence",
        ),
        "accent.background" => Some(
            "Menus and selected items; list.hover.background and scrollbar.thumb.background can use this when unset",
        ),
        "accent.foreground" => {
            Some("Text on accent surfaces; component-specific foreground roles take precedence")
        }
        _ => None,
    }
}
