use super::workspace::Workspace;
use gpui_kit::{App, Entity, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;

unsafe extern "C" {
    fn datalith_refresh_files();
    fn datalith_push_link(path: *const c_char);
    fn datalith_update_controls(mode: i32, writable: bool);
    fn datalith_install_navigation(view: *mut c_void, root: *const c_char);
}

enum Command {
    Open(PathBuf),
    ToggleMode,
    Delete,
}
type OpenDocument = Box<dyn FnMut(Command)>;
thread_local! { static OPEN: RefCell<Option<OpenDocument>> = RefCell::default(); }

/// Receives a UTF-8 filesystem path from the vault column on the UIKit main thread.
#[unsafe(no_mangle)]
unsafe extern "C" fn datalith_open_document(path: *const c_char) {
    if path.is_null() {
        return;
    }
    // UIKit retains the string for the duration of this synchronous call.
    let path = unsafe { CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();
    OPEN.with_borrow_mut(|callback| {
        if let Some(open) = callback {
            open(Command::Open(PathBuf::from(path)));
        }
    });
}

pub(super) fn attach(view: &Entity<Workspace>, window: &Window, cx: &App) {
    let Some(root) = view
        .read(cx)
        .root
        .as_ref()
        .and_then(|p| CString::new(p.as_os_str().as_encoded_bytes()).ok())
    else {
        return;
    };
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::UiKit(native) = handle.as_raw() else {
        return;
    };
    let handle = window.window_handle();
    let view = view.downgrade();
    let mut app = cx.to_async();
    OPEN.with_borrow_mut(|slot| {
        *slot = Some(Box::new(move |command| {
            let _ = handle.update(&mut app, |_, window, cx| {
                view.update(cx, |v, cx| match command {
                    Command::Open(path) => v.open(path, window, cx),
                    Command::ToggleMode => v.toggle_mode(window, cx),
                    Command::Delete => v.delete(cx),
                })
            });
        }))
    });
    // UIKit retains the GPUI controller and copies the directory before returning.
    unsafe {
        datalith_install_navigation(native.ui_view.as_ptr(), root.as_ptr());
    }
}

#[unsafe(no_mangle)]
extern "C" fn datalith_document_action(delete: bool) {
    OPEN.with_borrow_mut(|slot| {
        if let Some(dispatch) = slot {
            dispatch(if delete {
                Command::Delete
            } else {
                Command::ToggleMode
            });
        }
    });
}

pub(super) fn show_link(path: &std::path::Path) {
    if let Ok(path) = CString::new(path.as_os_str().as_encoded_bytes()) {
        // The main-thread host copies the path before scheduling the push.
        unsafe {
            datalith_push_link(path.as_ptr());
        }
    }
}

pub(super) fn update_controls(handler: Option<&datalith_ui::handler::FileHandler>) {
    let mode = handler
        .filter(|h| h.can_toggle_mode())
        .map_or(0, |h| if h.is_editing() { 2 } else { 1 });
    // UIKit coalesces identical state during GPUI rendering.
    unsafe {
        datalith_update_controls(mode, handler.is_some_and(|h| !h.is_read_only()));
    }
}

pub(super) fn refresh_files() {
    // The host schedules the vault list update on its main queue.
    unsafe {
        datalith_refresh_files();
    }
}
