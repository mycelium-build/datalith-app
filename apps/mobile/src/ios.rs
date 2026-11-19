use super::workspace::Workspace;
use gpui_kit::{App, Entity, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::RefCell, ffi::c_void};

unsafe extern "C" {
    fn UIAccessibilityIsReduceMotionEnabled() -> bool;
    fn datalith_install_controls(view: *mut c_void);
    fn datalith_set_controls(back: bool, mode: i32, writable: bool, background: u32);
}

type Dispatch = Box<dyn FnMut(i32)>;
thread_local! { static ACTION: RefCell<Option<Dispatch>> = RefCell::default(); }

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
        *slot = Some(Box::new(move |action| {
            let _ = handle.update(&mut app, |_, window, cx| {
                view.update(cx, |v, cx| match action {
                    0 => v.back(window, cx),
                    1 => v.toggle_mode(window, cx),
                    2 => v.delete(cx),
                    3 => cx.notify(),
                    _ => {}
                })
            });
        }));
    });
    // UIKit retains the window's content controller on the main thread.
    unsafe {
        datalith_install_controls(native.ui_view.as_ptr());
    }
}

#[unsafe(no_mangle)]
extern "C" fn datalith_mobile_action(action: i32) {
    ACTION.with_borrow_mut(|slot| {
        if let Some(dispatch) = slot {
            dispatch(action);
        }
    });
}

pub fn update(
    can_back: bool,
    handler: Option<&datalith_ui::handler::FileHandler>,
    background: gpui_kit::Hsla,
) {
    let mode = handler
        .filter(|h| h.can_toggle_mode())
        .map_or(0, |h| if h.is_editing() { 2 } else { 1 });
    // Called during main-thread layout. UIKit schedules and coalesces updates.
    unsafe {
        datalith_set_controls(
            can_back,
            mode,
            handler.is_some_and(|h| !h.is_read_only()),
            gpui_kit::Rgba::from(background).into(),
        );
    }
}

/// `UIKit` exposes the active device preference on its main thread.
pub fn reduce_motion() -> bool {
    unsafe { UIAccessibilityIsReduceMotionEnabled() }
}
