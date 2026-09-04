//! Minimal Windows system tray.
//!
//! The rich tray in `tray_menu.rs` is macOS-only, and on Windows closing the
//! main window used to quit Buzz outright — so background notifications
//! stopped the moment the window was closed, with no way to keep Buzz running
//! short of leaving the window open or minimised.
//!
//! This module adds just enough tray presence for the main window to hide
//! instead of quitting: an icon with `Open Buzz` and `Quit Buzz`, and a
//! double-click that restores the window. It deliberately stays a static
//! two-item menu rather than mirroring the macOS agent-activity menu; that
//! menu's dynamic sections are tied to macOS-only presentation APIs.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

const TRAY_ID: &str = "buzz-tray-windows";
const OPEN_BUZZ_ID: &str = "tray-open-buzz";
const QUIT_ID: &str = "tray-quit";

/// Set once the tray icon is live. Hiding the main window is only safe while
/// the tray exists, because the tray is the only way back to a hidden window.
static TRAY_READY: AtomicBool = AtomicBool::new(false);

/// Whether closing the main window may hide it instead of quitting Buzz.
pub(crate) fn hide_to_tray_enabled() -> bool {
    TRAY_READY.load(Ordering::Relaxed)
}

/// Restores the main window after it was hidden to the tray.
///
/// Mirrors `tray_menu::show_main_window`: unminimize before show, because a
/// window that was minimised before being hidden stays minimised otherwise.
pub(crate) fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.unminimize() {
        eprintln!("buzz-desktop: failed to restore main window from tray: {error}");
        return;
    }
    if let Err(error) = window.show() {
        eprintln!("buzz-desktop: failed to show main window from tray: {error}");
        return;
    }
    if let Err(error) = window.set_focus() {
        eprintln!("buzz-desktop: failed to focus main window from tray: {error}");
    }
}

/// Installs the tray icon.
///
/// A failure here is reported and swallowed rather than aborting startup: Buzz
/// stays fully usable without a tray, and `hide_to_tray_enabled` then keeps
/// the close button quitting as before, so a missing tray can never strand the
/// user with an invisible running app.
pub(crate) fn init<R: Runtime>(app: &AppHandle<R>) {
    if let Err(error) = build_tray(app) {
        eprintln!("buzz-desktop: failed to install the Windows tray icon: {error}");
        return;
    }
    TRAY_READY.store(true, Ordering::Relaxed);
}

fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, OPEN_BUZZ_ID, "Open Buzz", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, QUIT_ID, "Quit Buzz", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Buzz")
        .on_menu_event(|app, event| match event.id.as_ref() {
            OPEN_BUZZ_ID => show_main_window(app),
            // `exit` runs the normal shutdown path (RunEvent::ExitRequested),
            // so this is a real quit and not just a hidden window.
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}
