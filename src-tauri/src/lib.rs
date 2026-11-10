use std::{
    env,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

use itertools::Itertools;
use state::ManagerExt;
use tauri::{App, AppHandle, Listener, Manager, RunEvent};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};
use tracing::{debug, error, info, warn};

mod cli;
mod config;
mod db;
mod deep_link;
mod events;
mod game;
mod logger;
mod prefs;
mod profile;
mod state;
mod thunderstore;
mod util;

fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
    );

    if let Err(err) = rustls::crypto::aws_lc_rs::default_provider().install_default() {
        warn!(?err, "failed to install aws_lc_rs default crypto provider");
    }

    if let Err(err) = state::setup(app.handle()) {
        error!("setup error: {err:?}");

        // Linux dialog often won't work before the event loop starts and instead hangs the application
        #[cfg(target_os = "windows")]
        {
            use tauri_plugin_dialog::DialogExt;

            app.dialog()
                .message(format!("Failed to launch Gale: {err:?}"))
                .blocking_show();
        }

        return Err(err.into());
    }

    if let Some(window) = app.get_webview_window("main") {
        // Match the title bar to the persisted preference before restoring the
        // geometry, so the saved inner size isn't offset by a native frame that
        // is about to be removed (and no native bar shows over the menubar).
        if let Err(err) = window.set_decorations(initial_decorations(app.handle())) {
            warn!("failed to set initial window decorations: {err}");
        }

        // Restore the saved geometry here, while the plugin's cache still holds
        // the on-disk state and the window is hidden. Its own resize/move
        // listeners overwrite that cache with the startup geometry as soon as
        // the event loop runs, so a later restore would read the wrong values.
        // Maximized/fullscreen are deferred to the ready handler because
        // applying them shows the window before the webview has painted.
        if let Err(err) = window.restore_state(StateFlags::SIZE | StateFlags::POSITION) {
            warn!("failed to restore window geometry: {err}");
        }

        match window.set_background_color(Some(initial_bg_color(app.handle()))) {
            Ok(()) => debug!("set initial window background color"),
            Err(err) => warn!("failed to set initial window background color: {err}"),
        }
    }

    if let Err(err) = app.deep_link().register("ror2mm") {
        warn!("failed to register ror2mm deep link protocol: {:#}", err);
    }

    if let Err(err) = app.deep_link().register("gale") {
        warn!("failed to register gale deep link protocol: {:#}", err);
    }

    let args = env::args().collect_vec();
    if let Some(url) = args.get(1)
        && !deep_link::handle(app.handle(), url.to_owned())
    {
        cli::run(args, app.handle());
    }

    let handle = app.handle().clone();
    app.listen("ready", move |_| {
        let Some(window) = handle.get_webview_window("main") else {
            return;
        };

        // refresh the background color in case the setup-time call was too early
        let _ = window.set_background_color(Some(initial_bg_color(&handle)));

        // Apply maximized/fullscreen only once, right before the first show, so
        // they don't reveal the window while the webview is still blank and a
        // later page reload doesn't re-maximize a window the user restored.
        if !WINDOW_STATE_RESTORED.swap(true, Ordering::AcqRel) {
            if let Err(err) = window.restore_state(StateFlags::MAXIMIZED | StateFlags::FULLSCREEN) {
                warn!("failed to restore window maximized state: {err}");
            }
        }

        if let Err(err) = window.show() {
            warn!("failed to show main window: {err}");
        }

        let _ = window.set_focus();
    });

    // fallback in case the frontend never manages to signal readiness
    let fallback_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;

        if let Some(window) = fallback_handle.get_webview_window("main")
            && matches!(window.is_visible(), Ok(false))
        {
            warn!("frontend did not become ready in time, showing window anyway");
            let _ = window.show();
        }
    });

    let handle = app.handle().to_owned();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = game::update_list_task(&handle).await {
            warn!("failed to update games list: {err}");
        }
    });

    info!("setup done");

    Ok(())
}

// The webview paints its default background for a frame or two before the
// first content frame. Match it to the persisted theme early so that frame
// doesn't flash a foreign color; the frontend refines it to the exact color
// once the theme state has loaded.
fn initial_bg_color(app: &AppHandle) -> tauri::window::Color {
    let dark = app
        .path()
        .app_config_dir()
        .ok()
        .and_then(|dir| {
            let path = dir.join("ui-state.json");
            debug!("reading persisted UI state from {}", path.display());
            std::fs::read_to_string(path).ok()
        })
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| {
            let value = json.get("darkMode")?;
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_bool().map(|b| b.to_string()))
        })
        .map(|value| value != "false")
        .unwrap_or(true);

    debug!(dark, "resolved initial window background");

    if dark {
        // slate-800, the default primary color's 800 shade
        tauri::window::Color(30, 41, 59, 255)
    } else {
        tauri::window::Color(255, 255, 255, 255)
    }
}

