//! Auto-Clip: cut the interesting parts of a recording out into standalone clips.
//!
//! A clip is a stream copy (`-c copy`) of the source recording, so cutting a whole game
//! costs a second or two and loses no quality. The trade-off is that ffmpeg can only start
//! a copy on a keyframe, so a clip may begin up to a keyframe interval earlier than asked -
//! invisible behind a 30s lead-in.
//!
//! Clips are written to `<recordings>/Clips` and are addressed everywhere else in the app as
//! `Clips/<filename>.mp4`, so a single `video_id` string still identifies any video.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use riot_datatypes::{BuildingType, MonsterType, ParticipantId};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use crate::app::{action, AppEvent, EventManager};
use crate::constants::{CLIPS_FOLDER, CLIP_LEAD_IN_SECS, CLIP_LEAD_OUT_SECS};
use crate::recorder::{Event, GameEvent, GameMetadata, MetadataFile};
use crate::state::SettingsWrapper;

/// which event types the user wants clipped - the same seven categories as the marker legend
#[cfg_attr(test, derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSelection {
    pub kill: bool,
    pub death: bool,
    pub assist: bool,
    pub structure: bool,
    pub dragon: bool,
    pub herald: bool,
    pub baron: bool,
}

/// one clip that would be cut - the frontend shows these before anything is written
#[cfg_attr(test, derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipPlan {
    /// seconds into the source recording
    pub start: f64,
    pub end: f64,
    /// when the first event inside the clip happened, in seconds into the source recording
    pub event_time: f64,
    /// human label built from the events inside, e.g. `KILL x2 + ASSIST`
    pub label: String,
    pub event_count: usize,
}

