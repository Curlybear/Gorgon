//! The daemon's D-Bus service, which the CLI talks to.

use gorgon_core::CaptureMode;
use zbus::{fdo, interface, Connection};

pub const NAME: &str = "dev.gorgon.Gorgon";
pub const PATH: &str = "/dev/gorgon/Gorgon";
pub const INTERFACE: &str = "dev.gorgon.Gorgon1";

struct Service;

#[interface(name = "dev.gorgon.Gorgon1")]
impl Service {
    /// Starts a capture: "region", "screen" or "window", optionally opening the editor.
    fn capture(&self, mode: &str, edit: bool) -> fdo::Result<()> {
        let mode: CaptureMode = mode.parse().map_err(fdo::Error::InvalidArgs)?;
        crate::app::request_capture(mode, edit).map_err(fdo::Error::Failed)
    }

    /// Records a region: "mp4" or "gif" start (or stop a running recording),
    /// "stop" stops.
    fn record(&self, what: &str) -> fdo::Result<()> {
        if what == "stop" {
            crate::recorder::stop();
            return Ok(());
        }
        let format = what.parse().map_err(fdo::Error::InvalidArgs)?;
        crate::recorder::toggle(format).map_err(fdo::Error::Failed)
    }

    /// Opens a window: "history" or "settings".
    fn show_window(&self, name: &str) -> fdo::Result<()> {
        let name = match name {
            "history" => "history",
            "settings" => "settings",
            other => return Err(fdo::Error::InvalidArgs(format!("unknown window {other:?}"))),
        };
        crate::app::request_window(name).map_err(fdo::Error::Failed)
    }

    /// Opens an image file in the editor.
    fn edit_file(&self, path: &str) -> fdo::Result<()> {
        crate::app::request_edit_file(path.into()).map_err(fdo::Error::Failed)
    }
}

/// Whether a Gorgon daemon already owns the bus name.
pub async fn is_running(conn: &Connection) -> zbus::Result<bool> {
    let dbus = fdo::DBusProxy::new(conn).await?;
    Ok(dbus.name_has_owner(NAME.try_into()?).await?)
}

/// Publishes the service. Keep the returned connection alive.
pub async fn serve() -> zbus::Result<Connection> {
    zbus::connection::Builder::session()?
        .serve_at(PATH, Service)?
        .name(NAME)?
        .build()
        .await
}
