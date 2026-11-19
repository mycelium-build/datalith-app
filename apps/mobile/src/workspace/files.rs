use super::{Workspace, name};
use datalith_vault::{file_ops, source};
use gpui_kit::component::{
    IconName,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
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
}
pub(super) struct FileForm {
    action: Action,
    target: PathBuf,
    directory: PathBuf,
    input: Entity<InputState>,
    directories: Vec<PathBuf>,
}
impl Workspace {
    pub(crate) fn file_menu(path: &Path, cx: &Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.weak_entity();
        let path = path.to_owned();
        Button::new(format!("actions-{}", path.display()))
            .ghost()
            .w(px(48.))
            .h(px(48.))
            .icon(IconName::Ellipsis)
            .accessibility_label(format!("Actions for {}", name(&path)))
            .dropdown_menu(move |mut menu, _, _| {
                for action in [Action::Rename, Action::Move] {
                    let state = entity.clone();
                    let path = path.clone();
                    menu = menu.item(PopupMenuItem::new(action.label()).on_click(
                        move |_, w, cx| {
                            let _ =
                                state.update(cx, |s, cx| s.begin_form(action, path.clone(), w, cx));
                        },
                    ));
                }
                let duplicate = path.clone();
                let state = entity.clone();
                let target = path.clone();
                let owner = entity.clone();
                menu.item(PopupMenuItem::new("Duplicate").on_click(move |_, _, cx| {
                    let _ = state.update(cx, |s, cx| match file_ops::duplicate(&duplicate) {
                        Ok(p) => s.changed_files(vec![p], cx),
                        Err(err) => {
                            s.message = Some(err.to_string());
                            cx.notify();
                        }
                    });
                }))
                .item(PopupMenuItem::new("Delete").on_click(move |_, _, cx| {
                    let _ = owner.update(cx, |s, cx| s.remove_file(&target, cx));
                }))
            })
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
                if let Some(dir) = s.folders.last() {
                    s.begin_form(action, dir.path.clone(), w, cx);
                }
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
        let directory = if action == Action::Move {
            root
        } else if action == Action::Rename {
            target.parent().unwrap_or(&target).to_owned()
        } else {
            target.clone()
        };
        let value = if action == Action::Rename {
            name(&target)
        } else {
            String::new()
        };
        let input = cx.new(|cx| {
            InputState::new(w, cx)
                .default_value(value)
                .placeholder("Name")
        });
        self.file_form = Some(FileForm {
            action,
            target,
            directory: directory.clone(),
            input: input.clone(),
            directories: Vec::new(),
        });
        if action == Action::Move {
            self.move_directory(directory, cx);
        } else {
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
            Err(err) => self.message = Some(err.to_string()),
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
            .size_full()
            .overflow_y_scroll()
            .px_3()
            .gap_2()
            .child(div().text_xl().child(form.action.label()));
        if form.action == Action::Move {
            body = body.child(
                form.directory
                    .strip_prefix(self.root.as_deref().unwrap_or(&form.directory))
                    .unwrap_or(&form.directory)
                    .display()
                    .to_string(),
            );
            if self.root.as_ref() != Some(&form.directory)
                && let Some(parent) = form.directory.parent()
            {
                let path = parent.to_owned();
                body = body.child(
                    Button::new("up-destination")
                        .ghost()
                        .h(px(48.))
                        .label("Parent folder")
                        .on_click(
                            cx.listener(move |s, _, _, cx| s.move_directory(path.clone(), cx)),
                        ),
                );
            }
            for dir in &form.directories {
                let path = dir.clone();
                body = body.child(
                    Button::new(format!("move-{}", dir.display()))
                        .ghost()
                        .h(px(48.))
                        .icon(IconName::Folder)
                        .label(name(dir))
                        .on_click(
                            cx.listener(move |s, _, _, cx| s.move_directory(path.clone(), cx)),
                        ),
                );
            }
        } else {
            body = body.child(Input::new(&form.input));
            #[cfg(target_os = "ios")]
            {
                body = body.child(crate::ios_input::registration(&form.input));
            }
        }
        body.child(
            Button::new("file-submit")
                .h(px(48.))
                .label(if form.action == Action::Move {
                    "Move here"
                } else {
                    "Save"
                })
                .on_click(cx.listener(|s, _, w, cx| s.submit_file(w, cx))),
        )
        .child(
            Button::new("cancel-file")
                .ghost()
                .h(px(48.))
                .label("Cancel")
                .on_click(cx.listener(|s, _, w, cx| s.back(w, cx))),
        )
    }

    fn submit_file(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.file_form else { return };
        let text = form.input.read(cx).value().trim().to_owned();
        if form.action != Action::Move
            && (text.starts_with('.')
                || !matches!(Path::new(&text).components().collect::<Vec<_>>().as_slice(),[Component::Normal(name)] if *name==text.as_str()))
        {
            self.message = Some("Enter a single file or folder name".into());
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
        if source::exists(&destination) {
            self.message = Some("That name is already in use".into());
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
            self.message = Some(err.to_string());
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
        }
    }
    fn forget_files(&mut self, target: &Path) {
        self.documents.retain(|path, _| !path.starts_with(target));
        self.history.retain(|path| !path.starts_with(target));
    }
    pub(super) fn changed_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if let Some(c) = &self.catalog {
            c.refresh_paths(paths);
        }
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
