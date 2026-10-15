//! M0 spike: report what the current Wayland session offers Gorgon.
//!
//! Every subcommand prints plain text meant to be pasted into the spike
//! results issue, so results from different desktops can be compared.

use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use ashpd::desktop::{
    global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
    screenshot::{AvailableTargets, Screenshot, ScreenshotProxy},
    CreateSessionOptions,
};
use clap::{Parser, Subcommand, ValueEnum};
use futures_util::StreamExt;
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_registry,
    Connection, Dispatch, QueueHandle,
};

#[derive(Parser)]
#[command(about = "Probe Wayland and portal support for Gorgon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Session info, Wayland protocols and portal versions (default)
    Report,
    /// Take screenshots through the portal and time them
    Screenshot {
        /// Let the desktop show its own picker
        #[arg(long)]
        interactive: bool,
        /// Ask for a specific target (portal version 3+)
        #[arg(long)]
        target: Option<Target>,
        /// Number of screenshots to take, to see whether later ones prompt
        #[arg(long, default_value_t = 3)]
        runs: u32,
    },
    /// Register global shortcuts and print activations
    Shortcuts {
        /// How long to listen for, in seconds
        #[arg(long, default_value_t = 60)]
        seconds: u64,
    },
    /// Set the clipboard without a window, the way the daemon will after an upload
    Clipboard,
}

#[derive(Clone, Copy, ValueEnum)]
enum Target {
    Screen,
    Window,
    Area,
    ActiveWindow,
}

impl From<Target> for AvailableTargets {
    fn from(t: Target) -> Self {
        match t {
            Target::Screen => AvailableTargets::Screen,
            Target::Window => AvailableTargets::Window,
            Target::Area => AvailableTargets::Area,
            Target::ActiveWindow => AvailableTargets::ActiveWindow,
        }
    }
}

/// Wayland globals that matter to Gorgon, with why.
const INTERESTING_GLOBALS: &[(&str, &str)] = &[
    ("zwlr_layer_shell_v1", "overlay above everything"),
    ("zwlr_data_control_manager_v1", "clipboard without focus"),
    ("ext_data_control_manager_v1", "clipboard without focus"),
    ("zwlr_screencopy_manager_v1", "native capture backend"),
    (
        "ext_image_copy_capture_manager_v1",
        "native capture backend",
    ),
    ("ext_foreign_toplevel_list_v1", "list windows"),
    ("wp_fractional_scale_manager_v1", "fractional scaling"),
    ("xdg_activation_v1", "focus handoff"),
];

/// Portal interfaces Gorgon plans to use.
const PORTALS: &[&str] = &[
    "org.freedesktop.portal.Screenshot",
    "org.freedesktop.portal.ScreenCast",
    "org.freedesktop.portal.GlobalShortcuts",
    "org.freedesktop.portal.Notification",
];

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command.unwrap_or(Command::Report) {
        Command::Report => report().await,
        Command::Screenshot {
            interactive,
            target,
            runs,
        } => screenshot(interactive, target, runs).await,
        Command::Shortcuts { seconds } => shortcuts(seconds).await,
        // wl-clipboard-rs is blocking; keep it off the async runtime.
        Command::Clipboard => tokio::task::spawn_blocking(clipboard).await?,
    }
}

async fn report() -> Result<()> {
    println!("## Session");
    for var in [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_TYPE",
        "WAYLAND_DISPLAY",
        "QT_QPA_PLATFORM",
    ] {
        println!(
            "- {var}: {}",
            std::env::var(var).unwrap_or_else(|_| "(unset)".into())
        );
    }

    println!("\n## Wayland globals");
    match wayland_globals() {
        Ok(globals) => {
            for (name, why) in INTERESTING_GLOBALS {
                let found = globals.iter().find(|(n, _)| n == name);
                match found {
                    Some((_, v)) => println!("- [x] {name} v{v} ({why})"),
                    None => println!("- [ ] {name} ({why})"),
                }
            }
        }
        Err(e) => println!("- error: {e:#}"),
    }

    println!("\n## Portals");
    let conn = zbus::Connection::session().await?;
    for iface in PORTALS {
        match portal_version(&conn, iface).await {
            Ok(v) => println!("- [x] {iface} v{v}"),
            Err(_) => println!("- [ ] {iface}"),
        }
    }
    if let Ok(proxy) = ScreenshotProxy::new().await {
        if proxy.version() >= 3 {
            match proxy.available_targets().await {
                Ok(t) => println!("- Screenshot targets: {t:?}"),
                Err(e) => println!("- Screenshot targets: error {e}"),
            }
        }
    }
    Ok(())
}

