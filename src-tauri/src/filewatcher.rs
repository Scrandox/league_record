use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, Watcher};
use tauri::{AppHandle, Manager};

use crate::app::{AppEvent, EventManager};
use crate::library::RecordingLibrary;
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
                // the UI identifies recordings by video id, not by sidecar name
                let video_path = path.with_extension("mp4");
                if let Some(video_id) = video_path.file_name().and_then(OsStr::to_str) {
                    outcome.metadata_changed.push(video_id.to_owned());
                }
            }
            _ => {}
        }
    }

    outcome
}

/// Pairs up the halves of a video rename (`old.mp4` -> `new.mp4`). Windows reports a rename as a
/// `From` event followed by a `To` event, so the `From` path is kept in `pending` until then.
pub fn video_rename(kind: &EventKind, paths: &[PathBuf], pending: &mut Option<PathBuf>) -> Option<(PathBuf, PathBuf)> {
    let pair = match kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            *pending = paths.first().cloned();
            return None;
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => pending.take().zip(paths.first().cloned()),
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            pending.take();
            paths.first().cloned().zip(paths.get(1).cloned())
        }
        _ => {
            pending.take();
            None
        }
    };

    let is_mp4 = |path: &PathBuf| path.extension().and_then(OsStr::to_str) == Some("mp4");
    pair.filter(|(from, to)| is_mp4(from) && is_mp4(to))
}

pub fn replace(app_handle: &AppHandle, recordings_path: &Path) {
    let watcher = notify::recommended_watcher({
        let app_handle = app_handle.clone();
        let mut pending_rename = None;
        move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };

            let currently_recording = app_handle.state::<RecordingStatus>().current();

            // a video renamed outside the app takes its sidecar along, before the UI re-reads the list
            // and would create an empty sidecar for the new name
            if let Some((from, to)) = video_rename(&event.kind, &event.paths, &mut pending_rename) {
                let new_video_id = to.file_name().and_then(OsStr::to_str);
                if let (Some((library, video_id)), Some(new_video_id)) = (RecordingLibrary::locate(&from), new_video_id)
                {
                    if Some(from.as_path()) != currently_recording.as_deref() {
                        match library.move_metadata(&video_id, new_video_id) {
                            Ok(true) => log::info!("moved metadata of {video_id} to {new_video_id}"),
                            Ok(false) => {}
                            Err(e) => log::warn!("failed to move metadata of {video_id} to {new_video_id}: {e}"),
                        }
                    }
                }
            }

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
                metadata_changed: vec!["a.mp4".to_owned()],
            }
        );
    }

    #[test]
    fn rename_halves_are_paired() {
        let mut pending = None;
        let from = EventKind::Modify(ModifyKind::Name(RenameMode::From));
        let to = EventKind::Modify(ModifyKind::Name(RenameMode::To));

        assert_eq!(video_rename(&from, &paths(&["/rec/a.mp4"]), &mut pending), None);
        assert_eq!(
            video_rename(&to, &paths(&["/rec/b.mp4"]), &mut pending),
            Some((PathBuf::from("/rec/a.mp4"), PathBuf::from("/rec/b.mp4")))
        );

        // a `To` without a preceding `From` and renames of other files are not video renames
        assert_eq!(video_rename(&to, &paths(&["/rec/c.mp4"]), &mut pending), None);
        assert_eq!(video_rename(&from, &paths(&["/rec/a.json"]), &mut pending), None);
        assert_eq!(video_rename(&to, &paths(&["/rec/b.json"]), &mut pending), None);
    }

    #[test]
    fn unrelated_event_clears_pending_rename() {
        let mut pending = None;
        let from = EventKind::Modify(ModifyKind::Name(RenameMode::From));
        let to = EventKind::Modify(ModifyKind::Name(RenameMode::To));

        video_rename(&from, &paths(&["/rec/a.mp4"]), &mut pending);
        video_rename(&EventKind::Create(CreateKind::Any), &paths(&["/rec/x.mp4"]), &mut pending);
        assert_eq!(video_rename(&to, &paths(&["/rec/b.mp4"]), &mut pending), None);
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
