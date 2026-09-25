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
}