// The title bar follows the persisted native-menu preference. Resolve it the
// same way as the background color so the frame is correct before the window
// is shown, even if the frontend is slow to load.
fn initial_decorations(app: &AppHandle) -> bool {
    let native = app
        .path()
        .app_config_dir()
        .ok()
        .and_then(|dir| std::fs::read_to_string(dir.join("ui-state.json")).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| {
            let value = json.get("useNativeMenu")?;
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_bool().map(|b| b.to_string()))
        })
        .map(|value| value == "true")
        .unwrap_or(!cfg!(target_os = "windows"));

    debug!(native, "resolved initial window decorations");

    native
}

fn window_state_flags() -> tauri_plugin_window_state::StateFlags {
    // VISIBLE is excluded so the plugin never shows the window before the
    // frontend is ready. DECORATIONS is excluded because the title bar follows
    // the native-menu preference, not the saved window state; restoring it here
    // could re-enable the native frame on top of the custom menubar.
    StateFlags::all() - StateFlags::VISIBLE - StateFlags::DECORATIONS
}

const WINDOW_STATE_SAVE_DELAY: Duration = Duration::from_millis(500);

static WINDOW_STATE_RESTORED: AtomicBool = AtomicBool::new(false);
static WINDOW_STATE_CHANGES: AtomicU64 = AtomicU64::new(0);
static WINDOW_STATE_SAVE_PENDING: AtomicBool = AtomicBool::new(false);

// dragging the window fires move and resize events continuously; persist the
// geometry once the user stops instead of writing on every event
fn debounce_window_state_save(app: &AppHandle) {
    // events fired while the window is hidden are part of startup, before the
    // saved geometry has been restored, and must not overwrite it
    let shown = app
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);

    if !shown {
        return;
    }

    WINDOW_STATE_CHANGES.fetch_add(1, Ordering::Relaxed);

    if WINDOW_STATE_SAVE_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let changes = WINDOW_STATE_CHANGES.load(Ordering::Relaxed);
            tokio::time::sleep(WINDOW_STATE_SAVE_DELAY).await;

            if WINDOW_STATE_CHANGES.load(Ordering::Relaxed) != changes {
                continue;
            }

            let _ = app.save_window_state(window_state_flags());

            WINDOW_STATE_SAVE_PENDING.store(false, Ordering::Release);

            // a change arriving during the save saw the pending flag still
            // set, so it needs to be rescheduled here
            if WINDOW_STATE_CHANGES.load(Ordering::Relaxed) != changes {
                debounce_window_state_save(&app);
            }

            break;
        }
    });
}

fn event_handler(app: &AppHandle, event: RunEvent) {
    if let RunEvent::WindowEvent {
        event: tauri::WindowEvent::CloseRequested { .. },
        ..
    } = &event
    {
        // persist the geometry as soon as closing starts so it isn't lost
        // if the process exits before RunEvent::Exit is reached
        let _ = app.save_window_state(window_state_flags());
    }

    if let RunEvent::WindowEvent {
        event: tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_),
        ..
    } = &event
    {
        debounce_window_state_save(app);
    }

    if let RunEvent::ExitRequested { api, .. } = event {
        if !app.install_queue().lock().is_processing() {
            return;
        }

        api.prevent_exit();

        tauri::async_runtime::spawn(profile::install::handle_exit(app.to_owned()));
    }
}

fn handle_single_instance(app: &AppHandle, args: Vec<String>, _cwd: String) {
    let Some(url) = args.get(1) else {
        debug!("deep link has too few arguments");
        return;
    };

    if !deep_link::handle(app, url.to_owned()) {
        cli::run(args, app);
    }
}

#[tauri::command]
fn is_flatpak() -> bool {
    util::flatpak::is_flatpak()
}

