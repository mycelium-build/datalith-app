//! A small in-memory data set rendered by the same table as a Vault Base.
use std::{collections::HashMap, path::PathBuf};

use gpui_kit::component::input::EditorState;
use gpui_kit::{App, AppContext as _, Entity, Window};

use super::{BaseItem, BaseRow, BaseSnapshot, BaseStatus, BaseViewState, table};
use crate::document::{
    base::BaseDefinition,
    handler::{FileHandler, ViewMode},
};

impl BaseViewState {
    pub(crate) fn preview(window: &mut Window, cx: &mut App) -> anyhow::Result<Entity<Self>> {
        let definition = BaseDefinition::parse(
            "properties:\n  title:\n    displayName: Project\n  status:\n    displayName: Status\n  owner:\n    displayName: Owner\nviews:\n  - type: table\n    name: Projects\n    order: [title, status, owner]\n",
        )?;
        let rows: Vec<_> = [
            ("User journey", "In progress", "Alex"),
            ("Visual library", "Planned", "Sam"),
            ("Interactive prototype", "In progress", "Morgan"),
            ("Design review", "Complete", "Alex"),
            ("Documentation", "Planned", "Sam"),
        ]
        .into_iter()
        .map(|(title, status, owner)| BaseRow {
            path: PathBuf::from(format!("{title}.md")),
            values: vec![title.into(), status.into(), owner.into()],
            links: Vec::new(),
            class_hits: Vec::new(),
            image: None,
        })
        .collect();
        let projection_index = [
            ("title".into(), 0),
            ("status".into(), 1),
            ("owner".into(), 2),
        ]
        .into_iter()
        .collect();
        let snapshot = preview_snapshot(definition.clone(), rows, projection_index);
        let (input, handler) = preview_handles(window, cx);
        Ok(cx.new(|cx| {
            let mut state = Self::new(input, None, handler.downgrade(), cx);
            state.preview = true;
            let mut table = table::TableState::new();
            table.item_sizes = table::row_sizes(&snapshot);
            state.table = Some(table);
            state.definition = Some(definition);
            state.selected_view = Some("Projects".into());
            state.status = BaseStatus::Ready(snapshot);
            state
        }))
    }

    pub(crate) fn graph_preview(window: &mut Window, cx: &mut App) -> anyhow::Result<Entity<Self>> {
        let definition = BaseDefinition::parse(
            "views:\n  - type: graph\n    name: Connections\n    display:\n      legend: false\n",
        )?;
        let rows: Vec<_> = [
            (
                "Field notes.md",
                &["Project Atlas.md", "Design review.md"][..],
            ),
            (
                "Project Atlas.md",
                &["Visual library.md", "Prototype.md"][..],
            ),
            ("Visual library.md", &["Design review.md"][..]),
            ("Prototype.md", &["Design review.md"][..]),
            ("Design review.md", &["Field notes.md"][..]),
        ]
        .into_iter()
        .map(|(path, links)| BaseRow {
            path: PathBuf::from(path),
            values: Vec::new(),
            links: links.iter().map(ToString::to_string).collect(),
            class_hits: Vec::new(),
            image: None,
        })
        .collect();
        let view = definition
            .views
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("graph preview has no view"))?;
        anyhow::ensure!(
            view.as_graph().is_some(),
            "graph preview view is not a graph"
        );
        let snapshot = preview_snapshot(definition.clone(), rows, HashMap::new());
        let (input, handler) = preview_handles(window, cx);
        Ok(cx.new(|cx| {
            let mut state = Self::new(input, None, handler.downgrade(), cx);
            state.preview = true;
            state.update_graph(&snapshot, &view, true, cx);
            state.definition = Some(definition);
            state.selected_view = Some("Connections".into());
            state.status = BaseStatus::Ready(snapshot);
            state
        }))
    }
}

fn preview_snapshot(
    definition: BaseDefinition,
    rows: Vec<BaseRow>,
    projection_index: HashMap<String, usize>,
) -> BaseSnapshot {
    BaseSnapshot {
        id: 1,
        definition,
        view_index: 0,
        items: (0..rows.len())
            .map(|index| BaseItem::Row {
                index,
                ordinal: index,
            })
            .collect(),
        list_items: Vec::new(),
        projection_index,
        summaries: Vec::new(),
        group_summaries: HashMap::new(),
        total: rows.len(),
        omitted: 0,
        rows,
    }
}

/// An unused input and an unsubscribed, read-only handler for a preview state.
fn preview_handles(
    window: &mut Window,
    cx: &mut App,
) -> (Entity<EditorState>, Entity<FileHandler>) {
    let input = cx.new(|cx| EditorState::new(window, cx));
    let handler = cx.new(|_| FileHandler::new(ViewMode::View, None, None));
    (input, handler)
}
