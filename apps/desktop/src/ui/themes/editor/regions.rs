//! The variant navigation and the controls as views of their own. The editor
//! renders them cached, so a redraw caused elsewhere (the animated graph
//! preview, hovering another region) reuses their last frame. They observe
//! the editor and redraw whenever its state changes.
use gpui_kit::component::v_flex;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Subscription, WeakEntity, Window, div,
};

use super::ThemeEditor;
use crate::app::themes::ThemeLibrary;

#[derive(Clone, Copy)]
enum Region {
    Variants,
    Controls,
}

pub(super) struct EditorRegion {
    editor: WeakEntity<ThemeEditor>,
    region: Region,
    _editor_changes: Subscription,
}

pub(super) struct EditorRegions {
    pub(super) variants: Entity<EditorRegion>,
    pub(super) controls: Entity<EditorRegion>,
}

impl EditorRegions {
    pub(super) fn new(editor: &Entity<ThemeEditor>, cx: &mut Context<ThemeEditor>) -> Self {
        let mut region = |region| {
            cx.new(|cx| EditorRegion {
                editor: editor.downgrade(),
                region,
                _editor_changes: cx.observe(editor, |_, _, cx| cx.notify()),
            })
        };
        Self {
            variants: region(Region::Variants),
            controls: region(Region::Controls),
        }
    }
}

impl Render for EditorRegion {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let region = self.region;
        self.editor.upgrade().map_or_else(
            || div().into_any_element(),
            |editor| editor.update(cx, |editor, cx| editor.render_region(region, window, cx)),
        )
    }
}

impl ThemeEditor {
    fn render_region(&self, region: Region, window: &Window, cx: &Context<Self>) -> AnyElement {
        match region {
            Region::Variants => {
                let Some(family) = cx.global::<ThemeLibrary>().family(self.family_id) else {
                    return div().into_any_element();
                };
                if Self::is_narrow(window) {
                    self.render_variant_strip(family, cx).into_any_element()
                } else {
                    self.render_variant_rail(family, cx).into_any_element()
                }
            }
            Region::Controls => v_flex()
                .size_full()
                .min_w_0()
                .min_h_0()
                .child(self.render_category_tabs(cx))
                .child(self.render_properties(cx))
                .into_any_element(),
        }
    }
}
