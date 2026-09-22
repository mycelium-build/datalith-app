use super::*;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::scroll::ScrollableElement as _;

impl ThemeEditor {
    fn render_selection(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(div().text_sm().child("Active theme"))
            .child(
                Select::new(&self.selector)
                    .w_full()
                    .small()
                    .search_placeholder("Search themes…")
                    .accessibility_label("Active theme"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("new-theme")
                            .label("Create from current…")
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request(Pending::New, window, cx);
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if self.draft.mode().is_dark() {
                                "Dark"
                            } else {
                                "Light"
                            }),
                    ),
            )
    }

    fn render_fonts(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex().gap_3()
            .child(div().text_base().child("Fonts"))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Theme fonts apply to roles set to Theme in Settings. Application default leaves a role undefined."))
            .children(fonts::has_personal_fonts(cx).then(|| h_flex().gap_2().flex_wrap()
                .child(div().text_sm().child("Personal fonts are active."))
                .child(Button::new("editor-use-theme-fonts").small().label("Use theme fonts")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Err(error) = fonts::use_theme_fonts(cx) { this.error = Some(error.to_string()); }
                        cx.notify();
                    })))))
            .children(self.fonts.iter().map(|(role, state)| {
                let label = match role { FontRole::Interface => "Interface", FontRole::Reading => "Reading", FontRole::Headings => "Headings", FontRole::Code => "Code and editing" };
                let family = self.draft.font(*role).filter(|font| cx.global::<FontCatalog>().contains(font))
                    .map_or_else(|| fonts::default_family(*role, cx), |font| font.to_owned().into());
                v_flex().gap_1()
                    .child(div().text_sm().child(label))
                    .child(Select::new(state).w_full().small().search_placeholder("Search fonts…").accessibility_label(label))
                    .child(div().text_sm().font_family(family).child(match role {
                        FontRole::Interface => "Datalith · Files · Settings",
                        FontRole::Reading => "The quick brown fox · À bientôt",
                        FontRole::Headings => "A heading for your notes",
                        FontRole::Code => "let answer = 42; // 0O 1Il {} []",
                    }))
            }))
    }

    fn render_colors(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(div().text_base().child("Colors"))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Choose any theme color. Changes appear immediately in the workspace."),
            )
            .child(
                h_flex().gap_2().flex_wrap().children(
                    [
                        ("background", cx.theme().background),
                        ("foreground", cx.theme().foreground),
                        ("primary.background", cx.theme().primary),
                        ("border", cx.theme().border),
                        ("sidebar.background", cx.theme().sidebar),
                        ("accent.background", cx.theme().accent),
                    ]
                    .into_iter()
                    .map(|(token, color)| {
                        Button::new(SharedString::from(format!("theme-swatch-{token}")))
                            .small()
                            .label(" ")
                            .bg(color)
                            .tooltip(token)
                            .accessibility_label(token)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.color_token.update(cx, |state, cx| {
                                    state.set_selected_value(&token.into(), window, cx);
                                });
                                this.sync_color(window, cx);
                                cx.notify();
                            }))
                    }),
                ),
            )
            .child(
                Select::new(&self.color_token)
                    .w_full()
                    .small()
                    .search_placeholder("Search colors…")
                    .accessibility_label("Theme color"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.color_hex)
                                .id("theme-color-value")
                                .aria_label("Color value")
                                .small(),
                        ),
                    )
                    .child(
                        ColorPicker::new(&self.color_picker)
                            .small()
                            .accessibility_label("Choose color"),
                    )
                    .child(
                        Button::new("reset-theme-color")
                            .small()
                            .label("Reset")
                            .tooltip("Use this mode's default color")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.color_hex
                                    .update(cx, |input, cx| input.set_value("", window, cx));
                            })),
                    ),
            )
            .children(self.color_error.as_ref().map(|error| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
    }

    fn render_footer(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut footer = v_flex()
            .flex_none()
            .gap_2()
            .p_4()
            .border_t_1()
            .border_color(cx.theme().border)
            .children(self.error.as_ref().map(|error| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }));
        if self.pending.is_some() {
            footer = footer
                .child(
                    div()
                        .text_sm()
                        .child("Save your theme changes before leaving?"),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(
                            Button::new("theme-continue")
                                .small()
                                .label("Continue editing")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.pending = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("theme-discard")
                                .small()
                                .label("Discard")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(pending) = this.pending.take() {
                                        this.proceed(pending, window, cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new("theme-save-and-leave")
                                .primary()
                                .small()
                                .label("Save")
                                .disabled(!self.dirty || self.color_error.is_some())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let pending = this.pending.clone();
                                    if this.save(window, cx)
                                        && let Some(pending) = pending
                                    {
                                        this.proceed(pending, window, cx);
                                    }
                                })),
                        ),
                );
        } else {
            footer = footer.child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if self.dirty {
                                "Unsaved preview"
                            } else {
                                "Saved appearance"
                            }),
                    )
                    .child(
                        Button::new("save-theme")
                            .primary()
                            .small()
                            .label("Save theme")
                            .disabled(!self.dirty || self.color_error.is_some())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save(window, cx);
                            })),
                    ),
            );
        }
        footer
    }
}

impl Render for ThemeEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().id("theme-editor").size_full().min_h_0().overflow_hidden()
            .bg(cx.theme().background).text_color(cx.theme().foreground)
            .test_support()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && this.pending.take().is_some() {
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(div().id("theme-editor-scroll").flex_1().min_h_0()
                .child(v_flex().p_4().gap_6().w_full().max_w(rems(72.)).mx_auto()
                    .child(h_flex().items_start().gap_6().flex_wrap()
                        .child(div().flex_1().flex_basis(rems(20.)).min_w_0().child(self.render_selection(cx)))
                        .child(v_flex().flex_1().flex_basis(rems(20.)).min_w_0().gap_2()
                            .child(div().text_sm().child("Save as"))
                            .child(Input::new(&self.name).id("theme-name").aria_label("Save theme as").small())
                            .child(div().text_sm().text_color(cx.theme().muted_foreground)
                                .child("Built-in themes stay available. A new name creates a separate custom theme."))))
                    .child(h_flex().items_start().gap_6().flex_wrap()
                        .child(div().flex_1().flex_basis(rems(20.)).min_w_0().child(self.render_colors(cx)))
                        .child(div().flex_1().flex_basis(rems(20.)).min_w_0().child(self.render_fonts(cx)))))
                .overflow_y_scrollbar())
            .child(self.render_footer(cx))
    }
}
