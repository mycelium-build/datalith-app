//! Local server settings page: enable, port, token, live status.

use conv::ConvUtil as _;
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::Button,
    h_flex,
    setting::{NumberFieldOptions, SettingField, SettingGroup, SettingItem},
};
use gpui_kit::{App, IntoElement, ParentElement, Styled, div};
use std::fmt::Write as _;

use super::SettingsView;
use crate::app::{settings, system};
use crate::server;
use crate::ui::notifications;

impl SettingsView {
    pub(super) fn server_group() -> SettingGroup {
        SettingGroup::new().title("Local Server").items(vec![
            Self::enabled_item(),
            Self::port_item(),
            Self::token_item(),
            Self::status_item(),
        ])
    }

    fn enabled_item() -> SettingItem {
        SettingItem::new(
            "Enabled",
            SettingField::switch(
                |_cx| settings::snapshot().server.enabled(),
                |enabled: bool, cx| {
                    if let Err(error) = settings::set_server_enabled(enabled) {
                        notifications::push_window_notification(
                            cx,
                            notifications::settings_save_failed("server setting", &error),
                        );
                    }
                    sync_server(cx);
                },
            ),
        )
        .description("Let external apps interact with your vaults, like the Datalith Clipper browser extension.")
    }

    fn port_item() -> SettingItem {
        SettingItem::new(
            "Port",
            SettingField::number_input(
                NumberFieldOptions {
                    min: 1.0,
                    max: f64::from(u16::MAX),
                    step: 1.0,
                },
                |_cx| f64::from(settings::snapshot().server.port()),
                |value: f64, cx| {
                    let rounded = value.round();
                    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
                        return;
                    }
                    let Ok(port) = rounded.approx_as::<u16>() else {
                        return;
                    };
                    if port == settings::snapshot().server.port() {
                        return;
                    }
                    if let Err(error) = settings::set_server_port(port) {
                        notifications::push_window_notification(
                            cx,
                            notifications::settings_save_failed("server port", &error),
                        );
                        return;
                    }
                    sync_server(cx);
                },
            ),
        )
        .description("Local port the server listens on.")
    }

    fn token_item() -> SettingItem {
        SettingItem::render(move |_options, _window, cx| {
            let token = settings::snapshot().server.token().map(str::to_owned);
            let display = token.as_deref().map_or_else(
                || "Not set".to_owned(),
                |token| format!("{}…", token.get(..4).unwrap_or(token)),
            );
            h_flex()
                .flex_1()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_color(cx.theme().muted_foreground)
                        .child(display),
                )
                .child(
                    Button::new("server-token-generate")
                        .small()
                        .label("Generate")
                        .on_click(|_, _, cx| {
                            let token = generate_token();
                            apply_token(cx, Some(token));
                        }),
                )
                .child(
                    Button::new("server-token-copy")
                        .small()
                        .label("Copy")
                        .on_click(|_, _, cx| {
                            let Some(token) =
                                settings::snapshot().server.token().map(str::to_owned)
                            else {
                                return;
                            };
                            if let Err(error) = system::copy_text(&token) {
                                notifications::push_window_notification(
                                    cx,
                                    notifications::copy_token_failed(&error),
                                );
                            }
                        }),
                )
                .child(
                    Button::new("server-token-clear")
                        .small()
                        .label("Clear")
                        .on_click(|_, _, cx| apply_token(cx, None)),
                )
                .into_any_element()
        })
        .description("Optional bearer token clients must send. Empty disables authentication.")
    }

    fn status_item() -> SettingItem {
        SettingItem::render(move |_options, _window, cx| {
            let text = match server::status() {
                server::ServerStatus::Running(port) => {
                    format!("Running on {}:{port}", server::BIND_HOST)
                }
                server::ServerStatus::Stopped => "Stopped".to_owned(),
                server::ServerStatus::Failed(error) => error,
            };
            div()
                .text_color(cx.theme().muted_foreground)
                .child(text)
                .into_any_element()
        })
        .description("External apps connect to this address.")
    }
}

fn apply_token(cx: &mut App, token: Option<String>) {
    if let Err(error) = settings::set_server_token(token) {
        notifications::push_window_notification(
            cx,
            notifications::settings_save_failed("server token", &error),
        );
        return;
    }
    sync_server(cx);
}

fn sync_server(cx: &mut App) {
    if let Err(error) = server::sync() {
        let port = settings::snapshot().server.port();
        notifications::push_window_notification(
            cx,
            notifications::server_start_failed(port, &error),
        );
    }
    cx.refresh_windows();
}

fn generate_token() -> String {
    use rand::Rng as _;
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    let mut token = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write!(token, "{byte:02x}");
    }
    token
}
