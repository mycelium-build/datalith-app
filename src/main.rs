use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    list::ListItem,
    popover::Popover,
    sidebar::SidebarHeader,
    tree::{self, TreeItem, TreeState},
    ActiveTheme, Icon, IconName, Root, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use tantivy::doc;
use tantivy::{
    self, DocAddress, Index, IndexReader, TantivyDocument, collector::TopDocs, query::Query,
    schema::*,
};

actions!(datalith, [OpenCodex, ToggleSearch, CloseSearch]);

struct AppState {
    view: Option<Entity<DatalithView>>,
}

impl Global for AppState {}

#[allow(dead_code)]
struct SearchEngine {
    index: Index,
    reader: IndexReader,
    path_field: Field,
    name_field: Field,
    content_field: Field,
    fingerprint_field: Field,
}

#[derive(Clone)]
struct SearchResult {
    path: PathBuf,
    snippet: String,
}

impl SearchEngine {
    fn new(root: &Path) -> tantivy::Result<Self> {
        let index_path = root.join(".datalith").join("search_index");

        let mut schema_builder = Schema::builder();
        let path_field = schema_builder.add_text_field("path", STRING | STORED);
        let name_field = schema_builder.add_text_field("name", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let fingerprint_field = schema_builder.add_text_field("fingerprint", STRING | STORED);
        let schema = schema_builder.build();

        let (index, needs_build) = if index_path.exists() {
            (Index::open_in_dir(&index_path)?, false)
        } else {
            fs::create_dir_all(&index_path).map_err(|e| {
                tantivy::TantivyError::InvalidArgument(format!("Failed to create index dir: {e}"))
            })?;
            (Index::create_in_dir(&index_path, schema)?, true)
        };

        if needs_build {
            let mut writer = index.writer(50_000_000)?;
            index_files(
                &mut writer,
                root,
                path_field,
                name_field,
                content_field,
                fingerprint_field,
            )?;
            writer.commit()?;
        } else {
            incremental_update(
                &index,
                root,
                path_field,
                name_field,
                content_field,
                fingerprint_field,
            )?;
        }

        let reader = index.reader()?;

        Ok(Self {
            index,
            reader,
            path_field,
            name_field,
            content_field,
            fingerprint_field,
        })
    }

    fn search(&self, query_str: &str) -> Vec<SearchResult> {
        let searcher = self.reader.searcher();
        let query = self.build_query(query_str);
        let top_docs = match searcher.search(&query, &TopDocs::with_limit(20).order_by_score()) {
            Ok(docs) => docs,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
                let path = doc
                    .get_first(self.path_field)
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from);

                if let Some(path) = path {
                    let snippet = tantivy::snippet::SnippetGenerator::create(
                        &searcher,
                        &query,
                        self.content_field,
                    )
                    .ok()
                    .map(|generator| {
                        let snip = generator.snippet_from_doc(&doc);
                        snip.fragment().to_string()
                    })
                    .unwrap_or_default();

                    results.push(SearchResult { path, snippet });
                }
            }
        }
        results
    }

    fn build_query(&self, query_str: &str) -> Box<dyn Query> {
        use tantivy::Term;
        use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur};

        let subqueries: Vec<_> = query_str
            .split_whitespace()
            .map(|word| {
                let word_lower = word.to_lowercase();
                let name_term = Term::from_field_text(self.name_field, &word_lower);
                let content_term = Term::from_field_text(self.content_field, &word_lower);
                let name_clause = (
                    Occur::Should,
                    Box::new(FuzzyTermQuery::new_prefix(name_term, 2, true)) as Box<dyn Query>,
                );
                let content_clause = (
                    Occur::Should,
                    Box::new(FuzzyTermQuery::new_prefix(content_term, 2, true)) as Box<dyn Query>,
                );
                (
                    Occur::Must,
                    Box::new(BooleanQuery::new(vec![name_clause, content_clause]))
                        as Box<dyn Query>,
                )
            })
            .collect();

        Box::new(BooleanQuery::new(subqueries))
    }
}

