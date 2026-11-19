use super::workspace::{Command, ScreenControls, Workspace};
use gpui_kit::{App, Entity, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::RefCell,
    ffi::{CStr, CString, c_char, c_void},
};

unsafe extern "C" {
    fn UIAccessibilityIsReduceMotionEnabled() -> bool;
    fn datalith_install_controls(view: *mut c_void);
    fn datalith_set_controls_json(json: *const c_char, background: u32);
    fn datalith_preferred_text_size() -> f64;
}

/// Host events that are not user commands.
pub const LAYOUT: &str = "layout";
pub const FOLDER_PICKED: &str = "folder-picked";
pub const TEXT_SIZE: &str = "text-size";

type Dispatch = Box<dyn FnMut(&str)>;
thread_local! {
    static ACTION: RefCell<Option<Dispatch>> = RefCell::default();
    static APPLIED: RefCell<String> = RefCell::default();
}

pub fn attach(view: &Entity<Workspace>, window: &Window, cx: &App) {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::UiKit(native) = handle.as_raw() else {
        return;
    };
    let handle = window.window_handle();
    let view = view.downgrade();
    let mut app = cx.to_async();
    ACTION.with_borrow_mut(|slot| {
        *slot = Some(Box::new(move |id| {
            let _ = handle.update(&mut app, |_, window, cx| {
                view.update(cx, |v, cx| match id {
                    LAYOUT => cx.notify(),
                    FOLDER_PICKED => v.receive_folder(window, cx),
                    TEXT_SIZE => v.system_text_size_changed(cx),
                    id => {
                        if let Some(command) = Command::from_id(id) {
                            v.run(command, None, window, cx);
                        }
                    }
                })
            });
        }));
    });
    // UIKit retains the window's content controller on the main thread.
    unsafe {
        datalith_install_controls(native.ui_view.as_ptr());
    }
}

pub fn dispatch(id: &str) {
    ACTION.with_borrow_mut(|slot| {
        if let Some(dispatch) = slot {
            dispatch(id);
        }
    });
}

/// Called by `Controls.m` with a command id or a host event.
#[unsafe(no_mangle)]
pub extern "C" fn datalith_mobile_command(id: *const c_char) {
    if id.is_null() {
        return;
    }
    let id = unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned();
    dispatch(&id);
}

/// Sends the bar model to `UIKit` when it changes. Called during main-thread
/// layout; `UIKit` schedules and coalesces the update.
pub fn update(controls: &ScreenControls, background: gpui_kit::Hsla) {
    let Ok(json) = serde_json::to_string(controls) else {
        return;
    };
    let background: u32 = gpui_kit::Rgba::from(background).into();
    let key = format!("{background}:{json}");
    if APPLIED.with_borrow(|applied| *applied == key) {
        return;
    }
    let Ok(json) = CString::new(json) else {
        return;
    };
    APPLIED.with_borrow_mut(|applied| *applied = key);
    unsafe {
        datalith_set_controls_json(json.as_ptr(), background);
    }
}

/// The Dynamic Type body size, in points, when `UIKit` reports a usable one.
pub fn preferred_text_size() -> Option<f32> {
    let size = unsafe { datalith_preferred_text_size() };
    // Dynamic Type sizes are whole points from 14 to 53.
    format!("{size:.0}").parse().ok().filter(|s: &f32| *s > 0.)
}

/// `UIKit` exposes the active device preference on its main thread.
pub fn reduce_motion() -> bool {
    unsafe { UIAccessibilityIsReduceMotionEnabled() }
}
