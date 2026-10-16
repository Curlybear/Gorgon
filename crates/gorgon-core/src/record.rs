//! Screen recording: the ScreenCast portal hands over a PipeWire stream,
//! GStreamer (`gst-launch-1.0`) crops and encodes it.
//!
//! MP4 is encoded by GStreamer directly. For GIF, GStreamer outputs raw
//! frames that gifski turns into a high-quality GIF while recording.

use std::{
    fs::File,
    io::{BufWriter, Read},
    os::fd::{AsRawFd, OwnedFd},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread::JoinHandle,
};

use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType, StartCastOptions},
    PersistMode, Session,
};

use crate::output::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Mp4,
    Gif,
}

impl std::str::FromStr for Format {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "mp4" | "video" => Ok(Self::Mp4),
            "gif" => Ok(Self::Gif),
            other => Err(format!(
                "unknown recording format {other:?} (expected mp4 or gif)"
            )),
        }
    }
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Gif => "gif",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("screen recording was cancelled")]
    Cancelled,
    #[error("the desktop's screen recording portal failed: {0}")]
    Portal(#[from] ashpd::Error),
    #[error("the desktop shared no screen")]
    NoStream,
    #[error("{0}")]
    Missing(String),
    #[error("could not start GStreamer: {0}")]
    Spawn(std::io::Error),
    #[error("recording failed: {0}")]
    Failed(String),
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// A screen being shared through the portal. Keep it alive while recording.
pub struct Cast {
    session: Option<Session<Screencast>>,
    fd: Option<OwnedFd>,
    node: u32,
    /// Where the stream's monitor sits on the desktop, in logical pixels.
    pub position: (i32, i32),
    /// Its size in logical pixels.
    pub size: (i32, i32),
}

fn token_path() -> Option<PathBuf> {
    dirs::state_dir()
        .or_else(dirs::data_dir)
        .map(|d| d.join("gorgon").join("screencast-token"))
}

/// Asks the portal for the monitor to record. The desktop asks the user the
/// first time; the restore token it returns skips the question afterwards.
pub async fn open_cast() -> Result<Cast, RecordError> {
    if std::env::var_os("GORGON_TEST_RECORD").is_some() {
        // Headless tests: GStreamer's test pattern stands in for the screen.
        return Ok(Cast {
            session: None,
            fd: None,
            node: 0,
            position: (0, 0),
            size: (0, 0),
        });
    }
    let token = token_path().and_then(|p| std::fs::read_to_string(p).ok());
    let proxy = Screencast::new().await?;
    let session = proxy.create_session(Default::default()).await?;
    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Embedded)
                .set_sources(enumflags2::BitFlags::from(SourceType::Monitor))
                .set_multiple(false)
                .set_restore_token(token.as_deref().map(str::trim))
                .set_persist_mode(PersistMode::ExplicitlyRevoked),
        )
        .await?;
    let streams = match proxy
        .start(&session, None, StartCastOptions::default())
        .await?
        .response()
    {
        Ok(s) => s,
        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
            let _ = session.close().await;
            return Err(RecordError::Cancelled);
        }
        Err(e) => return Err(e.into()),
    };
    if let (Some(token), Some(path)) = (streams.restore_token(), token_path()) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, token);
    }
    let stream = streams
        .streams()
        .first()
        .cloned()
        .ok_or(RecordError::NoStream)?;
    let fd = proxy
        .open_pipe_wire_remote(&session, Default::default())
        .await?;
    Ok(Cast {
        node: stream.pipe_wire_node_id(),
        position: stream.position().unwrap_or((0, 0)),
        size: stream.size().unwrap_or((0, 0)),
        session: Some(session),
        fd: Some(fd),
    })
}

impl Cast {
    pub async fn close(mut self) {
        if let Some(s) = self.session.take() {
            let _ = s.close().await;
        }
    }
}

/// Whether a GStreamer element is installed.
fn has_element(name: &str) -> bool {
    Command::new("gst-inspect-1.0")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The video encoder to use, best first, and the file extension it means.
fn video_encoder() -> Result<(Vec<String>, &'static str), RecordError> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    if has_element("x264enc") && has_element("mp4mux") {
        // High profile with yuv420p plays everywhere (browsers, phones).
        return Ok((
            s(&[
                "x264enc",
                "speed-preset=veryfast",
                "tune=zerolatency",
                "!",
                "video/x-h264,profile=high",
                "!",
                "mp4mux",
            ]),
            "mp4",
        ));
    }
    if has_element("openh264enc") && has_element("mp4mux") {
        return Ok((s(&["openh264enc", "!", "mp4mux"]), "mp4"));
    }
    if has_element("vp8enc") && has_element("webmmux") {
        return Ok((s(&["vp8enc", "deadline=1", "!", "webmmux"]), "webm"));
    }
    Err(RecordError::Missing(
        "no video encoder: install GStreamer's x264 plugin (Ubuntu: gstreamer1.0-plugins-ugly)"
            .into(),
    ))
}

