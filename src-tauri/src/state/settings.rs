use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard};

use anyhow::{Context, Result};
use libobs_recorder::settings::{AudioSource, Framerate, StdResolution};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug)]
pub struct SettingsFile(PathBuf);

impl SettingsFile {
    pub fn new(pathbuf: PathBuf) -> Self {
        Self(pathbuf)
    }

    pub fn get(&self) -> &Path {
        self.0.as_path()
    }
}

/// Shared, thread-safe holder for the current [`Settings`].
///
/// This only guards access; everything that has to *happen* when the settings change
/// (watcher, logging, hotkey, autostart, ...) lives in the app layer.
#[derive(Debug)]
pub struct SettingsWrapper(RwLock<Settings>);

impl SettingsWrapper {
    pub fn new(settings: Settings) -> Self {
        Self(RwLock::new(settings))
    }

    pub fn read(&self) -> RwLockReadGuard<'_, Settings> {
        self.0.read().unwrap()
    }

    /// Replaces the current settings and returns the previous ones.
    pub fn replace(&self, settings: Settings) -> Settings {
        std::mem::replace(&mut *self.0.write().unwrap(), settings)
    }

    pub fn update(&self, f: impl FnOnce(&mut Settings)) {
        f(&mut *self.0.write().unwrap());
    }
}

#[cfg_attr(test, derive(specta::Type))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub marker_flags: MarkerFlags,
    pub check_for_updates: bool,
    pub debug_log: bool,
    pub recordings_folder: PathBuf,
    pub filename_format: String,
    pub encoding_quality: u32,
    pub output_resolution: Option<StdResolution>,
    pub framerate: Framerate,
    pub record_audio: AudioSource,
    pub only_record_ranked: bool,
    pub autostart: bool,
    pub max_recording_age_days: Option<u64>,
    pub max_recordings_size_gb: Option<u64>,
    pub confirm_delete: bool,
    pub hightlight_hotkey: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            marker_flags: MarkerFlags::default(),
            check_for_updates: true,
            debug_log: false,
            recordings_folder: PathBuf::from("league_recordings"),
            filename_format: String::from("%Y-%m-%d_%H-%M.mp4"),
            encoding_quality: 25,
            output_resolution: None,
            framerate: Framerate::new(30, 1),
            record_audio: AudioSource::APPLICATION,
            only_record_ranked: false,
            autostart: false,
            max_recording_age_days: None,
            max_recordings_size_gb: None,
            confirm_delete: true,
            hightlight_hotkey: None,
        }
    }
}

impl Settings {
    /// Parses settings leniently: unknown keys are ignored and every field that is missing or
    /// malformed falls back to its default instead of failing the whole document.
    pub fn parse(json: &str) -> Self {
        serde_json::from_str::<Settings>(json).unwrap_or_else(|e| {
            log::warn!("settings file is not valid JSON, using defaults: {e}");
            Settings::default()
        })
    }

    pub fn load(settings_file: &Path) -> Result<Self> {
        let json = fs::read_to_string(settings_file).context("failed to read settings file")?;
        Ok(Self::parse(&json))
    }

    pub fn save(&self, settings_file: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(settings_file, json).context("failed to write settings file")
    }

    /// Creates the settings file (and its directory) with default settings if it doesn't exist yet.
    pub fn ensure_file(settings_file: &Path) -> Result<()> {
        if settings_file.is_file() {
            return Ok(());
        }
        let parent = settings_file.parent().context("settings file has no parent directory")?;
        fs::create_dir_all(parent)?;
        Settings::default().save(settings_file)
    }
}

// Deserializes a single field, swallowing the error so the caller can fall back to its default.
// The value is buffered first so a malformed field never leaves the input half-consumed.
fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).ok())
}

// Intermediate shape with every field optional. `None` means "missing or malformed" and gets
// replaced by the corresponding value from `Settings::default()`.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PartialSettings {
    #[serde(deserialize_with = "lenient")]
    marker_flags: Option<MarkerFlags>,
    #[serde(deserialize_with = "lenient")]
    check_for_updates: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    debug_log: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    recordings_folder: Option<PathBuf>,
    #[serde(deserialize_with = "lenient")]
    filename_format: Option<String>,
    #[serde(deserialize_with = "lenient")]
    encoding_quality: Option<u32>,
    #[serde(deserialize_with = "lenient")]
    output_resolution: Option<StdResolution>,
    #[serde(deserialize_with = "lenient")]
    framerate: Option<Framerate>,
    #[serde(deserialize_with = "lenient")]
    record_audio: Option<AudioSource>,
    #[serde(deserialize_with = "lenient")]
    only_record_ranked: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    autostart: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    max_recording_age_days: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    max_recordings_size_gb: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    confirm_delete: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    hightlight_hotkey: Option<String>,
}

