use super::{GraphFocus, GraphState};
use crate::handler::FileHandlerEvent;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _, Stateful,
    StatefulInteractiveElement as _, Styled, div, px,
};

pub(super) fn canvas_events(canvas: Stateful<Div>, cx: &Context<GraphState>) -> Stateful<Div> {
    canvas
        .on_scroll(cx.listener(|state, event: &gpui_kit::ScrollEvent, _, cx| {
            let delta = event.delta.pixel_delta(px(24.));
            state.pan_by(delta.x.into(), delta.y.into(), cx);
            cx.stop_propagation();
        }))
        .on_pinch(cx.listener(|state, event: &gpui_kit::PinchEvent, _, cx| {
            if let Some((local, viewport)) =
                state.local_position(event.position).zip(state.viewport())
            {
                state
                    .camera
                    .zoom_at(state.camera.zoom * (1. + event.delta), local, viewport);
                cx.notify();
            }
        }))
        .on_click(cx.listener(|state, event: &gpui_kit::ClickEvent, _, cx| {
            let Some((local, viewport)) =
                state.local_position(event.position()).zip(state.viewport())
            else {
                return;
            };
            state.selected = state.snapshot.as_ref().and_then(|s| {
                s.nodes
                    .iter()
                    .filter_map(|node| {
                        let point = state.camera.world_to_screen(node.position, viewport);
                        let distance = (point.x - local.x).hypot(point.y - local.y);
                        (distance <= (node.radius * state.camera.zoom).max(24.))
                            .then_some((node, distance))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(node, _)| node.relative_path.clone())
            });
            cx.notify();
        }))
}

impl GraphState {
    pub(super) fn touch_layout(&self, canvas: AnyElement, cx: &Context<Self>) -> AnyElement {
        let Some(snapshot) = &self.snapshot else {
            return canvas;
        };
        let active = self
            .selected
            .as_ref()
            .and_then(|p| snapshot.nodes.iter().position(|n| n.relative_path == *p));
        let mut controls = h_flex().w_full().items_center().bg(cx.theme().background);
        for (id, symbol, name, factor) in [
            ("graph-out", "−", "Zoom out", 0.8),
            ("graph-in", "+", "Zoom in", 1.25),
            ("graph-fit", "⊡", "Fit graph", 0.),
        ] {
            controls = controls.child(
                Button::new(id)
                    .ghost()
                    .w(px(48.))
                    .h(px(48.))
                    .label(symbol)
                    .accessibility_label(name)
                    .on_click(cx.listener(move |state, _, _, cx| {
                        if factor == 0. {
                            state.reset_zoom(cx);
                        } else {
                            state.zoom_by(factor, cx);
                        }
                    })),
            );
        }
        controls = controls.child(Self::note_picker(None, cx));
        let mut screen = v_flex()
            .size_full()
            .child(div().flex_1().min_h_0().child(canvas));
        if let Some((ix, node)) = active.and_then(|ix| Some((ix, snapshot.nodes.get(ix)?))) {
            let path = node.relative_path.clone();
            let handler = self.handler.clone();
            screen = screen.child(
                h_flex()
                    .px_2()
                    .gap_1()
                    .child(
                        Button::new("open-graph-note")
                            .ghost()
                            .flex_1()
                            .h(px(48.))
                            .label(node.label.clone())
                            .accessibility_label(format!("Open {}", path.display()))
                            .on_click(move |_, _, cx| {
                                let _ = handler.update(cx, |_, cx| {
                                    cx.emit(FileHandlerEvent::VaultFileRequested(
                                        path.clone(),
                                        false,
                                    ));
                                });
                            }),
                    )
                    .child(Self::note_picker(Some(ix), cx))
                    .child(
                        Button::new("close-selection")
                            .ghost()
                            .h(px(48.))
                            .icon(IconName::Close)
                            .accessibility_label("Clear selection")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.selected = None;
                                cx.notify();
                            })),
                    ),
            );
        }
        screen.child(controls).into_any_element()
    }

    fn center_note(&mut self, path: &std::path::Path) {
        self.selected = Some(path.to_owned());
        if let Some(node) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.nodes.iter().find(|n| n.relative_path == path))
        {
            self.camera.pan = gpui_kit::point(
                -node.position.x * self.camera.zoom,
                -node.position.y * self.camera.zoom,
            );
        }
    }

    fn note_picker(source: Option<usize>, cx: &Context<Self>) -> impl IntoElement + use<> {
        let state = cx.weak_entity();
        let title = if source.is_some() {
            "Connections"
        } else {
            "Notes"
        };
        Button::new(title)
            .ghost()
            .h(px(48.))
            .label(title)
            .dropdown_caret(true)
            .dropdown_menu(move |menu, window, cx| {
                let mut menu = menu.scrollable(true).max_h(px(f32::from(
                    window.fully_visible_bounds().size.height,
                ) * 0.55));
                let Some(entity) = state.upgrade() else {
                    return menu;
                };
                let Some(snapshot) = &entity.read(cx).snapshot else {
                    return menu;
                };
                let focus = source.map(|ix| GraphFocus::new(snapshot, ix));
                let mut nodes: Vec<_> = snapshot
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(ix, _)| {
                        Some(*ix) != source && focus.as_ref().is_none_or(|f| f.includes_node(*ix))
                    })
                    .collect();
                nodes.sort_by(|(_, a), (_, b)| {
                    a.label
                        .cmp(&b.label)
                        .then_with(|| a.relative_path.cmp(&b.relative_path))
                });
                let mut labels = std::collections::HashSet::new();
                let duplicates: std::collections::HashSet<_> = nodes
                    .iter()
                    .filter_map(|(_, n)| (!labels.insert(&n.label)).then_some(&n.label))
                    .collect();
                for (_, node) in nodes {
                    let state = state.clone();
                    let path = node.relative_path.clone();
                    menu = menu.item(
                        PopupMenuItem::new(if duplicates.contains(&node.label) {
                            format!(
                                "{} · {}",
                                node.label,
                                path.parent()
                                    .and_then(|p| p.to_str())
                                    .filter(|p| !p.is_empty())
                                    .unwrap_or("Vault")
                            )
                        } else {
                            node.label.clone()
                        })
                        .on_click(move |_, _, cx| {
                            let _ = state.update(cx, |s, cx| {
                                s.center_note(&path);
                                cx.notify();
                            });
                        }),
                    );
                }
                menu
            })
    }
}

pub(super) fn selected_index(
    snapshot: &super::GraphSnapshot,
    path: Option<&std::path::Path>,
) -> Option<usize> {
    path.and_then(|path| snapshot.nodes.iter().position(|n| n.relative_path == path))
}
