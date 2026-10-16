//! Region recording: one at a time, started after a region is picked and
//! stopped by the same shortcut, the tray, the notification or the CLI.

use std::sync::Mutex;

use gorgon_core::{
    config::Config,
    history,
    output::Rect,
    record::{self, Format, Recording},
};

use crate::notify;

enum State {
    Idle,
    /// The portal is being asked; a second request in the meantime is ignored.
    Starting,
    Recording(Recording),
}

static STATE: Mutex<State> = Mutex::new(State::Idle);

pub fn is_recording() -> bool {
    matches!(
        *STATE.lock().unwrap_or_else(|e| e.into_inner()),
        State::Recording(_) | State::Starting
    )
}

/// Starts picking a region to record, or stops the current recording.
pub fn toggle(format: Format) -> Result<(), String> {
    if is_recording() {
        stop();
        return Ok(());
    }
    record::check_tools(format).map_err(|e| {
        notify::error(&e.to_string());
        e.to_string()
    })?;
    crate::app::request_record(format)
}

/// Where the region overlay's screenshot sits on the desktop, to place the
/// region on the recorded monitor.
#[derive(Clone, Copy, Debug)]
pub struct Desktop {
    /// Screenshot pixels per logical pixel.
    pub scale: f64,
    pub origin: (f64, f64),
}

/// Called once the user has picked `region` (in screenshot pixels).
pub fn start(format: Format, region: Rect, desktop: Desktop, config: Config) {
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(*state, State::Idle) {
            return;
        }
        *state = State::Starting;
    }
    crate::runtime().spawn(async move {
        let result = async {
            let cast = record::open_cast().await?;
            // Monitor offset on the desktop, in screenshot pixels.
            let dx = (f64::from(cast.position.0) - desktop.origin.0) * desktop.scale;
            let dy = (f64::from(cast.position.1) - desktop.origin.1) * desktop.scale;
            let region = Rect {
                x: region.x - dx,
                y: region.y - dy,
                ..region
            };
            let fps = match format {
                Format::Mp4 => config.video_fps,
                Format::Gif => config.gif_fps,
            };
            record::start(
                cast,
                region,
                format,
                fps.clamp(1, 60),
                &config.save_dir,
                &config.filename_pattern,
            )
        }
        .await;
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(rec) => {
                println!("recording {}", format.as_str());
                *state = State::Recording(rec);
                drop(state);
                crate::tray::refresh();
                notify::recording(format);
            }
            Err(record::RecordError::Cancelled) => *state = State::Idle,
            Err(e) => {
                *state = State::Idle;
                notify::error(&e.to_string());
            }
        }
    });
}

/// Stops the recording, then saves, uploads and notifies like a capture.
pub fn stop() {
    let rec = {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        match std::mem::replace(&mut *state, State::Idle) {
            State::Recording(rec) => rec,
            other => {
                *state = other;
                return;
            }
        }
    };
    crate::tray::refresh();
    notify::recording_stopped();
    std::thread::spawn(move || {
        let format = rec.format();
        let (result, cast) = rec.stop();
        if let Some(cast) = cast {
            crate::runtime().block_on(cast.close());
        }
        let path = match result {
            Ok(p) => p,
            Err(e) => {
                notify::error(&e.to_string());
                return;
            }
        };
        println!("saved {}", path.display());
        let (w, h) = match format {
            Format::Gif => image::image_dimensions(&path).unwrap_or((0, 0)),
            Format::Mp4 => (0, 0),
        };
        let id = history::add(history::Entry::new(&path, w, h))
            .inspect_err(|e| eprintln!("history: {e}"))
            .ok();
        crate::app::history_changed();
        let config = Config::load().unwrap_or_default();
        crate::app::upload_saved(path, config, false, id);
    });
}
