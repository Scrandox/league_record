use tauri::menu::{CheckMenuItemBuilder, Menu, MenuBuilder, MenuEvent, MenuItemBuilder};
use tauri::tray::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{async_runtime, AppHandle, Manager, Wry};

use super::{AppManager, AppWindow, WindowManager};
use crate::constants::{self, menu_item, EXIT_SUCCESS};
use crate::recorder::LeagueRecorder;
use crate::state::{RecordingStatus, Shutdown, TrayState};

pub trait SystemTrayManager {
    fn init_tray_menu(&self);

    fn set_tray_menu_update_available(&self, update_button: bool);

    /// Rebuilds the tray menu from the current app state.
    fn refresh_tray_menu(&self);
}

fn handle_system_tray_event(tray_icon: &TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = event {
        let app_handle = tray_icon.app_handle() as &AppHandle;
        app_handle.open_window(AppWindow::Main);
    }
}

fn handle_system_tray_menu_event(app_handle: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        menu_item::SETTINGS => app_handle.edit_settings(),
        menu_item::OPEN => app_handle.open_window(AppWindow::Main),
        menu_item::QUIT => {
            app_handle
                .webview_windows()
                .into_values()
                .for_each(|window| _ = window.close());

            async_runtime::spawn({
                let app_handle = app_handle.clone();
                async move {
                    app_handle.state::<windows_key_listener::KeyListener>().unlisten();
                    app_handle.state::<LeagueRecorder>().stop().await;

                    app_handle.state::<Shutdown>().set();
                    app_handle.exit(EXIT_SUCCESS);
                }
            });
        }
        menu_item::UPDATE => app_handle.update(),
        _ => {}
    }
}

impl SystemTrayManager for AppHandle {
    fn init_tray_menu(&self) {
        TrayIconBuilder::with_id(constants::TRAY_ID)
            .icon(self.default_window_icon().unwrap().clone())
            .title(constants::APP_NAME)
            .tooltip(constants::APP_NAME)
            .on_tray_icon_event(handle_system_tray_event)
            .menu(&create_tray_menu(self))
            .on_menu_event(handle_system_tray_menu_event)
            .show_menu_on_left_click(false)
            .build(self)
            .unwrap();
    }

    fn set_tray_menu_update_available(&self, update_available: bool) {
        self.state::<TrayState>().set_update_available(update_available);
        self.refresh_tray_menu();
    }

    fn refresh_tray_menu(&self) {
        // .unwrap on everything because creating the tray-icon is always the same and should never fail
        self.tray_by_id(constants::TRAY_ID)
            .unwrap()
            .set_menu(Some(create_tray_menu(self)))
            .unwrap();
    }
}

fn create_tray_menu(app_handle: &AppHandle) -> Menu<Wry> {
    let recording = app_handle.state::<RecordingStatus>().current().is_some();
    let update_available = app_handle.state::<TrayState>().update_available();

    // build the check item in its final state - toggling it after the menu is built doesn't redraw it
    let recording_item = CheckMenuItemBuilder::with_id(menu_item::RECORDING, "Recording")
        .checked(recording)
        .enabled(false)
        .build(app_handle)
        .unwrap();
    let settings = MenuItemBuilder::new("Settings")
        .id(menu_item::SETTINGS)
        .build(app_handle)
        .unwrap();
    let open = MenuItemBuilder::new("Open")
        .id(menu_item::OPEN)
        .build(app_handle)
        .unwrap();
    let quit = MenuItemBuilder::new("Quit")
        .id(menu_item::QUIT)
        .build(app_handle)
        .unwrap();
    let update = MenuItemBuilder::new("Update")
        .id(menu_item::UPDATE)
        .build(app_handle)
        .unwrap();

    let tray_menu = if update_available {
        MenuBuilder::new(app_handle)
            .item(&recording_item)
            .separator()
            .item(&settings)
            .item(&open)
            .item(&quit)
            .separator()
            .item(&update)
    } else {
        MenuBuilder::new(app_handle)
            .item(&recording_item)
            .separator()
            .item(&settings)
            .item(&open)
            .item(&quit)
    }
    .build()
    .unwrap();

    tray_menu
}
