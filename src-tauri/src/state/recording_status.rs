use std::{path::PathBuf, sync::Mutex};

/// Path of the recording that is currently being written, if any.
#[derive(Debug, Default)]
pub struct RecordingStatus(Mutex<Option<PathBuf>>);

impl RecordingStatus {
    pub fn start(&self, path: PathBuf) {
        *self.0.lock().unwrap() = Some(path);
    }

    pub fn finish(&self) {
        *self.0.lock().unwrap() = None;
    }

    pub fn current(&self) -> Option<PathBuf> {
        self.0.lock().unwrap().clone()
    }
}
