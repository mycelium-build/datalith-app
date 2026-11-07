//! Dock icon for unbundled macOS runs.
//!
//! Packaged builds get their icon from the `.app` bundle. A bare executable
//! (`cargo run`, `target/*/datalith`) has no bundle, so the Dock shows the
//! generic executable icon unless the application sets one at runtime.

use objc2::{AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::{NSBundle, NSData, ns_string};

macro_rules! channel_icon {
    ($channel:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/logo/",
            $channel,
            "/datalith-macos.png"
        ))
    };
}

const fn icon_bytes(channel: crate::channel::Channel) -> &'static [u8] {
    use crate::channel::Channel;
    match channel {
        Channel::Stable => channel_icon!("stable"),
        Channel::Preview => channel_icon!("preview"),
        Channel::Dev => channel_icon!("dev"),
    }
}

/// Set the channel's Dock icon when not running from inside a `.app` bundle.
pub fn install(channel: crate::channel::Channel) {
    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    if NSBundle::mainBundle()
        .bundlePath()
        .hasSuffix(ns_string!(".app"))
    {
        return;
    }
    let data = NSData::with_bytes(icon_bytes(channel));
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return;
    };
    // SAFETY: called on the main thread (checked above) with a valid image;
    // AppKit retains the image, so it may be dropped afterwards.
    unsafe {
        NSApplication::sharedApplication(main_thread).setApplicationIconImage(Some(&image));
    }
}
