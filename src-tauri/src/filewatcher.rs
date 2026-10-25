use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, Watcher};
use tauri::{AppHandle, Manager};

use crate::app::{AppEvent, EventManager};
use crate::state::{FileWatcher, RecordingStatus};

/// Which app events a filesystem event in the recordings folder translates to.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WatchOutcome {
    pub recordings_changed: bool,
    pub metadata_changed: Vec<String>,
}

/// `exclude` is the recording currently being written; changes to it are not interesting.
pub fn classify(kind: &EventKind, paths: &[PathBuf], exclude: Option<&Path>) -> WatchOutcome {
    let mut outcome = WatchOutcome::default();

    let relevant = matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Name(
                RenameMode::To | RenameMode::Both | RenameMode::Any | RenameMode::Other
            ))
    );
    if !relevant {
        return outcome;
    }

    for path in paths.iter().filter(|path| Some(path.as_path()) != exclude) {
        match path.extension().and_then(OsStr::to_str) {
            Some("mp4") => outcome.recordings_changed = true,
            Some("json") => {
                if let Some(video_id) = path.file_name().and_then(OsStr::to_str) {
                    outcome.metadata_changed.push(video_id.to_owned());
                }
            }
            _ => {}
        }
    }

    outcome
}

pub fn replace(app_handle: &AppHandle, recordings_path: &Path) {
    let watcher = notify::recommended_watcher({
        let app_handle = app_handle.clone();
        move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };

            let currently_recording = app_handle.state::<RecordingStatus>().current();
            let outcome = classify(&event.kind, &event.paths, currently_recording.as_deref());

            if outcome.recordings_changed {
                log::info!("filewatcher event contains .mp4 path");
                if let Err(e) = app_handle.send_event(AppEvent::RecordingsChanged { payload: () }) {
                    log::warn!("filewatcher failed to send event: {e:?}");
                }
            }

            if !outcome.metadata_changed.is_empty() {
                log::info!("filewatcher event json paths: {:?}", outcome.metadata_changed);
                let event = AppEvent::MetadataChanged {
                    payload: outcome.metadata_changed,
                };
                if let Err(e) = app_handle.send_event(event) {
                    log::warn!("filewatcher failed to send event: {e:?}");
                }
            }
        }
    });

    match watcher {
        Ok(mut watcher) => {
            _ = watcher.watch(recordings_path, notify::RecursiveMode::NonRecursive);
            // storing the watcher keeps it alive; the previous one is dropped and stops watching
            app_handle.state::<FileWatcher>().set(watcher);
        }
        Err(e) => log::error!("failed to start filewatcher: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::CreateKind;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn create_maps_to_both_events() {
        let outcome = classify(
            &EventKind::Create(CreateKind::Any),
            &paths(&["/rec/a.mp4", "/rec/a.json", "/rec/notes.txt"]),
            None,
        );
        assert_eq!(
            outcome,
            WatchOutcome {
                recordings_changed: true,
                metadata_changed: vec!["a.json".to_owned()],
            }
        );
    }

    #[test]
    fn rename_from_is_ignored() {
        let outcome = classify(
            &EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            &paths(&["/rec/a.mp4"]),
            None,
        );
        assert_eq!(outcome, WatchOutcome::default());
    }

    #[test]
    fn currently_recording_file_is_excluded() {
        let current = PathBuf::from("/rec/live.mp4");
        let outcome = classify(
            &EventKind::Create(CreateKind::Any),
            &paths(&["/rec/live.mp4"]),
            Some(&current),
        );
        assert_eq!(outcome, WatchOutcome::default());
    }
}
