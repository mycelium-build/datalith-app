//! Native hosts send semantic events to the GPUI window. An incoming URL can
//! arrive before the first window is attached on a cold start.
use crate::workspace::Workspace;
use gpui_kit::{Entity, Window};
use std::sync::LazyLock;

#[cfg_attr(
    not(any(target_os = "ios", target_os = "android")),
    expect(dead_code, reason = "Events originate in the native mobile hosts")
)]
enum Event {
    Link(String),
    Choice(i32),
}
static EVENTS: LazyLock<(async_channel::Sender<Event>, async_channel::Receiver<Event>)> =
    LazyLock::new(async_channel::unbounded);
#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn link(url: String) {
    let _ = EVENTS.0.try_send(Event::Link(url));
}
#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn choice(index: i32) {
    let _ = EVENTS.0.try_send(Event::Choice(index));
}

pub fn attach(view: &Entity<Workspace>, w: &Window, cx: &gpui_kit::App) {
    let view = view.downgrade();
    w.spawn(cx, async move |cx| {
        while let Ok(event) = EVENTS.1.recv().await {
            if view
                .update_in(cx, |state, w, cx| match event {
                    Event::Link(url) => state.open_link(&url, w, cx),
                    Event::Choice(index) => state.accept_choice(index, w, cx),
                })
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
}
#[cfg(target_os = "ios")]
pub fn show(title: &str, items: &[String], selected: i32) -> anyhow::Result<()> {
    use std::ffi::{CString, c_char};
    unsafe extern "C" {
        fn datalith_choose(title: *const c_char, json: *const c_char, selected: i32);
    }
    let title = CString::new(title)?;
    let json = CString::new(serde_json::to_string(items)?)?;
    unsafe {
        datalith_choose(title.as_ptr(), json.as_ptr(), selected);
    }
    Ok(())
}
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
extern "C" fn datalith_chose(index: i32) {
    choice(index);
}
#[cfg(target_os = "android")]
pub use crate::android_folder::show_options as show;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn show(_: &str, _: &[String], _: i32) -> anyhow::Result<()> {
    anyhow::bail!("A mobile host is required")
}
