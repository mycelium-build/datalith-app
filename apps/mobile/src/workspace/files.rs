use super::{Command, Workspace, name};
use datalith_ui::fonts::ResolvedFonts;
use datalith_vault::{file_ops, source};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement, Styled, Window,
    div, prelude::*, px,
};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Create,
    Folder,
    Rename,
    Move,
}
impl Action {
    const fn label(self) -> &'static str {
        match self {
            Self::Create => "New note",
            Self::Folder => "New folder",
            Self::Rename => "Rename",
            Self::Move => "Move",
        }
    }
    const fn title(self) -> &'static str {
        self.label()
    }
    const fn submit(self) -> &'static str {
        match self {
            Self::Create | Self::Folder => "Create",
            Self::Rename => "Rename",
            Self::Move => "Move here",
        }
    }
    fn default_name(self, target: &Path) -> String {
        match self {
            Self::Create => "New Note".into(),
            Self::Folder => "New Folder".into(),
            Self::Rename => name(target),
            Self::Move => String::new(),
        }
    }
}
pub(super) struct FileForm {
    action: Action,
    target: PathBuf,
    directory: PathBuf,
    input: Entity<InputState>,
    directories: Vec<PathBuf>,
    error: Option<String>,
    _enter: gpui_kit::Subscription,
}

/// A single, visible file or folder name.
fn validate_name(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        return Err("Enter a name");
    }
    if text.starts_with('.') {
        return Err("Names starting with “.” are hidden");
    }
    match Path::new(text).components().collect::<Vec<_>>().as_slice() {
        [Component::Normal(name)] if *name == text => Ok(()),
        _ => Err("Enter a single name without “/”"),
    }
}

/// The part of a name to select for replacement: the stem of a file name.
fn selection_for(name: &str) -> std::ops::Range<usize> {
    let end = Path::new(name)
        .file_stem()
        .filter(|_| Path::new(name).extension().is_some())
        .map_or(name.len(), std::ffi::OsStr::len);
    0..end
}

/// "Vault" for the root, otherwise the vault-relative path.
fn location(root: &Path, dir: &Path) -> String {
    match dir.strip_prefix(root) {
        Ok(relative) if !relative.as_os_str().is_empty() => {
            let parts: Vec<_> = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            format!("Vault / {}", parts.join(" / "))
        }
        _ => "Vault".into(),
    }
}
impl Workspace {
    pub(crate) fn file_menu(
        path: &Path,
        directory: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let path = path.to_owned();
        Button::new(gpui_kit::SharedString::from(format!(
            "actions:{}",
            path.display()
        )))
        .ghost()
        .w(px(48.))
        .h(px(48.))
        .text_color(cx.theme().muted_foreground)
        .icon(IconName::Ellipsis)
        .accessibility_label(format!("Actions for {}", name(&path)))
        .on_click(cx.listener(move |s, _, w, cx| {
            let choice = super::Choice::Actions {
                title: name(&path),
                target: Some(path.clone()),
                commands: Command::for_file(directory),
            };
            s.choose(choice, w, cx);
        }))
    }

    pub(super) fn perform_file_action(
        &mut self,
        path: PathBuf,
        command: Command,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if command == Command::Reveal {
            self.reveal(&path, w, cx);
            return;
        }
        if !self
            .root
            .as_ref()
            .is_some_and(|r| path.starts_with(r) && !source::is_read_only(r))
        {
            return;
        }
        match command {
            Command::Rename => self.begin_form(Action::Rename, path, w, cx),
            Command::Move => self.begin_form(Action::Move, path, w, cx),
            Command::Delete => self.remove_file(&path, cx),
            Command::Duplicate => match file_ops::duplicate(&path) {
                Ok(p) => self.changed_files(vec![p], cx),
                Err(e) => {
                    self.message = Some(e.to_string());
                    cx.notify();
                }
            },
            _ => {}
        }
    }

