//! The parts of the app outside the main window: the menu bar item with its
//! popover, and what closing, reopening and a second launch do.
//!
//! Downloads belong to the core, not to a window, so closing the main window
//! only hides it. The app ends when the user quits it.

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Manager, PhysicalPosition, Rect, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

use crate::constants::{MAIN_WINDOW, MENUBAR_WINDOW, PRODUCT_NAME, TRAY_ID};

/// The popover's size in the design.
const POPOVER_WIDTH: f64 = 360.0;
const POPOVER_HEIGHT: f64 = 476.0;

/// Creates the menu bar item and its hidden popover.
pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    let popover =
        WebviewWindowBuilder::new(app, MENUBAR_WINDOW, WebviewUrl::App("index.html".into()))
            .title(PRODUCT_NAME)
            .inner_size(POPOVER_WIDTH, POPOVER_HEIGHT)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .build()?;
    // A popover goes away when the user clicks elsewhere.
    let window = popover.clone();
    popover.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            let _ = window.hide();
        }
    });

    TrayIconBuilder::with_id(TRAY_ID)
        // The black-only mark from design/logo, as a template so macOS tints it.
        .icon(Image::from_bytes(include_bytes!(
            "../icons/tray-template@2x.png"
        ))?)
        .icon_as_template(true)
        .tooltip(PRODUCT_NAME)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                toggle_popover(tray.app_handle(), rect);
            }
        })
        .build(app)?;
    Ok(())
}

/// Shows the popover centred under the menu bar item, or hides it if it is open.
fn toggle_popover(app: &AppHandle, icon: Rect) {
    let Some(popover) = app.get_webview_window(MENUBAR_WINDOW) else {
        return;
    };
    if popover.is_visible().unwrap_or(false) {
        let _ = popover.hide();
        return;
    }
    let scale = popover.scale_factor().unwrap_or(1.0);
    let position = icon.position.to_physical::<f64>(scale);
    let size = icon.size.to_physical::<f64>(scale);
    let x = (position.x + size.width / 2.0 - POPOVER_WIDTH * scale / 2.0).max(0.0);
    let y = position.y + size.height;
    let _ = popover.set_position(PhysicalPosition::new(x, y));
    let _ = popover.show();
    let _ = popover.set_focus();
}

/// Brings the main window back, from the popover, the Dock or a second launch.
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    if let Some(popover) = app.get_webview_window(MENUBAR_WINDOW) {
        let _ = popover.hide();
    }
}

/// Shows the total speed beside the menu bar item while something is downloading.
pub fn set_menu_bar_speed(app: &AppHandle, bytes_per_second: u64) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let title = (bytes_per_second > 0)
        .then(|| format!("{:.1} MB/s", bytes_per_second as f64 / (1024.0 * 1024.0)));
    let _ = tray.set_title(title);
}
