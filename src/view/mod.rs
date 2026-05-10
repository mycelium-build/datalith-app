pub mod render;

use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui::*;
use gpui_component::{
    input::{InputEvent, InputState},
    tree::TreeState,
    VirtualListScrollHandle,
};

use crate::config::save_last_folder;
use crate::filetree::build_file_items;
use crate::search::{SearchEngine, SearchResult};

pub struct DatalithView {
    pub tree_state: Entity<TreeState>,
    pub root_path: Option<PathBuf>,
    pub root_name: SharedString,
    pub current_file: Option<PathBuf>,
    pub editor_state: Option<Entity<InputState>>,
    pub search_engine: Option<Arc<SearchEngine>>,
    pub search_open: bool,
    pub needs_search_focus: bool,
    pub search_input: Entity<InputState>,
    pub search_results: Vec<SearchResult>,
    pub search_scroll_handle: VirtualListScrollHandle,
    pub search_item_sizes: Rc<Vec<Size<Pixels>>>,
    pub search_selected: Option<usize>,
    pub _search_sub: Subscription,
    _editor_sub: Option<Subscription>,
}

impl DatalithView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search files..."));
        let search_sub = cx.subscribe_in(
            &search_input,
            window,
            move |this, input, event, window, cx| match event {
                InputEvent::Change => {
                    let query = input.read(cx).value();
                    this.search(query);
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(index) = this.search_selected {
                        if let Some(result) = this.search_results.get(index) {
                            let path = result.path.clone();
                            this.search_open = false;
                            this.search_selected = None;
                            this.search_results.clear();
                            this.open_file(path, window, cx);
                            cx.notify();
                        }
                    }
                }
                _ => {}
            },
        );

        Self {
            tree_state: cx.new(|cx| TreeState::new(cx)),
            root_path: None,
            root_name: "No folder open".into(),
            current_file: None,
            editor_state: None,
            search_engine: None,
            search_open: false,
            needs_search_focus: false,
            search_input,
            search_results: Vec::new(),
            search_scroll_handle: VirtualListScrollHandle::new(),
            search_item_sizes: Rc::new(Vec::new()),
            search_selected: None,
            _search_sub: search_sub,
            _editor_sub: None,
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

        _cx.notify();
    }

    pub fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "txt" && ext != "md" {
            return;
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        self.current_file = Some(path);
        self.editor_state = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .default_value(content)
        }));

        self.editor_state
            .as_ref()
            .unwrap()
            .focus_handle(cx)
            .focus(window, cx);

        self._editor_sub = Some(cx.subscribe_in(
            self.editor_state.as_ref().unwrap(),
            window,
            move |_view, editor, event, _window, _cx| {
                if let InputEvent::Change = event {
                    let content = editor.read(_cx).value();
                    if let Some(ref path) = _view.current_file {
                        let _ = fs::write(path, content.to_string());
                    }
                }
            },
        ));

        cx.notify();
    }

    pub fn scroll_to_selected(&mut self, index: usize) {
        self.search_scroll_handle.scroll_to_item(index, ScrollStrategy::Nearest);
    }

    pub fn search(&mut self, query: SharedString) {
        let query = query.trim().to_string();
        self.search_results = if query.is_empty() {
            Vec::new()
        } else {
            self.search_engine
                .as_ref()
                .map(|engine| engine.search(&query))
                .unwrap_or_default()
        };
        self.search_selected = None;
        self.search_item_sizes = Rc::new(vec![size(px(600.), px(70.)); self.search_results.len()]);
    }
}