    /// Creates in the folder on screen, or in the open document's folder.
    pub(super) fn begin_creation(&mut self, folder: bool, w: &mut Window, cx: &mut Context<Self>) {
        let action = if folder {
            Action::Folder
        } else {
            Action::Create
        };
        if let Some(dir) = self.folders.last() {
            self.begin_form(action, dir.path.clone(), w, cx);
        }
    }
    pub(super) fn create_button(folder: bool, cx: &Context<Self>) -> Button {
        let action = if folder {
            Action::Folder
        } else {
            Action::Create
        };
        Button::new(action.label())
            .ghost()
            .h(px(48.))
            .label(action.label())
            .on_click(cx.listener(move |s, _, w, cx| {
                s.begin_creation(action == Action::Folder, w, cx);
            }))
    }
    fn begin_form(
        &mut self,
        action: Action,
        target: PathBuf,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if source::is_read_only(&root) || !target.starts_with(&root) {
            return;
        }
        let directory = match action {
            Action::Move | Action::Rename => target.parent().unwrap_or(&root).to_owned(),
            Action::Create | Action::Folder => target.clone(),
        };
        let value = action.default_name(&target);
        let input = cx.new(|cx| {
            InputState::new(w, cx)
                .default_value(value.clone())
                .placeholder("Name")
        });
        let enter = cx.subscribe_in(&input, w, |s, _, event, w, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                s.submit_file(w, cx);
            }
        });
        self.file_form = Some(FileForm {
            action,
            target,
            directory: directory.clone(),
            input: input.clone(),
            directories: Vec::new(),
            error: None,
            _enter: enter,
        });
        self.advance(false);
        if action == Action::Move {
            self.move_directory(directory, cx);
        } else {
            // Select the stem so typing replaces the name and keeps ".md".
            let stem = selection_for(&value);
            input.update(cx, |i, cx| i.set_selected_range(stem, cx));
            input.focus_handle(cx).focus(w, cx);
            w.request_virtual_keyboard();
        }
        cx.notify();
    }
    fn move_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(form) = self.file_form.as_mut() else {
            return;
        };
        match source::read_dir(&path) {
            Ok(entries) => {
                form.directory = path;
                form.directories = entries
                    .into_iter()
                    .filter(|p| {
                        source::is_dir(p)
                            && !p.is_symlink()
                            && !name(p).starts_with('.')
                            && !p.starts_with(&form.target)
                    })
                    .collect();
                form.directories.sort();
            }
            Err(err) => form.error = Some(err.to_string()),
        }
        cx.notify();
    }
    pub(super) fn render_form(
        &self,
        form: &FileForm,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut body = v_flex()
            .id("file-form")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_3()
            .py_2()
            .gap_2()
            .child(
                div()
                    .font_family(cx.global::<ResolvedFonts>().headings.clone())
                    .text_xl()
                    .child(form.action.title()),
            );
        let root = self.root.clone().unwrap_or_default();
        if form.action == Action::Move {
            body = body
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("Move “{}” to", name(&form.target))),
                )
                .child(div().text_lg().child(location(&root, &form.directory)));
            if root != form.directory
                && let Some(parent) = form.directory.parent()
            {
                let path = parent.to_owned();
                body = body.child(
                    Button::new("up-destination")
                        .ghost()
                        .h(px(48.))
                        .icon(IconName::ChevronLeft)
                        .label("Enclosing folder")
                        .on_click(
                            cx.listener(move |s, _, _, cx| s.move_directory(path.clone(), cx)),
                        ),
                );
            }
            for dir in &form.directories {
                let path = dir.clone();
                body = body.child(
                    Button::new(gpui_kit::SharedString::from(format!(
                        "move:{}",
                        dir.display()
                    )))
                    .ghost()
                    .h(px(48.))
                    .icon(IconName::Folder)
                    .label(name(dir))
                    .on_click(cx.listener(move |s, _, _, cx| s.move_directory(path.clone(), cx))),
                );
            }
        } else {
            body = body.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("In {}", location(&root, &form.directory))),
            );
            body = body.child(Input::new(&form.input));
            #[cfg(target_os = "ios")]
            {
                body = body.child(crate::ios_input::registration(&form.input));
            }
        }
        if let Some(error) = &form.error {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        }
        let unchanged =
            form.action == Action::Move && form.target.parent() == Some(&form.directory);
        // Pinned under the fields, so it stays above the keyboard.
        let footer = h_flex()
            .gap_2()
            .px_3()
            .py_2()
            .justify_end()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("cancel-file")
                    .ghost()
                    .h(px(44.))
                    .label("Cancel")
                    .on_click(cx.listener(|s, _, w, cx| s.back(w, cx))),
            )
            .child(
                Button::new("file-submit")
                    .primary()
                    .h(px(44.))
                    .label(form.action.submit())
                    .disabled(unchanged)
                    .on_click(cx.listener(|s, _, w, cx| s.submit_file(w, cx))),
            );
        v_flex().size_full().child(body).child(footer)
    }

    fn submit_file(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &mut self.file_form else {
            return;
        };
        let text = form.input.read(cx).value().trim().to_owned();
        if form.action != Action::Move
            && let Err(e) = validate_name(&text)
        {
            form.error = Some(e.into());
            cx.notify();
            return;
        }
        let mut destination = form.directory.join(if form.action == Action::Move {
            name(&form.target)
        } else {
            text
        });
        if form.action == Action::Create && destination.extension().is_none() {
            destination.set_extension("md");
        }
        if destination == form.target {
            self.back(w, cx);
            return;
        }
        if source::exists(&destination) {
            form.error = Some(format!("“{}” already exists here", name(&destination)));
            cx.notify();
            return;
        }
        let old = form.target.clone();
        let action = form.action;
        let result = match action {
            Action::Create => {
                file_ops::create(&form.directory, &name(&destination), "").map(|_| ())
            }
            Action::Folder => source::create_dir(&destination).map_err(Into::into),
            Action::Rename | Action::Move => {
                let Some(c) = &self.catalog else { return };
                file_ops::rename(c, &old, &destination).map(|_| ())
            }
        };
        if let Err(err) = result {
            if let Some(form) = &mut self.file_form {
                form.error = Some(err.to_string());
            }
            cx.notify();
            return;
        }
        if matches!(action, Action::Rename | Action::Move) {
            self.forget_files(&old);
        }
        self.page = None;
        self.file_form = None;
        self.message = None;
        w.dismiss_virtual_keyboard();
        self.changed_files(vec![old, destination.clone()], cx);
        if action == Action::Create {
            self.select(destination, w, cx);
            // A new note is for writing: open it in Edit with the keyboard.
            if self
                .current()
                .is_some_and(|d| !d.handler.read(cx).is_editing())
            {
                self.toggle_mode(w, cx);
            }
        }
    }
    fn forget_files(&mut self, target: &Path) {
        self.documents.retain(|path, _| !path.starts_with(target));
        if self.active.as_ref().is_some_and(|p| p.starts_with(target)) {
            self.active = None;
        }
    }
    pub(super) fn changed_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if let Some(c) = &self.catalog {
            c.refresh_paths(paths);
        }
        self.persist_session(cx);
        self.refresh_folder();
        self.search_changed(cx);
        cx.notify();
    }
    pub(super) fn remove_file(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(root) = &self.root else { return };
        match file_ops::delete(root, path) {
            Ok(deleted) => {
                self.deleted_document = self
                    .current()
                    .filter(|d| d.path.starts_with(path))
                    .map(|d| d.path.clone());
                self.forget_files(path);
                self.deleted = Some(deleted);
                self.changed_files(vec![path.to_owned()], cx);
            }
            Err(err) => {
                self.message = Some(err.to_string());
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_must_be_single_and_visible() {
        assert!(validate_name("Ideas").is_ok());
        assert!(validate_name("Ideas.md").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name(".hidden").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("..").is_err());
    }

    #[test]
    fn renaming_selects_the_stem_only() {
        assert_eq!(selection_for("Welcome.md"), 0..7);
        assert_eq!(selection_for("New Note"), 0..8);
        assert_eq!(selection_for("archive.tar.gz"), 0..11);
    }

    #[test]
    fn locations_read_from_the_vault_root() {
        let root = Path::new("/v");
        assert_eq!(location(root, root), "Vault");
        assert_eq!(location(root, Path::new("/v/a/b")), "Vault / a / b");
    }

    #[test]
    fn new_notes_start_with_a_replaceable_name() {
        assert_eq!(Action::Create.default_name(Path::new("/v")), "New Note");
        assert_eq!(
            Action::Rename.default_name(Path::new("/v/Idea.md")),
            "Idea.md"
        );
        assert_eq!(Action::Create.submit(), "Create");
        assert_eq!(Action::Move.submit(), "Move here");
    }
}
