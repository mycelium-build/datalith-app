pub mod palette;
pub mod render;
pub mod sidebar;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::*;
use gpui_component::{
    input::{InputEvent, InputState},
    tree::TreeState,
};

use crate::config::save_last_folder;
use crate::filetree::build_file_items;
use crate::search::SearchEngine;
use palette::Palette;

pub(crate) struct OpenFile {
    pub(crate) path: PathBuf,
    pub(crate) state: Option<Entity<InputState>>,
    pub(crate) _sub: Option<Subscription>,
}

pub struct DatalithView {
    pub tree_state: Entity<TreeState>,
    pub root_path: Option<PathBuf>,
    pub root_name: SharedString,
    pub(crate) open_files: Vec<OpenFile>,
    pub active_tab: usize,
    pub search_engine: Option<Arc<SearchEngine>>,
    pub palette: Palette,
    _palette_sub: Subscription,
    _rename_sub: Option<Subscription>,
    _sidebar_blur_sub: Option<Subscription>,
    pub context_menu_target: Option<PathBuf>,
    pub rename_target: Option<PathBuf>,
    pub rename_state: Option<Entity<InputState>>,
    pub drag_hover: Rc<RefCell<Option<(PathBuf, Instant)>>>,
    pub focus_sidebar_requested: bool,
    pub(crate) pending_open: Option<PathBuf>,
    sidebar_focus_handle: FocusHandle,
}