fn file_fingerprint(path: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    if let Ok(meta) = fs::metadata(path) {
        meta.len().hash(&mut hasher);
        if let Ok(mtime) = meta.modified() {
            if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                dur.as_secs().hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

fn is_indexable(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("txt" | "md")
    )
}

fn incremental_update(
    index: &Index,
    root: &Path,
    path_field: Field,
    name_field: Field,
    content_field: Field,
    fingerprint_field: Field,
) -> tantivy::Result<()> {
    use std::collections::HashMap;
    use tantivy::Term;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let mut current: HashMap<PathBuf, u64> = HashMap::new();
    collect_files(root, &mut current);

    let mut writer = index.writer(50_000_000)?;

    for (segment_ord, seg_reader) in searcher.segment_readers().iter().enumerate() {
        let segment_ord = segment_ord as u32;
        for doc_id in 0..seg_reader.max_doc() {
            if seg_reader.is_deleted(doc_id) {
                continue;
            }
            let addr = DocAddress::new(segment_ord, doc_id);
            let doc: TantivyDocument = match searcher.doc(addr) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let path_str = doc.get_first(path_field).and_then(|v| v.as_str());
            let old_fp = doc
                .get_first(fingerprint_field)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok());

            let Some((path_str, old_fp)) = path_str.zip(old_fp) else {
                continue;
            };
            let path = PathBuf::from(path_str);

            match current.remove(&path) {
                Some(new_fp) if new_fp == old_fp => {} // unchanged
                _ => {
                    println!("Update: {}", path_str);
                    writer.delete_term(Term::from_field_text(path_field, path_str));
                }
            }
        }
    }

    drop(searcher);
    drop(reader);

    add_files(
        &mut writer,
        &current,
        path_field,
        name_field,
        content_field,
        fingerprint_field,
    )?;

    writer.commit()?;
    Ok(())
}

fn collect_files(dir: &Path, files: &mut HashMap<PathBuf, u64>) {
    if !dir.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, files);
            } else if is_indexable(&path) {
                files.insert(path.clone(), file_fingerprint(&path));
            }
        }
    }
}

fn add_files(
    writer: &mut tantivy::IndexWriter,
    files: &HashMap<PathBuf, u64>,
    path_field: Field,
    name_field: Field,
    content_field: Field,
    fingerprint_field: Field,
) -> tantivy::Result<()> {
    for (path, fp) in files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let content = fs::read_to_string(path).unwrap_or_default();
        writer.add_document(doc!(
            path_field => path.to_string_lossy().as_ref(),
            name_field => name,
            content_field => content.as_str(),
            fingerprint_field => fp.to_string(),
        ))?;
    }
    Ok(())
}

fn index_files(
    writer: &mut tantivy::IndexWriter,
    dir: &Path,
    path_field: Field,
    name_field: Field,
    content_field: Field,
    fingerprint_field: Field,
) -> tantivy::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                index_files(
                    writer,
                    &path,
                    path_field,
                    name_field,
                    content_field,
                    fingerprint_field,
                )?;
            } else {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "txt" || ext == "md" {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let content = fs::read_to_string(&path).unwrap_or_default();
                    let fp = file_fingerprint(&path);
                    writer.add_document(doc!(
                        path_field => path.to_string_lossy().as_ref(),
                        name_field => name,
                        content_field => content.as_str(),
                        fingerprint_field => fp.to_string(),
                    ))?;
                }
            }
        }
    }
    Ok(())
}

pub struct DatalithView {
    tree_state: Entity<TreeState>,
    root_path: Option<PathBuf>,
    root_name: SharedString,
    current_file: Option<PathBuf>,
    editor_state: Option<Entity<InputState>>,
    search_engine: Option<Arc<SearchEngine>>,
    search_open: bool,
    search_input: Entity<InputState>,
    search_focus: FocusHandle,
    search_results: Vec<SearchResult>,
    search_scroll_handle: VirtualListScrollHandle,
    search_item_sizes: Rc<Vec<Size<Pixels>>>,
    search_selected: Option<usize>,
    _search_sub: Subscription,
}

