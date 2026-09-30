//! GPUI Component 0.6 reads its palette from `App`, including during painting.
//! Scope those reads to a preview subtree and restore every theme global before
//! returning to the surrounding editor. Deferred popups need their own scope.
use std::{panic, rc::Rc};

use gpui_kit::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
    base::{TextViewDefaults, Theme as BaseTheme},
    component::Theme,
};

pub fn themed(theme: Rc<Theme>, element: impl IntoElement) -> AnyElement {
    ThemedElement {
        theme,
        element: element.into_element(),
    }
    .into_any_element()
}

fn with_theme<R>(theme: &Theme, cx: &mut App, render: impl FnOnce(&mut App) -> R) -> R {
    let original = Theme::global(cx).clone();
    let base = BaseTheme::global(cx);
    let text = TextViewDefaults::global(cx);
    cx.set_global(theme.clone());
    Theme::sync_base(cx);
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| render(cx)));
    cx.set_global(original);
    cx.set_global(base);
    text.install(cx);
    match result {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    }
}

struct ThemedElement<E> {
    theme: Rc<Theme>,
    element: E,
}

impl<E: Element> IntoElement for ThemedElement<E> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: Element> Element for ThemedElement<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.element.id()
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        self.element.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        with_theme(&self.theme, cx, |cx| {
            self.element.request_layout(id, inspector_id, window, cx)
        })
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        with_theme(&self.theme, cx, |cx| {
            self.element
                .prepaint(id, inspector_id, bounds, request_layout, window, cx)
        })
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        with_theme(&self.theme, cx, |cx| {
            self.element.paint(
                id,
                inspector_id,
                bounds,
                request_layout,
                prepaint,
                window,
                cx,
            );
        });
    }

    fn a11y_role(&self) -> Option<gpui_kit::Role> {
        self.element.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut gpui_kit::accesskit::Node) {
        self.element.write_a11y_info(node);
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut gpui_kit::A11ySubtreeBuilder,
    ) {
        self.element.a11y_synthetic_children(prepaint, builder);
    }
}
