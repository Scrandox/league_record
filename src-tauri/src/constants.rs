pub const APP_NAME: &str = "LeagueRecord";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

// see tauri.conf.json for tray_id
pub const TRAY_ID: &str = "mainTray";

pub const EXIT_SUCCESS: i32 = 0;

// Auto-Clip: clips live in this subfolder of the recordings folder and are
// addressed as '<CLIPS_FOLDER>/<filename>.mp4' everywhere a video_id is used
pub const CLIPS_FOLDER: &str = "Clips";

// how much of the recording a clip keeps around the event that caused it
pub const CLIP_LEAD_IN_SECS: f64 = 30.0;
pub const CLIP_LEAD_OUT_SECS: f64 = 15.0;

pub mod menu_item {
    pub const RECORDING: &str = "recording";
    pub const SETTINGS: &str = "settings";
    pub const OPEN: &str = "open";
    pub const QUIT: &str = "quit";
    pub const UPDATE: &str = "update";
}