impl DatalithView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let palette = Palette::new(window, cx);
        let palette_sub = Palette::input_subscription(palette.input.clone(), window, cx);
        let sidebar_focus_handle = cx.focus_handle();
        let tree_state = cx.new(|cx| TreeState::new(cx));

        let sidebar_blur_sub = cx.on_blur(&sidebar_focus_handle, window, {
            let tree_state = tree_state.clone();
            move |_this, _window, cx| {
                tree_state.update(cx, |state, cx| {
                    state.set_selected_index(None, cx);
                });
            }
        });

        Self {
            tree_state,
            root_path: None,
            root_name: "No folder open".into(),
            open_files: Vec::new(),
            active_tab: 0,
            search_engine: None,
            palette,
            _palette_sub: palette_sub,
            _rename_sub: None,
            _sidebar_blur_sub: Some(sidebar_blur_sub),
            context_menu_target: None,
            rename_target: None,
            rename_state: None,
            drag_hover: Rc::new(RefCell::new(None)),
            focus_sidebar_requested: false,
            pending_open: None,
            sidebar_focus_handle,
        }
    }

    pub fn set_root_path(&mut self, path: PathBuf, _cx: &mut Context<Self>) {
        self.root_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .into();
        self.root_path = Some(path.clone());
        save_last_folder(&path);

        let items = build_file_items(&path);
        self.tree_state.update(_cx, |state, cx| {
            state.set_items(items, cx);
        });

        self.search_engine = match SearchEngine::new(&path) {
            Ok(engine) => Some(Arc::new(engine)),
            Err(e) => {
                eprintln!("Failed to build search index: {e}");
                None
            }
        };

        self.palette.set_root(&path);

        _cx.notify();
    }

    pub fn open_file(
        &mut self,
        path: PathBuf,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "txt" && ext != "md" {
            return;
        }

        if let Some(index) = self.open_files.iter().position(|f| f.path == path) {
            self.active_tab = index;
            if let Some(ref state) = self.open_files[index].state {
                state.focus_handle(cx).focus(window, cx);
            }
            cx.notify();
            return;
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .searchable(true)
                .default_value(content)
        });

        let sub = {
            let path = path.clone();
            cx.subscribe_in(&state, window, move |_view, editor, event, _window, _cx| {
                if let InputEvent::Change = event {
                    let content = editor.read(_cx).value();
                    let _ = fs::write(&path, content.to_string());
                }
            })
        };

        if new_tab || self.open_files.is_empty() {
            state.focus_handle(cx).focus(window, cx);
            self.open_files.push(OpenFile {
                path,
                state: Some(state),
                _sub: Some(sub),
            });
            self.active_tab = self.open_files.len() - 1;
        } else {
            let active = self.active_tab.min(self.open_files.len() - 1);
            self.open_files[active] = OpenFile {
                path,
                state: Some(state),
                _sub: Some(sub),
            };
            if let Some(ref s) = self.open_files[active].state {
                s.focus_handle(cx).focus(window, cx);
            }
        }
        cx.notify();
    }

    pub fn new_empty_tab(&mut self, cx: &mut Context<Self>) {
        self.open_files.push(OpenFile {
            path: PathBuf::new(),
            state: None,
            _sub: None,
        });
        self.active_tab = self.open_files.len() - 1;
        cx.notify();
    }

    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if self.open_files.is_empty() {
            return;
        }
        let index = self.active_tab.min(self.open_files.len().saturating_sub(1));
        self.close_tab(index, cx);
    }

    pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.open_files.len() {
            return;
        }
        self.open_files.remove(index);
        if self.open_files.is_empty() {
            self.active_tab = 0;
        } else if self.active_tab > index && self.active_tab > 0 {
            self.active_tab -= 1;
        } else if self.active_tab >= self.open_files.len() {
            self.active_tab = self.open_files.len() - 1;
        }
        cx.notify();
    }

    fn parent_dir_for_target(&self, target: &Path) -> PathBuf {
        if target.is_dir() {
            target.to_path_buf()
        } else {
            target
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/"))
        }
    }

    pub(crate) fn unique_name(base_dir: &Path, name: &str) -> PathBuf {
        let (stem, ext) = if let Some(dot) = name.rfind('.') {
            (&name[..dot], &name[dot..])
        } else {
            (name, "")
        };
        let mut candidate = base_dir.join(name);
        let mut counter = 1;
        while candidate.exists() {
            candidate = base_dir.join(format!("{stem} {counter}{ext}"));
            counter += 1;
        }
        candidate
    }

    pub fn new_file_from_target(&mut self, target: &Path) -> Option<PathBuf> {
        let dir = self.parent_dir_for_target(target);
        let path = Self::unique_name(&dir, "untitled.md");
        match fs::write(&path, "") {
            Ok(()) => Some(path),
            Err(e) => {
                eprintln!("Failed to create file {:?}: {e}", path);
                None
            }
        }
    }

    pub fn new_folder_from_target(&mut self, target: &Path) -> Option<PathBuf> {
        let dir = self.parent_dir_for_target(target);
        let path = Self::unique_name(&dir, "untitled");
        match fs::create_dir(&path) {
            Ok(()) => Some(path),
            Err(e) => {
                eprintln!("Failed to create folder {:?}: {e}", path);
                None
            }
        }
    }

    pub fn delete_target(&mut self, target: &Path, cx: &mut Context<Self>) {
        let result = if target.is_dir() {
            fs::remove_dir_all(target)
        } else {
            fs::remove_file(target)
        };
        if let Err(e) = result {
            eprintln!("Failed to delete {:?}: {e}", target);
            return;
        }
        let mut i = 0;
        while i < self.open_files.len() {
            if self.open_files[i].path == target || self.open_files[i].path.starts_with(target) {
                self.close_tab(i, cx);
            } else {
                i += 1;
            }
        }
    }

    pub fn duplicate_target(&mut self, target: &Path) {
        if target.is_dir() {
            let parent = target.parent().unwrap_or_else(|| Path::new("/"));
            let name = target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("copy");
            let new_path = Self::unique_name(parent, name);
            if let Err(e) = Self::copy_dir(target, &new_path) {
                eprintln!("Failed to duplicate dir {:?}: {e}", target);
            }
        } else {
            let parent = target.parent().unwrap_or_else(|| Path::new("/"));
            let name = target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("copy");
            let new_path = Self::unique_name(parent, name);
            if let Err(e) = fs::copy(target, &new_path) {
                eprintln!("Failed to duplicate file {:?}: {e}", target);
            }
        }
    }

    fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let dest = dst.join(entry.file_name());
            if path.is_dir() {
                Self::copy_dir(&path, &dest)?;
            } else {
                fs::copy(&path, &dest)?;
            }
        }
        Ok(())
    }

    pub fn open_in_explorer(target: &Path) {
        let path_to_open = if target.is_dir() {
            target.to_path_buf()
        } else {
            target
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| target.to_path_buf())
        };
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg(&path_to_open)
                .spawn();
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(&path_to_open)
                .spawn();
        }
    }

    pub fn copy_path(target: &Path) {
        let path_str = target.to_string_lossy().to_string();
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&path_str);
        }
    }

    pub fn resolve_target(&self, cx: &Context<Self>) -> Option<PathBuf> {
        self.tree_state
            .read(cx)
            .selected_entry()
            .map(|e| PathBuf::from(e.item().id.to_string()))
            .or_else(|| {
                let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
                self.open_files.get(active).map(|f| f.path.clone())
            })
    }

    pub fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        if let Some(ref root) = self.root_path.clone() {
            let expanded_ids = self.tree_state.read(cx).expanded_ids();
            let mut items = build_file_items(root);
            for item in &mut items {
                if expanded_ids.contains(&item.id) {
                    item.set_expanded(true);
                }
            }
            self.tree_state.update(cx, |state, cx| {
                state.set_items(items, cx);
            });
            cx.notify();
        }
    }
}