impl DatalithView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search files..."));
        let search_focus = search_input.read(cx).focus_handle(cx);
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
            search_input,
            search_focus,
            search_results: Vec::new(),
            search_scroll_handle: VirtualListScrollHandle::new(),
            search_item_sizes: Rc::new(Vec::new()),
            search_selected: None,
            _search_sub: search_sub,
        }
    }

    fn set_root_path(&mut self, path: PathBuf, _cx: &mut Context<Self>) {
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

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
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

        let _subscription = cx.subscribe_in(
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
        );

        cx.notify();
    }

    fn scroll_to_selected(&mut self, index: usize) {
        self.search_scroll_handle.scroll_to_item(index, ScrollStrategy::Nearest);
    }


    fn search(&mut self, query: SharedString) {
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

impl Render for DatalithView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let search_input = self.search_input.clone();
        let root_path = self.root_path.clone();
        let search_open = self.search_open;

        h_flex()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(260.))
                    .h_full()
                    .bg(cx.theme().sidebar)
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(
                        SidebarHeader::new().p_2().child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .w_full()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::Folder))
                                        .child(self.root_name.clone()),
                                )
                                .child(div().flex_1())
                                .child(
                                    Popover::new("search-popover")
                                        .trigger(
                                            Button::new("search-trigger")
                                                .ghost()
                                                .icon(IconName::Search),
                                        )
                                        .anchor(Anchor::TopCenter)
                                        .open(search_open)
                                        .track_focus(&self.search_focus)
                                        .on_open_change(cx.listener(|this, open, _window, cx| {
                                            this.search_open = *open;
                                            cx.notify();
                                        }))
                                        .w(px(600.))
                                        .child(
                                            v_flex()
                                                .overflow_hidden()
                                                .on_key_down(cx.listener(
                                                    |this, event: &KeyDownEvent, _window, cx| {
                                                        let count = this.search_results.len();
                                                        if count == 0 {
                                                            return;
                                                        }
                                                        match event.keystroke.key.as_str() {
                                                            "down" => {
                                                                let next = match this
                                                                    .search_selected
                                                                {
                                                                    Some(i) if i + 1 < count => {
                                                                        i + 1
                                                                    }
                                                                    None => 0,
                                                                    _ => return,
                                                                };
                                                                this.search_selected = Some(next);
                                                                this.scroll_to_selected(next);
                                                                cx.notify();
                                                            }
                                                            "up" => {
                                                                let prev =
                                                                    match this.search_selected {
                                                                        Some(i) if i > 0 => i - 1,
                                                                        Some(_) => return,
                                                                        None => count - 1,
                                                                    };
                                                                this.search_selected = Some(prev);
                                                                this.scroll_to_selected(prev);
                                                                cx.notify();
                                                            }
                                                            _ => {}
                                                        }
                                                    },
                                                ))
                                                .child(Input::new(&search_input))
                                                .child(
                                                    v_virtual_list(
                                                        cx.entity().clone(),
                                                        "search-results",
                                                        self.search_item_sizes.clone(),
                                                        move |view, visible_range, _, cx| {
                                                            let selected_idx = view.search_selected;
                                                            visible_range.map(move |i| {
                                                                let r = &view.search_results[i];
                                                                let file_name = r.path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                                                                let bg = if Some(i) == selected_idx { cx.theme().muted } else { gpui::Hsla::default() };
                                                                let path = r.path.clone();
                                                                let snippet = r.snippet.clone();
                                                                div()
                                                                    .px_2()
                                                                    .py_1()
                                                                    .bg(bg)
                                                                    .hover(|s| s.bg(cx.theme().muted))
                                                                    .cursor_pointer()
                                                                    .child(
                                                                        v_flex()
                                                                            .child(
                                                                                h_flex()
                                                                                    .gap_2()
                                                                                    .items_center()
                                                                                    .child(Icon::new(IconName::File).size_3())
                                                                                    .child(file_name),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_sm()
                                                                                    .text_color(cx.theme().muted_foreground)
                                                                                    .pl_5()
                                                                                    .child(snippet),
                                                                            ),
                                                                    )
                                                                    .id(ElementId::Name(format!("result-{i}").into()))
                                                                    .on_click(cx.listener(move |view, _, window, cx| {
                                                                        view.search_open = false;
                                                                        view.open_file(path.clone(), window, cx);
                                                                    }))
                                                            }).collect()
                                                        },
                                                    )
                                                    .track_scroll(&self.search_scroll_handle)
                                                    .h(px(400.)),
                                                ),
                                        ),
                                ),
                        ),
                    )
                    .child(div().flex_1().child(tree::tree(&self.tree_state, {
                        let view = cx.entity();
                        move |ix, entry, selected, _window, cx| {
                            let is_folder = entry.is_folder();
                            let is_expanded = entry.is_expanded();
                            let item_id = entry.item().id.clone();
                            let item_label = entry.item().label.clone();
                            let depth = entry.depth();

                            view.update(cx, move |_this, cx| {
                                let icon = if !is_folder {
                                    IconName::File
                                } else if is_expanded {
                                    IconName::FolderOpen
                                } else {
                                    IconName::Folder
                                };

                                ListItem::new(ix)
                                    .selected(selected)
                                    .pl(px(16.) * depth + px(12.))
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(icon).size_4())
                                            .child(item_label.clone()),
                                    )
                                    .on_click(cx.listener({
                                        let item_id = item_id.clone();
                                        let is_folder = is_folder;
                                        move |this, _, window, cx| {
                                            if !is_folder {
                                                let path = PathBuf::from(item_id.to_string());
                                                this.open_file(path, window, cx);
                                            }
                                        }
                                    }))
                            })
                        }
                    }))),
            )
            .child(match self.editor_state.as_ref() {
                Some(editor) => Input::new(editor).h_full().into_any_element(),
                None => div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(match root_path {
                        Some(_) => "Select a file from the sidebar",
                        None => "Select a folder from the menu bar",
                    })
                    .into_any_element(),
            })
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    last_folder: Option<String>,
}

