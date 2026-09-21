use gpui_kit::base::actions::{Cancel, SelectLeft, SelectRight};
use gpui_kit::component::{
    IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{PopupMenu, PopupMenuItem},
};
use gpui_kit::{
    ClickEvent, Context, DismissEvent, Entity, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, MouseButton, OwnedMenu, OwnedMenuItem,
    ParentElement as _, Render, Role, StatefulInteractiveElement as _, Styled as _, Subscription,
    Window, anchored, deferred, div, prelude::FluentBuilder as _, px,
};

struct OpenMenu {
    ix: usize,
    popup: Entity<PopupMenu>,
    _dismiss: Subscription,
}

/// Owns the transient menu session; `PopupMenu` owns item navigation and dismissal.
/// GPUI Kit 0.6.1's `AppMenuBar` exposes no session control or events to support
/// collapsing back to the burger and search controls when a menu closes.
pub struct ApplicationMenu {
    menus: Vec<(&'static str, OwnedMenu)>,
    open: Option<OpenMenu>,
    action_context: Option<FocusHandle>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl ApplicationMenu {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus_out(&focus_handle, window, |this, _, window, cx| {
                // A newly opened popup is not in the previous frame's focus tree yet.
                let entering_popup = this
                    .open
                    .as_ref()
                    .is_some_and(|open| open.popup.focus_handle(cx).contains_focused(window, cx));
                if !entering_popup {
                    this.close(false, window, cx);
                }
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.close(true, window, cx);
                }
            }),
        ];
        Self {
            menus: crate::app::menus::definitions(cx)
                .into_iter()
                .map(|(id, menu)| (id, menu.owned()))
                .collect(),
            open: None,
            action_context: None,
            focus_handle,
            _subscriptions: subscriptions,
        }
    }

    fn open(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        // A header click is outside PopupMenu's bounds, so the popup may have
        // already yielded focus before this handler runs. Replace that closing
        // popup even when the pointer just selected the same header by hovering.
        if self.open.as_ref().is_some_and(|open| {
            open.ix == ix && open.popup.focus_handle(cx).contains_focused(window, cx)
        }) {
            return;
        }
        let Some((_, menu)) = self.menus.get(ix) else {
            return;
        };
        if menu.disabled {
            return;
        }
        if self.open.is_none() {
            self.action_context = window.focused(cx);
        }
        let items = menu.items.clone();
        let action_context = self.action_context.clone();
        let popup = PopupMenu::build(window, cx, |menu, window, cx| {
            populate_menu(menu, &items, action_context.as_ref(), window, cx)
        });
        let dismiss = cx.subscribe_in(
            &popup,
            window,
            |this, menu, _: &DismissEvent, window, cx| {
                // A dismissal from the previous popup must not close a newly selected menu.
                if this.open.as_ref().is_some_and(|open| &open.popup == menu) {
                    this.close(false, window, cx);
                }
            },
        );
        self.open = Some(OpenMenu {
            ix,
            popup: popup.clone(),
            _dismiss: dismiss,
        });
        popup.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn close(&mut self, restore_focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.open.take().is_none() {
            return;
        }
        if let Some(focus) = self.action_context.take()
            && restore_focus
        {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open.is_some() {
            self.close(true, window, cx);
        } else {
            self.open(0, window, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = &self.open {
            let ix = open
                .ix
                .checked_sub(1)
                .unwrap_or_else(|| self.menus.len().saturating_sub(1));
            self.open(ix, window, cx);
        }
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = &self.open {
            let next = open.ix.saturating_add(1);
            let ix = if next < self.menus.len() { next } else { 0 };
            self.open(ix, window, cx);
        }
    }

    fn render_menu(
        &self,
        ix: usize,
        id: &'static str,
        menu: &OwnedMenu,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let selected = self.open.as_ref().is_some_and(|open| open.ix == ix);
        div()
            .id(id)
            .relative()
            .child(
                Button::new("menu")
                    .ghost()
                    .small()
                    .compact()
                    .label(menu.name.clone())
                    .selected(selected)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.open(ix, window, cx);
                        }),
                    )
                    .on_click(cx.listener(move |this, event, window, cx| {
                        if !matches!(event, ClickEvent::Mouse(_)) {
                            this.open(ix, window, cx);
                        }
                    }))
                    .on_hover(cx.listener(move |this, hovered, window, cx| {
                        if *hovered && this.open.is_some() {
                            this.open(ix, window, cx);
                        }
                    })),
            )
            .when(selected, |element| {
                let popup = self.open.as_ref().map(|open| open.popup.clone());
                element.child(deferred(
                    anchored()
                        .snap_to_window_with_margin(px(window.rem_size().as_f32() * 0.5))
                        .child(div().top_1().occlude().children(popup)),
                ))
            })
    }
}

impl Render for ApplicationMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("title-bar-menu")
            .track_focus(&self.focus_handle)
            // Reuse GPUI's menu-bar actions; PopupMenu propagates Left/Right here.
            .key_context("AppMenuBar")
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(|this, _: &Cancel, window, cx| this.close(true, window, cx)))
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .gap_3()
            .child(
                Button::new("app-menu-trigger")
                    .ghost()
                    .small()
                    .icon(IconName::Menu)
                    .accessibility_label("Application menu")
                    .tooltip("Application menu")
                    .selected(self.open.is_some())
                    .toggled(self.open.is_some())
                    // Keep its focus identity for keyboard restoration, but remove
                    // it from layout and interaction while Datalith takes its place.
                    .when(self.open.is_some(), |button| {
                        button.absolute().invisible().tab_stop(false)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.toggle(window, cx);
                        }),
                    )
                    .on_click(cx.listener(|this, event, window, cx| {
                        cx.stop_propagation();
                        if !matches!(event, ClickEvent::Mouse(_)) {
                            this.toggle(window, cx);
                        }
                    })),
            )
            .map(|row| {
                if self.open.is_some() {
                    row.child(
                        h_flex()
                            .id("app-menu-bar")
                            .role(Role::MenuBar)
                            .min_w_0()
                            .gap_1()
                            .overflow_x_scroll()
                            .children(self.menus.iter().enumerate().map(|(ix, (id, menu))| {
                                self.render_menu(ix, id, menu, window, cx)
                            })),
                    )
                } else {
                    row.child(super::search_controls())
                }
            })
            // This region remains draggable even when the menu labels need to scroll.
            .child(div().flex_1().min_w_12().h_full())
    }
}

fn populate_menu(
    mut menu: PopupMenu,
    items: &[OwnedMenuItem],
    action_context: Option<&FocusHandle>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    if let Some(focus) = action_context {
        menu = menu.action_context(focus.clone());
    }
    for item in items {
        menu = match item {
            OwnedMenuItem::Action {
                name,
                action,
                checked,
                disabled,
                ..
            } => menu.item(
                PopupMenuItem::new(name.clone())
                    .action(action.boxed_clone())
                    .checked(*checked)
                    .disabled(*disabled),
            ),
            OwnedMenuItem::Separator => menu.separator(),
            OwnedMenuItem::Submenu(submenu) => {
                let items = submenu.items.clone();
                let action_context = action_context.cloned();
                menu.submenu(submenu.name.clone(), window, cx, move |menu, window, cx| {
                    populate_menu(menu, &items, action_context.as_ref(), window, cx)
                })
            }
            OwnedMenuItem::SystemMenu(_) => menu,
        };
    }
    menu
}
