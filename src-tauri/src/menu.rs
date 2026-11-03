//! The macOS menu bar. Besides the standard App/Edit/Window menus it lists the
//! app's own commands (Check for Updates…, Settings ⌘,, New Note, Show Sidebar, Chat…), so they
//! are discoverable and reachable without the hover-only sidebar.
//!
//! Each custom item emits a `menu` event carrying its id; the UI runs the same
//! action as the keyboard shortcut (and ignores the duplicate when both fire).

use tauri::menu::{AboutMetadata, Menu, MenuBuilder, MenuEvent, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter};

/// Custom items: (id, title, accelerator).
const APP_ITEMS: &[(&str, &str, &str)] = &[
    ("check-updates", "Check for Updates…", ""),
    ("settings", "Settings…", "CmdOrCtrl+,"),
];
const FILE_ITEMS: &[(&str, &str, &str)] = &[("new-note", "New Note", "CmdOrCtrl+N")];
const VIEW_ITEMS: &[(&str, &str, &str)] = &[
    ("toggle-sidebar", "Show or Hide Sidebar", "CmdOrCtrl+\\"),
    ("search", "Search Notes", "CmdOrCtrl+K"),
    ("related", "Related Notes", "CmdOrCtrl+J"),
    ("chat", "Chat", "CmdOrCtrl+L"),
];
const GO_ITEMS: &[(&str, &str, &str)] = &[
    ("go-write", "Write", ""),
    ("go-notes", "All Notes", ""),
    ("go-journal", "Journal", ""),
    ("go-ideas", "Ideas over Time", ""),
    ("go-models", "Language Models", ""),
];

pub fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let item = |(id, title, accel): &(&str, &str, &str)| {
        let b = MenuItemBuilder::with_id(*id, *title);
        if accel.is_empty() { b } else { b.accelerator(*accel) }.build(app)
    };
    let items = |list: &[(&str, &str, &str)]| list.iter().map(item).collect::<tauri::Result<Vec<_>>>();

    let mut app_menu = SubmenuBuilder::new(app, "Noughtpad")
        .about(Some(AboutMetadata::default()))
        .separator();
    for i in items(APP_ITEMS)? {
        app_menu = app_menu.item(&i);
    }
    let app_menu = app_menu
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let mut file = SubmenuBuilder::new(app, "File");
    for i in items(FILE_ITEMS)? {
        file = file.item(&i);
    }
    let file = file.separator().close_window().build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let mut view = SubmenuBuilder::new(app, "View");
    for i in items(VIEW_ITEMS)? {
        view = view.item(&i);
    }
    let view = view
        .separator()
        .item(&PredefinedMenuItem::fullscreen(app, None)?)
        .build()?;

    let mut go = SubmenuBuilder::new(app, "Go");
    for i in items(GO_ITEMS)? {
        go = go.item(&i);
    }
    let go = go.build()?;

    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().build()?;

    MenuBuilder::new(app)
        .items(&[&app_menu, &file, &edit, &view, &go, &window])
        .build()
}

pub fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    let known = [APP_ITEMS, FILE_ITEMS, VIEW_ITEMS, GO_ITEMS]
        .iter()
        .any(|l| l.iter().any(|(i, ..)| *i == id));
    if known {
        crate::lifecycle::show_main(app);
        let _ = app.emit("menu", id);
    }
}
