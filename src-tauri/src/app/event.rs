use serde::{Deserialize, Serialize};

#[cfg_attr(test, derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingState {
    Idle,
    Recording,
    Saving,
}

#[allow(clippy::enum_variant_names)]
#[cfg_attr(test, derive(specta::Type, tauri_specta::Event))]
#[derive(Debug, Clone, strum_macros::IntoStaticStr, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AppEvent {
    RecordingsChanged { payload: () },
    MetadataChanged { payload: Vec<String> },
    MarkerflagsChanged { payload: () },
    RecordingStateChanged { payload: RecordingState },
}

pub trait EventManager {
    fn send_event(&self, event: AppEvent) -> anyhow::Result<()>;
}

impl EventManager for tauri::AppHandle {
    fn send_event(&self, event: AppEvent) -> anyhow::Result<()> {
        use crate::app::AppWindow;
        use tauri::{Emitter, EventTarget};
        use AppEvent::*;

        match &event {
            RecordingsChanged { payload } => {
                self.emit_to(EventTarget::webview_window(AppWindow::Main), (&event).into(), payload)?
            }
            MetadataChanged { payload } => {
                self.emit_to(EventTarget::webview_window(AppWindow::Main), (&event).into(), payload)?
            }
            MarkerflagsChanged { payload } => {
                self.emit_to(EventTarget::webview_window(AppWindow::Main), (&event).into(), payload)?
            }
            RecordingStateChanged { payload } => {
                self.emit_to(EventTarget::webview_window(AppWindow::Main), (&event).into(), payload)?
            }
        };

        Ok(())
    }
}