async fn portal_version(conn: &zbus::Connection, iface: &str) -> Result<u32> {
    let proxy = zbus::Proxy::new(
        conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        iface.to_owned(),
    )
    .await?;
    Ok(proxy.get_property::<u32>("version").await?)
}

struct Globals;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Globals {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

fn wayland_globals() -> Result<Vec<(String, u32)>> {
    let conn = Connection::connect_to_env().context("not running under Wayland")?;
    let (globals, _queue) = registry_queue_init::<Globals>(&conn)?;
    Ok(globals
        .contents()
        .clone_list()
        .into_iter()
        .map(|g| (g.interface, g.version))
        .collect())
}

async fn screenshot(interactive: bool, target: Option<Target>, runs: u32) -> Result<()> {
    for run in 1..=runs {
        let start = Instant::now();
        let mut request = Screenshot::request().interactive(interactive).modal(true);
        if let Some(t) = target {
            request = request.target(AvailableTargets::from(t));
        }
        let result = request.send().await?.response();
        let elapsed = start.elapsed();
        match result {
            Ok(shot) => {
                let uri = shot.uri().to_string();
                let size = uri
                    .strip_prefix("file://")
                    .and_then(|p| png_size(p).ok())
                    .map(|(w, h)| format!("{w}x{h}"))
                    .unwrap_or_else(|| "unknown size".into());
                println!("run {run}: {} ms, {size}, {uri}", elapsed.as_millis());
            }
            Err(e) => println!("run {run}: failed after {} ms: {e}", elapsed.as_millis()),
        }
    }
    println!("\nNote whether a permission dialog appeared, and on which runs.");
    Ok(())
}

/// Reads width and height from a PNG header without decoding the image.
fn png_size(path: &str) -> Result<(u32, u32)> {
    let path = path.replace("%20", " ");
    let bytes = std::fs::read(&path)?;
    if bytes.len() < 24 || &bytes[1..4] != b"PNG" {
        bail!("not a PNG");
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into()?);
    Ok((w, h))
}

async fn shortcuts(seconds: u64) -> Result<()> {
    let portal = GlobalShortcuts::new()
        .await
        .context("GlobalShortcuts portal not available")?;
    println!("GlobalShortcuts portal v{}", portal.version());
    let session = portal
        .create_session(CreateSessionOptions::default())
        .await?;
    let wanted = [
        NewShortcut::new("capture-region", "Capture region").preferred_trigger("CTRL+SHIFT+4"),
        NewShortcut::new("capture-screen", "Capture screen").preferred_trigger("CTRL+SHIFT+3"),
    ];
    let bound = portal
        .bind_shortcuts(&session, &wanted, None, BindShortcutsOptions::default())
        .await?
        .response()?;
    for s in bound.shortcuts() {
        println!("bound {} -> {:?}", s.id(), s.trigger_description());
    }

    println!("Press the shortcuts now (listening {seconds}s)...");
    let mut activated = portal.receive_activated().await?;
    let deadline = tokio::time::sleep(Duration::from_secs(seconds));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            Some(a) = activated.next() => println!("activated {}", a.shortcut_id()),
        }
    }
    Ok(())
}

fn clipboard() -> Result<()> {
    use wl_clipboard_rs::copy::{MimeType, Options, Source};

    let text = format!("gorgon clipboard test {}", std::process::id());
    println!("Copying {text:?} with no window open...");
    let mut opts = Options::new();
    opts.foreground(true);
    // Blocks until something else takes the clipboard.
    match opts.copy(Source::Bytes(text.into_bytes().into()), MimeType::Text) {
        Ok(()) => println!("Clipboard was served until replaced: data-control works."),
        Err(e) => println!("Failed: {e}. The daemon will need the notification fallback here."),
    }
    Ok(())
}
