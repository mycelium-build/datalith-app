use gpui_kit::{Action, KeyBindingContextPredicate, Keymap, Menu, MenuItem};

use crate::app::keymap::WORKSPACE_CONTEXT;

/// `AppKit` installs a key equivalent for a menu action even when its GPUI
/// binding only applies to part of the window. Give explicit menu clicks
/// their own unbound action and let GPUI evaluate the original shortcut.
#[derive(Action)]
#[action(namespace = datalith, no_json, no_register)]
struct NativeMenuClick(Box<dyn Action>);

impl Clone for NativeMenuClick {
    fn clone(&self) -> Self {
        Self(self.0.boxed_clone())
    }
}

impl PartialEq for NativeMenuClick {
    fn eq(&self, other: &Self) -> bool {
        self.0.partial_eq(other.0.as_ref())
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn register_native_menu_action(cx: &mut gpui_kit::App) {
    cx.on_action(|click: &NativeMenuClick, cx| {
        let action = click.0.boxed_clone();
        cx.defer(move |cx| cx.dispatch_action(action.as_ref()));
    });
}

pub(super) fn scope_shortcuts(mut menu: Menu, keymap: &Keymap) -> Menu {
    for item in &mut menu.items {
        let MenuItem::Action { action, .. } = item else {
            continue;
        };
        let scoped = keymap.bindings_for_action(action.as_ref()).any(|binding| {
            binding.predicate().is_some_and(|predicate| {
                !matches!(predicate.as_ref(), KeyBindingContextPredicate::Identifier(id) if id == WORKSPACE_CONTEXT)
            })
        });
        if scoped {
            *action = Box::new(NativeMenuClick(action.boxed_clone()));
        }
    }
    menu
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::actions::{NewFile, Quit, Rename};
    use gpui_kit::KeyBinding;

    #[test]
    fn native_clicks_forward_scoped_commands_without_their_key_equivalents() {
        let keymap = Keymap::new(vec![
            KeyBinding::new("cmd-shift-r", Rename, Some("FileTree")),
            KeyBinding::new("cmd-n", NewFile, Some("Workspace && !Modal")),
            KeyBinding::new("cmd-q", Quit, Some(WORKSPACE_CONTEXT)),
        ]);
        let menu = scope_shortcuts(
            Menu::new("Commands").items([
                MenuItem::action("Rename", Rename),
                MenuItem::action("New note", NewFile),
                MenuItem::action("Quit", Quit),
            ]),
            &keymap,
        );
        let commands: [&dyn Action; 3] = [&Rename, &NewFile, &Quit];
        for (item, command) in menu.items.iter().zip(commands) {
            let MenuItem::Action { action, .. } = item else {
                panic!("expected command")
            };
            if command.partial_eq(&Quit) {
                assert!(action.partial_eq(command));
            } else {
                let click = action.as_any().downcast_ref::<NativeMenuClick>().unwrap();
                assert!(click.0.partial_eq(command));
                assert_eq!(keymap.bindings_for_action(click).count(), 0);
            }
        }
    }
}
