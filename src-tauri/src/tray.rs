use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};
use crate::state::{SharedSettings, OutputMode};

/// Prefix used for tray menu items that switch to a specific custom profile.
/// Format: `mode_custom_profile:<id>` (the id comes from CustomProfile).
const PROFILE_ITEM_PREFIX: &str = "mode_custom_profile:";

pub fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = {
        let s = app.state::<SharedSettings>();
        let s = s.lock().unwrap_or_else(|e| e.into_inner());
        build_menu(app, &s)?
    };

    let mut builder = TrayIconBuilder::with_id("main")
        .icon_as_template(true)
        .tooltip("TypeUrVoice")
        .menu(&menu)
        .on_menu_event(|app, event| {
            handle_menu_event(app, event.id.as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                open_settings(app);
            }
        });

    // Load icon: try bundled 32x32 first, fall back to default app icon
    if let Some(img) = load_tray_icon(app) {
        builder = builder.icon(img);
    } else if let Some(img) = app.default_window_icon().cloned() {
        builder = builder.icon(img);
    }

    builder.build(app)?;
    Ok(())
}

fn load_tray_icon<R: Runtime>(_app: &AppHandle<R>) -> Option<Image<'static>> {
    // Embed icon at compile time so it works in both dev and production
    let bytes: &'static [u8] = include_bytes!("../icons/32x32.png");
    Image::from_bytes(bytes).ok()
}

fn build_menu<R: Runtime>(app: &AppHandle<R>, settings: &crate::state::Settings) -> tauri::Result<Menu<R>> {
    let current = settings.output_mode.as_str();
    let header = MenuItem::with_id(app, "header", "TypeUrVoice", false, None::<&str>)?;

    let mode_raw = MenuItem::with_id(
        app, "mode_raw",
        if current == "raw" { "✓ Raw" } else { "  Raw" },
        true, None::<&str>,
    )?;
    let mode_clean = MenuItem::with_id(
        app, "mode_clean",
        if current == "clean" { "✓ Clean" } else { "  Clean" },
        true, None::<&str>,
    )?;
    let mode_polished = MenuItem::with_id(
        app, "mode_polished",
        if current == "polished" { "✓ Polished" } else { "  Polished" },
        true, None::<&str>,
    )?;

    // Custom: when there are profiles, expose them as a sub-submenu so the
    // user can switch profile + activate Custom mode in one click. With zero
    // profiles, fall back to a flat menu item that just selects Custom mode
    // (the OutputTab will prompt the user to create a profile).
    let custom_label = if current == "custom" { "✓ Custom" } else { "  Custom" };
    let custom_entry: Box<dyn tauri::menu::IsMenuItem<R>> = if settings.custom_profiles.is_empty() {
        Box::new(MenuItem::with_id(app, "mode_custom", custom_label, true, None::<&str>)?)
    } else {
        let active_id = settings.active_custom_profile_id.as_deref();
        let mut items: Vec<Box<dyn tauri::menu::IsMenuItem<R>>> = Vec::new();
        for profile in &settings.custom_profiles {
            let is_active = current == "custom" && active_id == Some(profile.id.as_str());
            let label = if is_active {
                format!("✓ {}", profile.name)
            } else {
                format!("  {}", profile.name)
            };
            let id = format!("{}{}", PROFILE_ITEM_PREFIX, profile.id);
            items.push(Box::new(MenuItem::with_id(app, &id, label, true, None::<&str>)?));
        }
        items.push(Box::new(PredefinedMenuItem::separator(app)?));
        items.push(Box::new(MenuItem::with_id(
            app, "manage_profiles", "Manage profiles…", true, None::<&str>,
        )?));
        let item_refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = items.iter().map(|b| b.as_ref()).collect();
        Box::new(Submenu::with_id_and_items(app, "mode_custom_submenu", custom_label, true, &item_refs)?)
    };

    let output_submenu = Submenu::with_id_and_items(
        app, "output_mode", "Output Mode", true,
        &[
            &mode_raw as &dyn tauri::menu::IsMenuItem<R>,
            &mode_clean,
            &mode_polished,
            custom_entry.as_ref(),
        ],
    )?;

    let recent  = MenuItem::with_id(app, "recent",   "Recent Activity", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit    = MenuItem::with_id(app, "quit",     "Quit TypeUrVoice",   true, None::<&str>)?;

    Menu::with_items(app, &[
        &header,
        &PredefinedMenuItem::separator(app)?,
        &output_submenu,
        &PredefinedMenuItem::separator(app)?,
        &recent,
        &settings_item,
        &PredefinedMenuItem::separator(app)?,
        &quit,
    ])
}

/// Rebuild the tray menu to reflect current settings (e.g. output mode checkmark).
/// Called from update_settings command so the tray stays in sync with the UI.
pub fn refresh_menu<R: Runtime>(app: &AppHandle<R>) {
    let snapshot = {
        let s = app.state::<SharedSettings>();
        let s = s.lock().unwrap_or_else(|e| e.into_inner());
        s.clone()
    };

    if let Some(tray) = app.tray_by_id("main") {
        if let Ok(menu) = build_menu(app, &snapshot) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn open_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        "settings" => open_settings(app),
        "manage_profiles" => {
            open_settings(app);
            if let Some(w) = app.get_webview_window("settings") {
                let _ = w.emit("navigate", "output");
            }
        }
        "recent" => {
            open_settings(app);
            if let Some(w) = app.get_webview_window("settings") {
                let _ = w.emit("navigate", "history");
            }
        }
        // Profile-specific picks: switch to Custom mode AND set active profile.
        id if id.starts_with(PROFILE_ITEM_PREFIX) => {
            let profile_id = &id[PROFILE_ITEM_PREFIX.len()..];
            let state = app.state::<SharedSettings>();
            {
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                // Refuse if the profile no longer exists (race with deletion in UI)
                if s.custom_profiles.iter().any(|p| p.id == profile_id) {
                    s.output_mode = OutputMode::Custom;
                    s.active_custom_profile_id = Some(profile_id.to_string());
                    let _ = crate::storage::settings::save(&s);
                }
            }
            refresh_menu(app);
            if let Some(w) = app.get_webview_window("settings") {
                let _ = w.emit("settings-changed", ());
            }
        }
        "mode_raw" | "mode_clean" | "mode_polished" | "mode_custom" => {
            let mode = id.trim_start_matches("mode_");
            let new_mode = match mode {
                "raw"      => OutputMode::Raw,
                "polished" => OutputMode::Polished,
                "custom"   => OutputMode::Custom,
                _          => OutputMode::Clean,
            };
            let state = app.state::<SharedSettings>();
            {
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                s.output_mode = new_mode;
                let _ = crate::storage::settings::save(&s);
            }
            refresh_menu(app);
            if let Some(w) = app.get_webview_window("settings") {
                let _ = w.emit("settings-changed", ());
            }
        }
        "quit" => {
            // Signal that this is an intentional quit, then exit
            if let Some(flag) = app.try_state::<crate::state::QuitFlag>() {
                flag.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            app.exit(0);
        }
        _ => {}
    }
}
