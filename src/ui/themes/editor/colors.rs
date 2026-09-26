//! Human-facing navigation for the complete color schema. Keys come from the
//! serialized dependency types, so optional colors are never omitted here.
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
        return match syntax {
            "keyword" => "Source syntax: keywords such as let, if and return",
            "comment" | "comment_doc" => "Source syntax: comments and documentation",
            "string" | "text.literal" | "text.code.span" => {
                "Source syntax: quoted text and literal values"
            }
            "number" | "boolean" | "constant" => {
                "Source syntax: numbers, true / false and constants"
            }
            "function" | "constructor" => "Source syntax: function names and constructors",
            "link_text" => "Markdown source: the visible text of a link",
            "link_uri" => "Markdown source: the destination of a link",
            "title" => "Markdown source: heading text",
            "emphasis" | "emphasis.strong" => "Markdown source: emphasized text",
            _ => "Source syntax: used when the language grammar recognizes this token",
        };
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
        "link" | "link.active" | "link.hover" => {
            "Link components; rendered note links use Primary accent instead"
        }
        _ if token.starts_with("highlight:") => {
            "Source diagnostics when provided; Datalith currently has no diagnostic provider"
        }
        _ if token.starts_with("sidebar.primary.") || token.starts_with("sidebar.accent.") => {
            "Library Sidebar states; Datalith file rows use the List colors instead"
        }
        _ if token.starts_with("sidebar.") => "File navigation: panel, text and item states",
        _ if token.starts_with("tab") => "Workspace tabs and the tab strip",
        _ if token.starts_with("title_bar.") => "The window title bar and its border",
        _ if token.starts_with("scrollbar.") => "Scroll track and draggable thumb",
        _ if token.starts_with("button.") => {
            "Button-specific override; takes precedence over its matching accent color"
        }
        _ if token.starts_with("primary.") => {
            "Primary actions and their hover / pressed states; also used by note links"
        }
        _ if token.starts_with("secondary.") => "Secondary actions and supporting surfaces",
        _ if token.starts_with("table.") => {
            "Table components in rendered notes and shortcuts; Base views use shared workspace colors"
        }
        _ if token.starts_with("list.") => "Menus, file lists and selection rows",
        _ if token.starts_with("base.") => {
            "Base palette used by default component colors; explicit component overrides take precedence"
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
