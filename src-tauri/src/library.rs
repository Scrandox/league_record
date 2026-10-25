use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context, Result};

use crate::recorder::{MetadataFile, NoData};
use crate::util;

/// The recordings folder: every video (`<id>.mp4`) with its metadata sidecar (`<id>.json`).
///
/// This is the only place that knows how a video id maps to files on disk.
#[derive(Debug, Clone)]
pub struct RecordingLibrary {
    root: PathBuf,
}

/// Which finished recordings may be deleted automatically. Favorites are never deleted.
#[derive(Debug, Clone, Copy, Default)]
pub struct CleanupPolicy {
    pub max_age: Option<Duration>,
    pub max_size: Option<u64>,
}

/// What [`CleanupPolicy::select`] needs to know about one finished recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub size: u64,
    pub created: SystemTime,
    pub favorite: bool,
}

impl CleanupPolicy {
    pub fn is_noop(&self) -> bool {
        self.max_age.is_none() && self.max_size.is_none()
    }

    /// Picks the recordings to delete. `in_progress_size` is the size of the recording currently
    /// being written so the total stays below the limit once it is finished.
    pub fn select(&self, now: SystemTime, in_progress_size: u64, mut entries: Vec<Entry>) -> Vec<Entry> {
        let mut delete = Vec::new();

        if let Some(max_age) = self.max_age {
            let too_old =
                |entry: &Entry| now.duration_since(entry.created).map(|age| age > max_age).unwrap_or(false);
            let (old, keep): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| !e.favorite && too_old(e));
            delete.extend(old);
            entries = keep;
        }

        if let Some(max_size) = self.max_size {
            // newest first, so the oldest recordings are the ones pushing the total over the limit
            entries.sort_by(|a, b| b.created.cmp(&a.created));

            let mut total = in_progress_size + entries.iter().filter(|e| e.favorite).map(|e| e.size).sum::<u64>();
            for entry in entries.into_iter().filter(|e| !e.favorite) {
                total += entry.size;
                if total > max_size {
                    delete.push(entry);
                }
            }
        }

        delete
    }
}

// a video id is a plain `*.mp4` file name - never a path
fn validate_video_id(video_id: &str) -> Result<()> {
    let is_plain_name = Path::new(video_id).file_name().and_then(OsStr::to_str) == Some(video_id);
    if video_id.is_empty() || !is_plain_name || video_id.contains(['/', '\\']) || !video_id.ends_with(".mp4") {
        bail!("invalid video id: {video_id:?}");
    }
    Ok(())
}

impl RecordingLibrary {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Library and video id for a recording the recorder just wrote.
    pub fn locate(video_path: &Path) -> Option<(Self, String)> {
        let root = video_path.parent()?.to_path_buf();
        let video_id = video_path.file_name()?.to_str()?.to_owned();
        Some((Self::new(root), video_id))
    }

    pub fn video_path(&self, video_id: &str) -> Result<PathBuf> {
        validate_video_id(video_id)?;
        Ok(self.root.join(video_id))
    }

    fn metadata_path(video_path: &Path) -> PathBuf {
        video_path.with_extension("json")
    }