impl From<PartialSettings> for Settings {
    fn from(p: PartialSettings) -> Self {
        let d = Settings::default();
        Settings {
            marker_flags: p.marker_flags.unwrap_or(d.marker_flags),
            check_for_updates: p.check_for_updates.unwrap_or(d.check_for_updates),
            debug_log: p.debug_log.unwrap_or(d.debug_log),
            recordings_folder: p.recordings_folder.unwrap_or(d.recordings_folder),
            filename_format: p.filename_format.unwrap_or(d.filename_format),
            encoding_quality: p.encoding_quality.unwrap_or(d.encoding_quality),
            output_resolution: p.output_resolution.or(d.output_resolution),
            framerate: p.framerate.unwrap_or(d.framerate),
            record_audio: p.record_audio.unwrap_or(d.record_audio),
            only_record_ranked: p.only_record_ranked.unwrap_or(d.only_record_ranked),
            autostart: p.autostart.unwrap_or(d.autostart),
            max_recording_age_days: p.max_recording_age_days.or(d.max_recording_age_days),
            max_recordings_size_gb: p.max_recordings_size_gb.or(d.max_recordings_size_gb),
            confirm_delete: p.confirm_delete.unwrap_or(d.confirm_delete),
            hightlight_hotkey: p.hightlight_hotkey.or(d.hightlight_hotkey),
        }
    }
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        PartialSettings::deserialize(deserializer).map(Settings::from)
    }
}

#[cfg_attr(test, derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MarkerFlags {
    pub kill: bool,
    pub death: bool,
    pub assist: bool,
    pub structure: bool,
    pub dragon: bool,
    pub herald: bool,
    pub atakhan: bool,
    pub baron: bool,
}

impl Default for MarkerFlags {
    fn default() -> Self {
        MarkerFlags {
            kill: true,
            death: true,
            assist: true,
            structure: true,
            atakhan: true,
            dragon: true,
            herald: true,
            baron: true,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct PartialMarkerFlags {
    #[serde(deserialize_with = "lenient")]
    kill: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    death: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    assist: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    structure: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    dragon: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    herald: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    atakhan: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    baron: Option<bool>,
}

impl From<PartialMarkerFlags> for MarkerFlags {
    fn from(p: PartialMarkerFlags) -> Self {
        let d = MarkerFlags::default();
        MarkerFlags {
            kill: p.kill.unwrap_or(d.kill),
            death: p.death.unwrap_or(d.death),
            assist: p.assist.unwrap_or(d.assist),
            structure: p.structure.unwrap_or(d.structure),
            dragon: p.dragon.unwrap_or(d.dragon),
            herald: p.herald.unwrap_or(d.herald),
            atakhan: p.atakhan.unwrap_or(d.atakhan),
            baron: p.baron.unwrap_or(d.baron),
        }
    }
}

impl<'de> Deserialize<'de> for MarkerFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        PartialMarkerFlags::deserialize(deserializer).map(MarkerFlags::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_fields_fall_back_to_defaults() {
        let settings = Settings::parse(
            r#"{
                "checkForUpdates": "yes",
                "encodingQuality": -3,
                "framerate": "fast",
                "recordingsFolder": 5,
                "maxRecordingAgeDays": "never",
                "markerFlags": { "kill": 1, "baron": false, "turret": true },
                "hightlightHotkey": "F9",
                "confirmDelete": false,
                "someUnknownKey": true
            }"#,
        );

        let defaults = Settings::default();
        assert_eq!(settings.check_for_updates, defaults.check_for_updates);
        assert_eq!(settings.encoding_quality, defaults.encoding_quality);
        assert_eq!(settings.recordings_folder, defaults.recordings_folder);
        assert_eq!(settings.max_recording_age_days, None);
        assert_eq!(settings.hightlight_hotkey.as_deref(), Some("F9"));
        assert!(!settings.confirm_delete);

        // good flags survive, bad ones default, unknown ones are ignored
        assert!(settings.marker_flags.kill);
        assert!(!settings.marker_flags.baron);
        assert!(settings.marker_flags.dragon);
    }

    #[test]
    fn invalid_json_yields_defaults() {
        let settings = Settings::parse("{ not json");
        assert_eq!(settings.encoding_quality, Settings::default().encoding_quality);
        assert_eq!(settings.marker_flags, MarkerFlags::default());
    }

    #[test]
    fn empty_object_yields_defaults() {
        let settings = Settings::parse("{}");
        assert!(settings.check_for_updates);
        assert_eq!(settings.filename_format, Settings::default().filename_format);
    }

    #[test]
    fn default_round_trips_through_json() {
        let json = serde_json::to_string(&Settings::default()).unwrap();
        let settings = Settings::parse(&json);
        assert_eq!(settings.marker_flags, MarkerFlags::default());
        assert_eq!(settings.encoding_quality, 25);
        assert_eq!(settings.recordings_folder, PathBuf::from("league_recordings"));
    }
}