impl ClipPlan {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

/// a single event that qualified for clipping, before merging
struct Hit {
    /// seconds into the source recording
    time: f64,
    label: &'static str,
}

/// work out which clips would be cut out of `video_id` for `selection`, merging any two
/// clips whose windows touch so a kill 10s after another kill produces one clip, not two
pub fn plan(app_handle: &AppHandle, video_id: &str, selection: ClipSelection) -> Result<Vec<ClipPlan>> {
    let video_path = resolve_video(app_handle, video_id)?;
    let metadata = match action::get_recording_metadata(&video_path, true)? {
        MetadataFile::Metadata(metadata) => metadata,
        _ => bail!("recording has no match data to clip from"),
    };

    let mut hits = Vec::new();
    for event in &metadata.events {
        let Some(label) = classify(event, metadata.participant_id, selection) else { continue };

        // events that happened before the recording started have no video to clip
        let time = event_time(event, &metadata);
        if time < 0.0 {
            continue;
        }

        hits.push(Hit { time, label });
    }

    hits.sort_by(|a, b| a.time.total_cmp(&b.time));

    // merge overlapping windows, collecting the labels of everything that ends up in one clip
    let mut merged: Vec<ClipPlan> = Vec::new();
    let mut labels: Vec<Vec<&'static str>> = Vec::new();
    for hit in hits {
        let start = (hit.time - CLIP_LEAD_IN_SECS).max(0.0);
        let end = hit.time + CLIP_LEAD_OUT_SECS;

        match (merged.last_mut(), labels.last_mut()) {
            (Some(last), Some(last_labels)) if start <= last.end => {
                last.end = last.end.max(end);
                last.event_count += 1;
                last_labels.push(hit.label);
            }
            _ => {
                merged.push(ClipPlan {
                    start,
                    end,
                    event_time: hit.time,
                    label: String::new(),
                    event_count: 1,
                });
                labels.push(vec![hit.label]);
            }
        }
    }

    for (plan, labels) in merged.iter_mut().zip(labels.iter()) {
        plan.label = label_for(labels);
    }

    Ok(merged)
}

/// cut every clip in the plan for `video_id`, returning the video_ids of what was created
pub fn create(app_handle: &AppHandle, video_id: &str, selection: ClipSelection) -> Result<Vec<String>> {
    let plans = plan(app_handle, video_id, selection)?;
    if plans.is_empty() {
        return Ok(vec![]);
    }

    let video_path = resolve_video(app_handle, video_id)?;
    let metadata = match action::get_recording_metadata(&video_path, true)? {
        MetadataFile::Metadata(metadata) => metadata,
        _ => bail!("recording has no match data to clip from"),
    };

    let ffmpeg = ffmpeg_path(app_handle)?;
    let clips_dir = clips_dir(app_handle);
    std::fs::create_dir_all(&clips_dir).context("failed to create the Clips folder")?;

    let source_name = Path::new(video_id)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("recording");

    let total = plans.len();
    let mut created = Vec::with_capacity(total);

    for (index, plan) in plans.iter().enumerate() {
        let clip_path = unique_path(&clips_dir, &clip_filename(source_name, plan));

        match cut(&ffmpeg, &video_path, &clip_path, plan) {
            Ok(()) => {
                if let Err(e) = action::save_recording_metadata(&clip_path, &clip_metadata(&metadata, plan)) {
                    log::error!("failed to write clip metadata for {}: {e}", clip_path.display());
                }

                if let Some(name) = clip_path.file_name().and_then(|name| name.to_str()) {
                    created.push(format!("{CLIPS_FOLDER}/{name}"));
                }
            }
            Err(e) => log::error!("failed to cut clip {}: {e}", clip_path.display()),
        }

        if let Err(e) = app_handle.send_event(AppEvent::ClipProgress {
            payload: ClipProgress {
                done: index + 1,
                total,
            },
        }) {
            log::warn!("failed to send clip progress event: {e}");
        }
    }

    if created.is_empty() {
        bail!("ffmpeg failed to cut any clip - see the log for details");
    }

    Ok(created)
}

/// progress of a running Auto-Clip run, one event per finished clip
#[cfg_attr(test, derive(specta::Type))]
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipProgress {
    pub done: usize,
    pub total: usize,
}

pub fn clips_dir(app_handle: &AppHandle) -> PathBuf {
    app_handle.state::<SettingsWrapper>().get_recordings_path().join(CLIPS_FOLDER)
}

/// resolve a video_id ('game.mp4' or 'Clips/clip.mp4') against the recordings folder
fn resolve_video(app_handle: &AppHandle, video_id: &str) -> Result<PathBuf> {
    // a video_id is a filename, optionally prefixed with the clips folder - anything that
    // tries to climb out of the recordings folder is not one
    if video_id.contains("..") {
        bail!("invalid video id: {video_id}");
    }

    let path = app_handle.state::<SettingsWrapper>().get_recordings_path().join(video_id);
    if !path.is_file() {
        bail!("no such recording: {video_id}");
    }
    Ok(path)
}

fn ffmpeg_path(app_handle: &AppHandle) -> Result<PathBuf> {
    // bundled next to the executable (see 'resources' in tauri.conf.json)
    if let Ok(path) = app_handle.path().resolve("ffmpeg.exe", BaseDirectory::Resource) {
        if path.is_file() {
            return Ok(path);
        }
    }

    // 'bun run tauri dev' doesn't stage resources - fall back to the checked-out copy
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/ffmpeg.exe");
    if dev_path.is_file() {
        return Ok(dev_path);
    }

    bail!("ffmpeg is missing - run scripts/fetch-ffmpeg.ps1 and reinstall LeagueRecord")
}

fn cut(ffmpeg: &Path, source: &Path, target: &Path, plan: &ClipPlan) -> Result<()> {
    let mut command = Command::new(ffmpeg);
    command.args([
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-y",
        // seeking before -i is the fast, keyframe-accurate seek
        "-ss",
        &format!("{:.3}", plan.start),
        "-i",
    ]);
    command.arg(source);
    command.args([
        "-t",
        &format!("{:.3}", plan.duration()),
        "-c",
        "copy",
        "-avoid_negative_ts",
        "make_zero",
    ]);
    command.arg(target);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let output = command.output().context("failed to run ffmpeg")?;
    if !output.status.success() {
        bail!("ffmpeg exited with {}: {}", output.status, String::from_utf8_lossy(&output.stderr));
    }

    // ffmpeg reports success for a zero-byte output in some edge cases (e.g. a start past
    // the end of the recording) - a clip that can't be played is not a clip
    let size = target.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    if size == 0 {
        _ = std::fs::remove_file(target);
        bail!("ffmpeg produced an empty file");
    }

    Ok(())
}

/// the clip's own metadata: the parent's, with the timeline shifted to the clip's start and
/// events outside the clip dropped, so markers and the timestamps dialog keep working
fn clip_metadata(source: &GameMetadata, plan: &ClipPlan) -> MetadataFile {
    let mut metadata = source.clone();

    metadata.favorite = false;
    metadata.ingame_time_rec_start_offset = source.ingame_time_rec_start_offset + plan.start;
    metadata.events.retain(|event| {
        let time = event_time(event, source);
        time >= plan.start && time <= plan.end
    });
    metadata.highlights.retain(|highlight| {
        let time = highlight / 1000.0 - source.ingame_time_rec_start_offset;
        time >= plan.start && time <= plan.end
    });

    MetadataFile::Metadata(metadata)
}

/// seconds into the recording at which an event happened
fn event_time(event: &GameEvent, metadata: &GameMetadata) -> f64 {
    event.timestamp as f64 / 1000.0 - metadata.ingame_time_rec_start_offset
}

fn classify(event: &GameEvent, participant_id: ParticipantId, selection: ClipSelection) -> Option<&'static str> {
    match &event.event {
        Event::ChampionKill {
            victim_id,
            killer_id,
            assisting_participant_ids,
            ..
        } => {
            if selection.kill && *killer_id == participant_id {
                Some("KILL")
            } else if selection.assist && assisting_participant_ids.contains(&participant_id) {
                Some("ASSIST")
            } else if selection.death && *victim_id == participant_id {
                Some("DEATH")
            } else {
                None
            }
        }
        Event::BuildingKill { building_type, .. } if selection.structure => match building_type {
            BuildingType::TowerBuilding { .. } => Some("TURRET"),
            BuildingType::InhibitorBuilding { .. } => Some("INHIBITOR"),
        },
        Event::EliteMonsterKill { monster_type, .. } => match monster_type {
            MonsterType::Horde if selection.herald => Some("VOIDGRUB"),
            MonsterType::Riftherald if selection.herald => Some("HERALD"),
            MonsterType::BaronNashor if selection.baron => Some("BARON"),
            MonsterType::Dragon { .. } if selection.dragon => Some("DRAGON"),
            _ => None,
        },
        _ => None,
    }
}

/// 'KILL', 'KILL x2', 'KILL x2 + ASSIST' - counted in the order the events happened
fn label_for(labels: &[&'static str]) -> String {
    let mut counted: Vec<(&str, usize)> = Vec::new();
    for label in labels {
        match counted.iter_mut().find(|(name, _)| name == label) {
            Some((_, count)) => *count += 1,
            None => counted.push((label, 1)),
        }
    }

    counted
        .into_iter()
        .map(|(name, count)| if count > 1 { format!("{name} x{count}") } else { name.to_owned() })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// '<source> - 12.34 KILL x2.mp4' - the timestamp keeps clips of one game in event order
fn clip_filename(source_name: &str, plan: &ClipPlan) -> String {
    let at = plan.event_time.max(0.0);
    let minutes = (at / 60.0) as u64;
    let seconds = (at % 60.0) as u64;

    sanitize(&format!("{source_name} - {minutes:02}.{seconds:02} {}.mp4", plan.label))
}

/// strip anything Windows won't accept in a filename
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect()
}

/// never overwrite an existing clip - '<name> (2).mp4', '<name> (3).mp4', ...
fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("clip");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");

    for n in 2..1000 {
        let candidate = dir.join(format!("{stem} ({n}).{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    candidate
}