    /// All finished recordings, newest first. `exclude` is the recording currently being written.
    pub fn list(&self, exclude: Option<&Path>) -> Vec<PathBuf> {
        let Ok(read_dir) = self.root.read_dir() else {
            return vec![];
        };

        let mut recordings: Vec<PathBuf> = read_dir
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && Some(path.as_path()) != exclude)
            .filter(|path| path.extension().and_then(OsStr::to_str) == Some("mp4"))
            .collect();
        recordings.sort_by(|a, b| util::compare_time(a, b).unwrap_or(Ordering::Equal));
        recordings
    }

    /// Total size of all finished recordings in bytes.
    pub fn size(&self, exclude: Option<&Path>) -> u64 {
        self.list(exclude)
            .iter()
            .filter_map(|path| path.metadata().ok())
            .map(|metadata| metadata.len())
            .sum()
    }

    /// Reads the metadata sidecar, creating an empty one if the video has none yet. Never fetches.
    pub fn read_metadata(&self, video_id: &str) -> Result<MetadataFile> {
        let video_path = self.video_path(video_id)?;
        if !video_path.is_file() {
            bail!("no such video: {video_id}");
        }

        let metadata_path = Self::metadata_path(&video_path);
        if metadata_path.is_file() {
            let reader = BufReader::new(File::open(&metadata_path)?);
            Ok(serde_json::from_reader(reader)?)
        } else {
            let metadata_file = MetadataFile::NoData(NoData { favorite: false });
            Self::write_file(&metadata_path, &metadata_file)?;
            Ok(metadata_file)
        }
    }

    pub fn write_metadata(&self, video_id: &str, metadata_file: &MetadataFile) -> Result<()> {
        let video_path = self.video_path(video_id)?;
        Self::write_file(&Self::metadata_path(&video_path), metadata_file)
    }

    fn write_file(metadata_path: &Path, metadata_file: &MetadataFile) -> Result<()> {
        let writer = BufWriter::new(File::create(metadata_path)?);
        Ok(serde_json::to_writer(writer, metadata_file)?)
    }

    /// Read-modify-write of the metadata sidecar; returns the written file.
    pub fn update_metadata(&self, video_id: &str, f: impl FnOnce(&mut MetadataFile)) -> Result<MetadataFile> {
        let mut metadata_file = self.read_metadata(video_id)?;
        f(&mut metadata_file);
        self.write_metadata(video_id, &metadata_file)?;
        Ok(metadata_file)
    }

    /// Flips the favorite flag and returns the new value.
    pub fn toggle_favorite(&self, video_id: &str) -> Result<bool> {
        let metadata_file = self.update_metadata(video_id, |m| m.set_favorite(!m.is_favorite()))?;
        Ok(metadata_file.is_favorite())
    }

    /// Renames video and sidecar together. Returns `Ok(false)` if the new name is already taken.
    pub fn rename(&self, video_id: &str, new_video_id: &str) -> Result<bool> {
        let video_path = self.video_path(video_id)?;
        let new_video_path = self.video_path(new_video_id)?;
        let metadata_path = Self::metadata_path(&video_path);
        let new_metadata_path = Self::metadata_path(&new_video_path);

        if new_video_path.exists() || new_metadata_path.exists() {
            return Ok(false);
        }

        // sidecar first: once the video is renamed the filewatcher moves a sidecar it finds left behind
        let has_metadata = metadata_path.is_file();
        if has_metadata {
            fs::rename(&metadata_path, &new_metadata_path).context("failed to rename metadata file")?;
        }
        if let Err(e) = fs::rename(&video_path, &new_video_path) {
            // roll back so video and sidecar never end up with different names
            if has_metadata {
                if let Err(rollback) = fs::rename(&new_metadata_path, &metadata_path) {
                    log::error!("failed to roll back rename of {}: {rollback}", new_metadata_path.display());
                }
            }
            return Err(e).context("failed to rename video");
        }

        Ok(true)
    }

    /// Moves the sidecar along after the video was renamed outside the app (e.g. in Explorer).
    /// Never overwrites an existing sidecar. Returns whether anything was moved.
    pub fn move_metadata(&self, video_id: &str, new_video_id: &str) -> Result<bool> {
        let metadata_path = Self::metadata_path(&self.video_path(video_id)?);
        let new_metadata_path = Self::metadata_path(&self.video_path(new_video_id)?);

        if !metadata_path.is_file() || new_metadata_path.exists() {
            return Ok(false);
        }

        fs::rename(&metadata_path, &new_metadata_path).context("failed to move metadata file")?;
        Ok(true)
    }

    /// Deletes video and sidecar. The video goes first so a failure never leaves a video
    /// without its metadata; a leftover sidecar is invisible since listing is driven by videos.
    pub fn delete(&self, video_id: &str) -> Result<()> {
        let video_path = self.video_path(video_id)?;
        fs::remove_file(&video_path).context("failed to delete video")?;

        match fs::remove_file(Self::metadata_path(&video_path)) {
            Err(e) if e.kind() != ErrorKind::NotFound => Err(e).context("failed to delete metadata file"),
            _ => Ok(()),
        }
    }

    /// Deletes finished recordings according to `policy`. `in_progress` is the recording
    /// currently being written; it is never touched but its size counts towards the limit.
    pub fn cleanup(&self, policy: &CleanupPolicy, in_progress: Option<&Path>) {
        if policy.is_noop() {
            return;
        }

        let in_progress_size = in_progress
            .and_then(|path| path.metadata().ok())
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        let entries = self
            .list(in_progress)
            .into_iter()
            .filter_map(|path| {
                let video_id = path.file_name()?.to_str()?;
                let metadata = path.metadata().ok()?;
                // if reading the sidecar fails treat the recording as favorite so it doesn't get deleted
                let favorite = self.read_metadata(video_id).map(|m| m.is_favorite()).unwrap_or(true);
                let created = metadata
                    .created()
                    .or_else(|_| metadata.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                Some(Entry {
                    size: metadata.len(),
                    created,
                    favorite,
                    path,
                })
            })
            .collect();

        for entry in policy.select(SystemTime::now(), in_progress_size, entries) {
            let Some(video_id) = entry.path.file_name().and_then(OsStr::to_str) else { continue };
            if let Err(e) = self.delete(video_id) {
                log::error!("cleanup failed to delete {}: {e}", entry.path.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);
    const GB: u64 = 1_000_000_000;

    fn entry(name: &str, size_gb: u64, age_days: u64, favorite: bool, now: SystemTime) -> Entry {
        Entry {
            path: PathBuf::from(format!("{name}.mp4")),
            size: size_gb * GB,
            created: now - DAY * age_days as u32,
            favorite,
        }
    }

    fn names(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.path.to_str().unwrap()).collect()
    }

    #[test]
    fn video_id_must_be_a_plain_mp4_file_name() {
        assert!(validate_video_id("2024-01-01_12-00.mp4").is_ok());
        assert!(validate_video_id("").is_err());
        assert!(validate_video_id("game.json").is_err());
        assert!(validate_video_id("../game.mp4").is_err());
        assert!(validate_video_id("sub/game.mp4").is_err());
        assert!(validate_video_id("sub\\game.mp4").is_err());
        assert!(validate_video_id("..").is_err());
    }

    fn temp_library(name: &str) -> RecordingLibrary {
        let root = std::env::temp_dir().join(format!("league_record_test_{name}_{}", std::process::id()));
        _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        RecordingLibrary::new(root)
    }

    #[test]
    fn move_metadata_follows_renamed_video_without_overwriting() {
        let library = temp_library("move_metadata");
        fs::write(library.root.join("new.mp4"), b"").unwrap();
        fs::write(library.root.join("old.json"), b"{}").unwrap();

        assert!(library.move_metadata("old.mp4", "new.mp4").unwrap());
        assert!(!library.root.join("old.json").exists());
        assert!(library.root.join("new.json").is_file());

        // nothing left to move, and an existing sidecar is never replaced
        fs::write(library.root.join("old.json"), b"{}").unwrap();
        assert!(!library.move_metadata("old.mp4", "new.mp4").unwrap());
        assert!(library.root.join("old.json").is_file());

        _ = fs::remove_dir_all(&library.root);
    }

    #[test]
    fn rename_moves_video_and_sidecar() {
        let library = temp_library("rename");
        fs::write(library.root.join("a.mp4"), b"").unwrap();
        fs::write(library.root.join("a.json"), b"{}").unwrap();

        assert!(library.rename("a.mp4", "b.mp4").unwrap());
        assert!(library.root.join("b.mp4").is_file());
        assert!(library.root.join("b.json").is_file());
        assert!(!library.root.join("a.mp4").exists());
        assert!(!library.root.join("a.json").exists());

        _ = fs::remove_dir_all(&library.root);
    }

    #[test]
    fn noop_policy_selects_nothing() {
        let now = SystemTime::now();
        let entries = vec![entry("a", 10, 100, false, now)];
        assert!(CleanupPolicy::default().select(now, 0, entries).is_empty());
    }

    #[test]
    fn age_policy_deletes_old_non_favorites() {
        let now = SystemTime::now();
        let policy = CleanupPolicy {
            max_age: Some(DAY * 30),
            max_size: None,
        };
        let entries = vec![
            entry("new", 1, 1, false, now),
            entry("old", 1, 31, false, now),
            entry("old_fav", 1, 31, true, now),
        ];
        assert_eq!(names(&policy.select(now, 0, entries)), ["old.mp4"]);
    }

    #[test]
    fn size_policy_deletes_oldest_first_and_keeps_favorites() {
        let now = SystemTime::now();
        let policy = CleanupPolicy {
            max_age: None,
            max_size: Some(5 * GB),
        };
        // 2 (fav) + 2 (newest) = 4, +2 (mid) = 6 > 5 -> mid and everything older goes
        let entries = vec![
            entry("oldest", 2, 3, false, now),
            entry("fav", 2, 4, true, now),
            entry("newest", 2, 1, false, now),
            entry("mid", 2, 2, false, now),
        ];
        assert_eq!(names(&policy.select(now, 0, entries)), ["mid.mp4", "oldest.mp4"]);
    }

    #[test]
    fn in_progress_recording_counts_towards_size_limit() {
        let now = SystemTime::now();
        let policy = CleanupPolicy {
            max_age: None,
            max_size: Some(5 * GB),
        };
        let entries = vec![entry("a", 2, 1, false, now), entry("b", 2, 2, false, now)];
        assert!(policy.select(now, 0, entries.clone()).is_empty());
        assert_eq!(names(&policy.select(now, 2 * GB, entries)), ["b.mp4"]);
    }

    #[test]
    fn age_is_applied_before_size() {
        let now = SystemTime::now();
        let policy = CleanupPolicy {
            max_age: Some(DAY * 10),
            max_size: Some(3 * GB),
        };
        let entries = vec![
            entry("ancient", 1, 20, false, now),
            entry("a", 2, 1, false, now),
            entry("b", 2, 2, false, now),
        ];
        assert_eq!(names(&policy.select(now, 0, entries)), ["ancient.mp4", "b.mp4"]);
    }
}
