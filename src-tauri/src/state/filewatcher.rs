use std::sync::Mutex;

/// Slot for the active recordings-folder watcher. Registered at startup so replacing the
/// watcher is a plain set; dropping the previous watcher stops it.
#[derive(Debug, Default)]
pub struct FileWatcher(Mutex<Option<notify::RecommendedWatcher>>);

impl FileWatcher {
    pub fn set(&self, watcher: notify::RecommendedWatcher) {
        drop(self.0.lock().unwrap().replace(watcher));
    }
}