#[tauri::command]
fn set_window_background_color(
    red: u8,
    green: u8,
    blue: u8,
    app: AppHandle,
) -> util::cmd::Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| eyre::eyre!("main window not found"))?;
    window.set_background_color(Some(tauri::window::Color(red, green, blue, 255)))?;
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "tauri's generate_handler macro cannot be split"
)]
pub fn run() {
    logger::setup().unwrap_or_else(|err| {
        eprintln!("failed to set up logger: {err:#}");
    });

    // SAFETY: no other threads have been spawned yet
    #[cfg(target_os = "linux")]
    unsafe {
        util::webkit::apply_workarounds();
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            is_flatpak,
            set_window_background_color,
            logger::open_gale_log,
            logger::log_err,
            state::is_first_run,
            thunderstore::commands::query_thunderstore,
            thunderstore::commands::stop_querying_thunderstore,
            thunderstore::commands::get_markdown,
            thunderstore::commands::set_api_token,
            thunderstore::commands::has_api_token,
            thunderstore::commands::clear_api_token,
            thunderstore::commands::trigger_mod_fetch,
            thunderstore::commands::get_categories,
            prefs::commands::get_prefs,
            prefs::commands::set_prefs,
            prefs::commands::zoom_window,
            prefs::commands::get_system_fonts,
            prefs::commands::get_system_accent_color,
            profile::commands::get_game_info,
            profile::commands::favorite_game,
            profile::commands::set_active_game,
            profile::commands::get_profile_info,
            profile::commands::set_active_profile,
            profile::commands::is_mod_installed,
            profile::commands::query_profile,
            profile::commands::get_dependants,
            profile::commands::create_profile,
            profile::commands::delete_profile,
            profile::commands::rename_profile,
            profile::commands::duplicate_profile,
            profile::commands::remove_mod,
            profile::commands::force_remove_mods,
            profile::commands::toggle_mod,
            profile::commands::force_toggle_mods,
            profile::commands::set_all_mods_state,
            profile::commands::remove_disabled_mods,
            profile::commands::open_profile_dir,
            profile::commands::open_mod_dir,
            profile::commands::open_game_log,
            profile::commands::create_desktop_shortcut,
            profile::commands::get_local_markdown,
            profile::commands::set_custom_args,
            profile::commands::set_profile_path,
            profile::commands::forget_profile,
            profile::commands::toggle_hidden_mod,
            profile::commands::get_hidden_mods,
            profile::launch::commands::launch_game,
            profile::launch::commands::get_launch_args,
            profile::launch::commands::open_game_dir,
            profile::launch::commands::get_steam_launch_options,
            profile::install::commands::install_mod,
            profile::install::commands::cancel_all_installs,
            profile::install::commands::has_pending_installations,
            profile::install::commands::is_installing,
            profile::install::commands::clear_download_cache,
            profile::install::commands::get_download_size,
            profile::update::commands::change_mod_version,
            profile::update::commands::update_mods,
            profile::update::commands::ignore_update,
            profile::update::commands::ignore_package_updates,
            profile::update::commands::resume_updates,
            profile::import::commands::import_profile,
            profile::import::commands::read_profile_code,
            profile::import::commands::read_profile_file,
            profile::import::commands::read_profile_base64,
            profile::import::commands::import_local_mod,
            profile::import::commands::import_local_mod_base64,
            profile::import::commands::get_r2modman_info,
            profile::import::commands::import_r2modman,
            profile::export::commands::export_code,
            profile::export::commands::export_file,
            profile::export::commands::export_pack,
            profile::export::commands::upload_pack,
            profile::export::commands::get_pack_args,
            profile::export::commands::set_pack_args,
            profile::export::commands::generate_changelog,
            profile::export::commands::copy_dependency_strings,
            profile::export::commands::export_dependency_strings,
            profile::export::commands::copy_debug_info,
            profile::export::commands::list_export_files,
            profile::export::commands::set_excluded_export_files,
            profile::sync::commands::read_sync_profile,
            profile::sync::commands::create_sync_profile,
            profile::sync::commands::disconnect_sync_profile,
            profile::sync::commands::delete_sync_profile,
            profile::sync::commands::push_sync_profile,
            profile::sync::commands::clone_sync_profile,
            profile::sync::commands::pull_sync_profile,
            profile::sync::commands::fetch_sync_profile,
            profile::sync::commands::set_sync_preserve_extras,
            profile::sync::commands::get_owned_sync_profiles,
            profile::sync::commands::login,
            profile::sync::commands::logout,
            profile::sync::commands::get_user,
            config::commands::get_config_files,
            config::commands::set_config_entry,
            config::commands::reset_config_entry,
            config::commands::reset_config_file,
            config::commands::open_config_file,
            config::commands::delete_config_file,
        ])
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        // The window is shown manually once the frontend signals readiness, so
        // the plugin must not restore on window-ready (that would apply the
        // saved maximized state and show the window while it is still blank).
        // setup() and the ready handler restore the state themselves instead.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(window_state_flags())
                .skip_initial_state("main")
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(handle_single_instance))
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(event_handler);
}
