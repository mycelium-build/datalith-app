//! Store vault-relative routes so sandbox relocation does not invalidate a session.
use super::{ViewMode, Workspace};
use datalith_vault::source;
use gpui_kit::{Context, Window, point, px};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct Route {
    folder: PathBuf,
    document: Option<PathBuf>,
    query: String,
    /// Written for earlier builds, which searched the folder by default.
    whole: bool,
    #[serde(default)]
    folder_only: bool,
    scroll: f32,
}
#[derive(Default, Clone, Serialize, Deserialize)]
struct DocumentState {
    editing: bool,
    selection: std::ops::Range<usize>,
    reversed: bool,
    scroll: [f32; 2],
}
#[derive(Default, Clone, Serialize, Deserialize)]
pub(super) struct Session {
    current: Route,
    back: Vec<Route>,
    forward: Vec<Route>,
    documents: BTreeMap<PathBuf, DocumentState>,
    /// Vault-relative, most recently used first.
    #[serde(default)]
    recent: Vec<PathBuf>,
}

fn inside(root: &Path, relative: &Path) -> Option<PathBuf> {
    relative
        .components()
        .all(|part| matches!(part,Component::Normal(n) if n!=".datalith"))
        .then(|| root.join(relative))
}
impl Workspace {
    fn session_key(&self) -> Option<String> {
        let root = self.root.as_ref()?;
        Some(if self.local_root.as_ref() == Some(root) {
            "local".into()
        } else {
            root.to_string_lossy().into_owned()
        })
    }
    pub(super) fn route(&self, cx: &Context<Self>) -> Option<Route> {
        let root = self.root.as_ref()?;
        let folder = self.folders.last()?;
        Some(Route {
            folder: folder.path.strip_prefix(root).ok()?.into(),
            document: self
                .active
                .as_ref()
                .and_then(|p| p.strip_prefix(root).ok())
                .map(Path::to_owned),
            query: self.search.query(cx),
            whole: !self.search.folder_only,
            folder_only: self.search.folder_only,
            scroll: folder.scroll_y(),
        })
    }
    pub(super) fn record_route(&mut self, previous: Option<Route>, cx: &Context<Self>) {
        if self.restoring {
            return;
        }
        let current = self.route(cx);
        self.history.record(previous, current.as_ref());
    }
    pub(super) fn persist_session(&mut self, cx: &Context<Self>) {
        if self.restoring {
            return;
        }
        let Some((key, current)) = self.session_key().zip(self.route(cx)) else {
            return;
        };
        let Some(root) = &self.root else {
            return;
        };
        let documents = self
            .documents
            .iter()
            .filter_map(|(p, d)| {
                let h = d.handler.read(cx);
                let mut state = DocumentState {
                    editing: h.is_editing(),
                    ..Default::default()
                };
                if let Some(i) = h.input() {
                    let i = i.read(cx);
                    state.selection = i.selected_range();
                    state.reversed = i.is_selection_reversed();
                    let offset = i.scroll_offset();
                    state.scroll = [offset.x.into(), offset.y.into()];
                }
                Some((p.strip_prefix(root).ok()?.to_owned(), state))
            })
            .collect();
        self.preferences.sessions.insert(
            key,
            Session {
                current,
                documents,
                recent: self
                    .search
                    .recent
                    .iter()
                    .filter_map(|p| p.strip_prefix(root).ok().map(Path::to_owned))
                    .collect(),
                back: self.history.backward.clone(),
                forward: self.history.forward.clone(),
            },
        );
        if let Err(e) = self.preferences.save() {
            self.message = Some(e.to_string());
        }
    }
    pub(super) fn restore_session(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self
            .session_key()
            .and_then(|key| self.preferences.sessions.get(&key))
            .cloned()
        else {
            return;
        };
        let Some(root) = self.root.clone() else {
            return;
        };
        self.restoring = true;
        self.search.recent = session
            .recent
            .iter()
            .filter_map(|relative| inside(&root, relative))
            .filter(|p| self.in_vault(p))
            .collect();
        for (relative, state) in session.documents {
            let Some(path) = inside(&root, &relative) else {
                continue;
            };
            self.open(path.clone(), w, cx);
            self.restore_document(&path, state, cx);
        }
        self.history = super::navigation::History {
            backward: session.back,
            forward: session.forward,
        };
        self.active = None;
        if !self.visit(&session.current, w, cx) {
            let mut fallback = session.current;
            fallback.document = None;
            while !self.visit(&fallback, w, cx) && fallback.folder.pop() {}
        }
        self.restoring = false;
    }
    fn restore_document(&self, path: &Path, state: DocumentState, cx: &mut Context<Self>) {
        let Some(doc) = self.documents.get(path) else {
            return;
        };
        let mode = if state.editing {
            ViewMode::Edit
        } else {
            ViewMode::View
        };
        let input = doc.handler.update(cx, |h, cx| {
            if h.can_toggle_mode() {
                h.set_mode(mode, cx);
            }
            h.input().cloned()
        });
        if let Some(input) = input {
            input.update(cx, |i, cx| {
                i.set_selected_range_with_direction(state.selection, state.reversed, cx);
                i.set_scroll_offset(point(px(state.scroll[0]), px(state.scroll[1])), cx);
            });
        }
    }
    fn visit(&mut self, route: &Route, w: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(root) = self.root.clone() else {
            return false;
        };
        let Some(folder) = inside(&root, &route.folder) else {
            return false;
        };
        let doc = route.document.as_deref().and_then(|p| inside(&root, p));
        if !source::is_dir(&folder)
            || (route.document.is_some() && !doc.as_ref().is_some_and(|p| self.in_vault(p)))
        {
            return false;
        }
        let mut dirs = vec![root.clone()];
        let mut dir = root;
        for part in route.folder.components() {
            dir.push(part);
            if dir.is_symlink() {
                return false;
            }
            dirs.push(dir.clone());
        }
        let shared = self
            .folders
            .iter()
            .zip(&dirs)
            .take_while(|(f, p)| &f.path == *p)
            .count();
        self.folders.truncate(shared);
        for dir in dirs.into_iter().skip(shared) {
            match crate::browser::Folder::new(dir) {
                Ok(f) => self.folders.push(f),
                Err(e) => {
                    self.message = Some(e.to_string());
                    return false;
                }
            }
        }
        if let Some(f) = self.folders.last() {
            f.restore(route.scroll);
        }
        if self.search.query(cx) != route.query {
            let query = route.query.clone();
            self.search
                .input
                .update(cx, |i, cx| i.set_value(query, w, cx));
        }
        self.search.folder_only = route.folder_only;
        self.active = None;
        self.page = None;
        self.file_form = None;
        self.deleted_document = None;
        if let Some(p) = doc {
            self.open(p, w, cx);
        }
        self.search_changed(cx);
        true
    }
    pub(super) fn travel(&mut self, forward: bool, w: &mut Window, cx: &mut Context<Self>) {
        let previous = self.route(cx);
        self.restoring = true;
        let mut history = std::mem::take(&mut self.history);
        history.step(forward, previous, |route| self.visit(route, w, cx));
        self.history = history;
        self.restoring = false;
        self.advance(!forward);
        self.persist_session(cx);
        cx.notify();
    }
    /// Shows the folder that holds `path`, as a new step in the history.
    pub(super) fn reveal(&mut self, path: &Path, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(folder) = path.parent() {
            self.show_folder(folder, w, cx);
        }
    }
    /// Shows `folder` (inside the vault) as a new step in the history.
    pub(super) fn show_folder(&mut self, folder: &Path, w: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self
            .root
            .as_ref()
            .and_then(|root| folder.strip_prefix(root).ok())
            .map(Path::to_owned)
        else {
            return;
        };
        let previous = self.route(cx);
        let route = Route {
            folder,
            whole: !self.search.folder_only,
            folder_only: self.search.folder_only,
            ..Route::default()
        };
        self.restoring = true;
        let visited = self.visit(&route, w, cx);
        self.restoring = false;
        if visited {
            self.record_route(previous, cx);
            self.advance(false);
            self.persist_session(cx);
        }
        cx.notify();
    }
    pub fn go_forward(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.travel(true, w, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_from_earlier_builds_still_restore() -> anyhow::Result<()> {
        let old = r#"{"current":{"folder":"Notes","document":"Notes/a.md","query":"x","whole":false,"scroll":12.0},
            "back":[{"folder":"","document":null,"query":"","whole":true,"scroll":0.0}],
            "forward":[],"documents":{}}"#;
        let session: Session = serde_json::from_str(old)?;
        anyhow::ensure!(session.recent.is_empty());
        anyhow::ensure!(
            !session.current.folder_only,
            "the vault is the default scope"
        );
        anyhow::ensure!(session.back.len() == 1 && session.current.query == "x");
        Ok(())
    }
}
