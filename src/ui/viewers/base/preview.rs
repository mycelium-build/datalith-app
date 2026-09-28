//! A small in-memory data set rendered by the same table as a Vault Base.
use std::{collections::HashMap, path::PathBuf};

use gpui_kit::component::input::EditorState;
use gpui_kit::{App, AppContext as _, Entity, Window};

use super::{BaseItem, BaseRow, BaseSnapshot, BaseStatus, BaseViewState, graph, table};
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
        let snapshot = BaseSnapshot {
            id: 1,
            definition: definition.clone(),
            view_index: 0,
            items: (0..rows.len())
                .map(|index| BaseItem::Row {
                    index,
                    ordinal: index,
                })
                .collect(),
            list_items: Vec::new(),
            projection_index: [
                ("title".into(), 0),
                ("status".into(), 1),
                ("owner".into(), 2),
            ]
            .into_iter()
            .collect(),
            summaries: Vec::new(),
            group_summaries: HashMap::new(),
            total: rows.len(),
            omitted: 0,
            rows,
        };
        let input = cx.new(|cx| EditorState::new(window, cx));
        let handler = cx.new(|_| FileHandler::new(ViewMode::View, None, None));
        Ok(cx.new(|cx| {
            let mut state = Self::new(input, None, handler.downgrade(), cx);
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
        let snapshot = BaseSnapshot {
            id: 1,
            definition: definition.clone(),
            view_index: 0,
            items: (0..rows.len())
                .map(|index| BaseItem::Row {
                    index,
                    ordinal: index,
                })
                .collect(),
            list_items: Vec::new(),
            projection_index: HashMap::new(),
            summaries: Vec::new(),
            group_summaries: HashMap::new(),
            total: rows.len(),
            omitted: 0,
            rows,
        };
        let input = cx.new(|cx| EditorState::new(window, cx));
        let handler = cx.new(|_| FileHandler::new(ViewMode::View, None, None));
        let graph_snapshot = {
            let view = snapshot
                .definition
                .views
                .first()
                .ok_or_else(|| anyhow::anyhow!("graph preview has no view"))?;
            let config = view
                .as_graph()
                .ok_or_else(|| anyhow::anyhow!("graph preview view is not a graph"))?;
            graph::build_graph_snapshot(
                config,
                std::path::Path::new(""),
                snapshot
                    .rows
                    .iter()
                    .map(|row| (row.path.clone(), row.links.clone(), row.class_hits.clone())),
                Vec::new(),
            )
        };
        let graph = cx.new(|cx| graph::GraphState::new(handler.downgrade(), cx));
        graph.update(cx, |graph, cx| {
            graph.set_snapshot(Some(graph_snapshot), true, cx);
        });
        Ok(cx.new(|cx| {
            let mut state = Self::new(input, None, handler.downgrade(), cx);
            state.definition = Some(definition);
            state.selected_view = Some("Connections".into());
            state.graph = Some(graph);
            state.status = BaseStatus::Ready(snapshot);
            state
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::{ActiveTheme as _, Root};
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{TestAppContext, px, size};

    #[test]
    fn table_uses_its_preview_palette_instead_of_the_global_palette() {
        let mut cx = TestAppContext::single();
        cx.update(gpui_kit::init);
        let handle = cx.open_window(size(px(640.), px(480.)), |window, cx| {
            let base = BaseViewState::preview(window, cx).unwrap();
            let global = cx.theme().background;
            let mut palette = cx.theme().clone();
            palette.background = gpui_kit::white();
            palette.foreground = gpui_kit::black();
            base.update(cx, |base, cx| {
                base.set_preview_appearance(std::rc::Rc::new(palette.clone()), cx);
                assert_eq!(base.theme(cx).background, palette.background);
                assert_eq!(base.theme(cx).foreground, palette.foreground);
                assert_eq!(cx.theme().background, global);
            });
            Root::new(base, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
    }

    #[test]
    fn graph_preview_keeps_production_interactions_without_link_navigation() {
        let mut cx = TestAppContext::single();
        cx.update(gpui_kit::init);
        let mut preview = None;
        let handle = cx.open_window(size(px(640.), px(480.)), |window, cx| {
            let base = BaseViewState::graph_preview(window, cx).unwrap();
            let appearance = std::rc::Rc::new(cx.theme().clone());
            base.update(cx, |base, cx| {
                base.set_preview_appearance(appearance, cx);
            });
            preview = Some(base.clone());
            Root::new(base, window, cx)
        });
        let preview = preview.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.render_frame(cx);
            assert!(window.find("graph-view").visible());
            let node = preview
                .read(cx)
                .graph
                .as_ref()
                .unwrap()
                .read(cx)
                .first_node_offset()
                .unwrap();
            window.click_at("graph-view", node, cx);
            assert!(
                preview
                    .read(cx)
                    .graph
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .has_pointer_state(),
                "the graph preview must keep pointer interaction"
            );
        })
        .unwrap();
    }
}
