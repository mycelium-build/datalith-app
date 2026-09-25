//! General settings for opening existing documents.

use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem};
use gpui_kit::{App, SharedString};

use crate::app::settings;
use crate::document::handler::ViewMode;
use crate::ui::notifications;

use super::SettingsView;

impl SettingsView {
    pub(super) fn general_groups(&self) -> Vec<SettingGroup> {
        let mut groups = vec![Self::open_new_tab_mode_group()];
        if self.has_updater {
            groups.push(Self::updates_group());
        }
        groups
    }

    fn open_new_tab_mode_group() -> SettingGroup {
        SettingGroup::new().title("Documents").items(vec![
            SettingItem::new(
                "Mode when opening in a new tab",
                SettingField::scrollable_dropdown(
                    vec![
                        ("view".into(), "Reading".into()),
                        ("edit".into(), "Editing".into()),
                    ],
                    |_| match settings::snapshot().open_new_tab_mode() {
                        ViewMode::View => "view".into(),
                        ViewMode::Edit => "edit".into(),
                    },
                    |value: SharedString, cx: &mut App| {
                        let mode = match value.as_str() {
                            "view" => ViewMode::View,
                            "edit" => ViewMode::Edit,
                            _ => return,
                        };
                        if let Err(error) = settings::set_open_new_tab_mode(mode) {
                            notifications::push_window_notification(
                                cx,
                                notifications::settings_save_failed("new-tab mode", &error),
                            );
                        }
                        cx.refresh_windows();
                    },
                ),
            )
            .description("Mode for existing documents opened in a new tab."),
        ])
    }

    fn updates_group() -> SettingGroup {
        SettingGroup::new().title("Updates").items(vec![
            SettingItem::new(
                format!(
                    "Automatically update {}",
                    crate::channel::Channel::current().product_name()
                ),
                SettingField::switch(
                    |_| settings::snapshot().automatic_updates,
                    |enabled, cx| {
                        if let Err(error) = settings::set_automatic_updates(enabled) {
                            notifications::push_window_notification(
                                cx,
                                notifications::settings_save_failed("automatic updates", &error),
                            );
                        }
                        cx.refresh_windows();
                    },
                ),
            )
            .description(
                if crate::channel::Channel::current() == crate::channel::Channel::Preview {
                    "Periodically check for and download release candidates."
                } else {
                    "Periodically check for and download updates."
                },
            ),
        ])
    }
}