fn config_file() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join("datalith")
        .join("config.json")
}

fn save_config(config: &Config) {
    let file = config_file();
    let _ = fs::create_dir_all(file.parent().unwrap());
    let _ = fs::write(&file, serde_json::to_string(config).unwrap_or_default());
}

fn load_config() -> Config {
    fs::read_to_string(config_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_last_folder(path: &Path) {
    let mut config = load_config();
    config.last_folder = Some(path.to_string_lossy().to_string());
    save_config(&config);
}

fn load_last_folder() -> Option<PathBuf> {
    let config = load_config();
    config.last_folder.map(PathBuf::from).filter(|p| p.is_dir())
}

fn build_file_items(path: &Path) -> Vec<TreeItem> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();

            if path.is_dir() {
                let children = build_file_items(&path);
                dirs.push((
                    name.clone(),
                    TreeItem::new(path.to_string_lossy().to_string(), name).children(children),
                ));
            } else {
                files.push((
                    name.clone(),
                    TreeItem::new(path.to_string_lossy().to_string(), name),
                ));
            }
        }
    }
    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    let mut items: Vec<TreeItem> = dirs.into_iter().map(|(_, item)| item).collect();
    items.extend(files.into_iter().map(|(_, item)| item));
    items
}

fn main() {
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        cx.set_global(AppState { view: None });
        cx.on_action(open_codex);
        cx.on_action(toggle_search);
        cx.on_action(close_search);
        cx.set_menus([Menu::new("datalith").items([
            MenuItem::action("Open codex", OpenCodex),
            MenuItem::action("Search files...", ToggleSearch),
        ])]);
        cx.bind_keys([
            KeyBinding::new("cmd-shift-f", ToggleSearch, None),
            KeyBinding::new("escape", CloseSearch, None),
        ]);

        let last_folder = load_last_folder();

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| DatalithView::new(window, cx));
                cx.update_global(|state: &mut AppState, _| {
                    state.view = Some(view.clone());
                });
                if let Some(ref path) = last_folder {
                    view.update(cx, |view, cx| {
                        view.set_root_path(path.clone(), cx);
                    });
                }
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}

fn open_codex(_: &OpenCodex, cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Select a folder".into()),
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = rx.await {
            if let Some(path) = paths.into_iter().next() {
                let view_opt = cx.read_global(|state: &AppState, _| state.view.clone());
                if let Some(view) = view_opt {
                    cx.update_entity(&view, |view, cx| {
                        view.set_root_path(path, cx);
                    });
                }
            }
        }
    })
    .detach();
}

fn toggle_search(_: &ToggleSearch, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            view.search_open = !view.search_open;
            if view.search_open {
                let query = view.search_input.read(cx).value();
                if !query.trim().is_empty() {
                    view.search(query);
                }
            }
            cx.notify();
        });
    }
}

fn close_search(_: &CloseSearch, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            if view.search_open {
                view.search_open = false;
                cx.notify();
            }
        });
    }
}
