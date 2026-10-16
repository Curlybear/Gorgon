//! Gorgon: screenshot and upload tool for Wayland.
//!
//! `gorgon` (or `gorgon daemon`) runs the tray app; `gorgon capture <mode>`
//! asks it to capture, starting it first if needed.

mod app;
mod autostart;
mod client;
mod dbus;
mod editor;
mod notify;
mod outputs;
mod pipeline;
mod recorder;
mod selection;
mod shortcuts;
mod tray;
mod uploader_cli;
mod windows;

use std::{process::ExitCode, sync::OnceLock};

use clap::{Parser, Subcommand};
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};
use gorgon_core::CaptureMode;

#[derive(Parser)]
#[command(version, about = "Screenshot and upload tool for Wayland")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run in the tray (the default)
    Daemon,
    /// Take a capture: region, screen or window
    Capture {
        #[arg(value_parser = parse_mode)]
        mode: CaptureMode,
        /// Open the editor before saving
        #[arg(long)]
        edit: bool,
    },
    /// Open an image in the editor
    Edit { file: std::path::PathBuf },
    /// Record a region: mp4 or gif (again, or `stop`, to finish)
    Record {
        #[arg(value_parser = ["mp4", "gif", "stop"])]
        what: String,
    },
    /// Show past captures and their links
    History,
    /// Open the settings window
    Settings,
    /// Start Gorgon when you log in: on, off, or status
    Autostart {
        #[arg(value_parser = ["on", "off", "status"], default_value = "status")]
        what: String,
    },
    /// Upload a file and print its link (shortened if a shortener is set)
    Upload {
        file: std::path::PathBuf,
        /// Uploader to use instead of the default
        #[arg(long)]
        with: Option<String>,
        /// Keep the uploader's own link
        #[arg(long)]
        no_shorten: bool,
    },
    /// Shorten a URL and print the short link
    Shorten {
        url: String,
        /// Shortener to use instead of the default
        #[arg(long)]
        with: Option<String>,
    },
    /// Manage custom uploaders and URL shorteners (ShareX .sxcu files)
    Uploader {
        #[command(subcommand)]
        command: UploaderCommand,
    },
}

#[derive(Subcommand)]
enum UploaderCommand {
    /// Import a .sxcu file; the first of each kind becomes the default
    Import { file: std::path::PathBuf },
    /// List imported uploaders and which are the defaults
    List,
    /// Make an imported uploader the default for its kind
    Default { name: String },
}

fn autostart_cli(what: &str) -> anyhow::Result<()> {
    match what {
        "status" => {
            println!(
                "Gorgon {} start at login.",
                if autostart::enabled() {
                    "does"
                } else {
                    "doesn't"
                }
            );
            Ok(())
        }
        on => {
            let path = autostart::set(on == "on")?;
            if on == "on" {
                println!("Gorgon will start at login ({}).", path.display());
            } else {
                println!("Gorgon won't start at login any more.");
            }
            Ok(())
        }
    }
}

fn parse_mode(s: &str) -> Result<CaptureMode, String> {
    s.parse()
}

/// Async work (portal calls, D-Bus, tray) runs here, off the Qt thread.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn main() -> ExitCode {
    match Cli::parse().command.unwrap_or(Command::Daemon) {
        Command::Capture { mode, edit } => report(runtime().block_on(client::capture(mode, edit))),
        Command::Edit { file } => report(runtime().block_on(client::edit(&file))),
        Command::Daemon => daemon(),
        Command::Record { what } => report(runtime().block_on(client::record(&what))),
        Command::History => report(runtime().block_on(client::show_window("history"))),
        Command::Settings => report(runtime().block_on(client::show_window("settings"))),
        Command::Autostart { what } => report(autostart_cli(&what)),
        Command::Upload {
            file,
            with,
            no_shorten,
        } => report(uploader_cli::upload(&file, with.as_deref(), no_shorten)),
        Command::Shorten { url, with } => report(uploader_cli::shorten(&url, with.as_deref())),
        Command::Uploader { command } => report(match command {
            UploaderCommand::Import { file } => uploader_cli::import(&file),
            UploaderCommand::List => uploader_cli::list(),
            UploaderCommand::Default { name } => uploader_cli::set_default(&name),
        }),
    }
}

fn report(result: anyhow::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gorgon: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn daemon() -> ExitCode {
    let already = runtime().block_on(async {
        let conn = zbus::Connection::session().await?;
        dbus::is_running(&conn).await
    });
    match already {
        Ok(true) => {
            eprintln!("gorgon is already running");
            return ExitCode::SUCCESS;
        }
        Ok(false) => {}
        Err(e) => {
            eprintln!("gorgon: could not reach the session bus: {e}");
            return ExitCode::FAILURE;
        }
    }

    let mut app = QGuiApplication::new();
    if let Some(app) = app.as_mut() {
        app.set_application_name(&QString::from("Gorgon"));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/dev/gorgon/qml/main.qml"));
    }

    // The App object now exists, so requests can reach the UI thread.
    let _bus = match runtime().block_on(dbus::serve()) {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("gorgon: could not publish the D-Bus service: {e}");
            return ExitCode::FAILURE;
        }
    };
    runtime().spawn(tray::run());
    runtime().spawn(shortcuts::run());
    println!("gorgon is running");

    if let Some(app) = app.as_mut() {
        app.exec();
    }
    ExitCode::SUCCESS
}
