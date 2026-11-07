//! Dock icon for unbundled Dev runs on macOS.
//!
//! Packaged builds get their Liquid Glass icon from the `.app` bundle (`Assets.car`).
//! A bare executable (`cargo run`, `target/*/datalith`) has no bundle, so the Dock
//! shows the generic executable icon unless the application sets one at runtime.
//! Only Dev builds compile this (`cfg(datalith_dev)`, set by `build.rs`).

use objc2::{AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::{NSBundle, NSData, ns_string};

/// The Dev icon on Apple's 1024 px grid (an 824 px icon with its margin), written by
/// `scripts/macos_icon.py`. A static rendition: the Dock only draws the dynamic
/// Liquid Glass icon for a bundle's `CFBundleIconName`.
const ICON: &[u8] = include_bytes!(env!("DATALITH_DOCK_ICON"));

/// Set the Dev Dock icon when not running from inside a `.app` bundle.
pub fn install() {
    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    if NSBundle::mainBundle()
        .bundlePath()
        .hasSuffix(ns_string!(".app"))
    {
        return;
    }
    let data = NSData::with_bytes(ICON);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return;
    };
    // SAFETY: called on the main thread (checked above) with a valid image;
    // AppKit retains the image, so it may be dropped afterwards.
    unsafe {
        NSApplication::sharedApplication(main_thread).setApplicationIconImage(Some(&image));
    }
}
