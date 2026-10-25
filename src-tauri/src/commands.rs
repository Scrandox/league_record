use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

use shaco::rest::LcuRestClient;
use tauri::{AppHandle, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::app::AppManager;
use crate::recorder::{self, MetadataFile, Retry};
use crate::state::{MarkerFlags, RecordingStatus, SettingsWrapper};

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn get_marker_flags(settings: State<SettingsWrapper>) -> MarkerFlags {
    settings.read().marker_flags.clone()
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn set_marker_flags(marker_flags: MarkerFlags, app_handle: AppHandle) {
    app_handle.update_settings(|settings| settings.marker_flags = marker_flags);
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn get_recordings_path(settings: State<SettingsWrapper>) -> PathBuf {
    settings.read().recordings_folder.clone()
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn get_recordings_size(app_handle: AppHandle) -> f32 {
    let in_progress = app_handle.state::<RecordingStatus>().current();
    app_handle.library().size(in_progress.as_deref()) as f32 / 1_000_000_000.0 // in Gigabyte
}

#[cfg_attr(test, derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    video_id: String,
    metadata: Option<MetadataFile>,
}

/// Lists recordings with whatever metadata is on disk. Deferred metadata is completed in the
/// background; the UI gets a `MetadataChanged` event per recording once that happens.
#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn get_recordings_list(app_handle: AppHandle) -> Vec<Recording> {
    let library = app_handle.library();
    let in_progress = app_handle.state::<RecordingStatus>().current();

    let mut deferred = Vec::new();
    let recordings = library
        .list(in_progress.as_deref())
        .into_iter()
        .filter_map(|path| path.file_name().and_then(OsStr::to_str).map(str::to_owned))
        .map(|video_id| {
            let metadata = library.read_metadata(&video_id).ok();
            if matches!(metadata, Some(MetadataFile::Deferred(_))) {
                deferred.push(video_id.clone());
            }
            Recording { video_id, metadata }
        })
        .collect();

    if !deferred.is_empty() {
        app_handle.resolve_deferred_in_background(deferred);
    }

    recordings
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn open_recordings_folder(settings: State<SettingsWrapper>) {
    let recordings_folder = settings.read().recordings_folder.clone();
    if let Err(e) = recordings_folder
        .canonicalize()
        .and_then(|path| Command::new("explorer").arg(path).spawn())
    {
        log::error!("failed to open recordings-folder: {e:?}");
    }
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn rename_video(video_id: String, new_video_id: String, app_handle: AppHandle) -> bool {
    app_handle
        .library()
        .rename(&video_id, &new_video_id)
        .unwrap_or_else(|e| {
            log::error!("failed to rename video: {e}");
            false
        })
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn delete_video(video_id: String, app_handle: AppHandle) -> bool {
    match app_handle.library().delete(&video_id) {
        Ok(_) => true,
        Err(e) => {
            log::error!("failed to delete video: {e}");
            false
        }
    }
}

/// Returns the metadata for a video, completing deferred metadata first if the LCU is reachable.
#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub async fn get_metadata(video_id: String, app_handle: AppHandle) -> Option<MetadataFile> {
    let library = app_handle.library();

    // only deferred metadata needs the LCU - don't fail (and warn) on complete metadata when League isn't running
    let metadata_file = library.read_metadata(&video_id).ok()?;
    if !matches!(metadata_file, MetadataFile::Deferred(_)) {
        return Some(metadata_file);
    }

    let resolved = async {
        let client = LcuRestClient::new()?;
        recorder::resolve_deferred(&library, &video_id, &client, Retry::NONE, &CancellationToken::new()).await
    }
    .await;

    match resolved {
        Ok(metadata_file) => Some(metadata_file),
        Err(e) => {
            log::warn!("failed to resolve metadata for {video_id}: {e}");
            library.read_metadata(&video_id).ok()
        }
    }
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn toggle_favorite(video_id: String, app_handle: AppHandle) -> Option<bool> {
    app_handle.library().toggle_favorite(&video_id).ok()
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn confirm_delete(settings: State<SettingsWrapper>) -> bool {
    settings.read().confirm_delete
}

#[cfg_attr(test, specta::specta)]
#[tauri::command]
pub fn disable_confirm_delete(app_handle: AppHandle) {
    app_handle.update_settings(|settings| settings.confirm_delete = false);
}
