use super::super::touch_choice;
use super::{GraphFocus, GraphState, NoteChoice};
use crate::handler::FileHandlerEvent;
use crate::touch::TouchSize as _;
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
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
        for (id, icon, name, factor) in [
            ("graph-out", IconName::Minus, "Zoom out", 0.8),
            ("graph-in", IconName::Plus, "Zoom in", 1.25),
            ("graph-fit", IconName::Maximize, "Fit graph", 0.),
        ] {
            controls = controls.child(
                Button::new(id)
                    .ghost()
                    .touch_size(48.)
                    .icon(icon)
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
        controls = controls.child(Self::note_picker(
            None,
            self.choosing_note == Some(NoteChoice::All),
            cx,
        ));
        let mut screen = v_flex()
            .size_full()
            .child(div().flex_1().min_h_0().child(canvas));
        if let Some((ix, node)) = active.and_then(|ix| Some((ix, snapshot.nodes.get(ix)?))) {
            screen = screen.child(self.selection_sheet(ix, node, cx));
        }
        if let Some(choice) = self.choosing_note {
            screen = screen.children(self.note_options(choice.source(), cx));
        }
        screen.child(controls).into_any_element()
    }

    fn selection_sheet(
        &self,
        ix: usize,
        node: &super::model::ViewNode,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let path = node.relative_path.clone();
        let handler = self.handler.clone();
        let folder = path
            .parent()
            .map(|p| p.display().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "Vault".into());
        // A selection sheet like the table's: the name is text, and
        // opening is an explicit, labelled action.
        v_flex()
            .px_3()
            .py_2()
            .gap_1()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(node.label.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(folder),
                            ),
                    )
                    .child(
                        Button::new("close-selection")
                            .ghost()
                            .touch_size(44.)
                            .icon(IconName::Close)
                            .accessibility_label("Clear selection")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.selected = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("open-graph-note")
                            .primary()
                            .touch_h(44.)
                            .label("Open note")
                            .on_click(move |_, _, cx| {
                                let _ = handler.update(cx, |_, cx| {
                                    cx.emit(FileHandlerEvent::VaultFileRequested(
                                        path.clone(),
                                        false,
                                    ));
                                });
                            }),
                    )
                    .child(Self::note_picker(
                        Some(ix),
                        self.choosing_note == Some(NoteChoice::Connections(ix)),
                        cx,
                    )),
            )
    }

    /// Back closes the selection sheet before leaving the Base.
    pub(crate) fn clear_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let cleared = self.selected.take().is_some();
        if cleared {
            cx.notify();
        }
        cleared
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

    fn note_picker(
        source: Option<usize>,
        open: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let (id, title) = if source.is_some() {
            ("graph-connections", "Connections")
        } else {
            ("graph-notes", "Notes")
        };
        touch_choice::trigger(id, title, "Choose a note", open).on_click(cx.listener(
            move |s, _, _, cx| {
                let choice = NoteChoice::new(source);
                s.choosing_note = (s.choosing_note != Some(choice)).then_some(choice);
                cx.notify();
            },
        ))
    }

    /// Back closes the note list before the selection and the Base.
    pub(crate) fn close_note_choice(&mut self, cx: &mut Context<Self>) -> bool {
        let closed = self.choosing_note.take().is_some();
        if closed {
            cx.notify();
        }
        closed
    }

    fn note_options(
        &self,
        source: Option<usize>,
        cx: &Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let snapshot = self.snapshot.as_ref()?;
        let focus = source.map(|ix| GraphFocus::new(snapshot, ix));
        let mut nodes: Vec<_> = snapshot
            .nodes
            .iter()
            .enumerate()
            .filter(|(ix, _)| {
                Some(*ix) != source && focus.as_ref().is_none_or(|f| f.includes_node(*ix))
            })
            .map(|(_, node)| node)
            .collect();
        nodes.sort_by(|a, b| {
            a.label
                .cmp(&b.label)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
        let mut labels = std::collections::HashSet::new();
        let duplicates: std::collections::HashSet<_> = nodes
            .iter()
            .filter_map(|n| (!labels.insert(&n.label)).then_some(n.label.clone()))
            .collect();
        let selected = self.selected.clone();
        Some(
            touch_choice::options(
                "graph-note-options",
                nodes.into_iter().map(|node| {
                    let path = node.relative_path.clone();
                    let label = if duplicates.contains(&node.label) {
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
                    };
                    touch_choice::option(
                        gpui_kit::SharedString::from(format!("graph-note:{}", path.display())),
                        label,
                        selected.as_ref() == Some(&path),
                    )
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.choosing_note = None;
                        s.center_note(&path);
                        cx.notify();
                    }))
                }),
            )
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover),
        )
    }
}

pub(super) fn selected_index(
    snapshot: &super::GraphSnapshot,
    path: Option<&std::path::Path>,
) -> Option<usize> {
    path.and_then(|path| snapshot.nodes.iter().position(|n| n.relative_path == path))
}
