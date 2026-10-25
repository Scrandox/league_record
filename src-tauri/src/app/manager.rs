use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use log::LevelFilter;
use semver::Version;
use shaco::rest::LcuRestClient;
use tauri::{async_runtime, AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_updater::UpdaterExt;
use tokio_util::sync::CancellationToken;

use super::{AppEvent, EventManager, SystemTrayManager};
use crate::constants::{APP_NAME, CURRENT_VERSION};
use crate::filewatcher;
use crate::library::{CleanupPolicy, RecordingLibrary};
use crate::recorder::{self, LeagueRecorder, MetadataFile, Retry};
use crate::state::{RecordingStatus, Settings, SettingsFile, SettingsWrapper};

pub trait AppManager {
    const SETTINGS_FILE: &'static str;

    fn setup(&self) -> Result<()>;

    /// Reads the settings file, resolves the recordings folder and writes the normalised result back.
    fn load_settings(&self, settings_file: &Path) -> Settings;
    /// Opens the settings file in an editor and applies whatever changed once the editor closes.
    fn edit_settings(&self);
    /// Mutates the in-memory settings and persists them.
    fn update_settings(&self, f: impl FnOnce(&mut Settings));
    /// Everything that has to happen when the settings change.
    fn apply_settings_change(&self, old: &Settings, new: &Settings);

    fn library(&self) -> RecordingLibrary;
    fn cleanup_recordings(&self);
    /// Fetches game data for recordings whose metadata is still deferred and notifies the UI per upgraded file.
    fn resolve_deferred_in_background(&self, video_ids: Vec<String>);
    /// Marks a recording as in progress (or none) and updates the tray accordingly.
    fn set_recording(&self, path: Option<PathBuf>);

    fn check_for_update(&self, callback: impl FnOnce(AppHandle) + Send + 'static);
    fn update(&self);

    fn add_log_plugin(&self) -> Result<()>;
    fn remove_log_plugin(&self);

    fn sync_autostart(&self);

    fn update_hightlight_hotkey(&self);
}

// the `-d` / `--debug` command line flag enables logging regardless of the settings
fn debug_log_enabled(settings: &Settings) -> bool {
    settings.debug_log || std::env::args().any(|e| e == "-d" || e == "--debug")
}

// watch the recordings folder for changes and let the webview load videos from it
fn watch_recordings_folder(app_handle: &AppHandle, recordings_folder: &Path) {
    filewatcher::replace(app_handle, recordings_folder);

    // the asset scope canonicalizes requested paths (`\\?\C:\...` on Windows) before matching, and only
    // registers both the verbatim and the plain form when given the verbatim one
    let scope_folder = recordings_folder
        .canonicalize()
        .unwrap_or_else(|_| recordings_folder.to_path_buf());
    if let Err(e) = app_handle.asset_protocol_scope().allow_directory(&scope_folder, false) {
        log::error!("failed to allow asset protocol access to recordings folder: {e}");
    }
}

impl AppManager for AppHandle {
    const SETTINGS_FILE: &'static str = "settings.json";

    fn setup(&self) -> Result<()> {
        let config_folder = self.path().app_config_dir().context("Error getting app directory")?;

        let settings_file = config_folder.join(Self::SETTINGS_FILE);
        Settings::ensure_file(&settings_file)?;
        let settings = self.load_settings(&settings_file);
        self.manage(SettingsWrapper::new(settings.clone()));
        self.manage(SettingsFile::new(settings_file));

        let debug_log = debug_log_enabled(&settings);
        if debug_log {
            self.add_log_plugin()?;
        }

        log::info!("{APP_NAME} v{CURRENT_VERSION}");
        log::info!("{}", chrono::Local::now().format("%d-%m-%Y %H:%M"));
        log::info!("debug_log: {}", if debug_log { "enabled" } else { "disabled" });
        log::info!("Settings: {settings:?}");

        // create system tray-icon
        self.init_tray_menu();
        if settings.check_for_updates {
            self.check_for_update(|app_handle| app_handle.set_tray_menu_update_available(true));
        }

        // check if app was updated
        let version_file = config_folder.join(".version");
        match fs::read_to_string(&version_file).map(|v| Version::parse(&v)) {
            // if we successfully read and parsed the version, we compare it to the version of this binary
            // if the version is smaller the app was just updated => show dialog
            Ok(Ok(version)) => {
                if version != Version::parse(CURRENT_VERSION)? {
                    self.dialog()
                        .message(format!("Successfully installed {APP_NAME} v{CURRENT_VERSION}"))
                        .title(format!("{APP_NAME} update successful!"))
                        .blocking_show();
                    _ = fs::write(&version_file, CURRENT_VERSION);
                }
            }
            // if the version can't be parsed overwrite it with a valid version
            Ok(Err(_)) => _ = fs::write(&version_file, CURRENT_VERSION),
            // if the version file is missing create one
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    _ = fs::write(&version_file, CURRENT_VERSION);
                }
            }
        }

        // make sure the system autostart setting for the app matches what is set in the settings
        self.sync_autostart();

        self.update_hightlight_hotkey();

        // start watching recordings folder for changes
        log::info!("recordings folder: {:?}", settings.recordings_folder);
        watch_recordings_folder(self, &settings.recordings_folder);

        // start checking for LoL games to record
        self.manage(LeagueRecorder::new(self.clone()));

        // cleanup recordings if they are too old or the total size of the recordings gets too big
        // this only happens if 'maxRecordingAge' or 'maxRecordingsSize' is configured in the settings
        async_runtime::spawn_blocking({
            let app_handle = self.clone();
            move || app_handle.cleanup_recordings()
        });

        Ok(())
    }

    fn load_settings(&self, settings_file: &Path) -> Settings {
        let mut settings = Settings::load(settings_file).unwrap_or_else(|e| {
            log::error!("{e}");
            Settings::default()
        });

        // if recordings_folder is relative the path gets appened to the system video directory
        if settings.recordings_folder.is_relative() {
            settings.recordings_folder = self
                .path()
                .video_dir()
                .expect("video_dir doesn't exist")
                .join(&settings.recordings_folder);
        }
        if let Err(e) = fs::create_dir_all(&settings.recordings_folder) {
            log::error!("unable to create recordings_folder: {e}");
        }

        // write parsed settings back to file so the internal settings and the content of the file stay in sync
        // to avoid confusing the user when editing the file
        if let Err(e) = settings.save(settings_file) {
            log::error!("{e}");
        }

        settings
    }

    fn edit_settings(&self) {
        let app_handle = self.clone();

        // spawn a separate thread to avoid blocking the main thread with Command::status()
        async_runtime::spawn_blocking(move || {
            let settings_file = app_handle.state::<SettingsFile>();
            let settings_file = settings_file.get();

            if let Err(e) = Settings::ensure_file(settings_file) {
                log::error!("failed to create settings file: {e}");
                return;
            }

            // hardcode 'notepad' since league_record currently only works on windows anyways
            if let Err(e) = Command::new("notepad").arg(settings_file).status() {
                log::error!("failed to start text editor: {e}");
                return;
            }

            let new = app_handle.load_settings(settings_file);
            log::info!("Settings updated: {new:?}");

            let old = app_handle.state::<SettingsWrapper>().replace(new.clone());
            app_handle.apply_settings_change(&old, &new);
        });
    }

    fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let settings = self.state::<SettingsWrapper>();
        settings.update(f);
        if let Err(e) = settings.read().save(self.state::<SettingsFile>().get()) {
            log::error!("{e}");
        };
    }

    fn apply_settings_change(&self, old: &Settings, new: &Settings) {
        // check and update autostart if necessary
        self.sync_autostart();

        // add / remove logs plugin if needed
        let new_log = debug_log_enabled(new);
        if debug_log_enabled(old) != new_log {
            if new_log {
                if self.add_log_plugin().is_err() {
                    // retry
                    self.remove_log_plugin();
                    _ = self.add_log_plugin();
                }
            } else {
                self.remove_log_plugin();
            }
        }

        // check if UI window needs to be updated
        if old.recordings_folder != new.recordings_folder {
            watch_recordings_folder(self, &new.recordings_folder);
            if let Err(e) = self.send_event(AppEvent::RecordingsChanged { payload: () }) {
                log::error!("failed to emit 'recordings_changed' event: {e}");
            }
        }

        if old.marker_flags != new.marker_flags {
            if let Err(e) = self.send_event(AppEvent::MarkerflagsChanged { payload: () }) {
                log::error!("failed to emit 'markerflags_changed' event: {e}");
            }
        }

        if old.hightlight_hotkey != new.hightlight_hotkey {
            self.update_hightlight_hotkey();
        }

        self.cleanup_recordings();
    }

    fn library(&self) -> RecordingLibrary {
        RecordingLibrary::new(self.state::<SettingsWrapper>().read().recordings_folder.clone())
    }

    fn cleanup_recordings(&self) {
        let policy = {
            let settings = self.state::<SettingsWrapper>();
            let settings = settings.read();
            CleanupPolicy {
                max_age: settings
                    .max_recording_age_days
                    .map(|days| Duration::from_secs(days * 24 * 60 * 60)),
                max_size: settings.max_recordings_size_gb.map(|gb| gb * 1_000_000_000),
            }
        };

        let in_progress = self.state::<RecordingStatus>().current();
        self.library().cleanup(&policy, in_progress.as_deref());
    }

    fn resolve_deferred_in_background(&self, video_ids: Vec<String>) {
        let app_handle = self.clone();
        async_runtime::spawn(async move {
            let client = match LcuRestClient::new() {
                Ok(client) => client,
                Err(e) => {
                    log::info!("LCU not available, deferred metadata stays deferred: {e}");
                    return;
                }
            };

            let library = app_handle.library();
            let cancel_token = CancellationToken::new();
            for video_id in video_ids {
                match recorder::resolve_deferred(&library, &video_id, &client, Retry::NONE, &cancel_token).await {
                    Ok(MetadataFile::Metadata(_)) => {
                        let event = AppEvent::MetadataChanged {
                            payload: vec![video_id],
                        };
                        if let Err(e) = app_handle.send_event(event) {
                            log::error!("failed to emit 'metadata_changed' event: {e}");
                        }
                    }
                    Ok(_) => {}
                    Err(e) => log::warn!("failed to resolve deferred metadata for {video_id}: {e}"),
                }
            }
        });
    }

    fn set_recording(&self, path: Option<PathBuf>) {
        let status = self.state::<RecordingStatus>();
        match path {
            Some(path) => status.start(path),
            None => status.finish(),
        }
        self.refresh_tray_menu();
    }

    fn check_for_update(&self, callback: impl FnOnce(AppHandle) + Send + 'static) {
        let app_handle = self.clone();
        async_runtime::spawn(async move {
            let update_available = match app_handle.updater().unwrap().check().await {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(e) => {
                    log::error!("update check failed: {e}");
                    false
                }
            };

            if update_available {
                callback(app_handle);
            }
        });
    }

    fn update(&self) {
        let app_handle = self.clone();

        // runs on the async runtime so the main thread never blocks on network or dialogs
        async_runtime::spawn(async move {
            let updater = match app_handle.updater() {
                Ok(updater) => updater,
                Err(e) => {
                    log::error!("failed to create updater: {e}");
                    return;
                }
            };

            let update = match updater.check().await {
                Ok(Some(update)) => update,
                Ok(None) => {
                    log::warn!("no update available");
                    return;
                }
                Err(e) => {
                    log::error!("update check failed: {e}");
                    return;
                }
            };

            let message = format!(
                "Version {} available!{}Do you want to update now?",
                &update.version,
                update
                    .body
                    .as_deref()
                    .map(|body| format!("\n\n{body}\n\n"))
                    .unwrap_or("\n".to_string())
            );

            // the confirmation dialog blocks, so show it on a blocking thread
            let confirmed = async_runtime::spawn_blocking({
                let app_handle = app_handle.clone();
                move || {
                    app_handle
                        .dialog()
                        .message(message)
                        .kind(MessageDialogKind::Info)
                        .title(format!("{APP_NAME} update available!"))
                        .buttons(MessageDialogButtons::OkCancel)
                        .blocking_show()
                }
            })
            .await
            .unwrap_or(false);

            if !confirmed {
                return;
            }

            app_handle
                .dialog()
                .message("LeagueRecord is updating in the background and will restart automatically\nYou will get a message when the update has completed")
                .title("Update in progress")
                .show(|_| {});

            if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
                app_handle
                    .dialog()
                    .message("Failed to download and install update, please try again later!")
                    .title("Update failed!")
                    .show(|_| {});
                log::error!("failed to download and install the update: {e}");
            } else {
                // "On macOS and Linux you will need to restart the app manually"
                // https://tauri.app/v1/guides/distribution/updater/
                app_handle.restart();
            }
        });
    }

    fn add_log_plugin(&self) -> Result<()> {
        let file_name = Some(format!("{}", chrono::Local::now().format("%Y-%m-%d_%H-%M")));
        let plugin = tauri_plugin_log::Builder::default()
            .targets([
                Target::new(TargetKind::LogDir { file_name }),
                Target::new(TargetKind::Stdout),
            ])
            .level(LevelFilter::Info)
            // libobs logs heavily while capturing; the 40KB default rotates away the recorder setup lines
            .max_file_size(50_000_000)
            .format(|out, msg, record| {
                out.finish(format_args!(
                    "[{}][{}]: {}",
                    chrono::Local::now().format("%H:%M:%S"),
                    record.level(),
                    msg
                ))
            })
            .build();

        Ok(self.plugin(plugin)?)
    }

    fn remove_log_plugin(&self) {
        // the name the tauri log plugin registers itself with is currently "log"
        // maybe this will change in the future?
        self.remove_plugin("log");
    }

    fn sync_autostart(&self) {
        use tauri_plugin_autostart::ManagerExt;

        let autostart = self.state::<SettingsWrapper>().read().autostart;
        let autostart_manager = self.autolaunch();

        match autostart_manager.is_enabled() {
            Ok(autostart_enabled) => {
                if autostart != autostart_enabled {
                    let result = if autostart {
                        autostart_manager.enable()
                    } else {
                        autostart_manager.disable()
                    };

                    if let Err(error) = result {
                        log::warn!("failed to set autostart to {autostart}: {error:?}");
                    }
                }
            }
            Err(error) => {
                log::warn!("unable to get current autostart state: {error:?}");
            }
        }
    }

    fn update_hightlight_hotkey(&self) {
        use std::sync::Arc;
        use tauri::{Emitter, EventTarget};

        _ = self.run_on_main_thread({
            let app_handle = self.clone();
            move || {
                let keylistener = app_handle.state::<windows_key_listener::KeyListener>();
                keylistener.unlisten();
                let hotkey = app_handle.state::<SettingsWrapper>().read().hightlight_hotkey.clone();
                if let Some(hotkey) = hotkey {
                    if let Err(e) = keylistener.listen(
                        &hotkey,
                        Duration::from_millis(200),
                        Arc::new({
                            let app_handle = app_handle.clone();
                            move || {
                                let _ = app_handle.emit_to(EventTarget::App, "shortcut-event", "");
                                true
                            }
                        }),
                    ) {
                        log::error!("failed to register key-listener: {e}");
                    }
                }
            }
        });
    }
}