/// Checks the tools recording needs, so the user hears about them up front.
pub fn check_tools(format: Format) -> Result<(), RecordError> {
    if Command::new("gst-launch-1.0")
        .arg("--version")
        .output()
        .is_err()
    {
        return Err(RecordError::Missing(
            "recording needs GStreamer: install gstreamer1.0-tools, gstreamer1.0-pipewire \
             and gstreamer1.0-plugins-good (and -ugly for MP4)"
                .into(),
        ));
    }
    if std::env::var_os("GORGON_TEST_RECORD").is_none() && !has_element("pipewiresrc") {
        return Err(RecordError::Missing(
            "recording needs GStreamer's PipeWire plugin (Ubuntu: gstreamer1.0-pipewire)".into(),
        ));
    }
    if format == Format::Mp4 {
        video_encoder()?;
    }
    Ok(())
}

/// A recording in progress.
pub struct Recording {
    child: Child,
    format: Format,
    path: PathBuf,
    gif: Option<JoinHandle<Result<(), String>>>,
    cast: Option<Cast>,
}

/// Starts recording `region` (in stream pixels) of the cast into `dir`/`stem`.
/// H.264 needs even sizes, so the region is rounded down to them.
pub fn start(
    cast: Cast,
    region: Rect,
    format: Format,
    fps: u32,
    dir: &Path,
    stem: &str,
) -> Result<Recording, RecordError> {
    let x = region.x.max(0.0).round() as u32;
    let y = region.y.max(0.0).round() as u32;
    let w = (region.width.round() as u32).max(2) & !1;
    let h = (region.height.round() as u32).max(2) & !1;

    let mut args: Vec<String> = vec!["-q".into(), "-e".into()];
    if std::env::var_os("GORGON_TEST_RECORD").is_some() {
        let (tw, th) = (x + w + 16, y + h + 16);
        args.extend(
            format!("videotestsrc is-live=true pattern=ball ! video/x-raw,width={tw},height={th},framerate=30/1")
                .split(' ')
                .map(String::from),
        );
    } else {
        args.extend([
            "pipewiresrc".into(),
            "fd=3".into(),
            format!("path={}", cast.node),
            "do-timestamp=true".into(),
            "keepalive-time=1000".into(),
            "always-copy=true".into(),
        ]);
    }
    // Crop the left/top edge; -1 makes videocrop trim the rest to the caps.
    let caps = format!("video/x-raw,width={w},height={h}");
    args.extend(
        format!("! videoconvert ! videocrop left={x} top={y} right=-1 bottom=-1 ! {caps} ! videorate ! video/x-raw,framerate={fps}/1 ! videoconvert")
            .split(' ')
            .map(String::from),
    );

    std::fs::create_dir_all(dir).map_err(|source| RecordError::Write {
        path: dir.to_owned(),
        source,
    })?;
    let (path, stdout) = match format {
        Format::Mp4 => {
            let (encoder, ext) = video_encoder()?;
            let path = crate::naming::unique_path(dir, stem, ext, crate::naming::now());
            args.extend(["!".into(), "video/x-raw,format=I420".into(), "!".into()]);
            args.extend(encoder);
            args.extend([
                "!".into(),
                "filesink".into(),
                format!("location={}", path.display()),
            ]);
            (path, Stdio::null())
        }
        Format::Gif => {
            let path = crate::naming::unique_path(dir, stem, "gif", crate::naming::now());
            args.extend(
                "! video/x-raw,format=RGBA ! fdsink fd=1"
                    .split(' ')
                    .map(String::from),
            );
            (path, Stdio::piped())
        }
    };

    let mut command = Command::new("gst-launch-1.0");
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::piped());
    if let Some(fd) = cast.fd.as_ref() {
        let raw = fd.as_raw_fd();
        // SAFETY: only async-signal-safe calls between fork and exec.
        unsafe {
            use std::os::unix::process::CommandExt;
            command.pre_exec(move || {
                if libc::dup2(raw, 3) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let mut child = command.spawn().map_err(RecordError::Spawn)?;

    let gif = match format {
        Format::Mp4 => None,
        Format::Gif => {
            let stdout = child.stdout.take().expect("piped");
            let out = path.clone();
            Some(std::thread::spawn(move || {
                encode_gif(stdout, w as usize, h as usize, fps, &out)
            }))
        }
    };
    Ok(Recording {
        child,
        format,
        path,
        gif,
        cast: Some(cast),
    })
}

/// Reads raw RGBA frames and writes them as a GIF.
fn encode_gif(
    mut input: impl Read,
    w: usize,
    h: usize,
    fps: u32,
    out: &Path,
) -> Result<(), String> {
    let (collector, writer) = gifski::new(gifski::Settings {
        width: None,
        height: None,
        quality: 90,
        fast: false,
        repeat: gifski::Repeat::Infinite,
    })
    .map_err(|e| e.to_string())?;
    let file = File::create(out).map_err(|e| format!("could not write {}: {e}", out.display()))?;
    let writing = std::thread::spawn(move || {
        writer
            .write(BufWriter::new(file), &mut gifski::progress::NoProgress {})
            .map_err(|e| e.to_string())
    });
    let mut buf = vec![0u8; w * h * 4];
    let mut index = 0;
    while input.read_exact(&mut buf).is_ok() {
        let pixels: Vec<rgb::RGBA8> = buf
            .chunks_exact(4)
            .map(|p| rgb::RGBA8::new(p[0], p[1], p[2], p[3]))
            .collect();
        let frame = imgref::ImgVec::new(pixels, w, h);
        if collector
            .add_frame_rgba(index, frame, index as f64 / f64::from(fps))
            .is_err()
        {
            break;
        }
        index += 1;
    }
    drop(collector);
    let written = writing
        .join()
        .map_err(|_| "the GIF encoder crashed".to_string())?;
    if index == 0 {
        let _ = std::fs::remove_file(out);
        return Err("no frames were recorded".into());
    }
    written
}

impl Recording {
    pub fn format(&self) -> Format {
        self.format
    }

    /// Whether GStreamer is still running.
    pub fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Stops recording and finishes the file. Blocks until it is written.
    /// Returns the screen share too: close it (async) once this returns.
    pub fn stop(mut self) -> (Result<PathBuf, RecordError>, Option<Cast>) {
        let cast = self.cast.take();
        (self.finish(), cast)
    }

    fn finish(&mut self) -> Result<PathBuf, RecordError> {
        // -e makes gst-launch end the stream cleanly on SIGINT, so the
        // MP4 gets its index.
        unsafe {
            libc::kill(self.child.id() as i32, libc::SIGINT);
        }
        let output = self.child.wait_with_output_stderr();
        if let Some(gif) = self.gif.take() {
            gif.join()
                .map_err(|_| RecordError::Failed("the GIF encoder crashed".into()))?
                .map_err(RecordError::Failed)?;
        }
        let ok = std::fs::metadata(&self.path).is_ok_and(|m| m.len() > 0);
        if !ok {
            let detail = output.trim();
            return Err(RecordError::Failed(if detail.is_empty() {
                "GStreamer produced no file".into()
            } else {
                detail.lines().last().unwrap_or(detail).to_owned()
            }));
        }
        Ok(self.path.clone())
    }
}

trait WaitStderr {
    fn wait_with_output_stderr(&mut self) -> String;
}

impl WaitStderr for Child {
    /// Waits for exit and returns what it printed on stderr.
    fn wait_with_output_stderr(&mut self) -> String {
        let mut err = String::new();
        if let Some(mut stderr) = self.stderr.take() {
            let _ = stderr.read_to_string(&mut err);
        }
        let _ = self.wait();
        err
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gst_available() -> bool {
        Command::new("gst-launch-1.0")
            .arg("--version")
            .output()
            .is_ok()
            && has_element("videotestsrc")
    }

    fn record(format: Format) -> PathBuf {
        std::env::set_var("GORGON_TEST_RECORD", "1");
        let dir = tempfile::tempdir().unwrap().keep();
        let cast = Cast {
            session: None,
            fd: None,
            node: 0,
            position: (0, 0),
            size: (0, 0),
        };
        let region = Rect {
            x: 10.0,
            y: 20.0,
            width: 121.0,
            height: 81.0,
        };
        let rec = start(cast, region, format, 15, &dir, "rec").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1500));
        rec.stop().0.unwrap()
    }

    #[test]
    fn records_gif() {
        if !gst_available() {
            eprintln!("skipped: no GStreamer");
            return;
        }
        let path = record(Format::Gif);
        let decoder = gif::DecodeOptions::new();
        let mut d = decoder.read_info(File::open(&path).unwrap()).unwrap();
        assert_eq!((d.width(), d.height()), (120, 80));
        let mut frames = 0;
        while d.read_next_frame().unwrap().is_some() {
            frames += 1;
        }
        assert!(frames >= 5, "only {frames} frames");
    }

    #[test]
    fn records_video() {
        if !gst_available() || video_encoder().is_err() {
            eprintln!("skipped: no GStreamer video encoder");
            return;
        }
        let path = record(Format::Mp4);
        assert!(std::fs::metadata(&path).unwrap().len() > 1000);
    }
}
